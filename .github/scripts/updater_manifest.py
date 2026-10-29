"""Generate the Tauri update feed from the exact signed release installers."""
import argparse
import json
from pathlib import Path


def build_manifest(release, signatures):
    platforms = {}
    for platform, suffix in [('linux-x86_64-appimage', '.AppImage'), ('windows-x86_64-nsis', '.exe'), ('windows-x86_64-msi', '.msi')]:
        matches = [asset for asset in release['assets'] if asset['name'].endswith(suffix)]
        if len(matches) != 1:
            raise ValueError(f'Expected exactly one {suffix} installer')
        asset = matches[0]
        signature = (signatures / (asset['name'] + '.sig')).read_text().strip()
        if not signature or asset.get('size', 0) <= 0:
            raise ValueError(f'Empty installer or signature: {asset["name"]}')
        url = asset['url']
        if not url.startswith('https://github.com/'):
            raise ValueError(f'Expected HTTPS GitHub download: {url}')
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
    args = parser.parse_args()
    manifest = build_manifest(json.loads(args.release.read_text()), args.signatures)
    args.output.write_text(json.dumps(manifest, indent=2) + '\n')
