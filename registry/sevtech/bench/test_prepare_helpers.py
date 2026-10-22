#!/usr/bin/env python3
"""Private comparator preparation checks without game, network or compiler execution."""
import ast
import io
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import Mock, patch
import zipfile

import cleanroom
import prepare
import run


class PrepareHelperTests(unittest.TestCase):
    def test_options_preserve_benchmark_protocol(self):
        with tempfile.TemporaryDirectory() as directory:
            game = Path(directory)
            values = prepare.configure_options(game)
            self.assertEqual(values["renderDistance"], "12")
            self.assertEqual((game / "options.txt").read_text(),
                             "".join(f"{key}:{value}\n" for key, value in values.items()))
            self.assertNotIn("fancyGraphics", values)
            self.assertEqual(values["maxFps"], "0")
            self.assertNotIn("guiScale", values)
            self.assertEqual(values["soundCategory_master"], "0.0")

    def test_reset_restores_publisher_files_but_preserves_mods_and_saves(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, game = root / "pack.zip", root / "game"
            game.mkdir()
            with zipfile.ZipFile(archive, "w") as source:
                source.writestr("overrides/config/publisher.cfg", "publisher")
                source.writestr("overrides/options.txt", "renderDistance:8\ncustom:true\n")
                source.writestr("overrides/mods/bundled.jar", "replacement")
            for name in ("mods", "saves", "config"):
                (game / name).mkdir()
                (game / name / "retained").write_text("existing")
            prepare.reset_overrides(archive, game)
            self.assertEqual((game / "config/publisher.cfg").read_text(), "publisher")
            self.assertFalse((game / "config/retained").exists())
            for name in ("mods", "saves"):
                self.assertEqual((game / name / "retained").read_text(), "existing")
            self.assertFalse((game / "mods/bundled.jar").exists())
            prepare.configure_options(game)
            first = (game / "options.txt").read_text()
            (game / "options.txt").write_text("custom:mutated\n")
            prepare.configure_options(game)
            self.assertEqual((game / "options.txt").read_text(), first)
            self.assertIn("custom:true\n", first)
            self.assertIn("renderDistance:12\n", first)

    def test_archive_overrides_rejects_traversal_and_links(self):
        for name, mode in (("overrides/../escape", 0), ("overrides/link", stat.S_IFLNK)):
            with self.subTest(name=name), io.BytesIO() as data:
                with zipfile.ZipFile(data, "w") as archive:
                    entry = zipfile.ZipInfo(name)
                    entry.external_attr = mode << 16
                    archive.writestr(entry, "target")
                with zipfile.ZipFile(data) as archive, self.assertRaises(ValueError):
                    list(prepare.archive_overrides(archive))

    def test_pins_allow_metadata_refresh_but_reject_changed_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            baseline = Path(directory)
            prism = baseline / "prism"
            (prism / "libraries").mkdir(parents=True)
            (prism / "meta").mkdir()
            artifact, metadata = prism / "libraries/library.jar", prism / "meta/index.json"
            artifact.write_bytes(b"immutable")
            metadata.write_text('{"url": "https://example.invalid/first"}')
            prepare.pin_downloads(baseline, prism, {})
            metadata.write_text('{"url": "https://example.invalid/second"}')
            prepare.pin_downloads(baseline, prism, {})
            artifact.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "changed or vanished"):
                prepare.pin_downloads(baseline, prism, {})

    def test_main_refuses_when_java_lookup_times_out(self):
        # Run the production CLI guard while retaining the patched main() dependencies.
        entrypoint = ast.parse(Path(prepare.__file__).read_text()).body[-1]
        code = compile(ast.Module(body=[entrypoint], type_ignores=[]), prepare.__file__, "exec")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            agent, output = root / "observer.jar", root / "output"
            agent.touch()
            error = prepare.subprocess.TimeoutExpired(["mise", "where", prepare.JAVA], 60)
            with patch.object(prepare, "__file__", str(root / "registry/sevtech/bench/prepare.py")), \
                    patch.object(prepare.sys, "argv", [
                        "prepare.py", "--agent", str(agent), "--output", str(output)]), \
                    patch.object(prepare, "verify_inputs", return_value=(root / "pack.zip", {}, {})), \
                    patch.object(prepare.subprocess, "run", side_effect=error) as command:
                with self.assertRaises(SystemExit) as refused:
                    exec(code, {**vars(prepare), "__name__": "__main__"})
            self.assertEqual(str(refused.exception), "baseline preparation refused: " + str(error))
            command.assert_called_once_with(
                ["mise", "where", prepare.JAVA], cwd=root, check=True,
                text=True, capture_output=True, timeout=60)
            self.assertFalse(output.exists())


