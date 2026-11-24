#!/usr/bin/env python3
"""Stage a Windows x64 payload and build its per-user Inno Setup installer."""
import argparse
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

import conpty
from release import RELEASE_VERSION

ROOT = Path(__file__).resolve().parent.parent


def check_pe(path, expected_subsystem):
    data = path.read_bytes()
    conpty.check_x64_pe(data, path)
    offset = struct.unpack_from('<I', data, 60)[0]
    optional = offset + 24
    if offset + 22 > len(data):
        raise ValueError(f"invalid PE32+ optional header: {path}")
    size = struct.unpack_from('<H', data, offset + 20)[0]
    if (size < 70 or optional + size > len(data)
            or struct.unpack_from('<H', data, optional)[0] != 0x20b):
        raise ValueError(f"invalid PE32+ optional header: {path}")
    subsystem = struct.unpack_from('<H', data, optional + 68)[0]
    if subsystem != expected_subsystem:
        raise ValueError(f"wrong Windows subsystem {subsystem}: {path}")


def stage(binaries, payload, version, pair, files=None):
    files = conpty.pin()["files"] if files is None else files
    if not RELEASE_VERSION.fullmatch(version):
        raise ValueError(f"unsupported release version: {version}")
    if payload.exists():
        raise ValueError(f"payload already exists: {payload}")
    for name, subsystem in (("app.exe", 2), ("panel-mcp.exe", 3)):
        check_pe(binaries / name, subsystem)
    verified = conpty.verify(pair, files)
    notices = binaries / "notices"
    if not notices.is_dir() or not any(p.is_file() for p in notices.rglob("*")):
        raise ValueError("missing Cargo dependency notices")
    payload.mkdir(parents=True)
    shutil.copyfile(binaries / "app.exe", payload / "Ivy.exe")
    shutil.copyfile(binaries / "panel-mcp.exe", payload / "panel-mcp.exe")
    # Flat beside Ivy.exe: the library is found by name in the application directory, and it
    # starts the host beside itself (D112).
    # The bytes that were checked are the bytes written, so nothing can swap the pair in between.
    for name, data in verified.items():
        (payload / name).write_bytes(data)
    licenses = payload / "licenses"
    licenses.mkdir()
    (licenses / "conpty").mkdir()
    shutil.copyfile(ROOT / "resources/licenses/windows/conpty-MIT.txt",
                    licenses / "conpty/LICENSE")
    for name in ("LICENSE", "COPYRIGHT", "THIRD-PARTY.md"):
        shutil.copyfile(ROOT / name, licenses / name)
    for name in ("LICENSE-MIT", "LICENSE-APACHE"):
        dest = licenses / "vendor/egui-winit" / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / "vendor/egui-winit" / name, dest)
    for source in (ROOT / "resources/themes").glob("*-LICENSE"):
        dest = licenses / "themes" / source.name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, dest)
    shutil.copytree(notices, licenses / "cargo")
    shutil.copyfile(ROOT / "packaging/windows/Ivy.ico", payload / "Ivy.ico")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--conpty-dir", required=True, type=Path,
                        help="directory holding the pair scripts/conpty.py fetched")
    parser.add_argument("--iscc", default=r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe")
    args = parser.parse_args()
    try:
        if sys.platform != "win32":
            raise ValueError("building the installer requires Windows and Inno Setup")
        output = args.output_dir.resolve()
        output.mkdir(parents=True, exist_ok=True)
        expected = output / f"Ivy-{args.version}-windows-x86_64-setup.exe"
        if expected.exists():
            raise ValueError(f"installer already exists: {expected}")
        with tempfile.TemporaryDirectory(prefix="ivy-windows-") as temporary:
            payload = Path(temporary) / "payload"
            stage(args.binary_dir.resolve(), payload, args.version, args.conpty_dir.resolve())
            subprocess.run([args.iscc, f"/DReleaseVersion={args.version}",
                            f"/DPayloadDir={payload}", f"/DOutputDir={output}",
                            str(ROOT / "packaging/windows/ivy.iss")], check=True)
        if not expected.is_file() or not expected.stat().st_size:
            raise ValueError("compiler did not produce the expected installer")
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"windows package: {error}\n")


if __name__ == "__main__":
    main()
