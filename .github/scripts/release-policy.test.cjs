const { test } = require('node:test');
const assert = require('node:assert/strict');
const { compareVersions, checkReleaseVersion, checkPublicationVersion } = require('./release-policy.cjs');

function fixture({ base = '2.1.3', next = '2.1.4', releases = [], tag = false } = {}) {
  const pr = {
    base: { ref: 'main', sha: 'base-sha' },
    head: { ref: 'release-please--branches--main--components--app', sha: 'head-sha' },
    user: { login: 'github-actions[bot]' },
  };
  const github = {
    rest: {
      repos: {
        getContent: async ({ ref }) => ({ data: { content: Buffer.from(JSON.stringify({
          version: ref === pr.base.sha ? base : next,
        })).toString('base64') } }),
        listReleases: 'releases',
      },
      git: { getRef: async () => {
        if (!tag) throw Object.assign(new Error('missing'), { status: 404 });
        return { data: {} };
      } },
    },
    paginate: async () => releases,
  };
  return { args: { github, context: { repo: { owner: 'owner', repo: 'app' } } }, pr };
}

test('release version comparison is numeric across components', () => {
  assert.equal(compareVersions('2.10.0', '2.9.99'), 1);
  assert.equal(compareVersions('3.0.0', '2.99.99'), 1);
  assert.equal(compareVersions('2.1.3', '2.1.3'), 0);
  assert.throws(() => compareVersions('2.1.4-beta.1', '2.1.3'), /SemVer/);
});

test('rejects the PR 35 regression and unchanged versions before merge', async () => {
  for (const next of ['2.1.2', '2.1.3']) {
    const f = fixture({ next });
    await assert.rejects(checkReleaseVersion(f.args, f.pr), /maior que/);
  }
});

test('PR policy invokes the version guard for a Release Please PR', async () => {
  const { checkPullRequest } = require('./automation.cjs');
  const f = fixture({ next: '2.1.2' });
  f.pr.title = 'chore(main): release 2.1.2';
  f.pr.number = 35;
  f.pr.base.repo = f.pr.head.repo = { full_name: 'owner/app' };
  f.args.github.rest.pulls = { get: async () => ({ data: f.pr }) };
  f.args.context.payload = { pull_request: { number: 35 } };
  f.args.context.eventName = 'pull_request';
  await assert.rejects(checkPullRequest(f.args), /maior que/);
});

test('rejects published, reserved and superseded versions as well as existing tags', async () => {
  for (const release of [
    { tag_name: 'v2.1.4', draft: false },
    { tag_name: 'v2.1.4', draft: true },
    { tag_name: 'v2.2.0', draft: false },
  ]) {
    const f = fixture({ releases: [release] });
    await assert.rejects(checkReleaseVersion(f.args, f.pr), /reservada/);
  }
  const f = fixture({ tag: true });
  await assert.rejects(checkReleaseVersion(f.args, f.pr), /tag.*existe/);
});

test('accepts a new stable version despite historical or prerelease names', async () => {
  const f = fixture({ releases: [
    { tag_name: 'main' }, { tag_name: 'v2.1.3' }, { tag_name: 'v3.0.0-beta.1' },
  ] });
  await checkReleaseVersion(f.args, f.pr);
});

test('ordinary PRs need no release API access and API failures fail closed', async () => {
  const f = fixture();
  await checkReleaseVersion({ github: {}, context: {} }, { ...f.pr, user: { login: 'owner' } });
  f.args.github.rest.git.getRef = async () => { throw new Error('API unavailable'); };
  await assert.rejects(checkReleaseVersion(f.args, f.pr), /API unavailable/);
});

test('resuming an old draft cannot replace a newer published stable release', async () => {
  for (const tag_name of ['v2.1.4', 'v2.2.0']) {
    const f = fixture({ releases: [{ tag_name, draft: false, prerelease: false }] });
    await assert.rejects(checkPublicationVersion(f.args, { tag_name: 'v2.1.4' }), /Não publique/);
  }
  const f = fixture({ releases: [
    { tag_name: 'v2.1.3', draft: false },
    { tag_name: 'v2.2.0', draft: true },
    { tag_name: 'main', prerelease: true },
  ] });
  await checkPublicationVersion(f.args, { tag_name: 'v2.1.4' });
});
