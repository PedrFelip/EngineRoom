#!/usr/bin/env python3
"""Build native Rune installers; run from any directory with Python 3.11+."""
import json
import os
from pathlib import Path
import platform
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def run(*args, cwd=ROOT, env=None):
    subprocess.run(args, cwd=cwd, env=env, check=True)


def main():
    system = platform.system()
    if system not in ("Linux", "Windows"):
        raise SystemExit("Only native Linux AppImage and Windows NSIS builds are supported")
    config = json.loads((ROOT / "packaging" / f"{system.lower()}.json").read_text())
    config["version"] = tomllib.loads(
        (ROOT / "crates/engineroom-rune/Cargo.toml").read_text()
    )["package"]["version"]
    run("node", "scripts/fetch-stockfish.mjs", cwd=ROOT.parent / "app")
    run("cargo", "build", "--release", "--locked", "-p", "engineroom-rune")
    output = ROOT / "target/packages"
    output.mkdir(parents=True, exist_ok=True)
    # Refuse stale artifacts after a failed build or verification.
    for pattern in ("*.AppImage", "*.exe"):
        for old in output.glob(pattern):
            old.unlink()
    # cargo-packager resolves paths relative to the generated config directory.
    for key in ("licenseFile", "binariesDir", "outDir"):
        config[key] = str((ROOT / config[key]).resolve())
    for key in ("externalBinaries", "resources", "icons"):
        config[key] = [str((ROOT / path).resolve()) for path in config[key]]
    if system == "Windows":
        config["nsis"]["installerIcon"] = str(
            (ROOT / config["nsis"]["installerIcon"]).resolve()
        )
    generated = ROOT / "target/packager.json"
    generated.write_text(json.dumps(config, indent=2))
    env = os.environ.copy()
    env.pop("APPIMAGE_EXTRACT_AND_RUN", None)
    run("cargo", "packager", "--release", "--config", str(generated), env=env)
    if system == "Linux":
        images = list(output.glob("*.AppImage"))
        if not images:
            raise SystemExit("cargo-packager produced no AppImage")
        try:
            run("python3", "scripts/verify_appimage.py", "--smoke", *map(str, images))
        except BaseException:
            for image in images:
                image.unlink()
            raise
    elif not list(output.glob("*.exe")):
        raise SystemExit("cargo-packager produced no NSIS installer")


if __name__ == "__main__":
    main()
