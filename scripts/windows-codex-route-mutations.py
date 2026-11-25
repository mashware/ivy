"""Run native route witnesses, then prove their individual guards can turn them red."""

import argparse
from pathlib import Path
import os
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
FILE = "crates/panel-contract/src/windows_private_file.rs"
CASES = [
    (
        "crates/panel-control/src/codex_route.rs", "panel-control",
        ["--test", "codex_route_windows_privacy"],
        "every_created_registry_object_has_an_independently_verified_private_descriptor",
        "panel_contract::windows_private_file::create_new(path)",
        "std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(path)",
    ),
    (
        "crates/panel-control/src/codex_route.rs", "panel-control",
        ["--test", "codex_route_windows_sharing"],
        "route_replacement_with_external_reader_is_atomic_or_preserves_previous_state",
        "fs::rename(&temp, path)",
        "fs::copy(&temp, path).map(|_| ())",
    ),
    (
        "crates/panel-control/src/codex_route.rs", "panel-control",
        ["--test", "codex_route_windows_sharing"],
        "mapping_replacement_with_external_reader_is_atomic_or_restores_displaced_routes",
        "fs::rename(&temp, &mapping)?;",
        "fs::copy(&temp, &mapping)?;",
    ),
    (
        "crates/panel-control/src/codex_route.rs", "panel-control", ["--lib"],
        "codex_route::tests::held_reader_observes_old_record_and_mapping_after_replacement",
        "fs::rename(&temp, path)",
        "fs::copy(&temp, path).map(|_| ())",
    ),
    (
        "crates/panel-control/src/codex_route.rs", "panel-control", ["--lib"],
        "codex_route::tests::held_reader_observes_old_record_and_mapping_after_replacement",
        "fs::rename(&temp, &mapping)?;",
        "fs::copy(&temp, &mapping)?;",
    ),
    (
        "crates/panel-control/src/codex_route.rs", "panel-control", ["--lib"],
        "codex_route::tests::subprocess_lock_witness",
        "    lock(&lockfile)?;",
        "    let _ = &lockfile;",
    ),
    (
        FILE, "panel-contract", ["--lib"],
        "windows_private_file::tests::rejects_broad_and_null_dacls_without_repair",
        "    validate_acl(handle.0, path, false)?;",
        "    let _ = validate_acl(handle.0, path, false);",
    ),
    (
        FILE, "panel-contract", ["--lib"],
        "windows_private_file::tests::file_symlink_with_private_acl_is_rejected",
        "info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0",
        "info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0",
    ),
    (
        FILE, "panel-contract", ["--lib"],
        "windows_private_file::tests::fresh_file_has_protected_noninheriting_user_and_system_acl",
        "Some(descriptor(false)?)",
        "Some(descriptor(true)?)",
    ),
    (
        FILE, "panel-contract", ["--lib"],
        "windows_private_file::tests::private_file_round_trips_and_shares_delete",
        "GENERIC_READ | GENERIC_WRITE | READ_CONTROL | FILE_READ_ATTRIBUTES,\n"
        "            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,",
        "GENERIC_READ | GENERIC_WRITE | READ_CONTROL | FILE_READ_ATTRIBUTES,\n"
        "            FILE_SHARE_READ | FILE_SHARE_WRITE,",
    ),
    (
        "crates/panel-control/src/bin/ivy-codex.rs", "panel-control",
        ["--test", "ivy_codex_launch"],
        "native_shell_entry::unsupported_windows_relay_explains_why_and_preserves_the_client_invocation",
        "shared panel routing is not supported on Windows: Ivy cannot yet verify the owning console for a shared conversation",
        "this platform or invocation cannot use Ivy's shared-panel bridge",
    ),
]


def run_case(package: str, target: list[str], case: str, label: str, directory: Path) -> int:
    run = subprocess.run(
        ["cargo", "test", "--locked", "-p", package, *target, "--", case, "--exact", "--nocapture"],
        cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=300,
    )
    output = run.stdout + run.stderr
    (directory / f"{label}.log").write_text(output, encoding="utf-8")
    expected = "ok" if label.endswith("baseline") else "FAILED"
    if f"test {case} ... {expected}" not in output:
        raise RuntimeError(f"{case}: no named {expected} result; see {label}.log")
    return run.returncode


def owner_case(label: str, directory: Path) -> subprocess.CompletedProcess[str]:
    # Windows PowerShell needs its own module path after pwsh -> Python -> powershell.exe.
    # https://learn.microsoft.com/powershell/module/microsoft.powershell.core/about/about_psmodulepath
    environment = {key: value for key, value in os.environ.items() if key.upper() != "PSMODULEPATH"}
    run = subprocess.run(
        ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
         str(ROOT / "packaging/windows/test-codex-route-owner.ps1"), "-Repo", str(ROOT)],
        cwd=ROOT, env=environment, capture_output=True, text=True,
        encoding="utf-8", errors="replace", timeout=300,
    )
    (directory / f"{label}.log").write_text(run.stdout + run.stderr, encoding="utf-8")
    return run


def mutate_owner(directory: Path) -> None:
    path = ROOT / "crates/panel-contract/src/windows_security.rs"
    original = path.read_bytes()
    needle = b"unsafe { EqualSid(owner, user.User.Sid) } == 0"
    if original.count(needle) != 1:
        raise RuntimeError("Expected exactly one owner validation guard")
    baseline = owner_case("owner-baseline", directory)
    if baseline.returncode != 0 or "PASS foreign-owner route file rejected" not in baseline.stdout:
        raise RuntimeError("Native owner baseline failed; see owner-baseline.log")
    try:
        path.write_bytes(original.replace(needle, b"false"))
        mutant = owner_case("owner-mutant", directory)
        if mutant.returncode == 0 or "Foreign-owner refusal failed" not in mutant.stdout + mutant.stderr:
            raise RuntimeError("Owner mutation was not detected by the opener; see owner-mutant.log")
    finally:
        path.write_bytes(original)
    print("PASS mutation owner: foreign-owned readable file was accepted only by the mutant")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owner-only", action="store_true", help="Check the native owner fixture before the full workspace.")
    arguments = parser.parse_args()
    if sys.platform != "win32":
        raise SystemExit("These witnesses require native Windows; cross-compilation is not execution.")
    directory = ROOT / "target/windows-codex-route-mutations"
    directory.mkdir(parents=True, exist_ok=True)
    if arguments.owner_only:
        mutate_owner(directory)
        print("Windows Codex route owner mutation was detected.")
        return
    for number, (filename, package, target, case, needle, replacement) in enumerate(CASES, 1):
        path = ROOT / filename
        original = path.read_bytes()
        newline = "\r\n" if b"\r\n" in original else "\n"
        old = needle.replace("\n", newline).encode()
        if original.count(old) != 1:
            raise RuntimeError(f"Expected one mutation point in {filename}: {needle}")
        if run_case(package, target, case, f"{number}-baseline", directory) != 0:
            raise RuntimeError(f"{case}: baseline failed")
        try:
            path.write_bytes(original.replace(old, replacement.replace("\n", newline).encode()))
            if run_case(package, target, case, f"{number}-mutant", directory) == 0:
                raise RuntimeError(f"{case}: mutation escaped its witness")
        finally:
            path.write_bytes(original)
        print(f"PASS mutation {number}: {case}")
    mutate_owner(directory)
    print(f"All {len(CASES) + 1} Windows Codex route mutations were detected.")


if __name__ == "__main__":
    main()
