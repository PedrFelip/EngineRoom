const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { GitHub, Manifest } = require('release-please');

const root = path.join(__dirname, '../..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const current = JSON.parse(read('.release-please-manifest.json')).app;
const releaseSha = 'a'.repeat(40);
const bootstrapSha = 'b'.repeat(40);
const logger = { info() {}, warn() {}, debug() {}, error() {}, trace() {} };

// Only GitHub I/O is faked. Config parsing, draft/tag creation, release
// discovery, commit boundaries and PR/version generation use the pinned library.
function fixture({ forceTag = true, freshCommit } = {}) {
  const tags = new Map();
  const drafts = [];
  const calls = [];
  const visitedCommits = [];
  const config = JSON.parse(read('release-please-config.json'));
  config['bootstrap-sha'] = bootstrapSha;
  if (!forceTag) delete config.packages.app['force-tag-creation'];
  const pr = {
    number: 34,
    title: `chore(main): release ${current}`,
    body: `:robot: I have created a release *beep* *boop*\n---\n\n## [${current}](https://example.com/compare)\n\n### Correções\n\n* correção já lançada\n\n---\nThis PR was generated with Release Please.`,
    headBranchName: 'release-please--branches--main--components--app',
    baseBranchName: 'main',
    sha: releaseSha,
    labels: ['autorelease: pending'],
    files: ['app/package.json', 'app/CHANGELOG.md'],
  };
  const github = new GitHub({
    repository: { owner: 'PedrFelip', repo: 'EngineRoom', defaultBranch: 'main' },
    logger,
    octokitAPIs: {
      octokit: {
        git: { createRef: async ({ ref, sha }) => { calls.push('tag'); tags.set(ref.replace('refs/tags/', ''), sha); } },
        repos: { createRelease: async data => {
          calls.push('draft');
          drafts.push(data);
          return { data: { ...data, id: 1, html_url: 'https://example.com/release' } };
        } },
        issues: {
          createComment: async () => ({ data: {} }),
          removeLabel: async () => {},
          addLabels: async () => {},
        },
      },
      graphql: async () => ({ repository: { releases: {
        nodes: drafts.map(draft => ({
          name: draft.name,
          tag: tags.has(draft.tag_name) ? { name: draft.tag_name } : null,
          tagCommit: tags.has(draft.tag_name) ? { oid: tags.get(draft.tag_name) } : null,
          description: draft.body,
          isDraft: draft.draft,
          url: 'https://example.com/release',
        })),
        pageInfo: { hasNextPage: false },
      } } }),
    },
  });
  github.getFileJson = async file => file === 'release-please-config.json'
    ? config : JSON.parse(read(file));
  github.getFileContentsOnBranch = async file => ({ parsedContent: read(file) });
  github.pullRequestIterator = async function* (_branch, state) {
    if (state === 'MERGED' && pr.labels.includes('autorelease: pending')) yield pr;
  };
  github.removeIssueLabels = async labels => { pr.labels = pr.labels.filter(label => !labels.includes(label)); };
  github.tagIterator = async function* () {
    for (const [name, sha] of tags) yield { name, sha };
  };
  github.mergeCommitIterator = async function* () {
    const history = [
      ...(freshCommit ? [{ sha: 'c'.repeat(40), message: freshCommit, files: ['app/src/App.tsx'] }] : []),
      { sha: releaseSha, message: pr.title, files: pr.files, pullRequest: pr },
      { sha: 'd'.repeat(40), message: 'fix: correção antiga\n\nRelease-As: 1.0.0', files: ['app/src/App.tsx'] },
      { sha: bootstrapSha, message: 'chore: bootstrap', files: [] },
    ];
    for (const commit of history) {
      visitedCommits.push(commit.sha);
      yield commit;
    }
  };
  return { github, tags, drafts, calls, visitedCommits };
}

async function cycle(f) {
  const manifest = await Manifest.fromManifest(f.github, 'main', undefined, undefined, { logger });
  const releases = await manifest.createReleases();
  assert.equal(releases.length, 1);
  assert.equal(releases[0].version, current);
  assert.equal(f.drafts[0].draft, true);
  return manifest.buildPullRequests();
}

test('draft creates its immutable tag before discovering the next release boundary', async () => {
  const f = fixture();
  assert.deepEqual(await cycle(f), []);
  assert.deepEqual(f.calls, ['tag', 'draft']);
  assert.equal(f.tags.get(`v${current}`), releaseSha);
  assert.deepEqual(f.visitedCommits, [releaseSha]);
});

test('next change uses only commits after the draft, excluding old Release-As and notes', async () => {
  const f = fixture({ freshCommit: 'fix: nova correção' });
  const [pr] = await cycle(f);
  const [major, minor, patch] = current.split('.').map(Number);
  assert.equal(pr.version.toString(), `${major}.${minor}.${patch + 1}`);
  assert.match(pr.body.toString(), /nova correção/);
  assert.doesNotMatch(pr.body.toString(), /correção antiga|correção já lançada/);
  assert.deepEqual(f.visitedCommits, ['c'.repeat(40), releaseSha]);
});

test('without early tag creation the historical bootstrap regression is reproduced', async () => {
  const f = fixture({ forceTag: false });
  const [pr] = await cycle(f);
  assert.equal(pr.version.toString(), '1.0.0');
  assert.match(pr.body.toString(), /correção antiga/);
  assert.ok(f.visitedCommits.includes(bootstrapSha));
  assert.deepEqual(f.calls, ['draft']);
});
