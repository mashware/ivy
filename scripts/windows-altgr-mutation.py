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
        result = subprocess.run(
            ["cargo", "run", "--locked", "-p", "app", "--example", "windows_altgr"],
            cwd=root,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=300,
        )
        (root / "windows-altgr-mutation.log").write_bytes(result.stdout)
        if result.returncode == 0 or b"Spanish left Ctrl+Alt+2 terminal bytes" not in result.stdout:
            raise SystemExit("Restored Ctrl suppression did not fail the native character assertion")
    finally:
        path.write_bytes(original)
    print("PASS native witness rejects the original Ctrl+Alt suppression")


if __name__ == "__main__":
    main()
