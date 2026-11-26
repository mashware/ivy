#!/usr/bin/env python3
"""Build the real helpers and require the named native hook witness to execute."""
import subprocess
import sys

NAMES = [
    "control::channel::windows_live_hooks::windows_live_hook_real_bridge_and_helper_clear_wait",
    "control::channel::windows_live_hooks::windows_fresh_launch_worker_installs_hook_authority_in_its_actual_pane",
]


def run(args):
    result = subprocess.run(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    print(result.stdout, end="", flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
    return result.stdout


def main():
    if sys.platform != "win32":
        raise SystemExit("native Windows is required for the live hook witness")
    run(["cargo", "build", "--locked", "-p", "panel-control", "--bins"])
    for name in NAMES:
        output = run(["cargo", "test", "--locked", "-p", "app", "--lib", "--", name,
                      "--exact", "--nocapture", "--test-threads=1"])
        if f"test {name} ... ok" not in output:
            raise SystemExit(f"named native witness did not run and pass: {name}")
    print("PASS native live hook witness")


if __name__ == "__main__":
    main()
