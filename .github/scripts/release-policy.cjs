const STABLE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

function compareVersions(left, right) {
  if (!STABLE.test(left) || !STABLE.test(right)) {
    throw new Error('A versão da release precisa usar SemVer estável.');
  }
  const a = left.split('.').map(BigInt);
  const b = right.split('.').map(BigInt);
  for (let index = 0; index < a.length; index++) {
    if (a[index] !== b[index]) return a[index] > b[index] ? 1 : -1;
  }
  return 0;
}

async function packageVersion(github, repo, ref) {
  const { data } = await github.rest.repos.getContent({
    ...repo, path: 'app/package.json', ref,
  });
  return JSON.parse(Buffer.from(data.content, 'base64').toString('utf8')).version;
}

async function checkReleaseVersion({ github, context }, pr) {
  if (pr.base.ref !== 'main' || pr.user.login !== 'github-actions[bot]' ||
      !pr.head.ref.startsWith('release-please--')) return;

  const [base, next] = await Promise.all([
    packageVersion(github, context.repo, pr.base.sha),
    packageVersion(github, context.repo, pr.head.sha),
  ]);
  if (compareVersions(next, base) <= 0) {
    throw new Error(`A versão ${next} precisa ser maior que a versão base ${base}.`);
  }
  const releases = await github.paginate(github.rest.repos.listReleases, {
    ...context.repo, per_page: 100,
  });
  for (const release of releases) {
    const version = release.tag_name.replace(/^v/, '');
    if (STABLE.test(version) && compareVersions(next, version) <= 0) {
      throw new Error(`A versão ${next} já foi reservada ou foi superada por ${release.tag_name}.`);
    }
  }
  try {
    await github.rest.git.getRef({ ...context.repo, ref: `tags/v${next}` });
  } catch (error) {
    if (error.status === 404) return;
    throw error;
  }
  throw new Error(`A tag v${next} já existe; não reutilize versões.`);
}

async function checkPublicationVersion({ github, context }, candidate) {
  const version = candidate.tag_name.replace(/^v/, '');
  if (!STABLE.test(version)) {
    throw new Error('A versão da release precisa usar SemVer estável.');
  }
  const releases = await github.paginate(github.rest.repos.listReleases, {
    ...context.repo, per_page: 100,
  });
  for (const release of releases) {
    if (release.draft || release.prerelease) continue;
    const published = release.tag_name.replace(/^v/, '');
    if (STABLE.test(published) && compareVersions(version, published) <= 0) {
      throw new Error(`Não publique ${candidate.tag_name} após ${release.tag_name}.`);
    }
  }
}

module.exports = { compareVersions, checkReleaseVersion, checkPublicationVersion };
