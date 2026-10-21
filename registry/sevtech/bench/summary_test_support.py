"""Private synthetic trial fixtures for offline summary tests."""
import csv
import json
from pathlib import Path
from typing import Any, Iterable

import summarize as summary


class SummaryFixture:
    def protocol(self) -> dict[str, Any]:
        return {"agentSha256": "a" * 64, "fixture": "sevtech-ages-3.2.3",
                "artifactCount": 259, "rendererProbes": False,
                "options": {"renderDistance": "12", "fullscreen": "false"}}

    def cohort(self) -> dict[str, Any]:
        return {"javaVersion": "fixture Java 8", "candidateSha256": None,
                "classpathSha256": ["f" * 64], "audio_driver": None,
                "argv": ["/java", "-Xmx8G", "-javaagent:/observer.jar=<trial>", "Main"]}

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
            samples = [[scene, (11 + index * 10) * 10**9, duration]
                       for index, scene in enumerate(summary.SCENES)
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

    def write_csv(self, root: Path, name: str, header: list[str],
                  data: Iterable[Iterable[str | int]]) -> None:
        with (root / name).open("w", newline="") as stream:
            writer = csv.writer(stream)
            writer.writerow(header)
            writer.writerows(data)
