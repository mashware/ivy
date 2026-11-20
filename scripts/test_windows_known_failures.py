import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "windows_known_failures", Path(__file__).resolve().parent / "windows-known-failures.py"
)
known_failures = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(known_failures)

ISSUE = "https://github.com/mashware/ivy/issues/1"
LABELS = {"app-123.exe": "app/app:lib", "batches-456.exe": "app/batches:test"}

RUN = """\
     Running unittests src\\lib.rs (target\\debug\\deps\\app-123.exe)

running 3 tests
test a::fails ... FAILED
test a::passes ... ok
test a::skipped ... ignored, unix-only: no
test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out

     Running tests\\batches.rs (target\\debug\\deps\\batches-456.exe)

running 1 test
test b::fails ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
"""


def entry(binary, name):
    return {"binary": binary, "name": name, "reason": "why", "issue": ISSUE}


class ParseRun(unittest.TestCase):
    def test_outcomes_are_attributed_to_their_binary(self):
        outcomes, unfinished = known_failures.parse_run(RUN, LABELS)
        self.assertEqual(outcomes[("app/app:lib", "a::fails")], "FAILED")
        self.assertEqual(outcomes[("app/app:lib", "a::passes")], "ok")
        self.assertEqual(outcomes[("app/app:lib", "a::skipped")], "ignored")
        self.assertEqual(outcomes[("app/batches:test", "b::fails")], "FAILED")
        self.assertEqual(unfinished, [])

    def test_a_binary_that_stops_without_a_result_is_reported(self):
        crashed = RUN.rsplit("test result:", 1)[0]
        _, unfinished = known_failures.parse_run(crashed, LABELS)
        self.assertEqual(unfinished, ["app/batches:test"])

    def test_an_executable_without_a_label_is_refused(self):
        with self.assertRaises(known_failures.KnownFailureError):
            known_failures.parse_run(RUN, {"app-123.exe": "app/app:lib"})


class Judge(unittest.TestCase):
    def setUp(self):
        self.outcomes, self.unfinished = known_failures.parse_run(RUN, LABELS)

    def known(self, *entries):
        return {(e["binary"], e["name"]): e for e in entries}

    def test_every_failure_listed_passes(self):
        known = self.known(entry("app/app:lib", "a::fails"), entry("app/batches:test", "b::fails"))
        self.assertEqual(known_failures.judge(known, self.outcomes, self.unfinished), [])

    def test_an_unlisted_failure_fails(self):
        known = self.known(entry("app/app:lib", "a::fails"))
        problems = known_failures.judge(known, self.outcomes, self.unfinished)
        self.assertEqual(problems, ["new Windows failure: app/batches:test::b::fails"])

    def test_a_listed_test_that_passes_fails(self):
        known = self.known(
            entry("app/app:lib", "a::fails"), entry("app/batches:test", "b::fails"), entry("app/app:lib", "a::passes")
        )
        problems = known_failures.judge(known, self.outcomes, self.unfinished)
        self.assertEqual(len(problems), 1)
        self.assertIn("now passes", problems[0])

    def test_a_listed_test_that_did_not_run_fails(self):
        known = self.known(
            entry("app/app:lib", "a::fails"), entry("app/batches:test", "b::fails"), entry("app/app:lib", "gone")
        )
        problems = known_failures.judge(known, self.outcomes, self.unfinished)
        self.assertEqual(len(problems), 1)
        self.assertIn("did not run", problems[0])

    def test_a_listed_test_that_is_now_ignored_fails(self):
        known = self.known(
            entry("app/app:lib", "a::fails"), entry("app/batches:test", "b::fails"), entry("app/app:lib", "a::skipped")
        )
        problems = known_failures.judge(known, self.outcomes, self.unfinished)
        self.assertEqual(len(problems), 1)
        self.assertIn("now ignored", problems[0])

    def test_a_crashed_binary_fails_even_with_everything_listed(self):
        known = self.known(entry("app/app:lib", "a::fails"), entry("app/batches:test", "b::fails"))
        problems = known_failures.judge(known, self.outcomes, ["app/batches:test"])
        self.assertEqual(problems, ["app/batches:test: the test binary stopped before reporting its result"])


class ParseKnown(unittest.TestCase):
    def parse(self, data):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "known.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            return known_failures.parse_known(path)

    def test_the_shipped_list_parses(self):
        known_failures.parse_known(known_failures.DEFAULT_LIST)

    def test_an_entry_without_an_issue_is_refused(self):
        with self.assertRaises(known_failures.KnownFailureError):
            self.parse([{"binary": "b", "name": "n", "reason": "r"}])

    def test_an_issue_that_is_not_a_github_issue_is_refused(self):
        with self.assertRaises(known_failures.KnownFailureError):
            self.parse([{"binary": "b", "name": "n", "reason": "r", "issue": "later"}])

    def test_a_duplicate_is_refused(self):
        with self.assertRaises(known_failures.KnownFailureError):
            self.parse([entry("b", "n"), entry("b", "n")])


if __name__ == "__main__":
    unittest.main()
