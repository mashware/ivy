#!/usr/bin/env python3
"""Build and check a native macOS trial bundle and drag-to-Applications disk image."""
import argparse
import hashlib
import json
import plistlib
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-(alpha|beta|rc)\.(0|[1-9][0-9]*))?")
ARCHES = ("arm64", "x86_64")
EXECUTABLES = ("app", "panel-mcp", "ivy-session")


def info(version):
    if not VERSION.fullmatch(version):
        raise ValueError(f"unsupported release version: {version}")
    base = version.split("-", 1)[0]
    return {
        "CFBundleIdentifier": "io.github.mashware.ivy",
        "CFBundleName": "Ivy", "CFBundleDisplayName": "Ivy",
        "CFBundleExecutable": "app", "CFBundlePackageType": "APPL",
        "CFBundleIconFile": "Ivy.icns", "CFBundleShortVersionString": base,
        "CFBundleVersion": base, "IvyReleaseVersion": version,
        "CFBundleGetInfoString": f"Ivy {version}",
        "LSMinimumSystemVersion": "15.0", "NSHighResolutionCapable": True,
        "LSApplicationCategoryType": "public.app-category.developer-tools",
    }


def check_contents(bundle, version):
    contents = bundle / "Contents"
    with (contents / "Info.plist").open("rb") as file:
        actual = plistlib.load(file)
    if actual != info(version):
        raise ValueError("bundle metadata does not match the declared release")
    for binary in EXECUTABLES:
        path = contents / "MacOS" / binary
        if not path.is_file() or not path.stat().st_mode & 0o111:
            raise ValueError(f"missing executable: {binary}")
    resources = contents / "Resources"
    for name in ("Ivy.icns", "licenses/LICENSE", "licenses/COPYRIGHT", "licenses/THIRD-PARTY.md",
                 "licenses/vendor/egui-winit/LICENSE-MIT", "licenses/vendor/egui-winit/LICENSE-APACHE",
                 "licenses/themes/catppuccin-LICENSE", "licenses/themes/github-LICENSE",
                 "licenses/themes/nord-LICENSE", "licenses/themes/rose-pine-LICENSE"):
        if not (resources / name).is_file() or not (resources / name).stat().st_size:
            raise ValueError(f"missing bundle resource: {name}")
    source = Path(__file__).resolve().parent.parent / "integrations/session-recovery"
    if resource_hashes(resources / "session-recovery") != resource_hashes(source):
        raise ValueError("session recovery resources differ from the release sources")
    notices = resources / "licenses/cargo"
    if not notices.is_dir() or not any(notices.iterdir()):
        raise ValueError("missing Cargo dependency notices")


def resource_hashes(directory):
    """Bind the complete tree, including hidden plugin metadata, to its exact source bytes."""
    if not directory.is_dir() or directory.is_symlink():
        raise ValueError("missing session recovery resources")
    result = {}
    for path in sorted(directory.rglob("*")):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ValueError("session recovery resources must be regular files")
        if path.is_file():
            with path.open("rb") as file:
                result[path.relative_to(directory).as_posix()] = hashlib.file_digest(file, "sha256").hexdigest()
    if not result:
        raise ValueError("missing session recovery resources")
    return result


def check_load_commands(load_commands, libraries):
    match = re.search(r"\bcmd LC_BUILD_VERSION\b(.*?)(?:\nLoad command|\Z)", load_commands, re.S)
    if not match or not re.search(r"\bplatform\s+(?:MACOS|1)\b", match[1]):
        raise ValueError("executable is not declared for macOS")
    minimum = re.search(r"\bminos\s+(\S+)", match[1])
    if not minimum or minimum[1] not in ("15.0", "15.0.0"):
        raise ValueError("executable deployment target must be macOS 15.0")
    for line in libraries.splitlines()[1:]:
        library = line.strip().split(" (", 1)[0]
        if library and not library.startswith(("/System/Library/", "/usr/lib/")):
            raise ValueError(f"external runtime dependency is not bundled: {library}")


def output(*args):
    return subprocess.check_output(args, text=True)


