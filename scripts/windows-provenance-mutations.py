#!/usr/bin/env python3
"""Require native Windows tests to reject broken process scope/publication effects."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
APP = "panel::provenance_route::windows_tests::windows_provenance_opened_panes_label_only_verified_outside_and_keep_unknown"
MUTATIONS = [
    (
        "parent-order-falls-back-to-wall-clock", "crates/term-core/src/foreground/windows/chain.rs",
        "id.sequence >= child.sequence", "id.birth >= child.birth", "term-core",
        "foreground::windows::chain::tests::later_reused_parent_is_unknown_despite_earlier_wall_clock_birth",
    ),
    (
        "malformed-executable-is-not-an-error", "crates/term-core/src/foreground/windows/arguments.rs",
        "if malformed || high {", "if false && (malformed || high) {", "term-core",
        "foreground::windows::arguments::tests::malformed_executable_is_rejected_by_both_invocation_readers",
    ),
    (
        "application-discards-publication-scope", "crates/app/src/control/channel.rs",
        "services.with_publication_scope(inspector.flatten())",
        "services.with_publication_scope(None)", "app", APP,
    ),
    (
        "outside-notice-is-suppressed", "crates/panel-control/src/windows_services.rs",
        "term_core::Ancestry::Outside => PublicationOrigin::Outside,",
        "term_core::Ancestry::Outside => PublicationOrigin::Inside,", "app", APP,
    ),
    (
        "closed-scope-stays-open", "crates/term-core/src/foreground/windows.rs",
        "pub(super) fn is_open(&self) -> bool {",
        "pub(super) fn is_open(&self) -> bool { return true;", "term-core",
        "foreground::windows::tests::windows_foreground_root_and_other_console_and_revocation",
    ),
    (
        "exited-process-stays-alive", "crates/term-core/src/foreground/windows/process.rs",
        "unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) == WAIT_TIMEOUT }",
        "unsafe { WaitForSingleObject(handle.as_raw_handle(), 0); true }", "term-core",
        "foreground::windows::tests::windows_foreground_descendant_lifetime_signals_exit_259",
    ),
    (
        "outside-anchor-is-ignored", "crates/term-core/src/foreground/windows/chain.rs",
        "if anchor == Some(id) {", "if false && anchor == Some(id) {", "term-core",
        "foreground::windows::chain::tests::windows_anchor_is_an_outside_boundary_without_reading_its_history",
    ),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-patterns", action="store_true")
    args = parser.parse_args()
    for label, relative, needle, _, _, _ in MUTATIONS:
        if (ROOT / relative).read_bytes().count(needle.encode()) != 1:
            raise RuntimeError(f"{label}: expected exactly one source anchor")
    if args.check_patterns:
        print(f"{len(MUTATIONS)} source anchors match; native tests were not executed")
        return 0
    if sys.platform != "win32":
        print("Native Windows required; no mutation executed", file=sys.stderr)
        return 2
    evidence = Path(os.environ.get("RUNNER_TEMP", ".")) / "windows-provenance-mutations"
    evidence.mkdir(parents=True, exist_ok=True)
    for label, relative, needle, replacement, package, test in MUTATIONS:
        source = ROOT / relative
        original = source.read_bytes()
        try:
            source.write_bytes(original.replace(needle.encode(), replacement.encode()))
            result = subprocess.run(
                ["cargo", "test", "--locked", "-p", package, "--lib", "--", test, "--exact", "--nocapture"],
                cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True, encoding="utf-8", errors="replace", timeout=240,
            )
        finally:
            source.write_bytes(original)
        (evidence / f"{label}.log").write_text(result.stdout, encoding="utf-8")
        if result.returncode == 0 or "running 1 test" not in result.stdout or "test result: FAILED" not in result.stdout:
            print(result.stdout)
            raise RuntimeError(f"{label}: witness did not fail at runtime (compile failure is not coverage)")
        print(f"{label}: rejected by {test}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
