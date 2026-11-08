#!/usr/bin/env python3
"""Validate the workspace release version and write the Linux release metadata."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib

SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)


def version(cargo):
    with cargo.open("rb") as source:
        value = tomllib.load(source)["workspace"]["package"]["version"]
    match = SEMVER.fullmatch(value)
    if not match or (match[4] and any(p.isdigit() and len(p) > 1 and p[0] == "0" for p in match[4].split("."))):
        raise ValueError(f"invalid workspace SemVer: {value}")
    return value


def deb_version(value):
    match = SEMVER.fullmatch(value)
    return value.replace("-", "~", 1) if match and match[4] else value


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def generate(cargo, directory, commit, built_on, tag=None):
    value = version(cargo)
    if not built_on.strip():
        raise ValueError("build environment must not be empty")
    if tag is not None and tag != "v" + value:
        raise ValueError(f"tag {tag!r} does not match workspace version {value}")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("commit must be a lowercase 40-character SHA")
    name = f"ivy_{deb_version(value)}_amd64.deb"
    package = directory / name
    if not package.is_file() or package.stat().st_size == 0:
        raise ValueError(f"missing or empty artifact: {name}")
    if {p.name for p in directory.glob("*.deb")} != {name}:
        raise ValueError("artifact set does not match the declared Linux target")
    manifest = {
        "version": value,
        "commit": commit,
        "artifacts": [{"name": name, "target": "x86_64-unknown-linux-gnu",
                       "built_on": built_on, "signing": "unsigned",
                       "sha256": digest(package)}],
    }
    manifest_path = directory / "release-manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    (directory / "SHA256SUMS").write_text(
        "".join(f"{digest(path)}  {path.name}\n" for path in (package, manifest_path)),
        encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("version", "generate"))
    parser.add_argument("--cargo", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--commit")
    parser.add_argument("--built-on")
    parser.add_argument("--tag")
    args = parser.parse_args()
    try:
        if args.command == "version":
            value = version(args.cargo)
            if args.tag is not None and args.tag != "v" + value:
                raise ValueError(f"tag {args.tag!r} does not match workspace version {value}")
            print(value)
        else:
            if args.directory is None or args.commit is None or args.built_on is None:
                parser.error("generate requires --directory, --commit and --built-on")
            generate(args.cargo, args.directory, args.commit, args.built_on, args.tag)
    except (OSError, ValueError, KeyError, TypeError, tomllib.TOMLDecodeError) as error:
        print(f"release: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
