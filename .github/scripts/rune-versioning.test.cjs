const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
require('release-please'); // Initialize the strategy registry before importing a strategy.
const { Rust } = require('release-please/build/src/strategies/rust');
const { Version } = require('release-please/build/src/version');
const { TagName } = require('release-please/build/src/util/tag-name');
const { parseConventionalCommits } = require('release-please/build/src/commit');
const { CommitExclude } = require('release-please/build/src/util/commit-exclude');
const { parse: parseToml } = require('@iarna/toml');

const root = path.join(__dirname, '../..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const config = JSON.parse(read('release-please-config.json')).packages['.'];
const manifest = JSON.parse(read('.release-please-manifest.json'));
const logger = { info() {}, warn() {}, debug() {}, error() {}, trace() {} };
const member = 'crates/engineroom-rune';
const nextPatch = version => version.replace(/^(\d+)\.(\d+)\.(\d+)$/, (_, a, b, c) => `${a}.${b}.${Number(c) + 1}`);
const nextMajor = version => version.replace(/^(\d+)\.(\d+)\.(\d+)$/, (_, a) => `${Number(a) + 1}.0.0`);

function files(version = manifest['.']) {
  return {
    'Cargo.toml': `[workspace]\nmembers = ["crates/engineroom-ui"]\n\n[package]\nname = "engineroom-rune"\nversion = "${version}"\nedition = "2024"\n`,
    'crates/engineroom-ui/Cargo.toml': '[package]\nname = "engineroom-ui"\nversion = "0.1.0"\nedition = "2024"\n',
    'Cargo.lock': `[[package]]\nname = "engineroom-rune"\nversion = "${version}"\n\n[[package]]\nname = "engineroom-ui"\nversion = "0.1.0"\n`,
    'CHANGELOG.md': `# Changelog\n\n## [${version}](https://example.com) (2026-11-26)\n\nCorreções\n`,
  };
}

function strategy(source = files()) {
  const github = {
    repository: { owner: 'PedrFelip', repo: 'EngineRoom', defaultBranch: 'main' },
    getFileContentsOnBranch: async file => ({ parsedContent: source[file] }),
  };
  return new Rust({
    github, path: '.', targetBranch: 'main', logger,
    includeComponentInTag: config['include-component-in-tag'],
    includeVInTag: config['include-v-in-tag'],
    changelogSections: config['changelog-sections'],
  });
}

async function candidate(message, previous = manifest['.'], source = files(), paths = [`${member}/src/main.rs`]) {
  const commits = parseConventionalCommits(
    [{ sha: 'a'.repeat(40), message, files: paths }], logger
  );
  return strategy(source).buildReleasePullRequest(commits, {
    tag: new TagName(Version.parse(previous)), sha: 'b'.repeat(40), notes: '',
  });
}

test('prepared root config uses the rust strategy with a stable draft tag', async () => {
  assert.equal(config['release-type'], 'rust');
  assert.equal(config.draft, true);
  assert.equal(config['force-tag-creation'], true);
  assert.equal(config['include-component-in-tag'], false);
  assert.match(manifest['.'], /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/);
  const next = nextPatch(manifest['.']);
  assert.equal((await candidate('fix: corrige inicialização')).version.toString(), next);
  assert.equal(await strategy().getComponent(), '');
  assert.equal(new TagName(Version.parse(next), '', '-', config['include-v-in-tag']).toString(), `v${next}`);
});

test('real Rust updaters bump root, members, lockfile and root changelog', async () => {
  const source = files();
  const next = nextPatch(manifest['.']);
  const pr = await candidate('fix: corrige inicialização', manifest['.'], source);
  const outputs = {};
  for (const update of pr.updates) {
    outputs[update.path] = update.updater.updateContent(source[update.path] ?? '# Changelog\n', logger);
  }
  assert.equal(parseToml(outputs['Cargo.toml']).package.version, next);
  assert.equal(parseToml(outputs['crates/engineroom-ui/Cargo.toml']).package.version, next);
  assert.deepEqual(
    parseToml(outputs['Cargo.lock']).package.map(pkg => pkg.version),
    [next, next],
  );
  assert.match(outputs['CHANGELOG.md'], new RegExp(next.replaceAll('.', '\\.')));
  assert.match(outputs['CHANGELOG.md'], /Correções/);
});

test('exclude-paths keeps automation-only changes out of a root-package release', () => {
  // A root package sees every commit, unlike the Tauri `app` package path, so
  // the config restores the same policy with exclude-paths.
  assert.deepEqual(config['exclude-paths'], ['.github/', 'docs/']);
  const exclude = new CommitExclude({ '.': { excludePaths: config['exclude-paths'] } });
  const commit = files => ({ sha: 'a'.repeat(40), files });
  const apply = paths => exclude.excludeCommits({ '.': [commit(paths)] })['.'];
  assert.deepEqual(apply(['.github/workflows/rune-installers.yml', 'docs/releases.md']), []);
  assert.equal(apply(['.github/workflows/rune-installers.yml', `${member}/src/main.rs`]).length, 1);
  assert.equal(apply([`${member}/src/main.rs`]).length, 1);
});

test('breaking changes still bump the Rune major', async () => {
  assert.equal(
    (await candidate('feat!: troca formato', manifest['.'])).version.toString(),
    nextMajor(manifest['.']),
  );
});
