"""Reject AppImages that shadow the host graphics stack. Never modify the artifact."""

import argparse
from fnmatch import fnmatchcase
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time


# These libraries must match the host's Mesa/proprietary driver. In particular,
# an older bundled Wayland client can prevent the host EGL driver from loading.
# Policy: https://github.com/AppImageCommunity/pkg2appimage/blob/master/excludelist
HOST_LIBRARIES = (
    "libwayland-client.so*",
    "libEGL*.so*",
    "libGL.so*",
    "libGLX*.so*",
    "libGLdispatch.so*",
    "libOpenGL.so*",
    "libgbm.so*",
    "libdrm*.so*",
)


def validate_appdir(appdir):
    if not (appdir / "AppRun").is_file():
        raise ValueError("Extracted AppImage is missing AppRun")
    forbidden = sorted(
        str(path.relative_to(appdir))
        for path in appdir.rglob("*")
        if any(fnmatchcase(path.name, pattern) for pattern in HOST_LIBRARIES)
    )
    if forbidden:
        raise ValueError(
            "AppImage shadows host graphics libraries; fix the bundler before publishing:\n"
            + "\n".join(forbidden)
        )


def webkit_running(process_group):
    for proc in Path("/proc").glob("[0-9]*"):
        try:
            if os.getpgid(int(proc.name)) == process_group and (proc / "exe").readlink().name == "WebKitWebProcess":
                return True
        except OSError:
            continue
    return False


def smoke_test(appdir):
    env = os.environ.copy()
    for key in ("APPDIR", "APPIMAGE", "LD_PRELOAD", "LD_LIBRARY_PATH", "EGL_PLATFORM"):
        env.pop(key, None)
    for key in ("XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME"):
        env[key] = str(appdir.parent / key.lower())
    # Exercise the host Mesa EGL driver even on CI machines without a GPU.
    env["LIBGL_ALWAYS_SOFTWARE"] = "1"
    with tempfile.TemporaryFile(mode="w+") as log:
        process = subprocess.Popen(
            [str(appdir / "AppRun")], cwd=appdir, env=env,
            stdout=log, stderr=log, start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 10
            while process.poll() is None and time.monotonic() < deadline:
                time.sleep(0.25)
            log.seek(0)
            output = log.read()
            webkit_alive = webkit_running(process.pid)
            if process.poll() is not None or not webkit_alive or "EGL_BAD" in output:
                raise RuntimeError(
                    f"AppImage/WebKit startup failed (exit={process.poll()}, WebKit alive={webkit_alive}):\n{output}"
                )
        finally:
            try:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            except ProcessLookupError:
                process.wait()


def verify(image, smoke=False):
    image = image.resolve(strict=True)
    # The AppImage runtime provides extraction without FUSE. Use a disposable
    # directory; no rewriting, format parsing, signing or repacking here.
    with tempfile.TemporaryDirectory(prefix="engineroom-appimage-") as tmp:
        try:
            subprocess.run(
                [str(image), "--appimage-extract"], cwd=tmp,
                check=True, capture_output=True, text=True,
            )
        except subprocess.CalledProcessError as error:
            error.add_note(error.stderr.strip())
            raise
        appdir = Path(tmp) / "squashfs-root"
        validate_appdir(appdir)
        if smoke:
            smoke_test(appdir)
    print(f"Verified {image.name}: uses the host graphics libraries")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--smoke", action="store_true", help="Also start the app and verify WebKit under an existing X display")
    parser.add_argument("images", type=Path, nargs="+")
    args = parser.parse_args()
    for image in args.images:
        verify(image, smoke=args.smoke)
