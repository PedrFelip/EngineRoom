"""Validate synchronized app versions and the snapshot selected for release."""

import json
import argparse
from pathlib import Path
import re
import tomllib


CONFIG = "app/src-tauri/tauri.conf.json"
SEMVER = re.compile(
    r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


def validate_versions(read):
    version = json.loads(read(CONFIG))["version"]
    if not isinstance(version, str) or not SEMVER.fullmatch(version):
        raise ValueError(f"Invalid release version: {version!r}")
    cargo = tomllib.loads(read("app/src-tauri/Cargo.toml"))["package"]
    lock = tomllib.loads(read("app/src-tauri/Cargo.lock"))["package"]
    app_entries = [
        p for p in lock if p["name"] == cargo["name"] and "source" not in p
    ]
    if len(app_entries) != 1:
        raise ValueError("Cargo.lock must contain exactly one local app package")
    versions = {
        "app/package.json": json.loads(read("app/package.json"))["version"],
        "app/src-tauri/Cargo.toml": cargo["version"],
        "app/src-tauri/Cargo.lock": app_entries[0]["version"],
    }
    for path, observed in versions.items():
        if observed != version:
            raise ValueError(f"{path}: version {observed!r} differs from {version!r}")
    return version



def validate_release(read, tag):
    version = validate_versions(read)
    if tag != f"v{version}" or not re.fullmatch(r"v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", tag):
        raise ValueError("Release tag must match the stable app version")
    manifest = json.loads(read(".release-please-manifest.json"))
    if manifest.get("app") != version:
        raise ValueError("Release Please manifest differs from the app version")
    changelog = read("app/CHANGELOG.md")
    if not re.search(r"^#{1,2} \[?" + re.escape(version) + r"(?:\]|\s|$)", changelog, re.MULTILINE):
        raise ValueError("Changelog must contain the selected release version")
    return version


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", help="Also validate the release manifest and changelog")
    args = parser.parse_args()
    read = lambda path: Path(path).read_text()
    version = validate_release(read, args.tag) if args.tag else validate_versions(read)
    print(f"Validated app version: {version}")


if __name__ == "__main__":
    main()
