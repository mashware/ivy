"""Unit tests for the compiler-backed Unix-only omission checker."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("windows-test-report.py")
SPEC = importlib.util.spec_from_file_location("windows_test_report", MODULE_PATH)
assert SPEC and SPEC.loader
reporter = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reporter)


def entry(name, binary="ivy/app:lib", reason="unix-only: fork"):
    return {"binary": binary, "name": name, "reason": reason}


def baseline(name, platform="windows", binary="ivy/app:lib", reason="manual diagnostic"):
    return {"platform": platform, "binary": binary, "name": name, "reason": reason}


class WindowsTestReportTests(unittest.TestCase):
    def test_inventory_requires_array_and_exact_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "inventory.json"
            path.write_text('{"binary":"ivy/app:lib"}', encoding="utf-8")
            with self.assertRaisesRegex(reporter.ReportError, "JSON array"):
                reporter.parse_inventory(path)
            path.write_text('[{"binary":"ivy/app:lib","name":"x","reason":"fork"}]', encoding="utf-8")
            with self.assertRaisesRegex(reporter.ReportError, "unix-only"):
                reporter.parse_inventory(path)
            path.write_text(
                '[{"binary":"ivy/app:lib","name":"x","reason":"unix-only: scope",'
                '"linux_ignore_reason":"requires a real systemd manager"}]',
                encoding="utf-8",
            )
            self.assertEqual("requires a real systemd manager", reporter.parse_inventory(path)[0]["linux_ignore_reason"])

    def test_baseline_is_platform_qualified_and_rejects_duplicates(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "baseline.json"
            good = baseline("tests::manual")
            path.write_text(json.dumps([good]), encoding="utf-8")
            self.assertEqual([good], reporter.parse_ignore_baseline(path))
            path.write_text(json.dumps([good, good]), encoding="utf-8")
            with self.assertRaisesRegex(reporter.ReportError, "duplicate ignored-test baseline"):
                reporter.parse_ignore_baseline(path)

    def test_duplicate_entries_are_rejected(self):
        compiled = {"ivy/app:lib": ({"tests::fork"}, {"tests::fork"})}
        with self.assertRaisesRegex(reporter.ReportError, "duplicate inventory"):
            reporter.validate([entry("tests::fork"), entry("tests::fork")], compiled, "windows")

    def test_missing_inventory_entry_is_rejected_against_compiled_ignored_set(self):
        compiled = {"ivy/app:lib": ({"tests::fork", "tests::socket"}, {"tests::fork", "tests::socket"})}
        errors = reporter.validate([entry("tests::fork")], compiled, "windows")
        self.assertTrue(any("tests::socket" in error for error in errors))

    def test_inventory_entry_must_be_a_compiled_ignored_test_on_windows(self):
        compiled = {"ivy/app:lib": ({"tests::fork", "tests::portable"}, {"tests::fork"})}
        errors = reporter.validate([entry("tests::portable")], compiled, "windows")
        self.assertTrue(any("tests::portable" in error for error in errors))

    def test_unrelated_ignore_baseline_is_distinct_and_checked(self):
        compiled = {"ivy/app:lib": ({"tests::fork", "tests::manual"}, {"tests::fork", "tests::manual"})}
        inventory = [entry("tests::fork")]
        known = [baseline("tests::manual")]
        self.assertEqual([], reporter.validate(inventory, compiled, "windows", known))
        errors = reporter.validate(inventory, compiled, "windows", [])
        self.assertTrue(any("tests::manual" in error for error in errors))
        errors = reporter.validate(inventory, compiled, "windows", known + [baseline("tests::removed")])
        self.assertTrue(any("tests::removed" in error for error in errors))

    def test_linux_baseline_requires_exact_existing_ignore_set(self):
        compiled = {"ivy/app:lib": ({"tests::fork", "tests::manual"}, {"tests::manual"})}
        inventory = [entry("tests::fork")]
        known = [baseline("tests::manual", platform="linux")]
        self.assertEqual([], reporter.validate(inventory, compiled, "linux", known))
        errors = reporter.validate(inventory, compiled, "linux", [])
        self.assertTrue(any("missing from inventory or baseline on Linux" in error for error in errors))
        compiled_without_ignore = {"ivy/app:lib": ({"tests::fork", "tests::manual"}, set())}
        errors = reporter.validate(inventory, compiled_without_ignore, "linux", known)
        self.assertTrue(any("baseline tests are not ignored on Linux" in error for error in errors))

    def test_preignored_linux_unix_omission_stays_in_inventory(self):
        item = entry("tests::systemd", reason="unix-only: systemd user scopes")
        item["linux_ignore_reason"] = "requires an owned systemd test scope"
        compiled = {"ivy/app:lib": ({"tests::systemd"}, {"tests::systemd"})}
        self.assertEqual([], reporter.validate([item], compiled, "linux"))
        errors = reporter.validate([item], {"ivy/app:lib": ({"tests::systemd"}, set())}, "linux")
        self.assertTrue(any("expected ignored on Linux" in error for error in errors))

    def test_linux_requires_each_omission_to_be_a_compiled_real_test(self):
        inventory = [entry("tests::fork")]
        self.assertEqual([], reporter.validate(inventory, {"ivy/app:lib": ({"tests::fork"}, set())}, "linux"))
        errors = reporter.validate(inventory, {"ivy/app:lib": ({"tests::other"}, set())}, "linux")
        self.assertTrue(any("tests::fork" in error for error in errors))
        errors = reporter.validate(inventory, {"ivy/app:lib": ({"tests::fork"}, {"tests::fork"})}, "linux")
        self.assertTrue(any("ignored on Linux" in error for error in errors))

    def test_truncated_or_changed_list_format_is_rejected(self):
        with self.assertRaisesRegex(reporter.ReportError, "count does not match"):
            reporter.parse_list_output("tests::fork: test\n2 tests, 0 benchmarks\n")

    def test_libtest_list_parser_uses_names_only(self):
        output = "tests::fork: test\ntests::ordinary: test\n\n2 tests, 0 benchmarks\n"
        self.assertEqual({"tests::fork", "tests::ordinary"}, reporter.parse_list_output(output))

    def test_libtest_list_rejects_duplicate_or_unrecognized_output(self):
        with self.assertRaisesRegex(reporter.ReportError, "duplicate test names"):
            reporter.parse_list_output("tests::fork: test\ntests::fork: test\n2 tests, 0 benchmarks\n")
        with self.assertRaisesRegex(reporter.ReportError, "unrecognized"):
            reporter.parse_list_output("some unexpected output\n")

    def test_exact_ignored_query_checks_reason_and_does_not_run_the_body(self):
        name = "tests::fork"
        reason = "unix-only: fork"
        output = (
            "running 1 test\n"
            f"test {name} ... ignored, {reason}\n\n"
            "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 1 filtered out; finished in 0.00s\n"
        )
        reporter.validate_ignored_reason_output(name, reason, output)

    def test_exact_ignored_query_rejects_reason_mismatch_or_missing_reason(self):
        name = "tests::fork"
        good_summary = "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 1 filtered out\n"
        with self.assertRaisesRegex(reporter.ReportError, "expected ignored reason"):
            reporter.validate_ignored_reason_output(
                name, "unix-only: fork", f"test {name} ... ignored, unix-only: sockets\n{good_summary}"
            )
        with self.assertRaisesRegex(reporter.ReportError, "expected ignored reason"):
            reporter.validate_ignored_reason_output(name, "unix-only: fork", f"test {name} ... ignored\n{good_summary}")

    def test_exact_ignored_query_rejects_accidentally_executed_test(self):
        name = "tests::fork"
        output = (
            f"test {name} ... ok\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out\n"
        )
        with self.assertRaisesRegex(reporter.ReportError, "exactly 0 passed, 1 ignored, 0 failed"):
            reporter.validate_ignored_reason_output(name, "unix-only: fork", output)


if __name__ == "__main__":
    unittest.main()
