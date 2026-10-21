#!/usr/bin/env python3
"""Runner policy checks without launching the game or probing hardware."""
import argparse
import json
import os
from pathlib import Path
import tempfile
from typing import TypedDict
from unittest import TestCase, main
from unittest.mock import patch

import run


class GeometryExport(TypedDict):
    schemaVersion: int
    selection: str
    backend: str
    fidelity: str
    upload: str
    metrics: dict[str, int]
    bakedMetrics: list[int]
    bakedWorld: list[int]
    shadedNativeResidency: list[int]
    shadedGlResidency: list[int]


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
                                  workload="vanilla", renderer="vanilla",
                                  functional_only=False, timeout=1800)
        with patch.object(run.subprocess, "run") as execute:
            run.matrix(args)
        self.assertEqual(execute.call_count, 3)
        for index, call in enumerate(execute.call_args_list, 1):
            command = call.args[0]
            self.assertIn(f"vanilla-test-warm-filesystem-{index}", command)
            self.assertEqual(command[command.index("--workload") + 1], "vanilla")
            self.assertEqual(command[command.index("--renderer") + 1], "vanilla")

    def test_matrix_forwards_geometry_proof_without_mutating_candidate(self) -> None:
        args = argparse.Namespace(matrix="geometry-test", instance="ravel",
                                  candidate=Path("candidate.json"), workload="vanilla",
                                  renderer="geometry-proof", functional_only=False, timeout=900)
        with patch.object(run.subprocess, "run") as execute:
            run.matrix(args)
        self.assertEqual(execute.call_count, 3)
        for call in execute.call_args_list:
            command = call.args[0]
            self.assertEqual(command[command.index("--renderer") + 1], "geometry-proof")
            self.assertEqual(command[command.index("--candidate") + 1],
                             str(args.candidate.resolve()))

    def test_cli_defaults_to_vanilla_renderer(self) -> None:
        with patch("sys.argv", ["run.py", "default-trial"]), patch.object(run, "run") as execute:
            run.main()
        self.assertEqual(execute.call_args.args[0].renderer, "vanilla")

    def test_cli_accepts_geometry_proof_for_ravel_vanilla(self) -> None:
        command = ["run.py", "geometry-trial", "--instance", "ravel", "--workload", "vanilla",
                   "--candidate", "candidate.json", "--renderer", "geometry-proof"]
        with patch("sys.argv", command), patch.object(run, "run") as execute:
            run.main()
        args = execute.call_args.args[0]
        self.assertEqual((args.instance, args.workload, args.renderer),
                         ("ravel", "vanilla", "geometry-proof"))

    def test_cli_rejects_geometry_proof_outside_ravel_vanilla(self) -> None:
        for options in (["--instance", "vanilla"],
                        ["--instance", "original"],
                        ["--instance", "cleanroom"],
                        ["--instance", "ravel", "--candidate", "candidate.json"]):
            with self.subTest(options=options), \
                 patch("sys.argv", ["run.py", "invalid", "--renderer", "geometry-proof", *options]), \
                 patch.object(run, "run") as execute, patch("sys.stderr"):
                with self.assertRaises(SystemExit) as error:
                    run.main()
                self.assertEqual(error.exception.code, 2)
                execute.assert_not_called()

    def test_geometry_export_accepts_clean_on_and_off_arms(self) -> None:
        for renderer in ("vanilla", "geometry-proof"):
            with self.subTest(renderer=renderer), tempfile.TemporaryDirectory() as raw:
                out = Path(raw)
                data = self.geometry_export(renderer)
                (out / "geometry-upload.json").write_text(json.dumps(data))
                run.validate_geometry_export(out, renderer, 0)

    def test_geometry_export_rejects_missing_stale_or_wrong_selection(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            out = Path(raw)
            with self.assertRaisesRegex(ValueError, "missing"):
                run.validate_geometry_export(out, "geometry-proof", 0)
            path = out / "geometry-upload.json"
            path.write_text(json.dumps(self.geometry_export("geometry-proof")))
            with self.assertRaisesRegex(ValueError, "stale"):
                run.validate_geometry_export(out, "geometry-proof", path.stat().st_mtime_ns + 1)
            with self.assertRaisesRegex(ValueError, "selection"):
                run.validate_geometry_export(out, "vanilla", 0)

    def test_geometry_export_rejects_invalid_work_bounds_and_cleanup(self) -> None:
        cases = (("schemaVersion", 2), ("backend", "unsupported"),
                 ("backend", "selected-not-initialized"),
                 ("copiedMeshes", 0), ("uploads", 0), ("submissions", 0),
                 ("cleanups", 0), ("maxResidentBytes", 1_179_649),
                 ("maxStagingBytes", 1_212_417), ("residentBytes", 1),
                 ("stagingBytes", 1), ("liveBuffers", 1), ("liveFramebuffers", 1),
                 ("uploads", True), ("uploadBytes", -1))
        for name, value in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw:
                data = self.geometry_export("geometry-proof")
                target = data if name in ("schemaVersion", "backend") else data["metrics"]
                target[name] = value
                out = Path(raw)
                (out / "geometry-upload.json").write_text(json.dumps(data))
                with self.assertRaises(ValueError):
                    run.validate_geometry_export(out, "geometry-proof", 0)
        with tempfile.TemporaryDirectory() as raw:
            data = self.geometry_export("vanilla")
            data["metrics"]["copiedMeshes"] = 1
            out = Path(raw)
            (out / "geometry-upload.json").write_text(json.dumps(data))
            with self.assertRaisesRegex(ValueError, "vanilla"):
                run.validate_geometry_export(out, "vanilla", 0)

    def test_selected_arm_requires_admitted_shaded_baked_comparisons(self) -> None:
        cases = ((0, 0), (4, 0), (5, 1), (10, 0), (12, 2), (13, 0), (16, 0), (20, 1),
                 (27, 1), (2, -1), (29, 1))
        for index, value in cases:
            with self.subTest(index=index), tempfile.TemporaryDirectory() as raw:
                data = self.geometry_export("geometry-proof")
                data["bakedMetrics"][index] = value
                out = Path(raw)
                (out / "geometry-upload.json").write_text(json.dumps(data))
                with self.assertRaises(ValueError):
                    run.validate_geometry_export(out, "geometry-proof", 0)
        for world in ([0, 0, 0, 0, 1, 1, 4], [9, 0, 0, 0, 1, 1, 4], [1, 0, 0, 0, 0, 1, 4],
                      [1, 1, 0, 0, 1, 1, 4], [1, 0, 1, 0, 1, 1, 4], [1, 0, 0, 1, 1, 1, 4],
                      [1, 4097, 0, 0, 1, 1, 4], [1, 0, 0]):
            with self.subTest(world=world), tempfile.TemporaryDirectory() as raw:
                data = self.geometry_export("geometry-proof")
                data["bakedWorld"] = world
                out = Path(raw)
                (out / "geometry-upload.json").write_text(json.dumps(data))
                with self.assertRaises(ValueError):
                    run.validate_geometry_export(out, "geometry-proof", 0)
        with tempfile.TemporaryDirectory() as raw:
            data = self.geometry_export("vanilla")
            data["bakedMetrics"][1] = 1
            out = Path(raw)
            (out / "geometry-upload.json").write_text(json.dumps(data))
            with self.assertRaisesRegex(ValueError, "baked"):
                run.validate_geometry_export(out, "vanilla", 0)

    def test_shaded_residency_requires_bounded_acknowledged_reuse_and_complete_drain(self) -> None:
        for field, index, value in (("shadedNativeResidency", 0, 1), ("shadedNativeResidency", 1, 1),
                ("shadedNativeResidency", 2, 5_636_193), ("shadedNativeResidency", 3, 0),
                ("shadedNativeResidency", 4, 0), ("shadedNativeResidency", 5, 1),
                ("shadedGlResidency", 1, 2_752_513), ("shadedGlResidency", 2, 2),
                ("shadedGlResidency", 3, 1), ("shadedGlResidency", 5, 2),
                ("shadedGlResidency", 6, 1), ("shadedGlResidency", 14, 0),
                ("shadedGlResidency", 15, 1), ("shadedGlResidency", 0, True)):
            data = self.geometry_export("geometry-proof")
            data[field][index] = value
            with self.subTest(field=field, index=index), self.assertRaises(ValueError):
                run.validate_shaded_export(data, "geometry-proof")
        for field in ("shadedNativeResidency", "shadedGlResidency"):
            data = self.geometry_export("geometry-proof")
            data[field] = []
            with self.assertRaises(ValueError):
                run.validate_shaded_export(data, "geometry-proof")
        data = self.geometry_export("vanilla")
        data["shadedNativeResidency"][3] = 1
        with self.assertRaisesRegex(ValueError, "vanilla"):
            run.validate_shaded_export(data, "vanilla")

    @staticmethod
    def geometry_export(renderer: str) -> GeometryExport:
        metrics = dict.fromkeys(run.GEOMETRY_METRICS, 0)
        baked = [0] * run.BAKED_METRICS
        baked[10] = -1
        world = [0] * 7
        native, gl = [0] * 12, [0] * 16
        if renderer == "geometry-proof":
            metrics.update(copiedMeshes=1, copiedBytes=100, uploads=1, uploadBytes=60,
                           submissions=1, cleanups=1, maxResidentBytes=60,
                           maxStagingBytes=1_212_416)
            for index, value in ((0, 2), (2, 1), (4, 6), (11, 1), (12, 1), (13, 2), (16, 224), (25, 1)):
                baked[index] = value
            world = [2, 0, 0, 0, 30, 30, 24]
            native[:5] = [0, 0, 100, 2, 2]
            gl[1:5] = [2_752_512, 3, 2, 1]
            gl[14] = 1
        return {"schemaVersion": 1, "selection": renderer,
                "backend": "OpenGL15" if renderer == "geometry-proof" else "not-selected",
                "fidelity": run.GEOMETRY_FIDELITY,
                "upload": "bounded-native-wire-java-owned-offscreen-gl", "metrics": metrics,
                "bakedMetrics": baked, "bakedWorld": world,
                "shadedNativeResidency": native, "shadedGlResidency": gl}


if __name__ == "__main__":
    main()
