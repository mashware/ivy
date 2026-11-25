"""Require the OpenCode boundary regressions to reject deliberate production defects."""

import argparse
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-dir")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    evidence = root / "target/opencode-evidence"
    evidence.mkdir(parents=True, exist_ok=True)
    cargo = ["cargo", "test", "--locked", "--manifest-path", str(root / "Cargo.toml")]
    if args.target_dir:
        cargo += ["--target-dir", args.target_dir]
    cargo += ["-p", "panel-control"]
    plugin = root / "resources/agent-integrations/opencode/ivy-opencode.tsx"
    fixture = root / "resources/agent-integrations/opencode/windows-helper.test.mjs"
    cases = [
        (
            "typed-envelope",
            root / "crates/panel-control/src/bin/ivy-session/signals.rs",
            'envelope.tool != "agent_signal"',
            'false',
            cargo + ["--bin", "ivy-session", "only_exact_opencode_envelopes_become_typed_metadata"],
            "only_exact_opencode_envelopes_become_typed_metadata",
        ),
        (
            "whole-exchange-deadline",
            root / "crates/panel-control/src/client/agent_signal.rs",
            "deadline\n        .checked_duration_since(Instant::now())",
            "(Instant::now() + EXCHANGE_BUDGET)\n        .checked_duration_since(Instant::now())",
            cargo + ["--lib", "delayed_write_flush_and_trickled_reply_share_one_absolute_deadline"],
            "delayed_write_flush_and_trickled_reply_share_one_absolute_deadline",
        ),
        (
            "pending-withdrawal",
            plugin,
            "const withdrawing = pending === null || sent === null",
            "const withdrawing = false",
            ["node", "--test", "--test-name-pattern=disposal preserves pending null", str(fixture)],
            "disposal preserves pending null",
        ),
        (
            "helper-byte-ceiling",
            plugin,
            "buffer.length + stop - offset > 65536",
            "buffer.length + stop - offset > 131072",
            ["node", "--test", "--test-name-pattern=helper framing bounds bytes", str(fixture)],
            "helper framing bounds bytes",
        ),
    ]
    if sys.platform == "win32":
        cases.append((
            "native-server-identity",
            root / "crates/panel-control/src/endpoint/windows/negotiation.rs",
            "bootstrap.verify_server_pid(expected_pid)?;",
            "let _ = expected_pid;",
            cargo + ["--test", "signals_helper", "native_helper_wrong_pid_sends_zero_bytes_before_failed_readiness"],
            "native_helper_wrong_pid_sends_zero_bytes_before_failed_readiness",
        ))
        cases.append((
            "native-private-directory",
            root / "crates/panel-control/src/client.rs",
            "endpoint::private_to_this_user(dir)?;",
            "let _ = dir;",
            cargo + ["--test", "signals_helper", "native_helper_unsafe_directory_sends_zero_bytes_before_failed_readiness"],
            "native_helper_unsafe_directory_sends_zero_bytes_before_failed_readiness",
        ))
    for name, path, before, after, command, test_name in cases:
        original = path.read_bytes()
        needle = before.encode()
        count = 2 if name == "native-private-directory" else 1
        if original.count(needle) != count:
            raise SystemExit(f"{name}: expected {count} production mutation anchors")
        baseline = subprocess.run(command, cwd=root, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, timeout=180, check=False)
        if baseline.returncode != 0:
            sys.stdout.buffer.write(baseline.stdout)
            raise SystemExit(f"{name}: baseline must pass")
        try:
            path.write_bytes(original.replace(needle, after.encode(), count))
            result = subprocess.run(command, cwd=root, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=180, check=False)
            output = result.stdout.decode("utf-8", errors="replace")
            (evidence / f"windows-opencode-mutation-{name}.log").write_text(output, encoding="utf-8")
            if result.returncode == 0 or test_name not in output or "error[E" in output:
                sys.stdout.buffer.write(result.stdout)
                raise SystemExit(f"{name}: mutation did not fail its behavior assertion")
            if "FAILED" not in output and "AssertionError" not in output:
                sys.stdout.buffer.write(result.stdout)
                raise SystemExit(f"{name}: failure was not an assertion")
        finally:
            path.write_bytes(original)
        print(f"PASS mutation {name}: behavior regression detected", flush=True)
    print(f"PASS all {len(cases)} OpenCode boundary mutations", flush=True)


if __name__ == "__main__":
    main()
