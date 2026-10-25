from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "app/scripts"))
from prepare_appimage import prepare, run


@unittest.skipUnless(shutil.which("mksquashfs") and shutil.which("unsquashfs"), "requires squashfs-tools")
class AppImageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        appdir = self.root / "AppDir"
        lib = appdir / "usr/lib"
        lib.mkdir(parents=True)
        (lib / "libwayland-client.so.0.20.0").write_bytes(b"old Wayland")
        (lib / "libwayland-client.so.0").symlink_to("libwayland-client.so.0.20.0")
        (lib / "libwebkit2gtk-4.1.so.0").write_bytes(b"WebKit")
        (appdir / "LICENSE").write_text("GPL-3.0-or-later")
        (appdir / "AppRun").write_text("#!/bin/sh\nexit 0\n")
        (appdir / "AppRun").chmod(0o755)
        filesystem = self.root / "original.squashfs"
        run("mksquashfs", str(appdir), str(filesystem), "-noappend", "-comp", "xz", "-b", "131072")
        # A small executable runtime exercises offset discovery and preservation
        # without requiring FUSE, Tauri or a downloaded AppImage in CI.
        self.runtime = b'#!/bin/sh\n[ "$1" = --appimage-offset ] || exit 1\necho 4096\nexit 0\n'.ljust(4096, b"\0")
        self.image = self.root / "EngineRoom test.AppImage"
        self.image.write_bytes(self.runtime + filesystem.read_bytes())
        self.image.chmod(0o755)

    def test_repacked_artifact_uses_host_wayland_and_preserves_other_files(self):
        prepare(self.image)
        prepare(self.image)  # Also handles a bundler that already excludes it.
        self.assertEqual(self.image.read_bytes()[:4096], self.runtime)
        self.assertEqual(self.image.stat().st_mode & 0o777, 0o755)
        extracted = self.root / "result"
        run("unsquashfs", "-o", "4096", "-d", str(extracted), str(self.image))
        self.assertEqual(list(extracted.rglob("libwayland-client.so*")), [])
        self.assertEqual((extracted / "usr/lib/libwebkit2gtk-4.1.so.0").read_bytes(), b"WebKit")
        self.assertEqual((extracted / "LICENSE").read_text(), "GPL-3.0-or-later")
        self.assertEqual((extracted / "AppRun").stat().st_mode & 0o777, 0o755)

    def test_failed_repack_does_not_replace_original(self):
        original = self.image.read_bytes()

        def fail_packing(*args):
            if args[0] == "mksquashfs":
                raise subprocess.CalledProcessError(1, args)
            return run(*args)

        with patch("prepare_appimage.run", side_effect=fail_packing):
            with self.assertRaises(subprocess.CalledProcessError):
                prepare(self.image)
        self.assertEqual(self.image.read_bytes(), original)
        self.assertEqual(list(self.root.glob(".appimage-*")), [])

    def test_invalid_filesystem_is_rejected_without_modification(self):
        original = self.runtime + b"invalid filesystem" * 2
        self.image.write_bytes(original)
        with self.assertRaises(ValueError):
            prepare(self.image)
        self.assertEqual(self.image.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
