"""Keep the host Mesa driver paired with the host Wayland client library.

Run after `tauri build` and before uploading/signing the AppImage. Requires
squashfs-tools; preserves the original AppImage runtime and SquashFS format.
"""

import argparse
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile


COMPRESSORS = {1: "gzip", 2: "lzma", 3: "lzo", 4: "xz", 5: "lz4", 6: "zstd"}


def run(*args):
    try:
        return subprocess.run(args, check=True, capture_output=True, text=True).stdout
    except subprocess.CalledProcessError as error:
        error.add_note(error.stderr.strip())
        raise


def wayland_clients(appdir):
    return sorted(appdir.rglob("libwayland-client.so*"))


def prepare(image):
    image = image.resolve(strict=True)
    offset = int(run(str(image), "--appimage-offset").strip())
    with image.open("rb") as source:
        if not 0 < offset < image.stat().st_size:
            raise ValueError("Invalid AppImage filesystem offset")
        runtime = source.read(offset)
        header = source.read(24)
    magic, _, _, block_size, _, compression, _ = struct.unpack("<4sIIIIHH", header)
    if magic != b"hsqs" or compression not in COMPRESSORS:
        raise ValueError("Unsupported AppImage SquashFS format")

    # Same filesystem as the destination so replacement is atomic. Any failed
    # extraction, packing or verification leaves the original artifact intact.
    with tempfile.TemporaryDirectory(prefix=".appimage-", dir=image.parent) as tmp:
        tmp = Path(tmp)
        appdir = tmp / "AppDir"
        run("unsquashfs", "-no-progress", "-o", str(offset), "-d", str(appdir), str(image))
        clients = wayland_clients(appdir)
        for library in clients:
            library.unlink()

        filesystem = tmp / "filesystem.squashfs"
        run(
            "mksquashfs", str(appdir), str(filesystem), "-noappend", "-no-progress",
            "-comp", COMPRESSORS[compression], "-b", str(block_size), "-all-root",
        )
        candidate = tmp / image.name
        with candidate.open("wb") as output, filesystem.open("rb") as source:
            output.write(runtime)
            shutil.copyfileobj(source, output)
        candidate.chmod(image.stat().st_mode & 0o777)

        # Inspect the actual repacked artifact, not just the staging directory.
        verified = tmp / "verified"
        run("unsquashfs", "-no-progress", "-o", str(offset), "-d", str(verified), str(candidate))
        if wayland_clients(verified):
            raise ValueError("AppImage still bundles libwayland-client")
        candidate.replace(image)
    print(f"Prepared {image.name}: removed {len(clients)} bundled Wayland client entries")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("images", type=Path, nargs="+")
    for image in parser.parse_args().images:
        prepare(image)
