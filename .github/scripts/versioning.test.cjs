const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
require('release-please'); // Initialize its strategy registry before importing a strategy.
const { Node } = require('release-please/build/src/strategies/node');
const { Version } = require('release-please/build/src/version');
const { TagName } = require('release-please/build/src/util/tag-name');
const { parseConventionalCommits } = require('release-please/build/src/commit');
const { parse: parseToml } = require('@iarna/toml');

const root = path.join(__dirname, '../..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const config = JSON.parse(read('release-please-config.json')).packages.app;
const logger = { info() {}, warn() {}, debug() {}, error() {} };

async function candidate(message, previous = '1.0.0') {
  const github = {
    repository: { owner: 'PedrFelip', repo: 'EngineRoom', defaultBranch: 'main' },
    getFileContentsOnBranch: async file => ({ parsedContent: read(file) }),
  };
  const strategy = new Node({
    github, path: 'app', targetBranch: 'main', logger,
    includeComponentInTag: config['include-component-in-tag'],
    includeVInTag: config['include-v-in-tag'],
    extraFiles: config['extra-files'], changelogSections: config['changelog-sections'],
  });
  const commits = parseConventionalCommits([{ sha: 'a'.repeat(40), message, files: ['app/src/App.tsx'] }], logger);
  return strategy.buildReleasePullRequest(commits, {
    tag: new TagName(Version.parse(previous)), sha: 'b'.repeat(40), notes: '',
  });
}

test('first version is 1.0.0; subsequent fixes, features and breaking changes follow SemVer', async () => {
  for (const [message, previous, expected] of [
    ['feat(ci): automatiza releases\n\nRelease-As: 1.0.0', '0.9.1', '1.0.0'],
    ['fix: corrige importação', '1.0.0', '1.0.1'],
    ['feat: exporta partidas', '1.0.0', '1.1.0'],
    ['feat!: altera formato', '1.0.0', '2.0.0'],
  ]) {
    assert.equal((await candidate(message, previous)).version.toString(), expected);
  }
});

test('administrative-only changes do not open a version PR', async () => {
  for (const message of ['docs: explica setup', 'chore: atualiza configuração', 'test: cobre cache']) {
    assert.equal(await candidate(message), undefined);
  }
});

test('real Release Please updaters synchronize all manifests without changing dependencies', async () => {
  const pr = await candidate('feat(ci): automatiza releases\n\nRelease-As: 1.0.0', '0.9.1');
  const outputs = {};
  for (const update of pr.updates) {
    if (fs.existsSync(path.join(root, update.path))) {
      outputs[update.path] = update.updater.updateContent(read(update.path), logger);
    }
  }
  assert.equal(JSON.parse(outputs['app/package.json']).version, '1.0.0');
  assert.equal(JSON.parse(outputs['app/src-tauri/tauri.conf.json']).version, '1.0.0');
  assert.equal(parseToml(outputs['app/src-tauri/Cargo.toml']).package.version, '1.0.0');
  const original = read('app/src-tauri/Cargo.lock');
  const expected = original.replace(/(\[\[package\]\]\nname = "app"\nversion = ")[^"]+"/, '$11.0.0"');
  assert.notEqual(expected, original);
  assert.equal(outputs['app/src-tauri/Cargo.lock'], expected);
  assert.match(outputs['app/CHANGELOG.md'], /1\.0\.0/);
  assert.match(outputs['app/CHANGELOG.md'], /Funcionalidades/);
});

test('Cargo.lock updater distinguishes a registry dependency also named app', async () => {
  const pr = await candidate('fix: corrige importação');
  const updater = pr.updates.find(update => update.path === 'app/src-tauri/Cargo.lock').updater;
  const source = '[[package]]\nname = "app"\nversion = "0.9.1"\n\n[[package]]\nname = "app"\nversion = "4.0.0"\nsource = "registry+https://example.com"\n';
  const updated = parseToml(updater.updateContent(source, logger));
  assert.equal(updated.package[0].version, '1.0.1');
  assert.equal(updated.package[1].version, '4.0.0');
});
