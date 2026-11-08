"""Ignored-test CI must distinguish completed targets from child fixture output."""
import contextlib
import io
import subprocess
import unittest
from unittest.mock import patch

from tools.ci import ignored_tests


def target(count, results, summary):
    return f"     Running unittests src/lib.rs (target/debug/deps/example)\nrunning {count} tests\n{results}{summary}"


def summary(passed=0, failed=0, ignored=0):
    verdict = "FAILED" if failed else "ok"
    return (f"test result: {verdict}. {passed} passed; {failed} failed; "
            f"{ignored} ignored; 0 measured; 12 filtered out; finished in 0.1s\n")


class IgnoredTests(unittest.TestCase):
    def run_output(self, output, code=0):
        with patch.object(ignored_tests.subprocess, "run", return_value=subprocess.CompletedProcess([], code, output)), contextlib.redirect_stdout(io.StringIO()) as log:
            result = ignored_tests.run("example", [])
        return result, log.getvalue()

    def test_success_and_multiple_targets(self):
        output = target(1, "test first ... ok\n", summary(passed=1))
        output += target(2, "test second ... ok\ntest third ... ok\n", summary(passed=2))
        result, log = self.run_output(output)
        self.assertEqual(result, {"first": True, "second": True, "third": True})
        self.assertIn("target 2 started 2 tests", log)
        self.assertIn("cargo exit 0", log)

    def test_known_failure_is_still_available_to_policy(self):
        result, _ = self.run_output(target(2, "test good ... ok\ntest bad ... FAILED\n", summary(passed=1, failed=1)), 101)
        self.assertEqual(result, {"good": True, "bad": False})
        with patch.object(ignored_tests, "known_failures", return_value={("example", "bad"): None}), patch.object(ignored_tests, "run", return_value=result), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(ignored_tests.main(["example"]), 0)

    def test_plain_known_failure_that_passes_requires_removal(self):
        result, _ = self.run_output(target(1, "test repaired ... ok\n", summary(passed=1)))
        with patch.object(ignored_tests, "known_failures", return_value={("example", "repaired"): None}), patch.object(ignored_tests, "run", return_value=result), contextlib.redirect_stdout(io.StringIO()) as log:
            self.assertEqual(ignored_tests.main(["example"]), 1)
        self.assertIn("repaired: passes now; take it off", log.getvalue())

    def test_unexplained_nonzero_status(self):
        result, _ = self.run_output(target(1, "test good ... ok\n", summary(passed=1)), 101)
        self.assertIsNone(result)

    def test_missing_summary_even_after_parsed_failure(self):
        result, _ = self.run_output(target(2, "test bad ... FAILED\n", ""), 101)
        self.assertIsNone(result)

    def test_child_summary_cannot_hide_aborted_parent(self):
        output = target(2401, "running 1 test\ntest child ... ok\n", summary(passed=1))
        result, _ = self.run_output(output, 101)
        self.assertIsNone(result)

    def test_small_summary_rejected_even_on_zero_exit(self):
        result, _ = self.run_output(target(2401, "test child ... ok\n", summary(passed=1)))
        self.assertIsNone(result)

    def test_missing_first_summary_not_hidden_by_next_target(self):
        output = target(10, "test bad ... FAILED\n", "")
        output += target(1, "test child ... ok\n", summary(passed=1))
        result, _ = self.run_output(output, 101)
        self.assertIsNone(result)

    def test_target_without_count_and_build_failure(self):
        for output in ["     Running unittests src/lib.rs (target/example)\n", "error: build failed\n"]:
            result, _ = self.run_output(output, 101)
            self.assertIsNone(result)

    def test_signal_diagnostic_does_not_echo_adjacent_output(self):
        output = target(2, "", "") + "error: process failed (signal: 9, private fixture data)\n"
        result, log = self.run_output(output, 101)
        self.assertIsNone(result)
        self.assertIn("(signal: 9)", log)
        self.assertNotIn("private fixture data", log)

    def test_ignored_tests_count_toward_completion(self):
        result, _ = self.run_output(target(2, "test good ... ok\ntest skipped ... ignored\n", summary(passed=1, ignored=1)))
        self.assertEqual(result, {"good": True})


if __name__ == "__main__":
    unittest.main()
