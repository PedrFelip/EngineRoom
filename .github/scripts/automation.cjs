const fs = require('node:fs');
const path = require('node:path');

const TITLE = /^(feat|fix|perf|docs|refactor|test|build|ci|chore|revert)(\([^\r\n()]+\))?!?: \S[^\r\n]*$/;
const TAG = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const SHA = /^[a-f0-9]{40}$/;

function validatePullRequest(pr) {
  if (!TITLE.test(pr.title)) {
    throw new Error('Use Conventional Commits no título: feat: ..., fix: ..., chore: ...');
  }
  const internal = pr.head.repo?.full_name === pr.base.repo.full_name;
  if (pr.base.ref === 'main' && !(
    internal && (pr.head.ref === 'develop' || pr.head.ref.startsWith('hotfix/') ||
      (pr.user.login === 'github-actions[bot]' && pr.head.ref.startsWith('release-please--')))
  )) {
    throw new Error('main recebe develop, hotfix/* ou o PR de versão do Release Please.');
  }
}

async function checkPullRequest({ github, context }) {
  const number = context.payload.pull_request?.number || Number(context.payload.inputs?.pr_number);
  if (!number) return;
  const { data: pr } = await github.rest.pulls.get({ ...context.repo, pull_number: number });
  validatePullRequest(pr);
  if (context.eventName === 'workflow_dispatch' && pr.head.sha !== context.sha) {
    throw new Error('O PR mudou: execute CI novamente na branch atual.');
  }
}

async function dispatchCI(github, repo, pr) {
  await github.rest.actions.createWorkflowDispatch({
    ...repo, workflow_id: 'ci.yml', ref: pr.head.ref,
    inputs: { pr_number: String(pr.number) },
  });
}

async function checkReleasePRs({ github, context }) {
  // Query open PRs even when Release Please reports no change, so failed dispatches can be retried.
  const prs = await github.paginate(github.rest.pulls.list, {
    ...context.repo, state: 'open', base: 'main', per_page: 100,
  });
  for (const pr of prs) {
    if (pr.user.login === 'github-actions[bot]' && pr.head.ref.startsWith('release-please--') &&
        pr.head.repo.full_name === context.payload.repository.full_name) {
      await dispatchCI(github, context.repo, pr);
    }
  }
}

async function tagCommit(github, repo, tag) {
  let object;
  try {
    object = (await github.rest.git.getRef({ ...repo, ref: `tags/${tag}` })).data.object;
  } catch (error) {
    if (error.status === 404) return undefined;
    throw error;
  }
  while (object.type === 'tag') {
    object = (await github.rest.git.getTag({ ...repo, tag_sha: object.sha })).data.object;
  }
  if (object.type !== 'commit') throw new Error(`${tag} não aponta para um commit.`);
  return object.sha;
}

async function selectDraft({ github, context, core }, requestedTag) {
  const manual = context.eventName === 'workflow_dispatch';
  if (context.ref !== 'refs/heads/main') throw new Error('Execute Release a partir da main.');
  const version = JSON.parse(fs.readFileSync(path.join(__dirname, '../../app/package.json'), 'utf8')).version;
  const tag = requestedTag || `v${version}`;
  if (!TAG.test(tag)) throw new Error('Informe uma tag estável no formato v1.2.3.');
  const releases = await github.paginate(github.rest.repos.listReleases, {
    ...context.repo, per_page: 100,
  });
  const release = releases.find(item => item.tag_name === tag);
  if (!release || (!manual && !release.draft)) {
    if (manual) throw new Error(`${tag} precisa ser uma release existente para retomar.`);
    core.setOutput('should_release', 'false');
    return;
  }
  // Release Please creates drafts at the merged release PR's immutable commit.
  const sha = release.target_commitish;
  if (!SHA.test(sha)) throw new Error('O rascunho precisa apontar para um SHA completo.');
  const comparison = (await github.rest.repos.compareCommitsWithBasehead({
    ...context.repo, basehead: `${sha}...main`,
  })).data;
  if (!['ahead', 'identical'].includes(comparison.status)) {
    throw new Error('O commit da release não pertence ao histórico da main.');
  }
  const existing = await tagCommit(github, context.repo, tag);
  if (existing && existing !== sha) throw new Error(`${tag} aponta para outro commit.`);
  if (manual && !existing) throw new Error('A retomada exige a tag criada pela preparação original.');
  if (!release.draft) {
    core.setOutput('should_release', 'false');
    core.setOutput('published', 'true');
    return;
  }
  if (!existing) {
    await github.rest.git.createRef({ ...context.repo, ref: `refs/tags/${tag}`, sha });
  }
  core.setOutput('should_release', 'true');
  core.setOutput('release_id', String(release.id));
  core.setOutput('sha', sha);
  core.setOutput('tag', tag);
}

function validateAssets(assets) {
  for (const extension of ['.AppImage', '.exe', '.msi']) {
    if (!assets.some(asset => asset.name.endsWith(extension) && asset.size > 0 && asset.state === 'uploaded')) {
      throw new Error(`Instalador ausente ou incompleto: ${extension}`);
    }
  }
}

async function publish({ github, context }, id, tag, sha) {
  const release = (await github.rest.repos.getRelease({ ...context.repo, release_id: Number(id) })).data;
  if (release.tag_name !== tag || release.target_commitish !== sha ||
      await tagCommit(github, context.repo, tag) !== sha) {
    throw new Error('A tag ou o commit da release mudou durante o build.');
  }
  const assets = await github.paginate(github.rest.repos.listReleaseAssets, {
    ...context.repo, release_id: release.id, per_page: 100,
  });
  validateAssets(assets);
  if (!release.draft) return; // A retry never republishes or changes a published release.
  await github.rest.repos.updateRelease({
    ...context.repo, release_id: release.id, draft: false, prerelease: false, make_latest: 'true',
  });
}

async function syncDevelop({ github, context }) {
  const comparison = (await github.rest.repos.compareCommitsWithBasehead({
    ...context.repo, basehead: 'develop...main',
  })).data;
  if (comparison.ahead_by === 0) return;
  const prs = (await github.rest.pulls.list({
    ...context.repo, state: 'open', base: 'develop', head: `${context.repo.owner}:main`,
  })).data;
  const pr = prs[0] || (await github.rest.pulls.create({
    ...context.repo, head: 'main', base: 'develop',
    title: 'chore: sincroniza main com develop',
    body: 'Traz versões, changelog e hotfixes da main para develop.\n\nUse **Create a merge commit** para preservar o histórico compartilhado.',
  })).data;
  await dispatchCI(github, context.repo, pr);
}

module.exports = { validatePullRequest, checkPullRequest, checkReleasePRs, tagCommit, selectDraft, validateAssets, publish, syncDevelop };
