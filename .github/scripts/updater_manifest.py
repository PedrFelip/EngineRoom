"""Generate the Tauri update feed from the exact signed release installers."""
import argparse
import json
from pathlib import Path
from urllib.parse import quote


def build_manifest(release, signatures, repository, tag):
    if release['tagName'] != tag or not tag or tag.startswith('untagged-'):
        raise ValueError('Release tag does not match the intended published tag')
    parts = repository.split('/')
    if len(parts) != 2 or any(not part or part in ('.', '..') for part in parts):
        raise ValueError('Expected GitHub repository as owner/name')
    platforms = {}
    for platform, suffix in [('linux-x86_64-appimage', '.AppImage'), ('windows-x86_64-nsis', '.exe'), ('windows-x86_64-msi', '.msi')]:
        matches = [asset for asset in release['assets'] if asset['name'].endswith(suffix)]
        if len(matches) != 1:
            raise ValueError(f'Expected exactly one {suffix} installer')
        asset = matches[0]
        signature = (signatures / (asset['name'] + '.sig')).read_text().strip()
        if not signature or asset.get('size', 0) <= 0:
            raise ValueError(f'Empty installer or signature: {asset["name"]}')
        # Draft release asset URLs can contain a temporary untagged-* identifier.
        # Build the future public URL from the intended tag and exact asset name.
        url = 'https://github.com/{}/{}/releases/download/{}/{}'.format(
            *(quote(part, safe='') for part in (*parts, tag, asset['name']))
        )
        platforms[platform] = {'signature': signature, 'url': url}
    return {
        'version': release['tagName'].removeprefix('v'),
        'notes': release.get('body') or '',
        'pub_date': release['createdAt'],
        'platforms': platforms,
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', type=Path, required=True)
    parser.add_argument('--signatures', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repository', required=True)
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    manifest = build_manifest(json.loads(args.release.read_text()), args.signatures, args.repository, args.tag)
    args.output.write_text(json.dumps(manifest, indent=2) + '\n')
