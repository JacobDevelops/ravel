"""Behavioral checks for summary CLI output and bounded Java probing."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import launch
import summarize
import test_bench


class AnalysisHelperTests(unittest.TestCase):
    def test_java_probe_propagates_timeout(self):
        with patch.object(launch.subprocess, "run", side_effect=subprocess.TimeoutExpired("java", 30)) as run:
            with self.assertRaises(subprocess.TimeoutExpired):
                launch.validate_java("/java", 27)
        self.assertEqual(run.call_args.kwargs["timeout"], 30)

    def test_summary_cli_writes_trial_and_group_reports(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            directories = []
            fixtures = test_bench.SummaryTests()
            for index in range(3):
                directory = root / str(index)
                directory.mkdir()
                fixtures.make_trial(directory)
                result_path = directory / "result.json"
                result = json.loads(result_path.read_text())
                result["trial"] = str(index)
                result_path.write_text(json.dumps(result))
                directories.append(directory)
            report = root / "report"
            arguments = ["summarize.py", *map(str, directories), "--output", str(report)]
            with patch.object(sys, "argv", arguments), contextlib.redirect_stdout(io.StringIO()) as stdout:
                summarize.main()
            document = json.loads((report / "summary.json").read_text())
            self.assertEqual(len(document["trials"]), 3)
            self.assertEqual(document["groups"][0]["trial_count"], 3)
            self.assertEqual(document["groups"][0]["metrics"]["wall_to_menu_ms"]["p50"], 500)
            for directory, trial in zip(directories, document["trials"]):
                self.assertEqual(json.loads((directory / "summary.json").read_text()), trial)
            self.assertEqual((report / "summary.md").read_text(), stdout.getvalue())

    def test_gc_parser_refuses_truncated_and_durationless_pauses(self):
        for text in ("1.0: [Full GC unfinished", "1.0: [Full GC no duration]"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                summarize.gc_pauses(text)


if __name__ == "__main__":
    unittest.main()
