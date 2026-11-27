"""Validate synchronized versions for the active product and release snapshot.

The package version and Cargo.lock entry identify the release snapshot.
"""

import argparse
import json
import re
import tomllib
from pathlib import Path

RUNE_CARGO = "Cargo.toml"
RUNE_LOCK = "Cargo.lock"
SEMVER = re.compile(
    r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)
STABLE = re.compile(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)")
def validate_rune_versions(read):
    """Validate the desktop crate version and its lockfile entry."""
    package = tomllib.loads(read(RUNE_CARGO))["package"]
    version = package.get("version")
    if not isinstance(version, str) or not SEMVER.fullmatch(version):
        raise ValueError(f"Invalid desktop version: {version!r}")
    entries = [p for p in tomllib.loads(read(RUNE_LOCK)).get("package", [])
               if p.get("name") == "engineroom-rune" and "source" not in p]
    if len(entries) != 1 or entries[0].get("version") != version:
        raise ValueError("Cargo.lock desktop version differs from Cargo.toml")
    return version


def validate_versions(read, product="rune"):
    if product != "rune":
        raise ValueError(f"Unknown release product: {product!r}")
    return validate_rune_versions(read)


def validate_release(read, tag, product="rune", manifest=None, changelog=None):
    version = validate_versions(read, product)
    if tag != f"v{version}" or not STABLE.fullmatch(tag.removeprefix("v")):
        raise ValueError("Release tag must match the stable app version")
    manifest = manifest or ".release-please-manifest.json"
    if json.loads(read(manifest)).get(".") != version:
        raise ValueError("Release Please manifest differs from the app version")
    changelog = changelog or "CHANGELOG.md"
    content = read(changelog)
    if not re.search(r"^#{1,2} \[?" + re.escape(version) + r"(?:\]|\s|$)", content, re.MULTILINE):
        raise ValueError("Changelog must contain the selected release version")
    return version


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", help="Also validate the release manifest and changelog")
    parser.add_argument("--product", choices=["rune"], default="rune")
    parser.add_argument("--manifest", help="Override the Release Please manifest path")
    parser.add_argument("--changelog", help="Override the changelog path")
    args = parser.parse_args()
    read = lambda path: Path(path).read_text()
    version = (
        validate_release(read, args.tag, args.product, args.manifest, args.changelog)
        if args.tag else validate_versions(read, args.product)
    )
    print(f"Validated {args.product} version: {version}")


if __name__ == "__main__":
    main()
