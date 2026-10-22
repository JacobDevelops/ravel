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

    def test_summary_cli_separates_alternating_renderer_arms(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            directories = []
            fixtures = test_bench.SummaryTests()
            for index in range(6):
                renderer = "vanilla" if index % 2 == 0 else "geometry-proof"
                directory = root / (str(index) + "-" + renderer)
                directory.mkdir()
                fixtures.make_trial(directory)
                result_path = directory / "result.json"
                result = json.loads(result_path.read_text())
                result["trial"] = directory.name
                if index >= 2:
                    result["renderer"] = renderer
                result_path.write_text(json.dumps(result))
                command_path = directory / "command.json"
                command = json.loads(command_path.read_text())
                command["argv"].insert(1, "-Dravel.renderer=" + renderer)
                command_path.write_text(json.dumps(command))
                (directory / "game.log").write_text(
                    "[12:00:00] [main/INFO]: Forge Mod Loader version 14 for Minecraft 1.12.2 loading\n"
                    "[12:00:01] [main/INFO]: LWJGL Version: fixture\n")
                directories.append(directory)
            report = root / "report"
            arguments = ["summarize.py", *map(str, directories), "--output", str(report)]
            with patch.object(sys, "argv", arguments), contextlib.redirect_stdout(io.StringIO()):
                summarize.main()
            document = json.loads((report / "summary.json").read_text())
            self.assertEqual([trial["renderer"] for trial in document["trials"]],
                             ["vanilla", "geometry-proof"] * 3)
            self.assertEqual({group["renderer"]: group["trial_count"] for group in document["groups"]},
                             {"vanilla": 3, "geometry-proof": 3})
            text = (report / "summary.md").read_text()
            for group in document["groups"]:
                heading = "## original / warm-filesystem / " + group["renderer"] + "\n"
                section = text.split(heading, 1)[1].split("\n## ", 1)[0]
                for trial in document["trials"]:
                    row = "| " + trial["trial"] + " |"
                    if trial["renderer"] == group["renderer"]:
                        self.assertIn(row, section)
                    else:
                        self.assertNotIn(row, section)

    def test_summary_explicit_renderer_takes_precedence_over_launch_argument(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            test_bench.SummaryTests().make_trial(directory)
            result_path = directory / "result.json"
            result = json.loads(result_path.read_text())
            result["renderer"] = "geometry-proof"
            result_path.write_text(json.dumps(result))
            command_path = directory / "command.json"
            command = json.loads(command_path.read_text())
            command["argv"].insert(1, "-Dravel.renderer=vanilla")
            command_path.write_text(json.dumps(command))
            self.assertEqual(summarize.summarize_trial(directory)["renderer"], "geometry-proof")

    def test_summary_default_discovery_excludes_matrix_and_preflight_directories(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runs = root / "runs"
            runs.mkdir()
            matrix = runs / "prefix"
            matrix.mkdir()
            (matrix / "matrix.json").write_text("{}")
            preflight = runs / "prefix-preflight"
            preflight.mkdir()
            (runs / "not-a-directory").touch()
            fixtures = test_bench.SummaryTests()
            for index in range(3):
                directory = runs / ("prefix-" + str(index))
                directory.mkdir()
                fixtures.make_trial(directory)
                result_path = directory / "result.json"
                result = json.loads(result_path.read_text())
                result["trial"] = directory.name
                result_path.write_text(json.dumps(result))
            report = root / "report"
            arguments = ["summarize.py", "--output", str(report)]
            with patch.object(summarize, "LOCAL", root), patch.object(sys, "argv", arguments), \
                    contextlib.redirect_stdout(io.StringIO()):
                summarize.main()
            document = json.loads((report / "summary.json").read_text())
            self.assertEqual(len(document["trials"]), 3)
            self.assertEqual(document["groups"][0]["renderer"], "vanilla")
            self.assertFalse((matrix / "summary.json").exists())
            self.assertFalse((preflight / "summary.json").exists())

    def test_summary_default_discovery_refuses_incomplete_trial_directory(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runs = root / "runs"
            runs.mkdir()
            incomplete = runs / "incomplete"
            incomplete.mkdir()
            fixtures = test_bench.SummaryTests()
            for index in range(3):
                directory = runs / str(index)
                directory.mkdir()
                fixtures.make_trial(directory)
                result_path = directory / "result.json"
                result = json.loads(result_path.read_text())
                result["trial"] = directory.name
                result_path.write_text(json.dumps(result))
            arguments = ["summarize.py", "--output", str(root / "report")]
            with patch.object(summarize, "LOCAL", root), patch.object(sys, "argv", arguments), \
                    self.assertRaises(FileNotFoundError) as raised:
                summarize.main()
            self.assertEqual(Path(raised.exception.filename), incomplete / "result.json")

    def test_summary_explicit_directory_without_result_is_not_silently_excluded(self):
        with tempfile.TemporaryDirectory() as raw:
            arguments = ["summarize.py", raw]
            with patch.object(sys, "argv", arguments), self.assertRaises(FileNotFoundError):
                summarize.main()

    def test_gc_parser_refuses_truncated_and_durationless_pauses(self):
        for text in ("1.0: [Full GC unfinished", "1.0: [Full GC no duration]"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                summarize.gc_pauses(text)


if __name__ == "__main__":
    unittest.main()
