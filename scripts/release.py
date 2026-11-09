#!/usr/bin/env python3
"""Validate release versions, preserve dependency notices and record the declared package set."""
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

# These published Mac dependencies omit upstream license files. Preserve the pinned
# project texts and original Cargo attribution; new versions still need verification.
MAC_MIT_NOTICES = {
    ("block2", "0.5.1"), ("block2", "0.6.2"), ("dispatch2", "0.3.1"), ("dispatch", "0.2.0"),
    ("objc-sys", "0.3.5"), ("objc2-app-kit", "0.2.2"), ("objc2-app-kit", "0.3.2"),
    ("objc2-core-data", "0.2.2"), ("objc2-core-foundation", "0.3.2"), ("objc2-core-graphics", "0.3.2"),
    ("objc2-core-image", "0.2.2"), ("objc2-core-text", "0.3.2"), ("objc2-encode", "4.1.0"),
    ("objc2-foundation", "0.2.2"), ("objc2-foundation", "0.3.2"), ("objc2-io-surface", "0.3.2"),
    ("objc2-metal", "0.2.2"), ("objc2-quartz-core", "0.2.2"), ("objc2", "0.5.2"), ("objc2", "0.6.4"),
}


def version(cargo):
    with cargo.open("rb") as source:
        value = tomllib.load(source)["workspace"]["package"]["version"]
    if not RELEASE_VERSION.fullmatch(value):
        raise ValueError(f"unsupported workspace release version: {value}")
    return value


def deb_version(value):
    match = RELEASE_VERSION.fullmatch(value)
    return value.replace("-", "~", 1) if match and match[4] else value


def arch_version(value):
    return re.sub(r"-(alpha|beta|rc)\.", r"\1", value)


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def generate(cargo, directory, commit, built_on, tag=None, include_arch=False, include_macos=False, include_windows=False):
    value = version(cargo)
    if not built_on.strip():
        raise ValueError("build environment must not be empty")
    if tag is not None and tag != "v" + value:
        raise ValueError(f"tag {tag!r} does not match workspace version {value}")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("commit must be a lowercase 40-character SHA")
    # GitHub replaces '~' in uploaded names, so filenames retain the SemVer spelling.
    names = [f"ivy_{value}_amd64.deb"]
    if include_arch:
        names.append(f"ivy-terminal-{arch_version(value)}-1-x86_64.pkg.tar.zst")
    if include_macos:
        names.extend(f"Ivy-{value}-macos-{arch}.dmg" for arch in ("arm64", "x86_64"))
    if include_windows:
        names.append(f"Ivy-{value}-windows-x86_64-setup.exe")
    packages = [directory / name for name in names]
    for package in packages:
        if not package.is_file() or package.stat().st_size == 0:
            raise ValueError(f"missing or empty artifact: {package.name}")
    actual = {p.name for pattern in ("*.deb", "*.pkg.tar.*", "*.dmg", "*.exe") for p in directory.glob(pattern)}
    if actual != set(names):
        raise ValueError("artifact set does not match the declared release targets")
    def artifact(package):
        result = {"name": package.name, "target": "x86_64-unknown-linux-gnu", "built_on": built_on,
                  "signing": "unsigned", "sha256": digest(package)}
        if package.suffix == ".zst":
            result["packaged_on"] = "Arch Linux (rolling)"
        elif package.suffix == ".dmg":
            arm = package.name.endswith("-arm64.dmg")
            result["target"] = "aarch64-apple-darwin" if arm else "x86_64-apple-darwin"
            result["built_on"] = "macos-15" if arm else "macos-15-intel"
        elif package.suffix == ".exe":
            result["target"] = "x86_64-pc-windows-msvc"
            result["built_on"] = "windows-2025"
        return result
    manifest = {
        "version": value,
        "commit": commit,
        "artifacts": [artifact(package) for package in packages],
    }
    manifest_path = directory / "release-manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    (directory / "SHA256SUMS").write_text(
        "".join(f"{digest(path)}  {path.name}\n" for path in (*packages, manifest_path)),
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
        fallback = not has_license and (declared in ("Apache-2.0", "MIT OR Apache-2.0", "BSL-1.0") or
                   (font_assets and declared == "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0"))
        mac_mit = not has_license and (package["name"], package["version"]) in MAC_MIT_NOTICES and declared in (
            "MIT", "Zlib OR Apache-2.0 OR MIT")
        if not has_license and not fallback and not mac_mit:
            raise ValueError(f"missing notice for {package['name']} {package['version']}")
        dest = directory / f"{package['name']}-{package['version']}"
        if mac_mit:
            original = Path(__file__).resolve().parent.parent / "resources/licenses/macos"
            dest.mkdir(parents=True, exist_ok=True)
            for name in ("MIT-LICENSE.txt", "README.md"):
                shutil.copyfile(original / name, dest / name)
            if package["name"] != "dispatch":
                shutil.copyfile(original / "OBJC2-LICENSE.md", dest / "UPSTREAM-PROJECT-LICENSING.md")
            shutil.copyfile(root / "Cargo.toml", dest / "UPSTREAM-Cargo.toml")
            vcs = root / ".cargo_vcs_info.json"
            if vcs.is_file():
                shutil.copyfile(vcs, dest / "source-vcs.json")
            (dest / "FALLBACK-ATTRIBUTION.txt").write_text(
                "The published crate omitted license text. Its declared MIT option is used;\n"
                "the preserved upstream text and provenance are included, not claimed as an archive original.\n"
                f"Name: {package['name']}\nVersion: {package['version']}\nLicense: {declared}\n"
                f"Repository: {package.get('repository')}\nAuthors: {', '.join(package.get('authors', []))}\n",
                encoding="utf-8")
        if fallback:
            dest.mkdir(parents=True, exist_ok=True)
            repository_root = Path(__file__).resolve().parent.parent
            selected_license = "BSL-1.0" if declared == "BSL-1.0" else "Apache-2.0"
            canonical = (repository_root / "resources/licenses/windows/BSL-1.0.txt" if declared == "BSL-1.0"
                         else repository_root / "vendor/egui-winit/LICENSE-APACHE")
            shutil.copyfile(canonical, dest / f"SPDX-FALLBACK-{selected_license.upper()}")
            (dest / "SPDX-FALLBACK-ATTRIBUTION.txt").write_text(
                "No license text was present in the source distribution.\n"
                f"{selected_license} is selected from the declared SPDX expression below;\n"
                f"SPDX-FALLBACK-{selected_license.upper()} contains its canonical text.\n"
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
    parser.add_argument("--include-arch", action="store_true")
    parser.add_argument("--include-macos", action="store_true")
    parser.add_argument("--include-windows", action="store_true")
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
            generate(args.cargo, args.directory, args.commit, args.built_on, args.tag,
                     args.include_arch, args.include_macos, args.include_windows)
    except (OSError, ValueError, KeyError, TypeError, tomllib.TOMLDecodeError) as error:
        print(f"release: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
