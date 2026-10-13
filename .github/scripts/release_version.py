"""Validate release manifests and select version-based GitHub Actions outputs."""

import json
import os
from pathlib import Path
import re
import subprocess
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


def select_release(version, event_name, event, previous_version=None):
    tag = f"v{version}"
    if event_name == "push" and event["ref"].startswith("refs/tags/"):
        if event["ref"] != f"refs/tags/{tag}":
            raise ValueError(f"Tag must match the app version: {tag}")
        should_release = True
    elif event_name == "push":
        if event["ref"] != "refs/heads/main":
            raise ValueError("Automatic releases require the main branch")
        should_release = previous_version != version
    elif event_name == "workflow_dispatch":
        should_release = True
    else:
        raise ValueError(f"Unsupported event: {event_name}")
    return {
        "should_release": str(should_release).lower(),
        "version": version,
        "tag": tag,
        "prerelease": str("-" in version.split("+")[0]).lower(),
    }


def main():
    version = validate_versions(lambda path: Path(path).read_text())
    event_name = os.environ["GITHUB_EVENT_NAME"]
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    previous_version = None
    if event_name == "push" and event["ref"] == "refs/heads/main":
        before = event["before"]
        if before != "0" * 40:
            previous_config = subprocess.check_output(
                ["git", "show", f"{before}:{CONFIG}"], text=True
            )
            previous_version = json.loads(previous_config)["version"]
    outputs = select_release(version, event_name, event, previous_version)
    outputs["sha"] = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], text=True
    ).strip()
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for key, value in outputs.items():
            output.write(f"{key}={value}\n")
    print(json.dumps(outputs))


if __name__ == "__main__":
    main()
