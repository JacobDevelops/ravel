#!/usr/bin/env python3
"""Runner policy checks without launching the game or probing hardware."""
import argparse
import os
from unittest import TestCase, main
from unittest.mock import patch

import run


class RunPolicyTests(TestCase):
    def test_physical_display_is_required(self) -> None:
        with patch.dict(os.environ, {"DISPLAY": ":99"}):
            with self.assertRaisesRegex(ValueError, "DISPLAY=:0"):
                run.validate_display(False)

    def test_software_rendering_is_not_a_physical_measurement(self) -> None:
        with patch.dict(os.environ, {"DISPLAY": ":0", "LIBGL_ALWAYS_SOFTWARE": "1"}):
            with self.assertRaisesRegex(ValueError, "software"):
                run.validate_display(False)

    def test_functional_run_requires_display(self) -> None:
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(ValueError, "DISPLAY"):
                run.validate_display(True)

    def test_physical_renderer_rejects_software_identity(self) -> None:
        with self.assertRaisesRegex(ValueError, "software"):
            run.validate_renderer("OpenGL renderer string: llvmpipe (LLVM)", False)
        run.validate_renderer("OpenGL renderer string: llvmpipe (LLVM)", True)
        run.validate_renderer("OpenGL renderer string: NVIDIA GeForce RTX 5090", False)

    def test_matrix_uses_three_warm_vanilla_trials(self) -> None:
        args = argparse.Namespace(matrix="vanilla-test", instance="vanilla", candidate=None,
                                  workload="vanilla", functional_only=False, timeout=1800)
        with patch.object(run.subprocess, "run") as execute:
            run.matrix(args)
        self.assertEqual(execute.call_count, 3)
        for index, call in enumerate(execute.call_args_list, 1):
            command = call.args[0]
            self.assertIn(f"vanilla-test-warm-filesystem-{index}", command)
            self.assertEqual(command[command.index("--workload") + 1], "vanilla")


if __name__ == "__main__":
    main()
