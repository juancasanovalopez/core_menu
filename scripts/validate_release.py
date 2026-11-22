"""Validate the stable release contract before building or publishing."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib


STABLE_VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")


def stable_version(version: str) -> tuple[int, int, int]:
    match = STABLE_VERSION.fullmatch(version)
    if match is None:
        raise ValueError(f"Expected stable MAJOR.MINOR.PATCH, got {version!r}")
    major, minor, patch = match.groups()
    return int(major), int(minor), int(patch)


def validate_release(root: Path, tag: str, tags: list[str]) -> None:
    if not tag.startswith("v"):
        raise ValueError("Release tag must begin with v")
    version = tag[1:]
    current = stable_version(version)
    with (root / "Cargo.toml").open("rb") as source:
        package = tomllib.load(source)["package"]
    if package["version"] != version:
        raise ValueError("Tag must match Cargo.toml")

    with (root / "Cargo.lock").open("rb") as source:
        packages = tomllib.load(source)["package"]
    locked = [
        entry["version"]
        for entry in packages
        if entry["name"] == package["name"] and "source" not in entry
    ]
    if locked != [version]:
        raise ValueError("Cargo.lock must contain the matching local package version")

    manifest = json.loads((root / ".release-please-manifest.json").read_text())
    if manifest["."] != version:
        raise ValueError("Tag must match .release-please-manifest.json")

    for previous_tag in tags:
        if previous_tag == tag or not previous_tag.startswith("v"):
            continue
        previous = previous_tag[1:]
        if STABLE_VERSION.fullmatch(previous) and current <= stable_version(previous):
            raise ValueError(f"Release must be newer than existing tag {previous_tag}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag", help="Release tag, for example v1.2.3")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    try:
        tags = subprocess.check_output(
            ["git", "tag", "--list"], cwd=root, text=True
        ).splitlines()
        validate_release(root, args.tag, tags)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release validation failed: {error}\n")
    print(f"Release {args.tag} validated")


if __name__ == "__main__":
    main()
