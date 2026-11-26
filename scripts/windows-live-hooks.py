#!/usr/bin/env python3
"""Run named native Windows hook and exact-directory witnesses."""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "target" / "windows-live-hooks"
TARGET = EVIDENCE / "cargo-target"
APP_TESTS = (
    "control::channel::windows_live_hooks::windows_live_hook_real_bridge_and_helper_clear_wait",
    "control::channel::windows_live_hooks::windows_fresh_launch_worker_installs_hook_authority_in_its_actual_pane",
    "control::channel::windows_live_hooks::windows_fresh_native_loader_failure_is_painted_without_fallback",
    "control::channel::windows_live_hooks::windows_fresh_codex_without_local_capability_is_painted_without_authority",
    "chrome::connection::tests::fresh_console_opening_is_subtracted_once_from_the_frame",
)
TERM_TESTS = (
    "tests::windows_exact_cwd_reaches_the_child",
    "tests::windows_exact_cwd_removed_after_validation_fails_instead_of_using_home",
)
VENDOR_TESTS = (
    "cmdbuilder::tests::required_cwd_is_exact_and_legacy_methods_reset_it",
    "cmdbuilder::tests::legacy_serialized_builder_defaults_to_non_required_cwd",
)


def execute(label, args, *, env=None, timeout=900):
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(args, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=timeout, check=False)
    output = result.stdout.decode("utf-8", errors="replace")
    (EVIDENCE / f"{label}.log").write_text(output, encoding="utf-8")
    print(output, end="", flush=True)
    return result.returncode, output


def require_pass(name, code, output):
    escaped = re.escape(name)
    if (code != 0 or not re.search(r"(?m)^running 1 test$", output)
            or not re.search(rf"(?m)^test {escaped} \.\.\. ok$", output)
            or not re.search(r"(?m)^test result: ok\. 1 passed; 0 failed;", output)):
        raise SystemExit(f"named witness did not execute and pass exactly once: {name}")


def named(label, package, name, env):
    command = ["cargo", "test", "--locked", "-p", package, "--lib", "--", name,
               "--exact", "--nocapture", "--test-threads=1"]
    require_pass(name, *execute(label, command, env=env))


def main():
    if sys.platform != "win32":
        raise SystemExit("native Windows is required for the live hook witnesses")
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(TARGET)
    code, _ = execute("build-helpers", ["cargo", "build", "--locked", "-p",
                                        "panel-control", "--bins"], env=env)
    if code:
        raise SystemExit("real helper build failed")
    for index, name in enumerate(APP_TESTS, 1):
        named(f"app-{index}", "app", name, env)
    for index, name in enumerate(TERM_TESTS, 1):
        named(f"term-{index}", "term-core", name, env)

    # Cargo may update this copy's lock for its serde_json dev-dependency only.
    # Never write the checked-in vendor lock or vendor/target.
    with tempfile.TemporaryDirectory(prefix="ivy-portable-pty-tests-") as temp:
        copy = Path(temp) / "portable-pty"
        shutil.copytree(ROOT / "vendor" / "portable-pty", copy,
                        ignore=shutil.ignore_patterns("target", ".git"))
        vendor_env = env.copy()
        vendor_env["CARGO_TARGET_DIR"] = str(EVIDENCE / "vendor-target")
        for index, name in enumerate(VENDOR_TESTS, 1):
            command = ["cargo", "test", "--manifest-path", str(copy / "Cargo.toml"),
                       "--features", "serde_support", "--lib", "--", name, "--exact",
                       "--nocapture", "--test-threads=1"]
            require_pass(name, *execute(f"vendor-{index}", command, env=vendor_env))
    count = len(APP_TESTS) + len(TERM_TESTS) + len(VENDOR_TESTS)
    print(f"PASS all {count} named native live hook and cwd witnesses", flush=True)


if __name__ == "__main__":
    main()