class CleanroomHelperTests(unittest.TestCase):
    def test_overlay_updates_components_and_only_selected_directories(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, instance = root / "overlay.zip", root / "instance"
            instance.mkdir()
            pack = {"components": [
                {"uid": "net.minecraft", "version": "1.12.2"},
                {"uid": "org.lwjgl", "version": "3.4.3"},
                {"uid": "net.minecraftforge", "version": cleanroom.VERSION}]}
            component = {"compatibleJavaMajors": [25]}
            with zipfile.ZipFile(archive, "w") as source:
                source.writestr("mmc-pack.json", json.dumps(pack))
                source.writestr("patches/net.minecraftforge.json", json.dumps(component))
                source.writestr("libraries/private.jar", "library")
                source.writestr(".minecraft/mods/unwanted.jar", "not copied")
            self.assertEqual(cleanroom.apply_overlay(archive, instance), component)
            self.assertEqual(json.loads((instance / "mmc-pack.json").read_text()), pack)
            self.assertEqual((instance / "libraries/private.jar").read_text(), "library")
            self.assertFalse((instance / ".minecraft").exists())

    def test_overlay_pins_refuse_mod_population_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "overlay.zip"
            cleanroom.write_pins(root, archive, {"mod.jar": (root / "mod.jar", "abc")})
            with self.assertRaisesRegex(ValueError, "input pins changed"):
                cleanroom.write_pins(root, archive, {"mod.jar": (root / "mod.jar", "def")})

    def test_clone_excludes_worlds_but_refuses_linked_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "saves").mkdir()
            (root / "config").symlink_to(root / "saves", target_is_directory=True)
            self.assertEqual(cleanroom.ignore_private(str(root), ["saves"]), ["saves"])
            with self.assertRaisesRegex(ValueError, "linked original input"):
                cleanroom.ignore_private(str(root), ["config"])

    def test_main_records_setup_blocked_when_setup_times_out(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            agent, output = root / "observer.jar", root / "output"
            agent.touch()
            error = cleanroom.subprocess.TimeoutExpired(["mise", "which", "java"], 60)
            with patch.object(cleanroom, "__file__", str(root / "registry/sevtech/bench/cleanroom.py")), \
                    patch.object(prepare.sys, "argv", [
                        "cleanroom.py", "--agent", str(agent), "--output", str(output)]), \
                    patch.object(cleanroom, "setup", side_effect=error) as setup:
                with self.assertRaises(SystemExit) as blocked:
                    cleanroom.main()
            self.assertEqual(str(blocked.exception), "Cleanroom optional setup blocked: " + str(error))
            setup.assert_called_once_with(
                root, root / "tests/fixtures/local" / prepare.FIXTURE / "baseline", agent, output)
            self.assertEqual(json.loads((output / "cleanroom-settings.json").read_text()), {
                "status": "setup-blocked", "optional": True, "blocker": str(error),
                "runtime_observed": False, "mod_changes": []})
            self.assertFalse((output / "settings.json").exists())


class RunHelperTests(unittest.TestCase):
    def test_runtime_status_retains_settings_and_records_observation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "cleanroom-settings.json"
            path.write_text('{"optional": true}')
            run.update_runtime_status(root, 1)
            self.assertEqual(json.loads(path.read_text()), {
                "optional": True, "status": "runtime-incomplete", "runtime_observed": False,
                "launcher_exit": 1})
            (root / "complete").touch()
            (root / "events.csv").touch()
            run.update_runtime_status(root, 0)
            self.assertEqual(json.loads(path.read_text())["status"], "complete")
            self.assertTrue(json.loads(path.read_text())["runtime_observed"])

    def test_java_resolution_checks_repository_release_pin(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "mise.toml").write_text('[tools]\njava = "temurin-27.0.1+12"\n')
            java = root / "jdk/bin/java"
            java.parent.mkdir(parents=True)
            java.touch()
            release = java.parent.parent / "release"
            release.write_text('JAVA_VERSION="27.0.1"\nJAVA_RUNTIME_VERSION="27.0.1+12"\n')
            with patch.object(cleanroom.subprocess, "run", return_value=Mock(stdout=str(java))):
                self.assertEqual(cleanroom.resolve_java(root),
                                 (str(java), "temurin-27.0.1+12", "27.0.1"))
                release.write_text('JAVA_VERSION="27.0.1"\nJAVA_RUNTIME_VERSION="27.0.1+13"\n')
                with self.assertRaisesRegex(ValueError, "differs from repository pin"):
                    cleanroom.resolve_java(root)

    def test_monitor_deadline_expires_without_querying_gpu(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(run.subprocess, "run") as command:
                with self.assertRaisesRegex(TimeoutError, "explicit trial deadline"):
                    run.monitor(Mock(), Mock(), Path(directory), 0)
                command.assert_not_called()

    def test_monitor_deadline_expires_after_entering_loop_without_querying_gpu(self):
        for elapsed in (110, 111):
            with self.subTest(elapsed=elapsed), tempfile.TemporaryDirectory() as directory:
                process, writer = Mock(), Mock()
                with patch.object(run.time, "monotonic", side_effect=[100, 101, elapsed]), \
                        patch.object(run, "java_process", return_value=None) as java_process, \
                        patch.object(run.subprocess, "run") as command, \
                        patch.object(run.time, "sleep") as sleep:
                    with self.assertRaisesRegex(TimeoutError, "explicit trial deadline"):
                        run.monitor(process, writer, Path(directory), 10)
                java_process.assert_called_once_with(Path(directory))
                command.assert_not_called()
                sleep.assert_not_called()
                writer.writerow.assert_not_called()
                process.poll.assert_not_called()
                self.assertEqual((Path(directory) / "gpu.csv").read_text(),
                                 "monotonic_ns,used_vram_mib,gpu_percent\n")

    def test_gpu_query_cannot_overrun_trial_deadline(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(run, "java_process", return_value=None), \
                    patch.object(run.time, "monotonic", side_effect=[100, 101, 102]), \
                    patch.object(run.subprocess, "run", side_effect=run.subprocess.TimeoutExpired(
                        "nvidia-smi", 8)) as command:
                with self.assertRaises(run.subprocess.TimeoutExpired):
                    run.monitor(Mock(), Mock(), Path(directory), 10)
            self.assertEqual(command.call_args.kwargs["timeout"], 8)

    def test_monitor_stops_when_observed_jvm_disappears(self):
        with tempfile.TemporaryDirectory() as directory:
            process, writer = Mock(), Mock()
            process.poll.return_value = None
            with patch.object(run, "java_process", side_effect=[Path("/proc/123"), None]), \
                    patch.object(run, "sample", return_value=[1, "123", 2, 3, 4]), \
                    patch.object(run.subprocess, "run", return_value=Mock(stdout="5,6")), \
                    patch.object(run.time, "sleep"):
                run.monitor(process, writer, Path(directory), 10)
            writer.writerow.assert_called_once_with([1, "123", 2, 3, 4])


if __name__ == "__main__":
    unittest.main()
