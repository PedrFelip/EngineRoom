import json
from pathlib import Path
import unittest

from release_version import CONFIG, select_release, validate_versions


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
        self.assertEqual(
            select_release(version, "workflow_dispatch", {})["tag"], f"v{version}"
        )

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

    def test_branch_push_only_releases_changed_version(self):
        event = {"ref": "refs/heads/main"}
        self.assertEqual(
            select_release("1.2.3", "push", event, "1.2.3")["should_release"], "false"
        )
        self.assertEqual(
            select_release("1.2.3", "push", event, "1.2.2")["should_release"], "true"
        )

    def test_manual_run_uses_version_not_branch(self):
        result = select_release(
            "1.2.3", "workflow_dispatch", {"ref": "refs/heads/main"}
        )
        self.assertEqual(result["tag"], "v1.2.3")
        self.assertEqual(result["should_release"], "true")

    def test_tag_must_match_version(self):
        self.assertEqual(
            select_release("1.2.3", "push", {"ref": "refs/tags/v1.2.3"})[
                "should_release"
            ],
            "true",
        )
        with self.assertRaises(ValueError):
            select_release("1.2.3", "push", {"ref": "refs/tags/v1.2.4"})

    def test_prerelease_excludes_build_metadata(self):
        self.assertEqual(
            select_release("1.2.3-rc.1", "workflow_dispatch", {})["prerelease"], "true"
        )
        self.assertEqual(
            select_release("1.2.3+build-test", "workflow_dispatch", {})["prerelease"],
            "false",
        )


if __name__ == "__main__":
    unittest.main()
