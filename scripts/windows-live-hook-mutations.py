#!/usr/bin/env python3
"""Demand named native assertions detect each independent live-hook regression.

This mutates checked-out source, so it is restricted to a disposable Windows CI
checkout. Every mutation is restored in a finally block, including interruptions.
"""
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "target" / "windows-live-hook-mutations"
TARGET = ROOT / "target" / "windows-live-hooks" / "cargo-target"
LIVE = "control::channel::windows_live_hooks::windows_live_hook_real_bridge_and_helper_clear_wait"
FRESH = "control::channel::windows_live_hooks::windows_fresh_launch_worker_installs_hook_authority_in_its_actual_pane"
CAPABILITY = "control::channel::windows_live_hooks::windows_fresh_codex_without_local_capability_is_painted_without_authority"
ACCOUNTING = "chrome::connection::tests::fresh_console_opening_is_subtracted_once_from_the_frame"
LAUNCHER = "chrome::connection::tests::fresh_launch_fields_and_button_are_reachable_in_a_short_modal"
IMAGE = "foreground::windows::fresh_tests::fresh_root_rejects_wrong_image_foreign_root_and_console_close"
IDENTITY = "foreground::windows::fresh_tests::fresh_registration_rechecks_captured_root_identity_and_sequence"
SEQUENCE = "foreground::windows::chain::tests::later_reused_parent_is_unknown_despite_earlier_wall_clock_birth"
CWD = "tests::windows_exact_cwd_removed_after_validation_fails_instead_of_using_home"

# label, source, unique before, replacement, package, exact test, assertion witness
CASES = (
    ("fresh-launcher-expanded-by-default", "crates/app/src/chrome/connection/fresh.rs",
     ".default_open(false)", ".default_open(true)",
     "app", LAUNCHER, "native launcher keyboard workflow:"),
    ("fresh-launcher-request", "crates/app/src/chrome/connection/fresh.rs",
     "form.request = Some(form.selection.clone());", "form.request = None;",
     "app", LAUNCHER, "fresh launch controls must submit the selected program"),
    ("fresh-launcher-focus-scroll", "crates/app/src/chrome/connection/fresh.rs",
     "response.scroll_to_me(None);", "let _ = &response;",
     "app", LAUNCHER, "native launcher keyboard workflow:"),
    ("fresh-opening-double-accounting", "crates/app/src/opening/fresh.rs",
     "window.bring_to_front(Ok(tab), ctx);",
     "let before = Instant::now(); window.bring_to_front(Ok(tab), ctx); window.timed_separately += before.elapsed();",
     "app", ACCOUNTING, "fresh console opening was subtracted twice from the frame"),
    ("codex-local-capability", "crates/app/src/launch/fresh.rs",
     "if selection.client == Client::Codex\n",
     "if false && selection.client == Client::Codex\n",
     "app", CAPABILITY, "fresh Codex capability refusal must be painted"),
    ("fresh-authority", "crates/app/src/launch.rs",
     "return crate::panel::overlay::Authority::fresh(scope, invocation);",
     "let _ = invocation; return crate::panel::overlay::Authority::new(scope);",
     "app", FRESH, "fresh worker hook authority:"),
    ("selected-root-image", "crates/term-core/src/foreground/windows/fresh.rs",
     "if actual != selected\n", "if (actual != selected && false)\n",
     "term-core", IMAGE, "assertion failed: first_scope.fresh_invocation(&exe, &[]).is_err()"),
    ("nested-client-image", "crates/term-core/src/foreground/windows/fresh.rs",
     "if root.images.contains(&image) {", "if false && root.images.contains(&image) {",
     "app", LIVE, "nested client reply:"),
    ("captured-root-identity", "crates/term-core/src/foreground/windows/fresh.rs",
     "|| process::identity(self.root.as_handle())? != self.root_identity",
     "|| (process::identity(self.root.as_handle())? != self.root_identity && false)",
     "term-core", IDENTITY, "a changed captured root birth must prevent registration"),
    ("captured-root-sequence", "crates/term-core/src/foreground/windows/fresh.rs",
     "|| ancestry::sequence(self.root.as_handle())? != root_sequence",
     "|| (ancestry::sequence(self.root.as_handle())? != root_sequence && false)",
     "term-core", IDENTITY, "a changed captured root sequence must prevent registration"),
    ("ancestry-sequence-order", "crates/term-core/src/foreground/windows/chain.rs",
     "id.sequence >= child.sequence", "id.sequence >= child.sequence && false",
     "term-core", SEQUENCE, "assertion `left == right` failed"),
    ("root-lifetime-at-apply", "crates/panel-control/src/windows_hooks.rs",
     "!self.revoked.load(Ordering::Acquire) && self.root.current()",
     "!self.revoked.load(Ordering::Acquire) && { let _ = &self.root; true }",
     "app", LIVE, "queued update is refused after root exit"),
    ("holder-generation", "crates/app/src/panel/overlay.rs",
     "hold.connection == connection && hold.incarnation == incarnation",
     "hold.connection == connection && { let _ = incarnation; true }",
     "app", LIVE, "the real queued overlay must not replace a newer generation"),
    ("release-token", "crates/panel-control/src/windows_hooks.rs",
     "release: Some(release),", "release: { drop(release); None },",
     "app", LIVE, "pressure reply:"),
    ("holder-actions", "crates/app/src/panel/overlay.rs",
     "merged.actions.clear();", "let _ = &merged.actions;",
     "app", LIVE, "Approve"),
    ("exact-working-directory", "crates/term-core/src/lib.rs",
     "cmd.cwd_required(directory);", "cmd.cwd(directory);",
     "term-core", CWD, "missing selected directory launched via home fallback"),
)