def check_native(bundle, version, arch):
    if sys.platform != "darwin":
        raise ValueError("native bundle checks require macOS")
    if arch not in ARCHES:
        raise ValueError("unsupported macOS architecture")
    check_contents(bundle, version)
    for name in EXECUTABLES:
        binary = str(bundle / "Contents/MacOS" / name)
        if output("lipo", "-archs", binary).strip().split() != [arch]:
            raise ValueError(f"wrong architecture in {name}; expected {arch}")
        check_load_commands(output("otool", "-l", binary), output("otool", "-L", binary))
        subprocess.run(["codesign", "--verify", "--strict", binary], check=True)
    subprocess.run(["codesign", "--verify", "--strict", str(bundle)], check=True)


def build(binary_dir, directory, version, arch):
    metadata = info(version)
    if sys.platform != "darwin" or arch not in ARCHES:
        raise ValueError("DMG creation requires macOS and a declared native architecture")
    repo = Path(__file__).resolve().parent.parent
    directory.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="ivy-dmg-") as temp:
        image = Path(temp) / "image"
        bundle = image / "Ivy.app"
        contents = bundle / "Contents"
        executables = contents / "MacOS"
        resources = contents / "Resources"
        executables.mkdir(parents=True)
        resources.mkdir()
        shutil.copytree(repo / "integrations/session-recovery", resources / "session-recovery")
        for name in EXECUTABLES:
            shutil.copy2(binary_dir / name, executables / name)
        with (contents / "Info.plist").open("wb") as file:
            plistlib.dump(metadata, file)
        (contents / "PkgInfo").write_bytes(b"APPL????")
        licenses = resources / "licenses"
        licenses.mkdir()
        for name in ("LICENSE", "COPYRIGHT", "THIRD-PARTY.md"):
            shutil.copyfile(repo / name, licenses / name)
        vendor = licenses / "vendor/egui-winit"
        vendor.mkdir(parents=True)
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            shutil.copyfile(repo / "vendor/egui-winit" / name, vendor / name)
        themes = licenses / "themes"
        themes.mkdir()
        for source in (repo / "resources/themes").glob("*-LICENSE"):
            shutil.copyfile(source, themes / source.name)
        shutil.copyfile(repo / "resources/themes/README.md", themes / "README.md")
        shutil.copytree(binary_dir / "notices", licenses / "cargo")
        iconset = Path(temp) / "Ivy.iconset"
        iconset.mkdir()
        for points in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                suffix = "@2x" if scale == 2 else ""
                size = str(points * scale)
                subprocess.run(["sips", "-z", size, size, str(repo / "resources/branding/ivy.png"),
                                "--out", str(iconset / f"icon_{points}x{points}{suffix}.png")], check=True)
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(resources / "Ivy.icns")], check=True)
        # Local integrity seals carry no publisher identity and do not provide notarization.
        for name in EXECUTABLES[1:]:
            subprocess.run(["codesign", "--force", "--sign", "-", "--timestamp=none",
                            str(executables / name)], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", "--timestamp=none", str(bundle)], check=True)
        check_native(bundle, version, arch)
        # Signing changes Mach-O bytes; bind the final image to the sealed staging executables.
        hashes = {}
        for name in EXECUTABLES:
            with (executables / name).open("rb") as file:
                hashes[name] = hashlib.file_digest(file, "sha256").hexdigest()
        sidecar = directory / f"Ivy-{version}-macos-{arch}.executables.json"
        sidecar.write_text(json.dumps(hashes, indent=2) + "\n", encoding="utf-8")
        (image / "Applications").symlink_to("/Applications")
        target = directory / f"Ivy-{version}-macos-{arch}.dmg"
        subprocess.run(["hdiutil", "create", "-ov", "-volname", "Ivy", "-srcfolder", str(image),
                        "-format", "UDZO", str(target)], check=True)
        print(target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "check"))
    parser.add_argument("--version", required=True)
    parser.add_argument("--arch", choices=ARCHES, required=True)
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--bundle", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "build":
            if args.binary_dir is None or args.directory is None:
                parser.error("build requires --binary-dir and --directory")
            build(args.binary_dir, args.directory, args.version, args.arch)
        else:
            if args.bundle is None:
                parser.error("check requires --bundle")
            check_native(args.bundle, args.version, args.arch)
    except (OSError, ValueError, subprocess.CalledProcessError, plistlib.InvalidFileException) as error:
        print(f"macOS package: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
