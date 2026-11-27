"""Regression tests for the release-blocking Rune AppImage inventory."""
from pathlib import Path
import tempfile
import unittest
from verify_appimage import validate_appdir


class AppImageInventoryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "usr/bin").mkdir(parents=True)
        for name in ("AppRun", "usr/bin/engineroom-rune", "usr/bin/stockfish"):
            path = self.root / name
            path.touch()
            path.chmod(0o755)

    def test_valid_inventory(self):
        validate_appdir(self.root)

    def test_host_graphics_must_not_be_bundled(self):
        for name in ("libwayland-client.so.0", "libEGL.so.1", "libGLESv2.so.2",
                     "libvulkan.so.1", "libvulkan_lvp.so", "libgbm.so.1"):
            with self.subTest(name=name):
                library = self.root / name
                library.touch()
                with self.assertRaisesRegex(ValueError, "shadows host graphics"):
                    validate_appdir(self.root)
                library.unlink()

    def test_missing_engine_blocks_release(self):
        (self.root / "usr/bin/stockfish").unlink()
        with self.assertRaisesRegex(ValueError, "usr/bin/stockfish"):
            validate_appdir(self.root)


if __name__ == "__main__":
    unittest.main()
