import tempfile
import unittest
from pathlib import Path

from updater_manifest import build_manifest


class UpdaterManifestTests(unittest.TestCase):
    def test_feed_contains_matching_signatures_and_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            signatures = Path(directory)
            assets = []
            for name in ['EngineRoom.AppImage', 'EngineRoom.exe', 'EngineRoom.msi']:
                (signatures / (name + '.sig')).write_text('signature-' + name)
                assets.append({'name': name, 'size': 100, 'url': 'https://github.com/owner/repo/releases/download/v3.0.0/' + name})
            release = {'tagName': 'v3.0.0', 'body': 'Changes', 'createdAt': '2026-10-26T00:00:00Z', 'assets': assets}
            manifest = build_manifest(release, signatures)
            self.assertEqual(manifest['version'], '3.0.0')
            self.assertEqual(manifest['platforms']['windows-x86_64-nsis']['signature'], 'signature-EngineRoom.exe')
            self.assertEqual(manifest['platforms']['linux-x86_64-appimage']['url'], assets[0]['url'])
            self.assertEqual(manifest['platforms']['windows-x86_64-msi']['signature'], 'signature-EngineRoom.msi')
            self.assertEqual(manifest['platforms']['windows-x86_64-msi']['url'], assets[2]['url'])
            self.assertNotIn('windows-x86_64', manifest['platforms'])
            (signatures / 'EngineRoom.exe.sig').unlink()
            with self.assertRaises(FileNotFoundError):
                build_manifest(release, signatures)
            (signatures / 'EngineRoom.exe.sig').write_text('')
            with self.assertRaises(ValueError):
                build_manifest(release, signatures)
            release['assets'] = assets[:1]
            with self.assertRaises(ValueError):
                build_manifest(release, signatures)
