"""Run producer boundaries: python3 -B -m unittest discover -s registry/waves."""

import sys

sys.dont_write_bytecode = True

import contextlib
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest

import route
import wave_common
from wave_test_support import patched_attribute


spec = importlib.util.spec_from_file_location(
    "wave_select", pathlib.Path(__file__).with_name("select.py")
)
wave_select = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wave_select)




class WaveBoundaries(unittest.TestCase):
    def test_demand_thresholds_and_client_screen_condition(self) -> None:
        self.assertEqual(route.demands({}), set())
        self.assertEqual(
            route.demands({
                "data": {"recipes": 1, "lootTables": 1, "tags": 0},
                "resources": {"models": 1},
                "indicative": {"classFiles": 1, "persistenceClasses": 1},
                "integration": {"accessTransformers": 1},
                "side": {"layout": "client-leaning"},
            }),
            {"recipes", "loot", "resources", "content-behaviour", "config",
             "persistence", "interception", "screens"},
        )
        self.assertEqual(route.demands({"resources": {"models": 1}}), {"resources"})

    def test_missing_plan_is_rejected_before_output_publication(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            inventory = root / "inventory" / route.FIXTURE
            inventory.mkdir(parents=True)
            (inventory / "surfaces.json").write_text(json.dumps({
                "artifacts": [{"artifact": "missing.jar"}],
            }))
            (inventory / "plans.json").write_text('{"plans": []}')
            output = root / "waves" / "routing.json"
            output.parent.mkdir()
            output.write_text("previous\n")
            with patched_attribute(route, "ROOT", root), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(route.main(), 1)
            self.assertEqual(output.read_text(), "previous\n")

    def test_atomic_json_failure_preserves_existing_file_and_cleans_temporary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "wave.json"
            output.write_text("previous\n")

            def failed_replace(path: pathlib.Path, target: pathlib.Path) -> None:
                raise OSError("cannot replace")

            with patched_attribute(pathlib.Path, "replace", failed_replace):
                with self.assertRaises(OSError):
                    wave_common.write_json(output, {"new": True})
            self.assertEqual(output.read_text(), "previous\n")
            self.assertEqual(list(output.parent.iterdir()), [output])
            wave_common.write_json(output, {"new": True})
            self.assertEqual(output.read_text(), '{\n  "new": true\n}\n')

    def test_atomic_json_replacement_preserves_existing_permissions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "wave.json"
            output.write_text("previous\n")
            output.chmod(0o640)
            wave_common.write_json(output, {"published": True})
            self.assertEqual(output.stat().st_mode & 0o7777, 0o640)
            self.assertEqual(json.loads(output.read_text()), {"published": True})

    def test_closure_terminates_on_cycles_and_preserves_shared_dependencies(self) -> None:
        self.assertEqual(
            wave_select.closure(["a", "b"], {"a": ["shared"], "b": ["shared"], "shared": ["a"]}),
            {"a", "b", "shared"},
        )

    def test_cluster_excludes_unsettled_dependency_closures(self) -> None:
        by_id = {
            "a": {"owner": "content-data"},
            "b": {"owner": "client-presentation"},
            "shared": {"owner": "sdk-platform"},
        }
        deps = {"a": ["shared"], "b": ["shared"], "shared": []}
        self.assertEqual(wave_select.cluster_candidates(by_id, deps, {"a", "b"}), [])
        self.assertEqual(
            wave_select.cluster_candidates(by_id, deps, set(by_id)),
            [(frozenset(by_id), ("a", "b"))],
        )


if __name__ == "__main__":
    unittest.main()
