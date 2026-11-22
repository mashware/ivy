#!/usr/bin/env python3
"""Require native Windows peer witnesses to reject specific broken implementations."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
PROCESS = "crates/panel-control/src/endpoint/windows/process.rs"
NEGOTIATION = "crates/panel-control/src/endpoint/windows/negotiation.rs"
CHILD = "endpoint::windows::peer_tests::exited_windows_peer_and_its_clones_never_become_alive_again"

# These mutations each remove one observable guarantee; compilation failure is not a kill.
MUTATIONS = [
    (
        "exited-process-stays-alive", PROCESS,
        "unsafe { WaitForSingleObject(self.lifetime.as_raw_handle(), 0) == WAIT_TIMEOUT }",
        "unsafe { WaitForSingleObject(self.lifetime.as_raw_handle(), 0); true }",
        CHILD,
    ),
    (
        "substituted-connector-is-accepted", PROCESS,
        "if connector_pid(stream)? != self.pid || !self.is_alive() {",
        "if !self.is_alive() {",
        "endpoint::windows::peer_tests::a_substituted_process_cannot_authorize_the_fresh_connector",
    ),
    (
        "creation-time-is-invented", PROCESS,
        "birth: ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64,",
        "birth: 1,",
        CHILD,
    ),
    (
        "raw-stream-gains-authority", NEGOTIATION,
        "bootstrap.prefetched.lock().unwrap().extend(first);",
        "bootstrap.verified_peer = WindowsPeer::candidate(&bootstrap).ok();\n"
        "        bootstrap.prefetched.lock().unwrap().extend(first);",
        "listener::windows_peer_tests::raw_windows_publications_remain_unknown_without_a_witness",
    ),
    (
        "bootstrap-tail-gains-authority", NEGOTIATION,
        "Ok(Some(verified)) => Ok(verified),",
        "Ok(Some(verified)) => {\n"
        "            verified.prefetched.lock().unwrap().extend(&first[line.len() + 1..]);\n"
        "            Ok(verified)\n        },",
        "endpoint::windows::negotiation::tests::bootstrap_pipeline_is_not_promoted_to_the_verified_stream",
    ),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-patterns", action="store_true", help="check source anchors only; do not execute tests")
    args = parser.parse_args()
    for label, relative, needle, _, _ in MUTATIONS:
        if (ROOT / relative).read_text(encoding="utf-8").count(needle) != 1:
            raise RuntimeError(f"{label}: expected exactly one mutation anchor in {relative}")
    if args.check_patterns:
        print(f"{len(MUTATIONS)} mutation anchors match; native tests were not executed")
        return 0
    if sys.platform != "win32":
        print("Native Windows is required; no mutation was executed", file=sys.stderr)
        return 2
    evidence = Path(os.environ.get("RUNNER_TEMP", ".")) / "windows-peer-mutations"
    evidence.mkdir(parents=True, exist_ok=True)
    for label, relative, needle, replacement, test in MUTATIONS:
        source = ROOT / relative
        original = source.read_bytes()
        try:
            source.write_text(original.decode("utf-8").replace(needle, replacement), encoding="utf-8")
            result = subprocess.run(
                ["cargo", "test", "--locked", "-p", "panel-control", "--lib", "--", test, "--exact", "--nocapture"],
                cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True, encoding="utf-8", errors="replace", timeout=180,
            )
        finally:
            source.write_bytes(original)
        (evidence / f"{label}.log").write_text(result.stdout, encoding="utf-8")
        if result.returncode == 0 or "running 1 test" not in result.stdout or "test result: FAILED" not in result.stdout:
            print(result.stdout)
            raise RuntimeError(f"{label}: witness did not fail at runtime (compile failure does not prove coverage)")
        print(f"{label}: rejected by {test}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
