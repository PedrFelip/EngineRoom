import json
from pathlib import Path
import unittest

from release_version import CONFIG, validate_release, validate_versions


class ReleaseVersionTests(unittest.TestCase):
    def manifests(self, version="1.2.3"):
        return {
            CONFIG: json.dumps({"version": version}),
            "app/package.json": json.dumps({"version": version}),
            "app/src-tauri/Cargo.toml": f'[package]\nname = "app"\nversion = "{version}"',
            "app/src-tauri/Cargo.lock": (
                f'[[package]]\nname = "app"\nversion = "{version}"\n'
                '[[package]]\nname = "dependency"\nversion = "9.0.0"'
            ),
        }

    def test_repository_manifests_are_synchronized(self):
        root = Path(__file__).resolve().parents[2]
        version = validate_versions(lambda path: (root / path).read_text())
        self.assertIsInstance(version, str)

    def test_valid_manifests(self):
        for version in ["1.2.3", "0.10.0-rc.1", "1.2.3+build.7"]:
            with self.subTest(version=version):
                self.assertEqual(
                    validate_versions(self.manifests(version).__getitem__), version
                )

    def test_each_mismatched_manifest_is_rejected(self):
        for path in self.manifests():
            if path == CONFIG:
                continue
            with self.subTest(path=path):
                files = self.manifests()
                files[path] = files[path].replace("1.2.3", "1.2.4")
                with self.assertRaises(ValueError):
                    validate_versions(files.__getitem__)

    def test_invalid_versions_are_rejected(self):
        for version in [
            "v1.2.3", "01.2.3", "1.2", "1.2.3-01", "1.2.3\ninjected=true"
        ]:
            with self.subTest(version=version), self.assertRaises(ValueError):
                validate_versions(self.manifests(version).__getitem__)

    def release_files(self):
        files = self.manifests()
        files[".release-please-manifest.json"] = '{"app": "1.2.3"}'
        files["app/CHANGELOG.md"] = "# Changelog\n\n## [1.2.3](https://example.com) (2026-10-23)\n\nCorreções"
        return files

    def test_release_snapshot(self):
        self.assertEqual(validate_release(self.release_files().__getitem__, "v1.2.3"), "1.2.3")

    def test_mismatched_tag_or_missing_release_metadata(self):
        for path, value in [
            (".release-please-manifest.json", '{"app": "1.2.2"}'),
            ("app/CHANGELOG.md", "# Changelog\n\n## 1.2.30\n"),
        ]:
            with self.subTest(path=path), self.assertRaises(ValueError):
                files = self.release_files()
                files[path] = value
                validate_release(files.__getitem__, "v1.2.3")
        with self.assertRaises(ValueError):
            validate_release(self.release_files().__getitem__, "v1.2.4")

    def test_only_stable_releases(self):
        with self.assertRaises(ValueError):
            validate_release(self.manifests("1.2.3-beta.1").__getitem__, "v1.2.3-beta.1")

    def test_duplicate_or_missing_local_app_is_rejected(self):
        for lock in [
            '[[package]]\nname = "app"\nversion = "1.2.3"\nsource = "registry+example"',
            '[[package]]\nname = "app"\nversion = "1.2.3"\n' * 2,
        ]:
            with self.subTest(lock=lock), self.assertRaises(ValueError):
                files = self.manifests()
                files["app/src-tauri/Cargo.lock"] = lock
                validate_versions(files.__getitem__)


if __name__ == "__main__":
    unittest.main()
