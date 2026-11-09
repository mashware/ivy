#!/usr/bin/env python3
"""Mount and copy a DMG in a temporary Applications directory; native CI, not desktop proof."""
import os
from pathlib import Path
import plistlib
import shlex
import shutil
import subprocess
import sys
import tempfile
import time


def main():
    if sys.platform != "darwin" or len(sys.argv) != 5:
        raise SystemExit("Usage on macOS: test-install.py DMG SEMVER ARCH NOTICES_DIR")
    dmg, version, arch, notices = sys.argv[1:]
    repo = Path(__file__).resolve().parents[2]
    # A short runtime path keeps the fixture's Unix socket below Darwin's path limit.
    with tempfile.TemporaryDirectory(prefix="ivy-mac-", dir="/tmp") as temp:
        root = Path(temp)
        mount = root / "volume"
        mount.mkdir()
        subprocess.run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", str(mount), dmg],
                       check=True, timeout=60)
        try:
            if not (mount / "Applications").is_symlink() or os.readlink(mount / "Applications") != "/Applications":
                raise ValueError("DMG has no Applications drop target")
            installed = root / "Applications/Ivy.app"
            installed.parent.mkdir()
            config = root / "config/agent-terminal"
            config.mkdir(parents=True)
            expected = {"config": b"existing preferences\n", "layout.json": b"existing saved layout\n"}
            for name, content in expected.items():
                (config / name).write_bytes(content)
            subprocess.run(["ditto", str(mount / "Ivy.app"), str(installed)], check=True)
            old_info = installed / "Contents/Info.plist"
            with old_info.open("rb") as file:
                fixture = plistlib.load(file)
            fixture.update(CFBundleVersion="0.0.0", CFBundleShortVersionString="0.0.0", IvyReleaseVersion="0.0.0")
            with old_info.open("wb") as file:
                plistlib.dump(fixture, file)
            (installed / "previous-install-marker").touch()
            shutil.rmtree(installed)
            subprocess.run(["ditto", str(mount / "Ivy.app"), str(installed)], check=True)
            assert not (installed / "previous-install-marker").exists()
            subprocess.run([sys.executable, str(repo / "scripts/package-macos.py"), "check",
                            "--bundle", str(installed), "--version", version, "--arch", arch], check=True)
            resources = installed / "Contents/Resources/licenses"
            subprocess.run(["diff", "-r", notices, str(resources / "cargo")], check=True)
            for name in ("LICENSE", "COPYRIGHT", "THIRD-PARTY.md"):
                assert (repo / name).read_bytes() == (resources / name).read_bytes()
            for source in (repo / "resources/themes").glob("*-LICENSE"):
                assert source.read_bytes() == (resources / "themes" / source.name).read_bytes()
            for name in ("LICENSE-MIT", "LICENSE-APACHE"):
                assert (repo / "vendor/egui-winit" / name).read_bytes() == (resources / "vendor/egui-winit" / name).read_bytes()
            for name in expected:
                assert (config / name).read_bytes() == expected[name]
            smoke(root, installed, version)
            other = "x86_64" if arch == "arm64" else "arm64"
            wrong_arch = subprocess.run([sys.executable, str(repo / "scripts/package-macos.py"), "check",
                                         "--bundle", str(installed), "--version", version, "--arch", other],
                                        capture_output=True)
            assert wrong_arch.returncode != 0 and b"wrong architecture" in wrong_arch.stderr
            # A real altered sealed resource must fail the same installed-bundle checker.
            icon = installed / "Contents/Resources/Ivy.icns"
            with icon.open("ab") as file:
                file.write(b"mutation")
            broken = subprocess.run([sys.executable, str(repo / "scripts/package-macos.py"), "check",
                                     "--bundle", str(installed), "--version", version, "--arch", arch],
                                    capture_output=True)
            assert broken.returncode != 0, "bundle signature check accepted altered resources"
            shutil.rmtree(installed)
            for name in expected:
                assert (config / name).read_bytes() == expected[name]
        finally:
            subprocess.run(["hdiutil", "detach", str(mount)], check=True, timeout=60)
    print("Mounted native Mac bundle, replacement/removal, notices and own-pane MCP smoke passed.")


def smoke(root, installed, version):
    probe = root / "probe.py"
    probe.write_text('''import json, os, pathlib, subprocess, time
assert os.environ.get("PANEL_ID") and os.environ.get("PANEL_DIR"), "pane identity missing"
deadline = time.monotonic() + 15
endpoint = pathlib.Path(os.environ["PANEL_DIR"]) / "control.sock"
while not endpoint.exists():
    assert time.monotonic() < deadline, "own-pane control endpoint did not appear"
    time.sleep(0.1)
requests = [
    {"jsonrpc":"2.0","id":1,"method":"initialize"},
    {"jsonrpc":"2.0","id":2,"method":"tools/list"},
    {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"panel_set","arguments":
      {"source":"mac-package-smoke","document":{"lines":["IVY_MAC_PACKAGE_OK"]}}}},
    {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"panel_get","arguments":{}}},
]
result = subprocess.run([os.environ["IVY_MAC_BRIDGE"]], input="".join(json.dumps(r)+"\\n" for r in requests),
                        text=True, capture_output=True, check=True, timeout=20)
answers = [json.loads(line) for line in result.stdout.splitlines()]
assert [a.get("id") for a in answers] == [1,2,3,4], answers
assert all("error" not in a and not a["result"].get("isError") for a in answers), answers
assert answers[0]["result"]["serverInfo"]["version"] == os.environ["IVY_MAC_VERSION"], answers[0]
assert {t["name"] for t in answers[1]["result"]["tools"]} == {
    "panel_set","panel_get","panel_patch","panel_retire","split_open","split_read","split_close"}
assert "IVY_MAC_PACKAGE_OK" in json.dumps(answers[3]["result"]["content"]), answers[3]
pathlib.Path(os.environ["IVY_MAC_PROOF"]).write_text("own-pane MCP passed\\n")
time.sleep(10)
''')
    shell = root / "probe-shell"
    probe_log = root / "probe.log"
    shell.write_text(f"#!/bin/sh\nexec {shlex.quote(sys.executable)} {shlex.quote(str(probe))} >{shlex.quote(str(probe_log))} 2>&1\n")
    shell.chmod(0o755)
    runtime = root / "run"
    runtime.mkdir(mode=0o700)
    env = dict(os.environ, SHELL=str(shell), XDG_RUNTIME_DIR=str(runtime),
               XDG_CONFIG_HOME=str(root / "runtime-config"), IVY_MAC_VERSION=version,
               IVY_MAC_BRIDGE=str(installed / "Contents/MacOS/panel-mcp"), IVY_MAC_PROOF=str(root / "proof"))
    log = root / "runtime.log"
    with log.open("wb") as file:
        process = subprocess.Popen([str(installed / "Contents/MacOS/app")], env=env, stdout=file, stderr=file)
        try:
            deadline = time.monotonic() + 45
            while not (root / "proof").exists():
                if process.poll() is not None or time.monotonic() >= deadline:
                    details = log.read_text(errors="replace")
                    if probe_log.exists():
                        details += probe_log.read_text(errors="replace")
                    raise RuntimeError("installed Mac runtime failed:\n" + details)
                time.sleep(0.1)
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=15)


if __name__ == "__main__":
    main()
