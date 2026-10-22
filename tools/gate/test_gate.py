from contextlib import ExitStack
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import gate


def metadata():
    packages = [{"id": name, "name": name,
                 "manifest_path": str(gate.ROOT / "crates" / name / "Cargo.toml")}
                for name in ("base", "ravel-composed-host", "dev")]
    nodes = [{"id": package["id"], "dependencies": [], "deps": []} for package in packages]
    nodes[1].update(dependencies=["base", "dev"], deps=[
        {"pkg": "base", "dep_kinds": [{"kind": None}]},
        {"pkg": "dev", "dep_kinds": [{"kind": "dev"}]}])
    return {"packages": packages, "workspace_members": [p["id"] for p in packages],
            "resolve": {"nodes": nodes}}


class GateTests(unittest.TestCase):
    def test_scope_includes_reverse_dependents(self):
        with patch.object(gate, "cargo_metadata", return_value=metadata()):
            self.assertEqual(gate.package_scope(["crates/base/src/lib.rs"]),
                             (["base"], ["base", "ravel-composed-host"]))

    def test_global_input_selects_workspace(self):
        with patch.object(gate, "cargo_metadata", return_value=metadata()):
            touched, affected = gate.package_scope(["minecraft/versions/new/adapter.toml"])
            self.assertEqual(touched, ["base", "dev", "ravel-composed-host"])
            self.assertEqual(touched, affected)
            self.assertEqual(gate.package_scope(["docs/guide.md"]), ([], []))

    def test_legacy_includes_runtime_but_not_dev_dependencies(self):
        with patch.object(gate, "cargo_metadata", return_value=metadata()):
            for path in ("jvm/bootstrap/Boot.java", "minecraft/versions/1.12.2/adapter.toml",
                         "crates/base/src/lib.rs", "tools/ravel-xtask/src/generate.rs"):
                with self.subTest(path=path):
                    self.assertTrue(gate.legacy_changed([path]))
            self.assertFalse(gate.legacy_changed(["crates/dev/src/lib.rs", "docs/guide.md"]))

    def test_changed_paths_supports_explicit_landing_base(self):
        with patch.dict(os.environ, {"RAVEL_GATE_BASE": "@"}), patch.object(
                gate, "capture", return_value="crates/base/src/lib.rs\ndocs/guide.md") as capture:
            self.assertEqual(gate.changed_paths("trunk()"),
                             ["crates/base/src/lib.rs", "docs/guide.md"])
            capture.assert_called_once_with("jj", "diff", "--from", "trunk()", "--to", "@", "--name-only")

    def test_receipt_key_changes_with_every_proof_input(self):
        values = ["tree", [["1.12.2", "live"]], {"jar": "digest"}, {"environment": "digest"}, False]
        original = gate.receipt_key(*values)
        self.assertEqual(original, gate.receipt_key(*values))
        for index, replacement in enumerate(("other", [], {}, {}, True)):
            changed = values.copy()
            changed[index] = replacement
            self.assertNotEqual(original, gate.receipt_key(*changed))

    def test_clean_room_listing_detects_add_remove_and_modify(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            empty = gate.listing_digest(directory)
            source = directory / "Source.java"
            source.write_text("class Source {}")
            present = gate.listing_digest(directory)
            self.assertNotEqual(empty, present)
            source.write_text("class DifferentSource {}")
            self.assertNotEqual(present, gate.listing_digest(directory))
            source.unlink()
            self.assertEqual(empty, gate.listing_digest(directory))
            (directory / "empty-source-tree").mkdir()
            self.assertNotEqual(empty, gate.listing_digest(directory))
            before_link = gate.listing_digest(directory)
            (directory / "dangling").symlink_to(directory / "absent")
            self.assertNotEqual(before_link, gate.listing_digest(directory))

    def test_receipt_context_tracks_manifest_and_gate_environment(self):
        with tempfile.TemporaryDirectory() as temporary, tempfile.TemporaryDirectory() as manifests, patch.object(gate, "capture", return_value="rustc test"):
            manifest = Path(manifests) / "manifest"
            manifest.write_text("first")
            environment = {"RAVEL_CLEAN_ROOM_SRC": temporary,
                           "RAVEL_CLEAN_ROOM_MANIFEST": str(manifest)}
            with patch.dict(os.environ, environment, clear=True):
                original = gate.receipt_context()
                with patch.dict(os.environ, {"RAVEL_MINECRAFT_VERSION": "1.12.2"}):
                    self.assertNotEqual(original, gate.receipt_context())
                with patch.dict(os.environ, {"RAVEL_FORCE_VERIFY": "1"}):
                    self.assertEqual(original, gate.receipt_context())
                with patch.dict(os.environ, {"CARGO_MAKE_GIT_HEAD_LAST_COMMIT_HASH": "different"}):
                    self.assertEqual(original, gate.receipt_context())
                manifest.write_text("second")
                self.assertNotEqual(original, gate.receipt_context())

    def test_legacy_live_builds_before_launching(self):
        with patch.object(gate, "run") as run:
            gate.legacy_live()
            self.assertEqual(run.call_args_list, [
                unittest.mock.call("cargo", "make", "ravel:legacy-agent-build"),
                unittest.mock.call("cargo", "make", "ravel:legacy-agent-live")])

    def test_landing_receipt_requires_full_and_legacy_proofs(self):
        with tempfile.TemporaryDirectory() as temporary, ExitStack() as stack:
            stack.enter_context(patch.object(gate, "ROOT", Path(temporary)))
            for name, result in (("tree_identity", "tree"), ("matrix_eligibility", []),
                                 ("input_digests", {}), ("receipt_context", {}),
                                 ("changed_paths", ["jvm/bootstrap/Boot.java"]),
                                 ("legacy_inputs", []), ("digest_paths", {})):
                stack.enter_context(patch.object(gate, name, return_value=result))
            run = stack.enter_context(patch.object(gate, "run"))
            stack.enter_context(patch.dict(os.environ, {}, clear=True))
            gate.land()
            gate.changed_paths.assert_called_with("trunk()")
            self.assertEqual(run.call_args_list, [
                unittest.mock.call("cargo", "make", "ravel:verify"),
                unittest.mock.call("cargo", "make", "ravel:legacy-agent-build"),
                unittest.mock.call("cargo", "make", "ravel:legacy-agent-live")])
            run.reset_mock()
            gate.land()
            run.assert_called_once_with("cargo", "make", "ravel:verify-stack")
            run.reset_mock()
            with patch.object(gate, "receipt_context", return_value={"changed": True}):
                gate.land()
            self.assertEqual(run.call_count, 3)

    def test_failed_legacy_launch_never_writes_receipt(self):
        with tempfile.TemporaryDirectory() as temporary, ExitStack() as stack:
            stack.enter_context(patch.object(gate, "ROOT", Path(temporary)))
            for name, result in (("tree_identity", "tree"), ("matrix_eligibility", []),
                                 ("input_digests", {}), ("receipt_context", {}),
                                 ("changed_paths", ["jvm/bootstrap/Boot.java"])):
                stack.enter_context(patch.object(gate, name, return_value=result))
            stack.enter_context(patch.object(gate, "run", side_effect=[None, None, RuntimeError("failed")]))
            with self.assertRaisesRegex(RuntimeError, "failed"):
                gate.land()
            self.assertFalse((Path(temporary) / "target/gate/verified").exists())


if __name__ == "__main__":
    unittest.main()
