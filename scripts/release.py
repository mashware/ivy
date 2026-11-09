#!/usr/bin/env python3
"""Validate the workspace release version and write the Linux release metadata."""
import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path
import re
import sys
import tomllib

RELEASE_VERSION = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-(alpha|beta|rc)\.(0|[1-9][0-9]*))?"
)


def version(cargo):
    with cargo.open("rb") as source:
        value = tomllib.load(source)["workspace"]["package"]["version"]
    if not RELEASE_VERSION.fullmatch(value):
        raise ValueError(f"unsupported workspace release version: {value}")
    return value


def deb_version(value):
    match = RELEASE_VERSION.fullmatch(value)
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


def notices(metadata, directory):
    """Copy original notices; use the canonical Apache text for supported SPDX fallbacks."""
    data = json.loads(metadata.read_text(encoding="utf-8"))
    packages = {package["id"]: package for package in data["packages"]}
    nodes = {node["id"]: node for node in data["resolve"]["nodes"]}
    roots = [package["id"] for package in packages.values()
             if package["id"] in data["workspace_members"]
             and package["name"] in ("app", "panel-control")]
    if len(roots) != 2:
        raise ValueError("metadata must contain both release workspace packages")
    pending, selected = roots[:], set()
    while pending:
        ident = pending.pop()
        if ident in selected:
            continue
        selected.add(ident)
        for dep in nodes[ident]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                pending.append(dep["pkg"])
    if directory.exists() and any(directory.iterdir()):
        raise ValueError(f"notice directory is not empty: {directory}")
    directory.mkdir(parents=True, exist_ok=True)
    for ident in sorted(selected):
        package = packages[ident]
        if package["source"] is None:
            continue
        root = Path(package["manifest_path"]).parent
        files = set()
        for current, dirs, names in os.walk(root):
            dirs[:] = sorted(d for d in dirs if d not in (".git", "target"))
            for name in names:
                if re.match(r"^(?:licen[sc]e|copying|notice|ofl)(?:$|[._-])", name, re.I):
                    files.add(Path(current, name))
        if package.get("license_file"):
            file = Path(package["license_file"])
            files.add(file if file.is_absolute() else root / file)
        # This crate embeds four fonts whose original terms have nonstandard names.
        font_assets = package["name"] == "epaint_default_fonts"
        if font_assets:
            files.update(root / "fonts" / name for name in (
                "OFL.txt", "UFL.txt", "Hack-Regular.txt", "emoji-icon-font-mit-license.txt"))
        if any(not file.is_file() for file in files):
            raise ValueError(f"missing notice for {package['name']} {package['version']}")
        has_license = any(file.parent == root and
                          re.match(r"^(?:licen[sc]e|copying|ofl)(?:$|[._-])", file.name, re.I)
                          for file in files) or bool(package.get("license_file"))
        declared = package.get("license")
        fallback = not has_license and (declared in ("Apache-2.0", "MIT OR Apache-2.0") or
                   (font_assets and declared == "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0"))
        if not has_license and not fallback:
            raise ValueError(f"missing notice for {package['name']} {package['version']}")
        dest = directory / f"{package['name']}-{package['version']}"
        if fallback:
            dest.mkdir(parents=True, exist_ok=True)
            canonical = Path(__file__).resolve().parent.parent / "vendor/egui-winit/LICENSE-APACHE"
            shutil.copyfile(canonical, dest / "SPDX-FALLBACK-APACHE-2.0")
            (dest / "SPDX-FALLBACK-ATTRIBUTION.txt").write_text(
                "No license text was present in the source distribution.\n"
                "Apache-2.0 is selected from the declared SPDX expression below;\n"
                "SPDX-FALLBACK-APACHE-2.0 contains its canonical text.\n"
                f"Name: {package['name']}\nVersion: {package['version']}\n"
                f"License: {package.get('license')}\nRepository: {package.get('repository')}\n"
                f"Authors: {', '.join(package.get('authors', []))}\n", encoding="utf-8")
        for file in sorted(files):
            relative = file.relative_to(root)
            target = dest / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(file, target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("version", "generate", "notices"))
    parser.add_argument("--cargo", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--metadata", type=Path)
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
        elif args.command == "notices":
            if args.directory is None or args.metadata is None:
                parser.error("notices requires --metadata and --directory")
            notices(args.metadata, args.directory)
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
