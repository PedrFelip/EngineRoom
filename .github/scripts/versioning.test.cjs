const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
require('release-please'); // Initialize its strategy registry before importing a strategy.
const { Node } = require('release-please/build/src/strategies/node');
const { Version } = require('release-please/build/src/version');
const { TagName } = require('release-please/build/src/util/tag-name');
const { parseConventionalCommits } = require('release-please/build/src/commit');
const { CommitSplit } = require('release-please/build/src/util/commit-split');
const { parse: parseToml } = require('@iarna/toml');

const root = path.join(__dirname, '../..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const config = JSON.parse(read('release-please-config.json')).packages.app;
const logger = { info() {}, warn() {}, debug() {}, error() {} };

async function candidate(message, previous = '1.0.0', readFile = read, files = ['app/src/App.tsx']) {
  const github = {
    repository: { owner: 'PedrFelip', repo: 'EngineRoom', defaultBranch: 'main' },
    getFileContentsOnBranch: async file => ({ parsedContent: readFile(file) }),
  };
  const strategy = new Node({
    github, path: 'app', targetBranch: 'main', logger,
    includeComponentInTag: config['include-component-in-tag'],
    includeVInTag: config['include-v-in-tag'],
    extraFiles: config['extra-files'], changelogSections: config['changelog-sections'],
  });
  // Manifest splits by package path before the strategy sees any commits.
  const split = new CommitSplit({ packagePaths: ['app'] }).split([{ sha: 'a'.repeat(40), message, files }]);
  const commits = parseConventionalCommits(split.app || [], logger);
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

test('AppImage packaging fixes produce a patch release through manifest path filtering', async () => {
  const script = 'app/scripts/verify_appimage.py';
  assert.ok(fs.existsSync(path.join(root, script)), 'The packaging script must live in the released app path');
  const message = 'fix: corrige inicialização do AppImage com Mesa recente\n\nRelease-As: 2.1.1';
  const files = [script, '.github/workflows/release.yml'];
  assert.equal((await candidate(message, '2.1.0', read, files)).version.toString(), '2.1.1');
  assert.equal(await candidate(message, '2.1.0', read, ['.github/scripts/prepare_appimage.py', '.github/workflows/release.yml']), undefined);
});

async function verifyManifestUpdates(readFile) {
  const current = JSON.parse(readFile('app/package.json')).version;
  const [major, minor, patch] = current.split('.').map(Number);
  const next = `${major}.${minor}.${patch + 1}`;
  const pr = await candidate('fix: corrige importação', current, readFile);
  assert.equal(pr.version.toString(), next);
  const outputs = {};
  for (const update of pr.updates) {
    if (fs.existsSync(path.join(root, update.path))) {
      outputs[update.path] = update.updater.updateContent(readFile(update.path), logger);
    }
  }
  assert.equal(JSON.parse(outputs['app/package.json']).version, next);
  assert.equal(JSON.parse(outputs['app/src-tauri/tauri.conf.json']).version, next);
  assert.equal(parseToml(outputs['app/src-tauri/Cargo.toml']).package.version, next);
  const original = readFile('app/src-tauri/Cargo.lock');
  const expected = original.replace(/(\[\[package\]\]\nname = "app"\nversion = ")[^"]+"/, (_, prefix) => `${prefix}${next}"`);
  assert.ok(expected !== original, 'The fixture must exercise an actual version change');
  // Keep failure output short: a full lockfile diff obscures the actual CI error.
  assert.ok(outputs['app/src-tauri/Cargo.lock'] === expected, 'Only the local app version may change in Cargo.lock');
  assert.ok(outputs['app/CHANGELOG.md'].includes(next));
  assert.match(outputs['app/CHANGELOG.md'], /Correções/);
}

test('real Release Please updaters synchronize all manifests without changing dependencies', async () => {
  await verifyManifestUpdates(read);
});

test('manifest checks also work inside the 1.0.0 release PR and later releases', async () => {
  for (const version of ['1.0.0', '2.3.4']) {
    const files = {
      'app/package.json': JSON.stringify({ ...JSON.parse(read('app/package.json')), version }),
      'app/src-tauri/tauri.conf.json': JSON.stringify({ ...JSON.parse(read('app/src-tauri/tauri.conf.json')), version }),
      'app/src-tauri/Cargo.toml': read('app/src-tauri/Cargo.toml').replace(/(\[package\][\s\S]*?\nversion = ")[^"]+"/, (_, prefix) => `${prefix}${version}"`),
      'app/src-tauri/Cargo.lock': read('app/src-tauri/Cargo.lock').replace(/(\[\[package\]\]\nname = "app"\nversion = ")[^"]+"/, (_, prefix) => `${prefix}${version}"`),
    };
    await verifyManifestUpdates(file => files[file] ?? read(file));
  }
});

test('Cargo.lock updater distinguishes a registry dependency also named app', async () => {
  const pr = await candidate('fix: corrige importação');
  const updater = pr.updates.find(update => update.path === 'app/src-tauri/Cargo.lock').updater;
  const source = '[[package]]\nname = "app"\nversion = "0.9.1"\n\n[[package]]\nname = "app"\nversion = "4.0.0"\nsource = "registry+https://example.com"\n';
  const updated = parseToml(updater.updateContent(source, logger));
  assert.equal(updated.package[0].version, '1.0.1');
  assert.equal(updated.package[1].version, '4.0.0');
});
