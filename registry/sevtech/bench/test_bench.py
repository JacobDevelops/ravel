#!/usr/bin/env python3
"""Deterministic stdlib checks; no game, launcher, network, or compiler execution."""
import importlib.util
import csv
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("bench_summary", HERE / "summarize.py")
summary = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(summary)
PREP_SPEC = importlib.util.spec_from_file_location("bench_prepare", HERE / "prepare.py")
prepare = importlib.util.module_from_spec(PREP_SPEC)
PREP_SPEC.loader.exec_module(prepare)


class SummaryTests(unittest.TestCase):
    def protocol(self):
        return {"agentSha256": "a" * 64, "fixture": "sevtech-ages-3.2.3",
                "artifactCount": 259, "options": {"renderDistance": "12", "fullscreen": "false"}}

    def cohort(self):
        return {"javaVersion": "fixture Java 8", "candidateSha256": None,
                "classpathSha256": ["f" * 64],
                "argv": ["/java", "-Xmx8G", "-javaagent:/observer.jar=<trial>", "Main"]}

    def test_nearest_rank(self):
        self.assertEqual(summary.quantile([4, 1, 3, 2], 0.5), 2)
        self.assertEqual(summary.quantile(range(1, 101), 0.95), 95)
        self.assertEqual(summary.quantile(range(1, 101), 0.99), 99)
        with self.assertRaises(ValueError):
            summary.quantile([], 0.5)

    def test_gc_pauses_exclude_concurrent_and_duplicate_records(self):
        text = (
            "2026-10-18T12:00:00.000+0000: 1.0: [GC pause (G1 Evacuation Pause) (young), 0.010 secs]\n"
            "   [Parallel Time: 9.0 ms, GC Workers: 4]\n"
            " [Times: user=0.01 sys=0.00, real=0.01 secs]\n"
            "1.0: [GC pause (G1 Evacuation Pause) (young), 0.010 secs]\n"
            "2.0: [GC concurrent-mark-end, 0.20 secs]\n"
            "3.0: [Full GC (Allocation Failure) [PSYoungGen: 4K->0K(8K)] 8K->4K(16K), 0.040 secs]\n"
        )
        self.assertEqual(summary.gc_pauses(text), [10.0, 40.0])

    def test_gc_multiline_root_pause(self):
        text = "1.0: [Full GC (System.gc())\n[ParOldGen: 10K->5K(20K)]\n, 0.125 secs]\n"
        self.assertEqual(summary.gc_pauses(text), [125.0])

    def test_gc_java8_remark_and_cleanup_are_stop_the_world(self):
        text = ("1.0: [GC remark [Finalize Marking, 0.001 secs], 0.004 secs]\n"
                "2.0: [GC cleanup 10M->8M(32M), 0.002 secs]\n"
                "3.0: [GC concurrent-cleanup-end, 0.300 secs]\n")
        self.assertEqual(summary.gc_pauses(text), [4.0, 2.0])

    def test_unified_gc_pauses_exclude_concurrent_start_and_duplicates(self):
        text = ("[1.0s][info][gc,start] GC(0) Pause Young (Normal)\n"
                "[1.1s][info][gc] GC(0) Pause Young (Normal) 20M->5M(32M) 12.3ms\n"
                "[1.1s][info][gc] GC(0) Pause Young (Normal) 20M->5M(32M) 12.3ms\n"
                "[2.0s][info][gc] GC(1) Concurrent Mark Cycle 99.0ms\n"
                "[3.0s][info][gc] GC(1) Pause Remark 10M->8M(32M) 0.5ms\n")
        self.assertEqual(summary.gc_pauses(text), [12.3, 0.5])

    def test_missing_trial_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises((ValueError, FileNotFoundError)):
                summary.summarize_trial(Path(directory))

    def test_incomplete_trial_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "result.json").write_text('{"complete": false}')
            with self.assertRaises(ValueError):
                summary.summarize_trial(root)

    def test_three_completed_trials_required_per_group(self):
        trials = [{"trial": str(index), "instance": "original", "cache": "warm-filesystem",
                   "workload_protocol": self.protocol(),
                   "runtime_cohort": self.cohort(),
                   "metrics": {"startup_ms": 10.0 + index}} for index in range(3)]
        with self.assertRaises(ValueError):
            summary.aggregate(trials[:2])
        result = summary.aggregate(trials)
        self.assertEqual(result[0]["trial_count"], 3)
        self.assertEqual(result[0]["metrics"]["startup_ms"]["p50"], 11.0)
        self.assertEqual(result[0]["metrics"]["startup_ms"]["p99"], 12.0)

    def test_sparse_gc_has_no_false_trial_distribution(self):
        trials = [{"trial": str(index), "instance": "original", "cache": "warm-filesystem",
                   "workload_protocol": self.protocol(),
                   "runtime_cohort": self.cohort(),
                   "metrics": {"gc_pause.p95_ms": None if index == 0 else 10.0}} for index in range(3)]
        metric = summary.aggregate(trials)[0]["metrics"]["gc_pause.p95_ms"]
        self.assertEqual(metric["trial_samples"], 2)
        self.assertIsNone(metric["p95"])

    def test_complete_trial_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_trial(root)
            result = summary.summarize_trial(root)
            self.assertEqual(result["workload_protocol"], self.protocol())
            self.assertEqual(result["metrics"]["agent_startup_ms"], 1000)
            self.assertEqual(result["metrics"]["wall_to_menu_ms"], 500)
            self.assertEqual(result["steady_state_scope"]["scene"], "loaded_chunk_stress")
            self.assertEqual(result["metrics"]["frames.spawn_idle.p95_ms"], 20)
            self.assertEqual(result["metrics"]["cpu_percent.sample_interval_mean"], 100)
            self.assertEqual(result["metrics"]["rss_mib.peak"], 4)
            self.assertEqual(result["metrics"]["gpu_device_vram_mib.steady_last30s_mean"], 50)
            self.assertEqual(result["sample_counts"]["gc_pauses"], 0)
            self.assertIsNone(result["metrics"]["gc_pause.p50_ms"])

    def test_startup_anchors_are_ordered_without_inventing_phases(self):
        source = ("[23:59:58] [main/INFO]: Forge Mod Loader version 14.23.5.2860 for Minecraft 1.12.2 loading\n"
                  "[23:59:59] [Client thread/INFO]: Injecting itemstacks\n"
                  "[00:00:01] [Client thread/INFO]: Forge Mod Loader has identified 273 mods to load\n"
                  "untimestamped Injecting itemstacks ignored\n"
                  "[00:00:04] [Client thread/INFO]: Injecting itemstacks\n"
                  "[00:00:09] [Client thread/INFO]: Injecting itemstacks\n")
        result = summary.startup_phases(source)
        self.assertEqual(result["markers"][0]["source_text"], source.splitlines()[0])
        self.assertEqual(result["markers"][1]["timestamp"], "00:00:01")
        self.assertEqual([row["seconds"] for row in result["observed_marker_intervals"]], [3, 3, 5])
        self.assertEqual([row["phase"] for row in result["markers"]],
                         ["fml_launch", "mods_identified", "preinit_complete", "init_complete"])
        self.assertEqual([row["source_line"] for row in result["markers"]], [1, 3, 5, 6])
        self.assertEqual(summary.startup_phases("")["observed_marker_intervals"], [])

    def test_missing_full_steady_window_refused(self):
        measurements = [{"monotonic_ns": 99_000_000_000, "value": 1}]
        with self.assertRaises(ValueError):
            summary.sampled_metrics(measurements, {"value": ("metric", 1)}, 100_000_000_000)

    def test_runtime_specific_settings_excluded_from_common_protocol(self):
        settings = {**self.protocol(), "instance": "ravel", "javaPath": "/different/java",
                    "jvmArgs": ["-Xmx4G"], "components": [{"uid": "ravel"}],
                    "output": "/different/trial"}
        self.assertEqual(summary.workload_protocol(settings), self.protocol())

    def test_different_workload_protocols_refused_across_groups(self):
        for field, value in (("agentSha256", "b" * 64), ("options", {"renderDistance": "8"}),
                             ("artifactCount", 258), ("fixture", "different-pack")):
            with self.subTest(field=field):
                trials = [{"trial": str(index), "instance": "original" if index < 3 else "ravel",
                           "cache": "warm-filesystem", "workload_protocol": self.protocol(),
                           "metrics": {"startup_ms": 10.0}} for index in range(6)]
                for trial in trials[3:]:
                    trial["workload_protocol"][field] = value
                with self.assertRaises(ValueError):
                    summary.aggregate(trials)

    def test_missing_workload_provenance_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_trial(root)
            settings = root / "settings.json"
            settings.unlink()
            with self.assertRaises(FileNotFoundError):
                summary.summarize_trial(root)
            for field in self.protocol():
                with self.subTest(field=field):
                    protocol = self.protocol()
                    del protocol[field]
                    settings.write_text(json.dumps(protocol))
                    with self.assertRaises(ValueError):
                        summary.summarize_trial(root)
        trials = [{"trial": str(index), "instance": "original", "cache": "warm-filesystem",
                   "metrics": {"startup_ms": 10.0}} for index in range(3)]
        with self.assertRaises(ValueError):
            summary.aggregate(trials)

    def test_runtime_cohort_mismatch_refused_only_within_group(self):
        for field, value in (("javaVersion", "different Java"), ("candidateSha256", "b" * 64),
                             ("argv", ["/java", "-Xmx4G"]), ("classpathSha256", ["e" * 64])):
            with self.subTest(field=field):
                trials = [{"trial": str(index), "instance": "original", "cache": "warm-filesystem",
                           "workload_protocol": self.protocol(), "runtime_cohort": self.cohort(),
                           "metrics": {"startup_ms": 10.0}} for index in range(3)]
                trials[-1]["runtime_cohort"][field] = value
                with self.assertRaises(ValueError):
                    summary.aggregate(trials)
                trials[-1]["runtime_cohort"] = self.cohort()
                candidates = [{**trial, "instance": "ravel",
                               "runtime_cohort": {**self.cohort(), field: value}} for trial in trials]
                self.assertEqual(len(summary.aggregate(trials + candidates)), 2)

    def test_missing_actual_java_version_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_trial(root)
            path = root / "command.json"
            command = json.loads(path.read_text())
            del command["provenance"]["javaVersion"]
            path.write_text(json.dumps(command))
            with self.assertRaises(ValueError):
                summary.summarize_trial(root)

    def test_saving_gap_excluded_and_final_scene_endpoint_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_trial(root)
            result = summary.summarize_trial(root)
            self.assertEqual(result["steady_state_scope"]["window"], "measurement_final30s")
            with (root / "events.csv").open(newline="") as stream:
                events = list(csv.reader(stream))
            events = [row for row in events if row[0] != "shutdown_requested"]
            self.write_csv(root, "events.csv", events[0], events[1:])
            self.assertEqual(summary.summarize_trial(root)["metrics"]["cpu_percent.steady_last30s_mean"], 100)

    def make_trial(self, root: Path) -> None:
        (root / "complete").touch()
        (root / "gc.log").write_text("")
        (root / "game.log").write_text("[12:00:00] [Client/INFO]: Loading mods\n")
        result = {"complete": True, "trial": "fixture", "instance": "original",
                  "cache": "warm-filesystem", "clock_ticks_per_second": 100,
                  "start_monotonic_ns": 500_000_000}
        (root / "result.json").write_text(json.dumps(result))
        (root / "settings.json").write_text(json.dumps(self.protocol()))
        command = {"argv": ["/java", "-Xmx8G", "-javaagent:/observer.jar=" + str(root.resolve()), "Main"],
                   "instance": "original", "provenance": {"javaVersion": "fixture Java 8",
                   "classpath": [{"path": "/runtime.jar", "sha256": "f" * 64}]}}
        (root / "command.json").write_text(json.dumps(command))
        events = [["menu_ready", 1_000_000_000, "startup_ns=1000000000"],
                  ["world_ready", 2_000_000_000, "world_creation_ns=1000000000"]]
        for index, scene in enumerate(summary.SCENES):
            events += [["scene_start", (10 + index * 10) * 10**9, scene],
                       ["scene_end", (20 + index * 10) * 10**9, scene + ";duration_ns=10000000000"]]
        events.append(["shutdown_requested", 40_500_000_000, "done"])
        events.append(["benchmark_complete", 100_000_000_000, "done"])
        self.write_csv(root, "events.csv", ["event", "monotonic_ns", "detail"], events)
        for filename, field in (("frames.csv", "frame_ns"), ("ticks.csv", "tick_ns")):
            samples = [[scene, 15_000_000_000, duration] for scene in summary.SCENES
                       for duration in (10_000_000, 20_000_000)]
            self.write_csv(root, filename, ["scene", "monotonic_ns", field], samples)
        samples = [[second * 10**9, 42, 2048, 4096, second * 100] for second in range(101)]
        self.write_csv(root, "process.csv",
                       ["monotonic_ns", "pid", "rss_kib", "peak_rss_kib", "cpu_ticks"], samples)
        self.write_csv(root, "gpu.csv", ["monotonic_ns", "used_vram_mib", "gpu_percent"],
                       [[second * 10**9, 50, 10] for second in range(101)])
        self.write_csv(root, "jvm.csv",
                       ["monotonic_ns", "heap_used_bytes", "heap_committed_bytes", "loaded_classes"],
                       [[second * 10**9, 1048576, 2097152, 300] for second in range(101)])

    def write_csv(self, root, name, header, data):
        with (root / name).open("w", newline="") as stream:
            writer = csv.writer(stream)
            writer.writerow(header)
            writer.writerows(data)

    def test_publisher_overrides_reset_without_touching_worlds_or_mods(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            game = root / "game"
            for name in ("config", "scripts", "mods", "saves"):
                (game / name).mkdir(parents=True)
            (game / "config/generated.cfg").write_text("runtime mutation")
            (game / "scripts/pack.zs").write_text("runtime mutation")
            (game / "mods/pack.jar").write_bytes(b"verified original")
            (game / "saves/world.dat").write_bytes(b"preserved world")
            outside = root / "outside"
            outside.mkdir()
            (outside / "keep").write_text("untouched")
            (game / "resources").symlink_to(outside, target_is_directory=True)
            archive = root / "pack.zip"
            with zipfile.ZipFile(archive, "w") as source:
                source.writestr("overrides/config/pack.cfg", "publisher config")
                source.writestr("overrides/scripts/pack.zs", "publisher script")
                source.writestr("overrides/resources/pack.txt", "publisher resource")
                source.writestr("overrides/mods/pack.jar", b"not recopied")
                source.writestr("overrides/saves/world.dat", b"not recopied")
            prepare.reset_overrides(archive, game)
            self.assertFalse((game / "config/generated.cfg").exists())
            self.assertEqual((game / "config/pack.cfg").read_text(), "publisher config")
            self.assertEqual((game / "scripts/pack.zs").read_text(), "publisher script")
            self.assertFalse((game / "resources").is_symlink())
            self.assertEqual((outside / "keep").read_text(), "untouched")
            self.assertEqual((game / "mods/pack.jar").read_bytes(), b"verified original")
            self.assertEqual((game / "saves/world.dat").read_bytes(), b"preserved world")
            prepare.reset_overrides(archive, game)
            self.assertEqual((game / "scripts/pack.zs").read_text(), "publisher script")
            linked = root / "linked-game"
            linked.symlink_to(game, target_is_directory=True)
            with self.assertRaises(ValueError):
                prepare.reset_overrides(archive, linked)

    def test_shared_cache_links_become_private_copies(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prism, shared = root / "prism", root / "shared"
            prism.mkdir()
            for name in ("libraries", "assets", "meta"):
                (shared / name).mkdir(parents=True)
                (shared / name / "input").write_text("original")
                (prism / name).symlink_to(shared / name, target_is_directory=True)
            prepare.configure_caches(prism, shared)
            for name in ("libraries", "assets", "meta"):
                self.assertFalse((prism / name).is_symlink())
                (prism / name / "input").write_text("private")
                self.assertEqual((shared / name / "input").read_text(), "original")
            prepare.configure_caches(prism, shared)
            self.assertEqual((prism / "meta/input").read_text(), "private")

    def test_artifacts_immutable_but_metadata_refreshes(self):
        with tempfile.TemporaryDirectory() as directory:
            baseline = Path(directory)
            prism = baseline / "prism"
            for name in ("libraries", "assets", "meta"):
                (prism / name).mkdir(parents=True)
            artifact = prism / "libraries/mod.jar"
            artifact.write_bytes(b"pinned bytes")
            metadata = prism / "meta/index.json"
            metadata.write_text('{"url": "https://example.test/v1", "version": 1}')
            prepare.pin_downloads(baseline, prism, {})
            metadata.write_text('{"url": "https://example.test/v2", "version": 2}')
            prepare.pin_downloads(baseline, prism, {})
            pins = prepare.tomllib.loads((baseline / "inputs.toml").read_text())
            self.assertEqual(pins["metadata_sha256"]["prism/meta/index.json"], prepare.digest(metadata))
            self.assertEqual(pins["metadata_observed_urls"]["prism/meta/index.json"], ["https://example.test/v2"])
            artifact.write_bytes(b"changed bytes")
            with self.assertRaises(ValueError):
                prepare.pin_downloads(baseline, prism, {})

    def test_forge_extraction_directories_require_embedded_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            game = Path(directory)
            mods = game / "mods"
            repository = mods / "memory_repo"
            repository.mkdir(parents=True)
            nested = prepare.io.BytesIO()
            manifest = b"Manifest-Version: 1.0\r\nMaven-Artifact: test:nested:1\r\n"
            with zipfile.ZipFile(nested, "w") as child:
                child.writestr("META-INF/MANIFEST.MF", manifest)
            host = mods / "host.jar"
            with zipfile.ZipFile(host, "w") as archive:
                archive.writestr("META-INF/libraries/nested.jar", nested.getvalue())
            extracted = repository / "nested.jar"
            extracted.write_bytes(nested.getvalue())
            (repository / "nested.jar.meta").write_bytes(manifest)
            artifacts = {"host.jar": (host, prepare.digest(host))}
            prepare.verify_mods(game, artifacts)
            extracted.write_bytes(b"modified jar")
            with self.assertRaises(ValueError):
                prepare.verify_mods(game, artifacts)
            extracted.write_bytes(nested.getvalue())
            repository.rename(mods / "1.12.2")
            prepare.verify_mods(game, artifacts)
            (mods / "1.12.2/nested.jar").write_bytes(b"modified legacy jar")
            with self.assertRaises(ValueError):
                prepare.verify_mods(game, artifacts)

    def test_cache_copy_rejects_unexpected_file_without_mutating_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prism, shared = root / "prism", root / "shared"
            prism.mkdir()
            (shared / "libraries").mkdir(parents=True)
            (shared / "libraries/input").write_text("original")
            (prism / "libraries").write_text("unexpected")
            with self.assertRaises(ValueError):
                prepare.configure_caches(prism, shared)
            self.assertEqual((prism / "libraries").read_text(), "unexpected")
            self.assertEqual((shared / "libraries/input").read_text(), "original")

    def test_standard_options_disable_snooper(self):
        with tempfile.TemporaryDirectory() as directory:
            game = Path(directory)
            options = game / "options.txt"
            options.write_text("snooperEnabled:true\ncustomOption:keep\n")
            (game / ".benchmark-publisher-options.txt").write_text("publisherOption:keep\n")
            values = prepare.configure_options(game)
            self.assertEqual(values["snooperEnabled"], "false")
            self.assertIn("snooperEnabled:false\n", options.read_text())
            self.assertNotIn("customOption:keep\n", options.read_text())
            self.assertIn("publisherOption:keep\n", options.read_text())
            self.assertNotIn("snooperEnabled:true", options.read_text())

    def test_bench_python_syntax(self):
        for path in HERE.glob("*.py"):
            with self.subTest(file=path.name):
                compile(path.read_text(), path.name, "exec")


if __name__ == "__main__":
    unittest.main()
