#!/usr/bin/env python3
"""Run the workspace tests and hold Windows to its list of known failures.

The first native Windows run found tests that fail there for reasons nobody had looked at yet.
Each one is listed by name in `windows-known-failures.json` with the issue that tracks it. The
list is a ratchet: a failure not on it fails the job, and so does a listed test that now passes or
no longer runs, so an entry is removed by the change that fixes it and nothing can join it in
silence. An entry marked `intermittent` is one Windows CI has seen both pass and fail: it may do
either, and it still has to run.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LIST = ROOT / "scripts" / "windows-known-failures.json"


class KnownFailureError(Exception):
    pass


def parse_known(path: Path) -> dict[tuple[str, str], dict[str, str]]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise KnownFailureError(f"cannot read known-failure list {path}: {exc}") from exc
    if not isinstance(data, list):
        raise KnownFailureError("known-failure list must be a JSON array")
    required = {"binary", "name", "reason", "issue"}
    known: dict[tuple[str, str], dict[str, str]] = {}
    for index, entry in enumerate(data):
        if not isinstance(entry, dict) or set(entry) - {"intermittent"} != required:
            raise KnownFailureError(f"entry {index} must contain exactly {sorted(required)} and optional intermittent")
        if not all(isinstance(entry[key], str) and entry[key] for key in required):
            raise KnownFailureError(f"entry {index} fields must be nonempty strings")
        if "intermittent" in entry and entry["intermittent"] is not True:
            raise KnownFailureError(f"entry {index} intermittent must be true when present")
        if not re.fullmatch(r"https://github\.com/[^/]+/[^/]+/issues/\d+", entry["issue"]):
            raise KnownFailureError(f"entry {index} issue must be a GitHub issue URL")
        key = entry["binary"], entry["name"]
        if key in known:
            raise KnownFailureError(f"duplicate known failure: {key[0]}::{key[1]}")
        known[key] = entry
    return known


def parse_run(output: str, labels: dict[str, str]) -> tuple[dict[tuple[str, str], str], list[str]]:
    """Return {(binary, test): outcome} and the binaries that started but never reported a result.

    `labels` maps an executable's file name to its binary label. Doc-test sections are labelled
    `<crate>:doc`.
    """
    outcomes: dict[tuple[str, str], str] = {}
    unfinished: list[str] = []
    current: str | None = None
    finished = True
    for raw in output.splitlines():
        line = raw.rstrip()
        running = re.match(r"^\s+Running .*\((.+)\)$", line)
        doc = re.match(r"^\s+Doc-tests (\S+)$", line)
        if running or doc:
            if current is not None and not finished:
                unfinished.append(current)
            if running:
                name = Path(running.group(1).replace("\\", "/")).name
                if name not in labels:
                    raise KnownFailureError(f"no binary label for test executable {name}")
                current = labels[name]
            else:
                current = f"{doc.group(1)}:doc"
            finished = False
            continue
        if current is None:
            continue
        result = re.match(r"^test (.+?) \.\.\. (ok|FAILED|ignored.*)$", line)
        if result:
            outcome = result.group(2)
            outcomes[(current, result.group(1))] = "ignored" if outcome.startswith("ignored") else outcome
        elif line.startswith("test result: "):
            finished = True
    if current is not None and not finished:
        unfinished.append(current)
    return outcomes, unfinished


def judge(
    known: dict[tuple[str, str], dict[str, str]],
    outcomes: dict[tuple[str, str], str],
    unfinished: list[str],
) -> list[str]:
    problems = [f"{binary}: the test binary stopped before reporting its result" for binary in unfinished]
    for key, outcome in sorted(outcomes.items()):
        if outcome == "FAILED" and key not in known:
            problems.append(f"new Windows failure: {key[0]}::{key[1]}")
    for key, entry in sorted(known.items()):
        outcome = outcomes.get(key)
        # An intermittent entry may pass or fail, but it still has to run: one that disappears or is
        # ignored is as stale as any other.
        if outcome == "ok" and not entry.get("intermittent"):
            problems.append(f"listed failure now passes, remove it from the list: {key[0]}::{key[1]} ({entry['issue']})")
        elif outcome is None:
            problems.append(f"listed failure did not run: {key[0]}::{key[1]} ({entry['issue']})")
        elif outcome == "ignored":
            problems.append(f"listed failure is now ignored, remove it from the list: {key[0]}::{key[1]}")
    return problems


def executable_labels() -> dict[str, str]:
    spec = importlib.util.spec_from_file_location("windows_test_report", ROOT / "scripts" / "windows-test-report.py")
    report = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(report)
    try:
        executables = report.cargo_test_executables()
    except report.ReportError as exc:
        raise KnownFailureError(str(exc)) from exc
    return {path.name: label for label, path in executables}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", type=Path, default=DEFAULT_LIST)
    args = parser.parse_args()
    try:
        known = parse_known(args.list)
        labels = executable_labels()
        try:
            proc = subprocess.run(
                ["cargo", "test", "--locked", "--workspace", "--no-fail-fast"],
                cwd=ROOT,
                text=True,
                encoding="utf-8",
                errors="replace",
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
                timeout=40 * 60,
            )
        except subprocess.TimeoutExpired as exc:
            raise KnownFailureError("cargo test exceeded 40 minutes") from exc
        print(proc.stdout)
        outcomes, unfinished = parse_run(proc.stdout, labels)
        if not outcomes:
            raise KnownFailureError("cargo test reported no test results")
        problems = judge(known, outcomes, unfinished)
        failed = sum(outcome == "FAILED" for outcome in outcomes.values())
        for key, entry in sorted(known.items()):
            kind = "intermittent" if entry.get("intermittent") else "known"
            print(f"{kind} Windows failure: {key[0]}::{key[1]} — {entry['reason']} ({entry['issue']})")
        print(f"Known Windows failures: {len(known)}; failed in this run: {failed}")
        if problems:
            raise KnownFailureError("\n".join(problems))
        return 0
    except KnownFailureError as exc:
        print(f"windows-known-failures: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
