#!/usr/bin/env python3
"""Prove the native daemon-attribution witness rejects an omitted launch correction."""

import pathlib
import platform
import subprocess


ROOT = pathlib.Path(__file__).resolve().parent.parent
TEST = "tests::ordinary_daemon_start_separates_native_responsibility_but_explicit_admin_keeps_it"
COMMAND = [
    "cargo", "test", "--locked", "-p", "panel-control", "--test", "ivy_codex_launch",
    "--", TEST, "--exact", "--nocapture",
]


def run():
    result = subprocess.run(COMMAND, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=180)
    print(result.stdout, flush=True)
    return result


def main():
    if platform.system() != "Darwin":
        raise SystemExit("This witness requires native macOS responsibility state.")
    mutants = [
        ("ordinary launch bypasses the native adapter",
         "crates/panel-control/src/codex_relay.rs",
         "macos_daemon::spawn(command)?", "command.spawn()?"),
        ("native exec omits responsibility disclamation",
         "crates/panel-control/src/codex_relay/macos_daemon.rs",
         "errno(unsafe { disclaim(&mut attributes.0, true) })?;", "let _ = disclaim;"),
    ]
    for name, relative, before, after in mutants:
        path = ROOT / relative
        original = path.read_bytes()
        source = original.decode()
        if source.count(before) != 1:
            raise SystemExit(f"Mutation anchor is not unique: {relative}")
        try:
            path.write_text(source.replace(before, after))
            result = run()
            # A compilation error or an unmatched filter is not a killed mutation.
            if result.returncode == 0 or f"test {TEST} ... FAILED" not in result.stdout:
                raise SystemExit(f"Native regression did not reject mutation: {name}")
            print(f"Mutation rejected: {name}", flush=True)
        finally:
            path.write_bytes(original)
    restored = run()
    if restored.returncode != 0 or f"test {TEST} ... ok" not in restored.stdout:
        raise SystemExit("Restored native regression did not run and pass.")


if __name__ == "__main__":
    main()
