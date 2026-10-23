const { test } = require('node:test');
const assert = require('node:assert/strict');
const { validatePullRequest, checkPullRequest, checkReleasePRs, selectDraft, publish, syncDevelop } = require('./automation.cjs');

const sha = 'a'.repeat(40);
const repo = { owner: 'owner', repo: 'app' };
const context = { repo, ref: 'refs/heads/main', sha, eventName: 'push', payload: { repository: { full_name: 'owner/app' } } };
const pr = (overrides = {}) => ({
  number: 12, title: 'feat(review): adiciona exportação', user: { login: 'owner' },
  base: { ref: 'develop', repo: { full_name: 'owner/app' } },
  head: { ref: 'feature/export', sha, repo: { full_name: 'owner/app' } }, ...overrides,
});

function fixture({ draft = true, assets, tagSha = sha, tag = 'v1.0.0' } = {}) {
  const calls = [];
  const outputs = {};
  const release = { id: 1, tag_name: tag, target_commitish: sha, draft };
  const installers = assets || ['linux.AppImage', 'setup.exe', 'setup.msi'].map(name => ({ name, size: 100, state: 'uploaded' }));
  const github = {
    rest: {
      repos: {
        listReleases: 'releases', listReleaseAssets: 'assets',
        getRelease: async () => ({ data: release }),
        compareCommitsWithBasehead: async () => ({ data: { status: 'ahead', ahead_by: 1 } }),
        updateRelease: async data => { calls.push(['publish', data]); release.draft = false; },
      },
      git: {
        getRef: async () => {
          if (!tagSha) throw Object.assign(new Error('not found'), { status: 404 });
          return { data: { object: { type: 'commit', sha: tagSha } } };
        },
        createRef: async data => calls.push(['tag', data]),
      },
      pulls: {
        list: async () => ({ data: [] }),
        create: async data => { calls.push(['pr', data]); return { data: pr({ head: { ref: 'main' } }) }; },
      },
      actions: { createWorkflowDispatch: async data => calls.push(['ci', data]) },
    },
    paginate: async endpoint => endpoint === 'releases' ? [release] : endpoint === 'assets' ? installers : [],
  };
  const core = { setOutput: (key, value) => { outputs[key] = value; } };
  return { github, core, context, calls, outputs, release };
}

test('PR titles carry semantic intent and main only accepts supported sources', () => {
  validatePullRequest(pr());
  validatePullRequest(pr({ title: 'feat!: altera formato' }));
  assert.throws(() => validatePullRequest(pr({ title: 'melhorias' })), /Conventional/);
  const main = { ref: 'main', repo: { full_name: 'owner/app' } };
  assert.throws(() => validatePullRequest(pr({ base: main })), /main recebe/);
  validatePullRequest(pr({ base: main, head: { ref: 'develop', repo: { full_name: 'owner/app' } } }));
  assert.throws(() => validatePullRequest(pr({ base: main, head: { ref: 'develop', repo: { full_name: 'fork/app' } } })), /main recebe/);
});

test('dispatched CI rejects a PR that advanced after dispatch', async () => {
  const f = fixture();
  f.github.rest.pulls.get = async () => ({ data: pr({ head: { sha: 'b'.repeat(40) } }) });
  await assert.rejects(checkPullRequest({ ...f, context: { ...context, eventName: 'workflow_dispatch', payload: { inputs: { pr_number: '12' } } } }), /PR mudou/);
});

test('bot PR CI dispatch works even if the version PR was not updated', async () => {
  const f = fixture();
  f.github.paginate = async () => [pr({ user: { login: 'github-actions[bot]' }, head: { ref: 'release-please--branches--main', repo: { full_name: 'owner/app' } } })];
  await checkReleasePRs(f);
  assert.equal(f.calls[0][0], 'ci');
  assert.equal(f.calls[0][1].inputs.pr_number, '12');
});

test('draft creates its tag once and retains the exact release commit', async () => {
  const f = fixture({ tagSha: undefined });
  // Explicitly simulate a missing reference.
  f.github.rest.git.getRef = async () => { throw Object.assign(new Error(), { status: 404 }); };
  await selectDraft(f, 'v1.0.0');
  assert.equal(f.outputs.sha, sha);
  assert.equal(f.outputs.should_release, 'true');
  assert.equal(f.calls.filter(([type]) => type === 'tag').length, 1);
});

test('retry uses existing tag and refuses mismatches or commits outside main', async () => {
  const f = fixture();
  await selectDraft({ ...f, context: { ...context, eventName: 'workflow_dispatch' } }, 'v1.0.0');
  assert.equal(f.calls.length, 0);
  await assert.rejects(selectDraft(fixture({ tagSha: 'b'.repeat(40) }), 'v1.0.0'), /outro commit/);
  f.github.rest.repos.compareCommitsWithBasehead = async () => ({ data: { status: 'diverged' } });
  await assert.rejects(selectDraft(f, 'v1.0.0'), /histórico/);
});

test('manual retry cannot invent a release or run from develop', async () => {
  const f = fixture();
  await assert.rejects(selectDraft({ ...f, context: { ...context, eventName: 'workflow_dispatch' } }, 'v9.9.9'), /existente/);
  await assert.rejects(selectDraft({ ...f, context: { ...context, ref: 'refs/heads/develop' } }, 'v1.0.0'), /main/);
});

test('missing installer or changed tag prevents publication', async () => {
  const f = fixture({ assets: [{ name: 'linux.AppImage', size: 1, state: 'uploaded' }] });
  await assert.rejects(publish(f, '1', 'v1.0.0', sha), /Instalador/);
  assert.equal(f.release.draft, true);
  await assert.rejects(publish(fixture({ tagSha: 'b'.repeat(40) }), '1', 'v1.0.0', sha), /mudou/);
});

test('complete installers publish once; a retry does not republish', async () => {
  const f = fixture();
  await publish(f, '1', 'v1.0.0', sha);
  await publish(f, '1', 'v1.0.0', sha);
  assert.equal(f.calls.filter(([type]) => type === 'publish').length, 1);
});

test('published release retry resumes synchronization without rebuilding', async () => {
  const f = fixture({ draft: false });
  await selectDraft({ ...f, context: { ...context, eventName: 'workflow_dispatch' } }, 'v1.0.0');
  assert.equal(f.outputs.should_release, 'false');
  assert.equal(f.outputs.published, 'true');
});

test('synchronization reuses its open PR and dispatches CI', async () => {
  const f = fixture();
  f.github.rest.pulls.list = async () => ({ data: [pr({ head: { ref: 'main' } })] });
  await syncDevelop(f);
  assert.deepEqual(f.calls.map(([type]) => type), ['ci']);
  f.github.rest.repos.compareCommitsWithBasehead = async () => ({ data: { ahead_by: 0 } });
  await syncDevelop(f);
  assert.equal(f.calls.length, 1);
});
