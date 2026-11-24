#!/usr/bin/env python3
"""Require native Windows fixture witnesses to reject their original failure modes."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = "crates/app/src/tests/windows_fixture.rs"
MUTATIONS = [
    (
        "timestamp-handle-is-read-only", "crates/app/src/panel.rs",
        'std::fs::OpenOptions::new()\n            .write(true)\n'
        '            .open(dir.path().join(PANEL_FILE))\n            .expect("open")\n'
        '            .set_modified(SystemTime::now() - how_much)',
        'std::fs::File::open(dir.path().join(PANEL_FILE))\n            .expect("open")\n'
        '            .set_modified(SystemTime::now() - how_much)',
        "panel::tests::a_panel_that_has_been_quiet_longer_than_its_due_confesses_it",
    ),
    (
        "child-does-not-change-directory", FIXTURE,
        "Set-Location -LiteralPath {}; [Environment]::CurrentDirectory = (Get-Location).ProviderPath; ",
        "$null = {}; ",
        "tests::windows_fixture::directory_and_input_fixtures_set_cwd_and_record_received_utf8_without_bom",
    ),
    (
        "child-does-not-write-input", FIXTURE,
        "$file.Write($bytes, 0, $bytes.Length); $file.Flush()",
        "$file.Flush()",
        "tests::windows_fixture::directory_and_input_fixtures_set_cwd_and_record_received_utf8_without_bom",
    ),
    (
        "child-does-not-wait-for-input", FIXTURE,
        "$null = [IvyFixtureInput]::ReadLine(); [Console]::Write({});",
        "[Console]::Write({});",
        "tests::windows_fixture::gated_output_is_written_only_after_the_child_receives_input",
    ),
]


def source_text(text: str, original: bytes) -> bytes:
    """Encode an anchor using the checkout's newline convention without rewriting it."""
    newline = "\r\n" if b"\r\n" in original else "\n"
    return text.replace("\n", newline).encode("utf-8")


def run(test: str, log: Path, passes: bool) -> None:
    command = ["cargo", "test", "--locked", "-p", "app", "--lib", test,
               "--", "--exact", "--nocapture"]
    try:
        result = subprocess.run(
            command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, encoding="utf-8", errors="replace", timeout=600,
        )
    except subprocess.TimeoutExpired as error:
        output = error.stdout or b""
        if isinstance(output, bytes):
            output = output.decode("utf-8", errors="replace")
        log.write_text(output + "\nMutation driver: cargo timed out.\n", encoding="utf-8")
        raise RuntimeError(f"{log.stem}: cargo exceeded 600 seconds; see {log}") from error
    log.write_text(result.stdout, encoding="utf-8")
    status = "ok" if passes else "FAILED"
    named_result = re.search(rf"^test {re.escape(test)} \.\.\. {status}\s*$",
                             result.stdout, re.MULTILINE)
    summary = "test result: ok. 1 passed; 0 failed; 0 ignored;" if passes else (
        "test result: FAILED. 0 passed; 1 failed; 0 ignored;"
    )
    if (result.returncode == 0) != passes or not named_result or summary not in result.stdout:
        raise RuntimeError(f"{log.stem}: expected exactly one named runtime {status}; see {log}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-patterns", action="store_true",
                        help="check source anchors only; do not execute tests")
    args = parser.parse_args()
    for label, relative, needle, _, _ in MUTATIONS:
        original = (ROOT / relative).read_bytes()
        if original.count(source_text(needle, original)) != 1:
            raise RuntimeError(f"{label}: expected exactly one anchor in {relative}")
    if args.check_patterns:
        print(f"{len(MUTATIONS)} mutation anchors match; native tests were not executed")
        return 0
    if sys.platform != "win32":
        print("Native Windows is required; no mutation was executed", file=sys.stderr)
        return 2
    evidence = ROOT / "target" / "windows-fixture-mutations"
    evidence.mkdir(parents=True, exist_ok=True)
    # Every selected witness must pass unmodified before any source is changed.
    selected = set()
    for label, _, _, _, test in MUTATIONS:
        if test in selected:
            continue
        run(test, evidence / f"baseline-{label}.log", passes=True)
        selected.add(test)
    for label, relative, needle, replacement, test in MUTATIONS:
        source = ROOT / relative
        original = source.read_bytes()
        try:
            anchor = source_text(needle, original)
            if original.count(anchor) != 1:
                raise RuntimeError(f"{label}: source changed since anchor validation")
            source.write_bytes(original.replace(anchor, source_text(replacement, original)))
            run(test, evidence / f"{label}.log", passes=False)
        finally:
            source.write_bytes(original)
        print(f"{label}: rejected by {test}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
