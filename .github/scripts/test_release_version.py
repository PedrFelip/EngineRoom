import json
from pathlib import Path
import unittest

from release_version import validate_release, validate_versions


class RuneRootWorkspaceVersionTests(unittest.TestCase):
    """Root desktop workspace release validation."""

    member = "crates/engineroom-rune"
    manifest = ".release-please-manifest.json"
    changelog = "CHANGELOG.md"

    def manifests(self, version="3.0.0"):
        return {
            "Cargo.toml": (
                '[workspace]\nmembers = []\n\n'
                '[workspace.package]\nversion = "9.9.9"\n\n'
                f'[package]\nname = "engineroom-rune"\nversion = "{version}"\n'
            ),
            "Cargo.lock": (
                f'[[package]]\nname = "engineroom-rune"\nversion = "{version}"\n'
            ),
        }

    def validate(self, files, tag=None):
        return (
            validate_release(files.__getitem__, tag, "rune", self.manifest, self.changelog)
            if tag else validate_versions(files.__getitem__, "rune")
        )

    def test_root_package_version_is_the_single_source_of_truth(self):
        self.assertEqual(self.validate(self.manifests()), "3.0.0")
        files = self.manifests("2.3.4")
        self.assertEqual(self.validate(files), "2.3.4")

    def test_mismatched_lock_is_rejected(self):
        files = self.manifests()
        files["Cargo.lock"] = files["Cargo.lock"].replace(
            'name = "engineroom-rune"\nversion = "3.0.0"',
            'name = "engineroom-rune"\nversion = "3.0.1"',
        )
        with self.assertRaises(ValueError):
            self.validate(files)

    def test_invalid_root_versions_are_rejected(self):
        for version in ["v3.0.0", "03.0.0", "3.0"]:
            with self.subTest(version=version), self.assertRaises(ValueError):
                self.validate(self.manifests(version))

    def test_prerelease_cannot_be_published_as_stable(self):
        files = self.manifests("3.0.0-alpha.3")
        files[self.manifest] = '{".": "3.0.0-alpha.3"}'
        files[self.changelog] = "# Changelog\n\n## 3.0.0-alpha.3\n"
        with self.assertRaises(ValueError):
            self.validate(files, "v3.0.0-alpha.3")

    def test_duplicate_or_registry_only_lock_entry_is_rejected(self):
        for lock in [
            '[[package]]\nname = "engineroom-rune"\nversion = "3.0.0"\n' * 2,
            '[[package]]\nname = "engineroom-rune"\nversion = "3.0.0"\nsource = "registry+example"\n',
        ]:
            files = self.manifests()
            files["Cargo.lock"] = lock
            with self.subTest(lock=lock), self.assertRaises(ValueError):
                self.validate(files)

    def test_release_snapshot_uses_root_manifest_and_changelog(self):
        files = self.manifests()
        files[self.manifest] = '{".": "3.0.0"}'
        files[self.changelog] = "# Changelog\n\n## [3.0.0](https://example.com) (2026-11-26)\n\nCorreções"
        self.assertEqual(self.validate(files, "v3.0.0"), "3.0.0")
        with self.assertRaises(ValueError):
            self.validate(files, "v3.0.1")

    def test_missing_or_wrong_manifest_and_changelog_are_rejected(self):
        files = self.manifests()
        files[self.manifest] = '{"app": "3.0.0"}'
        files[self.changelog] = "# Changelog\n\n## 3.0.01\n"
        with self.assertRaises(ValueError):
            self.validate(files, "v3.0.0")


if __name__ == "__main__":
    unittest.main()
