from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "app/scripts"))
from verify_appimage import smoke_test, validate_appdir


class AppImageInventoryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.appdir = Path(self.tmp.name)
        (self.appdir / "AppRun").write_text("#!/bin/sh\nexit 0\n")

    def library(self, relative):
        path = self.appdir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"library")
        return path

    def test_accepts_private_app_libraries_and_does_not_change_files(self):
        path = self.library("usr/lib/libwebkit2gtk-4.1.so.0")
        self.library("usr/lib/libglib-2.0.so.0")
        self.library("usr/lib/libwayland-cursor.so.0")
        validate_appdir(self.appdir)
        self.assertEqual(path.read_bytes(), b"library")

    def test_rejects_driver_dependencies_including_versioned_and_multiarch_paths(self):
        for relative in [
            "usr/lib/libwayland-client.so.0",
            "usr/lib/x86_64-linux-gnu/libwayland-client.so.0.20.0",
            "usr/lib/libEGL.so.1", "usr/lib/libGL.so.1",
            "usr/lib/libEGL_mesa.so.0", "usr/lib/libGLX_mesa.so.0",
            "usr/lib/libGLX.so.0", "usr/lib/libGLdispatch.so.0",
            "usr/lib/libOpenGL.so.0", "usr/lib64/libgbm.so.1",
            "usr/lib/libdrm.so.2", "usr/lib/libdrm_amdgpu.so.1",
        ]:
            with self.subTest(relative=relative):
                path = self.library(relative)
                with self.assertRaises(ValueError) as error:
                    validate_appdir(self.appdir)
                self.assertIn(relative, str(error.exception))
                self.assertTrue(path.exists(), "Verification must not delete libraries")
                path.unlink()

    def test_rejects_symlink_even_when_target_is_missing(self):
        path = self.appdir / "libwayland-client.so.0"
        path.symlink_to("missing-library")
        with self.assertRaises(ValueError):
            validate_appdir(self.appdir)
        self.assertTrue(path.is_symlink())

    def test_empty_or_failed_extraction_cannot_pass(self):
        (self.appdir / "AppRun").unlink()
        with self.assertRaisesRegex(ValueError, "missing AppRun"):
            validate_appdir(self.appdir)

    def test_startup_failure_blocks_smoke_check(self):
        apprun = self.appdir / "AppRun"
        apprun.write_text("#!/bin/sh\necho startup-failure >&2\nexit 42\n")
        apprun.chmod(0o755)
        with self.assertRaisesRegex(RuntimeError, "exit=42") as error:
            smoke_test(self.appdir)
        self.assertIn("startup-failure", str(error.exception))


if __name__ == "__main__":
    unittest.main()
