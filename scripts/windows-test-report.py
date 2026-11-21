#!/usr/bin/env python3
"""Check the Windows Unix-only test inventory against compiled libtest harnesses."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INVENTORY = ROOT / "scripts" / "windows-test-omissions.json"
DEFAULT_IGNORE_BASELINE = ROOT / "scripts" / "windows-test-ignore-baseline.json"


class ReportError(Exception):
    pass


def parse_inventory(path: Path) -> list[dict[str, str]]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReportError(f"cannot read omission inventory {path}: {exc}") from exc
    if not isinstance(data, list):
        raise ReportError("omission inventory must be a JSON array")
    required = {"binary", "name", "reason"}
    result = []
    for index, entry in enumerate(data):
        optional = {"linux_ignore_reason"}
        if not isinstance(entry, dict) or not (set(entry) == required or set(entry) == required | optional):
            raise ReportError(f"inventory entry {index} must contain {sorted(required)} and optional linux_ignore_reason")
        if not all(isinstance(entry[key], str) and entry[key] for key in required):
            raise ReportError(f"inventory entry {index} fields must be nonempty strings")
        if not re.fullmatch(r"unix-only: .+", entry["reason"]):
            raise ReportError(f"inventory entry {index} reason must start with 'unix-only: '")
        if "linux_ignore_reason" in entry and (
            not isinstance(entry["linux_ignore_reason"], str) or not entry["linux_ignore_reason"]
        ):
            raise ReportError(f"inventory entry {index} linux_ignore_reason must be a nonempty string")
        result.append(entry)
    return result


def parse_ignore_baseline(path: Path) -> list[dict[str, str]]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReportError(f"cannot read ignored-test baseline {path}: {exc}") from exc
    if not isinstance(data, list):
        raise ReportError("ignored-test baseline must be a JSON array")
    required = {"platform", "binary", "name", "reason"}
    seen: set[tuple[str, str, str]] = set()
    result = []
    for index, entry in enumerate(data):
        if not isinstance(entry, dict) or set(entry) != required:
            raise ReportError(f"baseline entry {index} must contain exactly {sorted(required)}")
        if not all(isinstance(entry[key], str) and entry[key] for key in required):
            raise ReportError(f"baseline entry {index} fields must be nonempty strings")
        if entry["platform"] not in {"linux", "windows"}:
            raise ReportError(f"baseline entry {index} platform must be linux or windows")
        key = entry["platform"], entry["binary"], entry["name"]
        if key in seen:
            raise ReportError(f"duplicate ignored-test baseline entry: {key[0]} {key[1]}::{key[2]}")
        seen.add(key)
        result.append(entry)
    return result


def parse_list_output(output: str) -> set[str]:
    """Extract test names from stable libtest --list lines (including ignored entries)."""
    names: list[str] = []
    for line in output.splitlines():
        match = re.fullmatch(r"(.+): test", line.strip())
        if match:
            names.append(match.group(1))
    summary = re.search(r"(?m)^(\d+) tests?, \d+ benchmarks?\s*$", output)
    if summary is None:
        raise ReportError("unrecognized libtest --list output (missing test summary)")
    if len(names) != int(summary.group(1)):
        raise ReportError("libtest listing count does not match its test summary")
    found: set[str] = set()
    duplicates: set[str] = set()
    for name in names:
        if name in found:
            duplicates.add(name)
        found.add(name)
    if duplicates:
        raise ReportError(f"duplicate test names in libtest listing: {', '.join(sorted(duplicates))}")
    return found


def validate_ignored_reason_output(name: str, reason: str, output: str) -> None:
    """Check default libtest selection reports this exact ignored case and reason."""
    summary = re.search(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        output,
        re.MULTILINE,
    )
    if not summary or summary.groups() != ("0", "0", "1"):
        raise ReportError(f"{name}: filtered run did not report exactly 0 passed, 1 ignored, 0 failed")
    expected_line = f"test {name} ... ignored, {reason}"
    if expected_line not in output.splitlines():
        raise ReportError(f"{name}: libtest did not report expected ignored reason {reason!r}")


def validate(
    inventory: list[dict[str, str]],
    compiled: dict[str, tuple[set[str], set[str]]],
    platform: str,
    unrelated_baseline: list[dict[str, str]] | None = None,
) -> list[str]:
    """Validate inventory against binary -> (all test names, ignored test names)."""
    if platform not in {"linux", "windows"}:
        raise ReportError(f"unsupported platform {platform!r}")
    seen: set[tuple[str, str]] = set()
    for entry in inventory:
        key = entry["binary"], entry["name"]
        if key in seen:
            raise ReportError(f"duplicate inventory entry: {key[0]}::{key[1]}")
        seen.add(key)
        if key[0] not in compiled:
            raise ReportError(f"inventory binary did not compile: {key[0]}")

    unrelated_baseline = [entry for entry in (unrelated_baseline or []) if entry["platform"] == platform]
    baseline_keys = {(entry["binary"], entry["name"]) for entry in unrelated_baseline}
    for binary, name in baseline_keys:
        if binary not in compiled:
            raise ReportError(f"unrelated-ignore baseline binary did not compile on {platform}: {binary}")
        if (binary, name) in seen:
            raise ReportError(f"inventory and unrelated-ignore baseline overlap: {binary}::{name}")

    mismatches: list[str] = []
    for binary, (all_tests, ignored) in compiled.items():
        declared = {name for name_bin, name in seen if name_bin == binary}
        baseline_names = {name for bin_name, name in baseline_keys if bin_name == binary}
        if platform == "windows":
            expected_ignored = declared | baseline_names
            missing = expected_ignored - ignored
            unexpected = ignored - expected_ignored
            if missing:
                mismatches.append(f"{binary}: expected ignored tests not compiled as ignored: {', '.join(sorted(missing))}")
            if unexpected:
                mismatches.append(f"{binary}: ignored tests missing from omission inventory or unrelated baseline: {', '.join(sorted(unexpected))}")
        else:
            baseline_missing = baseline_names - ignored
            linux_ignored = {item["name"] for item in inventory if item["binary"] == binary and "linux_ignore_reason" in item}
            declared_real = declared - linux_ignored
            absent = declared - all_tests
            wrongly_ignored = declared_real & ignored
            missing_linux_ignored = linux_ignored - ignored
            if absent:
                mismatches.append(f"{binary}: inventory tests not compiled on Linux: {', '.join(sorted(absent))}")
            if wrongly_ignored:
                mismatches.append(f"{binary}: inventory tests are ignored on Linux: {', '.join(sorted(wrongly_ignored))}")
            if baseline_missing:
                mismatches.append(f"{binary}: baseline tests are not ignored on Linux: {', '.join(sorted(baseline_missing))}")
            if missing_linux_ignored:
                mismatches.append(f"{binary}: inventory tests expected ignored on Linux are not ignored: {', '.join(sorted(missing_linux_ignored))}")
            expected_ignored = linux_ignored | baseline_names
            unexpected = ignored - expected_ignored
            if unexpected:
                mismatches.append(f"{binary}: ignored tests missing from inventory or baseline on Linux: {', '.join(sorted(unexpected))}")
    return mismatches


def cargo_test_executables() -> list[tuple[str, Path]]:
    try:
        metadata = subprocess.run(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=None,
            check=False,
            timeout=120,
        )
    except subprocess.TimeoutExpired as exc:
        raise ReportError("cargo metadata exceeded 120 seconds") from exc
    if metadata.returncode:
        raise ReportError(f"cargo metadata failed with exit code {metadata.returncode}")
    package_names = {package["id"]: package["name"] for package in json.loads(metadata.stdout)["packages"]}
    try:
        proc = subprocess.run(
            ["cargo", "test", "--locked", "--workspace", "--no-run", "--message-format=json"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=None,
            check=False,
            timeout=30 * 60,
        )
    except subprocess.TimeoutExpired as exc:
        raise ReportError("cargo test --no-run exceeded 30 minutes") from exc
    if proc.returncode:
        raise ReportError(f"cargo test --no-run failed with exit code {proc.returncode}")
    found: dict[tuple[str, str, str], Path] = {}
    for line in proc.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact" or not message.get("executable"):
            continue
        target = message.get("target", {})
        profile = message.get("profile", {})
        if not profile.get("test"):
            continue
        package = package_names.get(message.get("package_id", ""))
        target_name = target.get("name")
        kinds = target.get("kind", [])
        if package and target_name and kinds:
            found[(package, target_name, ",".join(sorted(kinds)))] = Path(message["executable"])
    if not found:
        raise ReportError("cargo emitted no compiled test executables")
    return [(f"{package}/{target}:{kind}", path) for (package, target, kind), path in sorted(found.items())]


def load_compiled(executables: Iterable[tuple[str, Path]]) -> dict[str, tuple[set[str], set[str]]]:
    compiled = {}
    for binary, executable in executables:
        try:
            all_proc = subprocess.run([str(executable), "--list"], text=True, capture_output=True, check=False, timeout=30)
        except subprocess.TimeoutExpired as exc:
            raise ReportError(f"{binary}: --list exceeded 30 seconds") from exc
        if all_proc.returncode:
            raise ReportError(f"{binary}: --list failed: {all_proc.stderr.strip()}")
        try:
            ignored_proc = subprocess.run(
                [str(executable), "--list", "--ignored"], text=True, capture_output=True, check=False, timeout=30
            )
        except subprocess.TimeoutExpired as exc:
            raise ReportError(f"{binary}: --list --ignored exceeded 30 seconds") from exc
        if ignored_proc.returncode:
            raise ReportError(f"{binary}: --list --ignored failed: {ignored_proc.stderr.strip()}")
        compiled[binary] = (parse_list_output(all_proc.stdout), parse_list_output(ignored_proc.stdout))
    return compiled


def verify_ignored_reason(executable: Path, name: str, reason: str) -> None:
    """Ask libtest to select one test normally; ignored bodies remain unexecuted."""
    try:
        proc = subprocess.run(
            [str(executable), "--exact", name, "--color", "never"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=15,
        )
    except subprocess.TimeoutExpired as exc:
        raise ReportError(f"{name}: exact ignored-reason query exceeded 15 seconds") from exc
    if proc.returncode:
        raise ReportError(f"{name}: exact ignored-reason query failed with exit code {proc.returncode}: {proc.stdout}")
    validate_ignored_reason_output(name, reason, proc.stdout)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("linux", "windows"), default=None)
    parser.add_argument("--inventory", type=Path, default=DEFAULT_INVENTORY)
    parser.add_argument("--baseline", type=Path, default=DEFAULT_IGNORE_BASELINE)
    args = parser.parse_args()
    platform = args.platform or ("windows" if os.name == "nt" else "linux")
    try:
        inventory = parse_inventory(args.inventory)
        baseline = parse_ignore_baseline(args.baseline)
        executables = cargo_test_executables()
        compiled = load_compiled(executables)
        mismatches = validate(inventory, compiled, platform, baseline)
        if mismatches:
            raise ReportError("\n".join(mismatches))
        executable_by_binary = dict(executables)
        reason_checks = []
        if platform == "windows":
            reason_checks.extend((entry["binary"], entry["name"], entry["reason"]) for entry in inventory)
        else:
            reason_checks.extend(
                (entry["binary"], entry["name"], entry["linux_ignore_reason"])
                for entry in inventory if "linux_ignore_reason" in entry
            )
        reason_checks.extend(
            (entry["binary"], entry["name"], entry["reason"])
            for entry in baseline if entry["platform"] == platform
        )
        for binary, name, reason in reason_checks:
            verify_ignored_reason(executable_by_binary[binary], name, reason)
        for entry in sorted(inventory, key=lambda item: (item["binary"], item["name"])):
            print(f"omitted {entry['binary']}::{entry['name']} — {entry['reason']}")
            if platform == "linux" and "linux_ignore_reason" in entry:
                print(f"  existing Linux ignore: {entry['linux_ignore_reason']}")
        print(f"Windows Unix-only omissions: {len(inventory)}")
        for entry in sorted((item for item in baseline if item["platform"] == platform), key=lambda item: (item["binary"], item["name"])):
            print(f"unrelated ignored test [{platform}]: {entry['binary']}::{entry['name']} — {entry['reason']}")
        print(f"Unrelated ignored tests [{platform}]: {sum(item['platform'] == platform for item in baseline)}")
        print(f"Compiled test executables checked: {len(compiled)} ({platform})")
        return 0
    except ReportError as exc:
        print(f"windows-test-report: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
