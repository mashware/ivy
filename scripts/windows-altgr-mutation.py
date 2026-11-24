"""Prove the native keyboard witness fails when the pre-fix Ctrl filter is restored."""

import os
from pathlib import Path
import subprocess
import sys


def main():
    if sys.platform != "win32" or os.environ.get("IVY_DISPOSABLE_RUNNER") != "yes":
        raise SystemExit("Native input mutation requires a disposable Windows desktop")
    root = Path(__file__).resolve().parent.parent
    path = root / "vendor/egui-winit/src/lib.rs"
    original = path.read_bytes()
    anchor = b"if altgr::consume("
    if original.count(anchor) != 1:
        raise SystemExit("Expected exactly one production Ctrl+Alt dispatch")
    try:
        path.write_bytes(original.replace(anchor, b"if false && altgr::consume(", 1))
        # Keep partial output even if compilation or the native event loop times out.
        with (root / "windows-altgr-mutation-build.log").open("wb") as log:
            subprocess.run(
                ["cargo", "build", "--locked", "-p", "app", "--example", "windows_altgr"],
                cwd=root, stdout=log, stderr=subprocess.STDOUT, timeout=600, check=True,
            )
        executable = root / "target/debug/examples/windows_altgr.exe"
        output = root / "windows-altgr-mutation.log"
        with output.open("wb") as log:
            result = subprocess.run(
                [str(executable)], cwd=root, stdout=log, stderr=subprocess.STDOUT, timeout=40,
            )
        if result.returncode == 0 or b"Spanish left Ctrl+Alt+2 terminal bytes" not in output.read_bytes():
            raise SystemExit("Restored Ctrl suppression did not fail the native character assertion")
    finally:
        path.write_bytes(original)
    print("PASS native witness rejects the original Ctrl+Alt suppression")


if __name__ == "__main__":
    main()
