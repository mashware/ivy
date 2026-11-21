#!/usr/bin/env python3
"""Fetch Microsoft's ConPTY pair (conpty.dll + OpenConsole.exe) pinned in packaging/windows/conpty.json.

portable-pty prefers a `conpty.dll` it finds by name over kernel32's own pseudoconsole, and that
library is reported (a disassembly of 1.25) to start an `OpenConsole.exe` it finds beside itself
(D112). So the two files travel
together, flat, beside `Ivy.exe`. Each file is pinned by its own SHA-256 rather than the package's:
nuget.org can re-sign a package, which changes the archive and not the binaries.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
PIN = ROOT / "packaging/windows/conpty.json"


# Far above either file (about 1 MiB each); a member claiming more is refused before it is inflated.
MEMBER_CEILING = 16 * 1024 * 1024


def pin():
    """The pin, refused with a one-line reason when it is not the shape the scripts read."""
    wanted = json.loads(PIN.read_text())
    files = wanted.get("files") if isinstance(wanted, dict) else None
    if not isinstance(files, dict) or not files or not all(
            isinstance(entry, dict) and isinstance(entry.get("member"), str)
            and isinstance(entry.get("sha256"), str) for entry in files.values()):
        raise ValueError(f"{PIN} does not pin each file by its member and sha256")
    return wanted


def check_x64_pe(data, name):
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError(f"not a Windows executable: {name}")
    offset = struct.unpack_from("<I", data, 60)[0]
    if offset + 6 > len(data) or data[offset:offset + 4] != b"PE\0\0":
        raise ValueError(f"missing PE header: {name}")
    if struct.unpack_from("<H", data, offset + 4)[0] != 0x8664:
        raise ValueError(f"not a Windows x64 executable: {name}")


def check_pinned(data, name, files):
    check_x64_pe(data, name)
    digest = hashlib.sha256(data).hexdigest()
    if digest != files[name]["sha256"]:
        raise ValueError(f"{name} checksum {digest} differs from the pin {files[name]['sha256']}")


def extract(package, directory, files):
    """Write the pinned pair flat into `directory`, only once every member has been verified."""
    contents = {}
    with zipfile.ZipFile(package) as archive:
        for name, entry in files.items():
            try:
                size = archive.getinfo(entry["member"]).file_size
            except KeyError:
                raise ValueError(f"ConPTY package lacks {entry['member']}") from None
            if size > MEMBER_CEILING:
                raise ValueError(f"{entry['member']} claims {size} bytes, over {MEMBER_CEILING}")
            data = archive.read(entry["member"])
            check_pinned(data, name, files)
            contents[name] = data
    directory.mkdir(parents=True, exist_ok=True)
    for name, data in contents.items():
        (directory / name).write_bytes(data)


def verify(directory, files):
    """The pair in `directory`, as the bytes that were checked; refused unless it is the pinned one."""
    contents = {}
    for name in files:
        data = (directory / name).read_bytes()
        check_pinned(data, name, files)
        contents[name] = data
    return contents


def fetch(directory):
    wanted = pin()
    package_id = wanted["package"].lower()
    version = wanted["version"]
    url = (f"https://api.nuget.org/v3-flatcontainer/{package_id}/{version}/"
           f"{package_id}.{version}.nupkg")
    with tempfile.TemporaryDirectory(prefix="ivy-conpty-") as temporary:
        package = Path(temporary) / "conpty.nupkg"
        with urllib.request.urlopen(url, timeout=120) as response:
            package.write_bytes(response.read())
        extract(package, directory, wanted["files"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--to", required=True, type=Path, help="directory that receives the pair")
    args = parser.parse_args()
    try:
        fetch(args.to.resolve())
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        parser.exit(1, f"conpty: {error}\n")


if __name__ == "__main__":
    main()