def invoke(label, command, env):
    try:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=900, check=False)
        output = result.stdout.decode("utf-8", errors="replace")
        errors = (result.stderr or b"").decode("utf-8", errors="replace")
        code = result.returncode
    except subprocess.TimeoutExpired as error:
        output = (error.output or b"").decode("utf-8", errors="replace")
        errors = (error.stderr or b"").decode("utf-8", errors="replace") + "\nTIMEOUT\n"
        code = None
    log = f"=== {label}: stdout ===\n{output}\n=== {label}: stderr ===\n{errors}"
    (EVIDENCE / f"{label}.log").write_text(log, encoding="utf-8")
    print(log, end="", flush=True)
    return code, output, errors


def test_command(package, name):
    return ["cargo", "test", "--locked", "-p", package, "--lib", "--", name,
            "--exact", "--show-output", "--test-threads=1"]


def passed_once(name, code, output, errors=""):
    return (code == 0 and re.search(r"(?m)^running 1 test$", output)
            and re.search(rf"(?m)^test {re.escape(name)} \.\.\. ok$", output)
            and re.search(r"(?m)^test result: ok\. 1 passed; 0 failed;", output))


def detected(name, marker, code, output, errors=""):
    # The test summary and its named failures entry rule out zero tests, compiler
    # errors, unrelated panics, and setup errors masquerading as mutation proof.
    return (code not in (None, 0)
            and re.search(r"(?m)^running 1 test$", output)
            and re.search(rf"(?m)^    {re.escape(name)}$", output)
            and re.search(r"(?m)^test result: FAILED\. 0 passed; 1 failed;", output)
            and (marker in output or marker in errors)
            and not re.search(r"(?m)^error(?:\[E\d+\])?: could not compile", output + "\n" + errors))


def main():
    if sys.platform != "win32" or os.environ.get("CI", "").lower() != "true":
        raise SystemExit("mutations require a disposable native Windows CI checkout")
    def interrupted(signum, _frame):
        raise KeyboardInterrupt(f"interrupted by signal {signum}")
    for name in ("SIGINT", "SIGTERM", "SIGBREAK"):
        if hasattr(signal, name):
            signal.signal(getattr(signal, name), interrupted)
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    inventory = [{"case": case[0], "source": case[1], "test": case[5],
                  "expected_assertion": case[6], "status": "not_run"} for case in CASES]
    inventory_path = EVIDENCE / "inventory.json"
    def save_inventory():
        inventory_path.write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
    save_inventory()
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(TARGET)
    code, _, _ = invoke("baseline-suite", [sys.executable, str(ROOT / "scripts/windows-live-hooks.py")], env)
    if code != 0:
        raise SystemExit("the named native baseline witnesses must pass first")
    for label, name in [("selected-root-image", IMAGE), ("captured-root-identity", IDENTITY),
                        ("ancestry-sequence-order", SEQUENCE)]:
        code, output, errors = invoke(f"baseline-{label}", test_command("term-core", name), env)
        if not passed_once(name, code, output, errors):
            raise SystemExit(f"the named {label} baseline did not pass exactly once")

    for item, (label, relative, before, after, package, name, marker) in zip(inventory, CASES):
        path = ROOT / relative
        original = path.read_bytes()
        newline = b"\r\n" if b"\r\n" in original else b"\n"
        old = before.encode().replace(b"\n", newline)
        new = after.encode().replace(b"\n", newline)
        if original.count(old) != 1:
            raise SystemExit(f"{label}: expected exactly one mutation anchor")
        try:
            item["status"] = "running"
            save_inventory()
            path.write_bytes(original.replace(old, new, 1))
            code, output, errors = invoke(label, test_command(package, name), env)
            if not detected(name, marker, code, output, errors):
                item["status"] = "not_detected"
                raise SystemExit(f"{label}: expected named assertion did not detect mutation")
            item["status"] = "detected"
        finally:
            path.write_bytes(original)
            item["restored"] = path.read_bytes() == original
            save_inventory()
        if not item["restored"]:
            raise SystemExit(f"{label}: source restoration failed")
        print(f"PASS mutation {label}: named assertion detected regression", flush=True)
    print(f"PASS all {len(CASES)} native live hook and cwd mutations", flush=True)


if __name__ == "__main__":
    main()
