#!/usr/bin/env python3
"""Stage a Windows x64 payload and build its per-user Inno Setup installer."""
import argparse
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

from release import RELEASE_VERSION

ROOT = Path(__file__).resolve().parent.parent


def check_pe(path):
    data = path.read_bytes()
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError(f"not a Windows executable: {path}")
    offset = struct.unpack_from("<I", data, 60)[0]
    if offset + 6 > len(data) or data[offset:offset + 4] != b"PE\0\0":
        raise ValueError(f"missing PE header: {path}")
    if struct.unpack_from("<H", data, offset + 4)[0] != 0x8664:
        raise ValueError(f"not a Windows x64 executable: {path}")


def stage(binaries, payload, version):
    if not RELEASE_VERSION.fullmatch(version):
        raise ValueError(f"unsupported release version: {version}")
    if payload.exists():
        raise ValueError(f"payload already exists: {payload}")
    for name in ("app.exe", "panel-mcp.exe"):
        check_pe(binaries / name)
    notices = binaries / "notices"
    if not notices.is_dir() or not any(p.is_file() for p in notices.rglob("*")):
        raise ValueError("missing Cargo dependency notices")
    payload.mkdir(parents=True)
    shutil.copyfile(binaries / "app.exe", payload / "Ivy.exe")
    shutil.copyfile(binaries / "panel-mcp.exe", payload / "panel-mcp.exe")
    licenses = payload / "licenses"
    licenses.mkdir()
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
            stage(args.binary_dir.resolve(), payload, args.version)
            subprocess.run([args.iscc, f"/DReleaseVersion={args.version}",
                            f"/DPayloadDir={payload}", f"/DOutputDir={output}",
                            str(ROOT / "packaging/windows/ivy.iss")], check=True)
        if not expected.is_file() or not expected.stat().st_size:
            raise ValueError("compiler did not produce the expected installer")
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"windows package: {error}\n")


if __name__ == "__main__":
    main()
