#!/usr/bin/env python3
"""Summarize private trials without pooling distributions; reports are regenerable.

Frame stalls use scene-start-relative first-five-second and full-scene windows.
Chunk compile/upload timings are observational, never inferred from frame intervals.
Functional-only trials are retained as evidence but excluded from physical groups.
Default discovery skips matrix records and preflights, not incomplete trial evidence.
Older results infer their renderer from the effective launch command, defaulting to vanilla.
"""
import argparse
from collections import defaultdict
import csv
import json
import math
import os
from pathlib import Path
import re
import statistics
import sys
from typing import Any, Iterable, Sequence

SCENES = ("spawn_idle", "square_exploration", "loaded_chunk_stress")
LOCAL = Path(__file__).resolve().parents[3] / "tests/fixtures/local/sevtech-ages-3.2.3/baseline"


def require(condition: Any, message: str) -> None:
    if not condition:
        raise ValueError(message)


def quantile(values: Iterable[float], fraction: float) -> float:
    ordered = sorted(values)
    require(bool(ordered) and 0 < fraction <= 1, "quantile requires samples and 0 < fraction <= 1")
    require(all(math.isfinite(value) for value in ordered), "nonfinite sample")
    return ordered[math.ceil(len(ordered) * fraction) - 1]


def distribution(values: Sequence[float]) -> dict[str, float]:
    return {"p50": quantile(values, 0.50), "p95": quantile(values, 0.95),
            "p99": quantile(values, 0.99)}


def rows(directory: Path, name: str) -> list[dict[str, str]]:
    with (directory / name).open(newline="") as stream:
        result = list(csv.DictReader(stream))
    require(bool(result), "empty measurement: " + name)
    return result


def gc_pauses(text: str) -> list[float]:
    """Read root stop-the-world durations, not concurrent phases or Times details."""
    pauses, seen = [], set()
    starts = re.finditer(r"\[(?:Full GC\b|GC (?:pause|remark|cleanup)\b|GC\s*\()", text)
    for match in starts:
        depth, end = 0, None
        for index in range(match.start(), len(text)):
            if text[index] == "[":
                depth += 1
            elif text[index] == "]":
                depth -= 1
                if depth == 0:
                    end = index + 1
                    break
        require(end is not None, "truncated GC pause record")
        record = text[match.start():end]
        duration = re.search(r",\s*([0-9]+(?:\.[0-9]+)?)\s+secs\s*\]$", record)
        require(duration is not None, "GC pause lacks root duration")
        prefix = text[text.rfind("\n", 0, match.start()) + 1:match.start()]
        timestamp = re.search(r"([0-9]+\.[0-9]+):\s*$", prefix)
        key = (timestamp.group(1), record) if timestamp else (match.start(), record)
        if key not in seen:
            pauses.append(float(duration.group(1)) * 1000)
            seen.add(key)
    pattern = r"GC\((\d+)\)\s+(Pause [^\r\n]*?)\s+([0-9]+(?:\.[0-9]+)?)(ms|s)\s*$"
    for match in re.finditer(pattern, text, re.M):
        key = ("unified", *match.groups())
        if key not in seen:
            pauses.append(float(match.group(3)) * (1 if match.group(4) == "ms" else 1000))
            seen.add(key)
    return pauses


def event_metrics(events: list[dict[str, str]]) -> dict[str, float]:
    result = {}
    for event, field, metric in (("menu_ready", "startup_ns", "agent_startup_ms"),
                                  ("world_ready", "world_creation_ns", "world_creation_ms")):
        matches = [row for row in events if row["event"] == event]
        require(len(matches) == 1, "missing or duplicate event: " + event)
        detail = dict(part.split("=", 1) for part in matches[0]["detail"].split(";") if "=" in part)
        value = int(detail[field])
        require(value > 0, "invalid timing: " + event)
        result[metric] = value / 1e6
    for scene in SCENES:
        starts = [int(row["monotonic_ns"]) for row in events
                  if row["event"] == "scene_start" and row["detail"] == scene]
        ends = [int(row["monotonic_ns"]) for row in events
                if row["event"] == "scene_end" and row["detail"].split(";", 1)[0] == scene]
        require(len(starts) == len(ends) == 1 and ends[0] > starts[0], "incomplete scene: " + scene)
        result[scene + ".duration_s"] = (ends[0] - starts[0]) / 1e9
    return result


# Forge 1.12.2 lifecycle lines in their emission order. Each anchor is the first
# match after the previous observed anchor, so the second "Injecting itemstacks"
# (end of FMLInitialization) is never confused with the first (end of preinit).
STARTUP_ANCHORS = (
    ("fml_launch", r"Forge Mod Loader version \S+ for Minecraft \S+ loading"),
    ("display_created", r"LWJGL Version: "),
    ("mods_identified", r"Forge Mod Loader has identified \d+ mods? to load"),
    ("preinit_complete", r"Injecting itemstacks"),
    ("texture_atlas_created", r"Created: \d+x\d+ textures-atlas"),
    ("init_complete", r"Injecting itemstacks"),
    ("load_complete", r"Forge Mod Loader has successfully loaded \d+ mods?"),
)


def startup_phases(text: str) -> dict[str, Any]:
    """Retain ordered Forge lifecycle anchors; intervals are gaps between observed anchors."""
    markers, day, last, start = [], 0, None, 0
    lines = text.splitlines()
    for phase, pattern in STARTUP_ANCHORS:
        for number in range(start, len(lines)):
            line = lines[number]
            timestamp = re.match(r"^\[(?P<time>\d{2}:\d{2}:\d{2}(?:[.,]\d+)?)\]", line)
            if timestamp is None or re.search(pattern, line) is None:
                continue
            raw = timestamp.group("time")
            hour, minute, second = raw.replace(",", ".").split(":")
            seconds = int(hour) * 3600 + int(minute) * 60 + float(second)
            if last is not None and seconds < last - 43200:
                day += 86400
            last = seconds
            markers.append({"phase": phase, "timestamp": raw, "clock_seconds": day + seconds,
                            "source_line": number + 1, "source_text": line})
            start = number + 1
            break
    intervals = [{"from_phase": left["phase"], "to_phase": right["phase"],
                  "from_timestamp": left["timestamp"], "to_timestamp": right["timestamp"],
                  "from_source_line": left["source_line"], "to_source_line": right["source_line"],
                  "seconds": right["clock_seconds"] - left["clock_seconds"]}
                 for left, right in zip(markers, markers[1:])]
    return {"markers": markers, "observed_marker_intervals": intervals,
            "meaning": "Gaps between ordered Forge lifecycle log anchors at log-timestamp resolution; "
                       "absent anchors are not inferred."}


def timing_metrics(directory: Path, filename: str, field: str,
                   label: str) -> tuple[dict[str, float], dict[str, int]]:
    measurements = rows(directory, filename)
    metrics, counts = {}, {}
    for scene in SCENES:
        values = [int(row[field]) / 1e6 for row in measurements if row["scene"] == scene]
        require(bool(values) and min(values) > 0, "missing or invalid " + label + " samples: " + scene)
        counts[label + "." + scene] = len(values)
        for percentile, value in distribution(values).items():
            metrics[label + "." + scene + "." + percentile + "_ms"] = value
    return metrics, counts


def frame_stalls(directory: Path, events: list[dict[str, str]]) -> dict[str, float]:
    measurements = rows(directory, "frames.csv")
    metrics = {}
    for scene in SCENES:
        start = next(int(row["monotonic_ns"]) for row in events
                     if row["event"] == "scene_start" and row["detail"] == scene)
        end = next(int(row["monotonic_ns"]) for row in events
                   if row["event"] == "scene_end" and row["detail"].split(";", 1)[0] == scene)
        samples = [row for row in measurements if row["scene"] == scene]
        require(all(start <= int(row["monotonic_ns"]) <= end for row in samples),
                "frame sample outside scene: " + scene)
        early = [int(row["frame_ns"]) / 1e6 for row in samples
                 if int(row["monotonic_ns"]) <= start + 5_000_000_000]
        require(bool(early), "missing first-five-second frame samples: " + scene)
        metrics["frames." + scene + ".first5s_max_ms"] = max(early)
        metrics["frames." + scene + ".max_ms"] = max(int(row["frame_ns"]) / 1e6 for row in samples)
    return metrics


def chunk_samples(directory: Path, events: list[dict[str, str]], required: bool
                  ) -> list[dict[str, str]]:
    kinds = ("compile", "upload")
    if required:
        hooks = {row["detail"] for row in events if row["event"] == "renderer_probe_hook"}
        require(all(kind in hooks for kind in kinds), "missing renderer probe hooks")
    path = directory / "chunks.csv"
    require(not required or path.is_file(), "missing chunk samples")
    if not path.is_file():
        return []
    with path.open(newline="") as stream:
        measurements = list(csv.DictReader(stream))
    require(not required or bool(measurements), "missing chunk samples")
    for row in measurements:
        require(row["scene"] in (*SCENES, "startup", "menu_wait", "world_creation", "settle", "complete")
                and row["kind"] in kinds and
                int(row["monotonic_ns"]) > 0 and int(row["duration_ns"]) > 0,
                "invalid chunk sample")
    return measurements


def chunk_distribution(label: str, values: list[float]) -> dict[str, Any]:
    percentiles = distribution(values) if values else dict.fromkeys(("p50", "p95", "p99"))
    metrics = {label + "." + percentile + "_ms": value
               for percentile, value in percentiles.items()}
    metrics[label + ".max_ms"] = max(values) if values else None
    metrics[label + ".total_ms"] = sum(values) if values else None
    return metrics


def chunk_metrics(directory: Path, events: list[dict[str, str]], required: bool
                  ) -> tuple[dict[str, Any], dict[str, int]]:
    kinds = ("compile", "upload")
    measurements = chunk_samples(directory, events, required)
    if not measurements:
        return {}, {}
    measurements = [row for row in measurements if row["scene"] in SCENES]
    require(not required or all(any(row["kind"] == kind for row in measurements) for kind in kinds),
            "missing chunk compile/upload samples")
    metrics, counts = {}, {}
    for kind in kinds:
        for scene in SCENES:
            values = [int(row["duration_ns"]) / 1e6 for row in measurements
                      if row["scene"] == scene and row["kind"] == kind]
            label = "chunks." + kind + "." + scene
            counts[label] = len(values)
            metrics.update(chunk_distribution(label, values))
    return metrics, counts


def sampled_metrics(measurements: list[dict[str, str]], fields: dict[str, tuple[str, float]],
                    end: int) -> dict[str, float]:
    ordered = sorted(measurements, key=lambda row: int(row["monotonic_ns"]))
    ordered = [row for row in ordered if int(row["monotonic_ns"]) <= end]
    require(bool(ordered), "samples do not precede completion")
    cutoff = end - 30_000_000_000
    require(int(ordered[0]["monotonic_ns"]) <= cutoff and
            end - int(ordered[-1]["monotonic_ns"]) <= 3_000_000_000,
            "missing full last-30-second steady-state window")
    steady = [row for row in ordered if int(row["monotonic_ns"]) >= cutoff]
    require(len(steady) >= 2, "insufficient steady-state samples")
    metrics = {}
    for field, (label, scale) in fields.items():
        values = [float(row[field]) * scale for row in ordered]
        tail = [float(row[field]) * scale for row in steady]
        require(all(math.isfinite(v) and v >= 0 for v in values), "invalid sample: " + field)
        metrics[label + ".peak"] = max(values)
        metrics[label + ".steady_last30s_mean"] = statistics.fmean(tail)
    return metrics


def jvm_metrics(measurements: list[dict[str, str]], end: int) -> dict[str, Any]:
    metrics = sampled_metrics(measurements, {
        "heap_used_bytes": ("heap_used_mib", 1 / 1048576),
        "heap_committed_bytes": ("heap_committed_mib", 1 / 1048576),
        "loaded_classes": ("loaded_classes", 1),
    }, end)
    for field in ("nonheap_used_bytes", "direct_used_bytes", "mapped_used_bytes"):
        if field not in measurements[0]:
            continue
        label = field.replace("_bytes", "_mib")
        values = [int(row[field]) for row in measurements if int(row["monotonic_ns"]) <= end]
        require(all(value >= -1 for value in values), "invalid sample: " + field)
        if -1 in values:
            metrics[label + ".peak"] = None
            metrics[label + ".steady_last30s_mean"] = None
        else:
            metrics.update(sampled_metrics(measurements, {field: (label, 1 / 1048576)}, end))
    return metrics


def process_metrics(measurements: list[dict[str, str]], end: int,
                    ticks_per_second: int) -> dict[str, float]:
    metrics = sampled_metrics(measurements, {"rss_kib": ("rss_mib", 1 / 1024)}, end)
    ordered = sorted((row for row in measurements if int(row["monotonic_ns"]) <= end),
                     key=lambda row: int(row["monotonic_ns"]))
    metrics["rss_mib.peak"] = max(float(row["peak_rss_kib"]) / 1024 for row in ordered)
    require(len({row["pid"] for row in ordered}) == 1, "process identity changed within trial")
    require(ticks_per_second > 0, "invalid process clock tick frequency")
    elapsed = (int(ordered[-1]["monotonic_ns"]) - int(ordered[0]["monotonic_ns"])) / 1e9
    ticks = int(ordered[-1]["cpu_ticks"]) - int(ordered[0]["cpu_ticks"])
    require(elapsed > 0 and ticks >= 0, "invalid CPU counter interval")
    metrics["cpu_percent.sample_interval_mean"] = 100 * ticks / ticks_per_second / elapsed
    tail = [row for row in ordered if int(row["monotonic_ns"]) >= end - 30_000_000_000]
    elapsed = (int(tail[-1]["monotonic_ns"]) - int(tail[0]["monotonic_ns"])) / 1e9
    require(elapsed > 0, "insufficient CPU steady-state interval")
    ticks = int(tail[-1]["cpu_ticks"]) - int(tail[0]["cpu_ticks"])
    require(ticks >= 0, "CPU counter regressed")
    metrics["cpu_percent.steady_last30s_mean"] = 100 * ticks / ticks_per_second / elapsed
    return metrics


def workload_protocol(settings: dict[str, Any]) -> dict[str, Any]:
    require(isinstance(settings, dict), "missing workload provenance")
    agent = settings.get("agentSha256")
    require(isinstance(agent, str) and re.fullmatch(r"[0-9a-f]{64}", agent),
            "missing or invalid observer build provenance")
    options = settings.get("options")
    require(isinstance(options, dict) and bool(options) and
            all(isinstance(key, str) and isinstance(value, str) for key, value in options.items()),
            "missing or invalid effective options provenance")
    count = settings.get("artifactCount")
    fixture = settings.get("fixture")
    require(isinstance(fixture, str) and bool(fixture), "missing fixture provenance")
    require(type(count) is int and (count > 0 or count == 0 and fixture == "vanilla-1.12.2"),
            "missing or invalid artifact count provenance")
    probes = settings.get("rendererProbes", False)
    require(type(probes) is bool, "invalid renderer probe provenance")
    return {"agentSha256": agent, "options": dict(options), "artifactCount": count,
            "fixture": fixture, "rendererProbes": probes}


def classpath_hashes(provenance: dict[str, Any]) -> list[str]:
    hashes = []
    for entry in provenance.get("classpath", []):
        if isinstance(entry, dict):
            value = entry.get("sha256")
        else:
            matches = [record["sha256"] for key, record in provenance.get("inputs", {}).items()
                       if str(entry).endswith("/" + key)]
            require(len(matches) == 1, "missing unambiguous classpath build provenance")
            value = matches[0]
        require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value),
                "invalid classpath build provenance")
        hashes.append(value)
    require(bool(hashes), "missing runtime build provenance")
    return hashes


def runtime_inputs(inputs: Any, directory: Path) -> list[dict[str, str]]:
    require(isinstance(inputs, list) and bool(inputs), "missing runtime input provenance")
    records = []
    for entry in inputs:
        require(isinstance(entry, dict), "invalid runtime input provenance")
        path, value = entry.get("path"), entry.get("sha256")
        require(isinstance(path, str) and bool(path) and isinstance(value, str) and
                re.fullmatch(r"[0-9a-f]{64}", value), "invalid runtime input provenance")
        records.append({"path": path.replace(str(directory.resolve()), "<trial>"), "sha256": value})
    return records


def runtime_cohort(directory: Path) -> dict[str, Any]:
    command = json.loads((directory / "command.json").read_text())
    result = json.loads((directory / "result.json").read_text())
    provenance = command.get("provenance", {})
    version = provenance.get("javaVersion")
    require(isinstance(version, str) and bool(version), "missing actual Java version provenance")
    argv = command.get("argv")
    require(isinstance(argv, list) and argv and all(isinstance(value, str) for value in argv),
            "missing effective launch command")
    hashes = classpath_hashes(provenance)
    candidate = provenance.get("candidateSha256")
    require(command.get("instance") != "ravel" or
            isinstance(candidate, str) and re.fullmatch(r"[0-9a-f]{64}", candidate),
            "missing candidate build provenance")
    cohort = {"javaVersion": version, "candidateSha256": candidate, "classpathSha256": hashes,
              "audio_driver": result.get("audio_driver"),
              "argv": [value.replace(str(directory.resolve()), "<trial>") for value in argv]}
    if "runtimeInputs" in provenance:
        cohort["runtimeInputs"] = runtime_inputs(provenance["runtimeInputs"], directory)
    return cohort


def validate_ravel_runtime(directory: Path) -> None:
    """Require successful vanilla Ravel transformation, native hooks, and child-loader evidence."""
    text = (directory / "launcher.log").read_text()
    require(re.search(r"failed to transform|failed target:|no distribution entry point loaded|"
                      r"Ravel's prologue did not run", text, re.I) is None,
            "Ravel runtime reported a transformation or entry-point failure")
    patterns = {
        "counts": r"rewrote ([1-9]\d*) classes and ([1-9]\d*) methods; "
                  r"([1-9]\d*) hook invocations reached native code",
        "loader": r"rewrittenBy=dev\.ravel\.legacy\.launch\.LegacyLoader@[0-9a-f]+ "
                  r"system=\S+ sameObject=false parentChain=.+",
        "targets": r"rewritten targets: [^\s=,]+=[1-9]\d*(?:,[^\s=,]+=[1-9]\d*)*",
    }
    observed = defaultdict(int)
    for line in text.splitlines():
        if "[ravel]" not in line:
            continue
        message = line.split("[ravel]", 1)[1].strip()
        matches = [name for name, pattern in patterns.items() if re.fullmatch(pattern, message)]
        require(len(matches) == 1, "Ravel runtime diagnostic or invalid report: " + message)
        observed[matches[0]] += 1
    require(all(observed[name] == 1 for name in patterns),
            "missing or duplicate positive Ravel runtime report")


def trial_endpoint(events: list[dict[str, str]]) -> int:
    completion = [int(row["monotonic_ns"]) for row in events if row["event"] == "benchmark_complete"]
    require(len(completion) == 1, "missing or duplicate completion event")
    stops = [int(row["monotonic_ns"]) for row in events if row["event"] == "shutdown_requested"]
    if not stops:
        stops = [int(row["monotonic_ns"]) for row in events if row["event"] == "scene_end" and
                 row["detail"].split(";", 1)[0] == "loaded_chunk_stress"]
    require(len(stops) == 1 and stops[0] <= completion[0], "missing or invalid measurement endpoint")
    return stops[0]


def summarize_trial(directory: Path) -> dict[str, Any]:
    result = json.loads((directory / "result.json").read_text())
    require(result.get("complete") is True and (directory / "complete").is_file() and
            not (directory / "error.txt").exists(), "trial is incomplete")
    require(result["instance"] in ("original", "cleanroom", "ravel", "vanilla") and
            result["cache"] in ("cold-data-cache", "warm-filesystem"), "invalid trial grouping")
    measurement = result.get("measurement", "physical")
    require(measurement in ("physical", "functional-only"), "invalid measurement classification")
    protocol = workload_protocol(json.loads((directory / "settings.json").read_text()))
    if result["instance"] == "ravel" and protocol["fixture"] == "vanilla-1.12.2":
        validate_ravel_runtime(directory)
    events = rows(directory, "events.csv")
    end = trial_endpoint(events)
    metrics = event_metrics(events)
    menu = next(row for row in events if row["event"] == "menu_ready")
    wall_to_menu = int(menu["monotonic_ns"]) - int(result["start_monotonic_ns"])
    require(wall_to_menu > 0, "menu precedes launcher start")
    metrics["wall_to_menu_ms"] = wall_to_menu / 1e6
    counts = {}
    for filename, field, label in (("frames.csv", "frame_ns", "frames"), ("ticks.csv", "tick_ns", "ticks")):
        timings, samples = timing_metrics(directory, filename, field, label)
        metrics.update(timings)
        counts.update(samples)
    metrics.update(frame_stalls(directory, events))
    timings, samples = chunk_metrics(directory, events, protocol["rendererProbes"])
    metrics.update(timings)
    counts.update(samples)
    frequency = int(result.get("clock_ticks_per_second", os.sysconf("SC_CLK_TCK")))
    metrics.update(process_metrics(rows(directory, "process.csv"), end, frequency))
    metrics.update(sampled_metrics(rows(directory, "gpu.csv"),
                                   {"used_vram_mib": ("gpu_device_vram_mib", 1)}, end))
    metrics.update(jvm_metrics(rows(directory, "jvm.csv"), end))
    pauses = gc_pauses((directory / "gc.log").read_text())
    counts["gc_pauses"] = len(pauses)
    metrics["gc_pause_total_ms"] = sum(pauses)
    for percentile in ("p50", "p95", "p99"):
        metrics["gc_pause." + percentile + "_ms"] = distribution(pauses)[percentile] if pauses else None
    metrics["gc_pause_max_ms"] = max(pauses) if pauses else None
    cohort = runtime_cohort(directory)
    renderer = result.get("renderer")
    if "renderer" not in result:
        renderer = next((value.split("=", 1)[1] for value in reversed(cohort["argv"])
                         if value.startswith("-Dravel.renderer=")), "vanilla")
    return {"trial": result["trial"], "instance": result["instance"], "cache": result["cache"],
            "renderer": renderer, "measurement": measurement,
            "clock_ticks_per_second": frequency, "sample_counts": counts, "metrics": metrics,
            "workload_protocol": protocol,
            "runtime_cohort": cohort,
            "startup_phases": startup_phases((directory / "game.log").read_text()),
            "frame_stall_scope": {
                "window": "scene_start <= frame monotonic_ns <= scene_start + 5 seconds (first5s); full scene for max",
                "meaning": "Maximum observed frame interval, assigned by frame sample timestamp; first 5 seconds include the boundary."},
            "chunk_scope": {
                "rendererProbes": protocol["rendererProbes"],
                "window": "Measured scenes only; startup, menu_wait, world_creation, settle and complete are validated but excluded.",
                "meaning": "Chunk compile/upload durations are observational hook intervals, not GPU execution time or inferred frame stalls; absent samples are not zero."},
            "steady_state_scope": {"window": "measurement_final30s", "scene": "loaded_chunk_stress",
                                   "includes": "Final 30 seconds before shutdown_requested or loaded_chunk_stress scene_end; excludes saving"}}


def aggregate(trials: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups = defaultdict(list)
    protocol = None
    require(bool(trials), "no completed trials supplied")
    for trial in trials:
        measurement = trial.get("measurement", "physical")
        require(measurement in ("physical", "functional-only"), "invalid measurement classification")
        if measurement == "functional-only":
            continue
        current = workload_protocol(trial.get("workload_protocol"))
        require(protocol is None or current == protocol,
                "workload protocols differ across supplied trials")
        protocol = current
        groups[trial["instance"], trial["cache"], trial.get("renderer", "vanilla")].append(trial)
    output = []
    for (instance, cache, renderer), members in sorted(groups.items()):
        require(len(members) >= 3, "need at least three completed trials per instance/cache/renderer group")
        require(len({member["trial"] for member in members}) == len(members), "duplicate trial identity")
        cohort = members[0].get("runtime_cohort")
        require(isinstance(cohort, dict) and bool(cohort) and
                all(member.get("runtime_cohort") == cohort for member in members),
                "missing or differing runtime/build cohorts within instance/cache/renderer group")
        keys = set(members[0]["metrics"])
        require(all(set(member["metrics"]) == keys for member in members),
                "trial metric populations differ; cannot aggregate missing distributions")
        metrics = {}
        for key in sorted(keys):
            values = [member["metrics"][key] for member in members if member["metrics"][key] is not None]
            percentiles = distribution(values) if len(values) >= 3 else dict.fromkeys(("p50", "p95", "p99"))
            metrics[key] = {"trial_samples": len(values), **percentiles}
        output.append({"instance": instance, "cache": cache, "renderer": renderer,
                       "trial_count": len(members), "workload_protocol": protocol,
                       "runtime_cohort": cohort, "metrics": metrics})
    return output


def display(value: float | None) -> str:
    return "n/a" if value is None else format(value, ".4g")


def markdown(trials: list[dict[str, Any]], groups: list[dict[str, Any]]) -> str:
    lines = ["# Benchmark summary", "", "Nearest-rank quantiles; no interpolation or pooled trial samples.",
             "Aggregate p50/p95/p99 describe trial-level metrics, not a pooled frame/tick distribution.",
             "Metrics with fewer than three nonmissing trial values have no aggregate quantiles (n/a).",
             "Steady state is the final 30 seconds before shutdown_requested (or loaded_chunk_stress scene_end), excluding saving.",
             "Frame stalls disclose the first 5 seconds from scene_start (inclusive timestamp boundary) and full-scene maximum; intervals are assigned by frame sample timestamp.",
             "Chunk compile/upload metrics are observational hook durations, not GPU execution time; absent instrumentation is not a zero measurement.",
             "Functional-only trials are excluded from all physical comparison groups.",
             "CPU 100% equals one logical CPU; VRAM is whole-device usage, not per-process attribution.",
             "JVM nonheap and direct/mapped buffer usage are independently attributed counters, not total native memory or RSS-minus-heap; unavailable counters remain n/a.",
             "Cold-data-cache labels describe advisory file eviction, not a proven globally cold host.", ""]
    functional = [str(trial["trial"]) for trial in trials
                  if trial.get("measurement", "physical") == "functional-only"]
    if functional:
        lines += ["Functional-only evidence (not compared): " + ", ".join(functional), ""]
    for group in groups:
        lines += ["## " + group["instance"] + " / " + group["cache"] + " / " + group["renderer"], "",
                  "Completed trials: " + str(group["trial_count"]), "",
                  "| Metric | Trial samples | Trial-metric p50 | p95 | p99 |", "|---|---:|---:|---:|---:|"]
        for key, value in group["metrics"].items():
            cells = [str(value["trial_samples"])] + [display(value[p]) for p in ("p50", "p95", "p99")]
            lines.append("| " + key + " | " + " | ".join(cells) + " |")
        members = [trial for trial in trials if trial["instance"] == group["instance"] and
                   trial["cache"] == group["cache"] and
                   trial.get("renderer", "vanilla") == group["renderer"] and
                   trial.get("measurement", "physical") == "physical"]
        lines += ["", "### Per-trial values", "", "| Trial | Metric | Value |", "|---|---|---:|"]
        for trial in members:
            label = re.sub(r"[^A-Za-z0-9_.-]", "_", str(trial["trial"]))
            for key, value in sorted(trial["metrics"].items()):
                lines.append("| " + label + " | " + key + " | " + display(value) + " |")
        lines.append("")
        lines += ["### Observed startup marker gaps", "",
                  "These are source-log marker intervals, not inferred phase durations; exact source text is retained in JSON.",
                  "", "| Trial | From marker | To marker | Source timestamps | Seconds |",
                  "|---|---|---|---|---:|"]
        for trial in members:
            label = re.sub(r"[^A-Za-z0-9_.-]", "_", str(trial["trial"]))
            intervals = trial.get("startup_phases", {}).get("observed_marker_intervals", [])
            for interval in intervals:
                cells = [label, interval["from_phase"], interval["to_phase"],
                         interval["from_timestamp"] + " → " + interval["to_timestamp"],
                         display(interval["seconds"])]
                lines.append("| " + " | ".join(cells) + " |")
        lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trials", nargs="*", type=Path, help="completed trial directories; default: local runs")
    parser.add_argument("--output", type=Path, default=LOCAL / "report", help="private summary output directory")
    args = parser.parse_args()
    directories = args.trials or sorted(path for path in (LOCAL / "runs").iterdir()
                                       if not (path / "matrix.json").is_file() and
                                       not path.name.endswith("-preflight"))
    directories = [path for path in directories if path.is_dir()]
    require(bool(directories), "no trial directories supplied")
    trials = [summarize_trial(path) for path in directories]
    groups = aggregate(trials)
    text = markdown(trials, groups)
    args.output.mkdir(parents=True, exist_ok=True)
    for directory, trial in zip(directories, trials):
        (directory / "summary.json").write_text(json.dumps(trial, indent=2) + "\n")
    (args.output / "summary.json").write_text(json.dumps({"trials": trials, "groups": groups}, indent=2) + "\n")
    (args.output / "summary.md").write_text(text + "\n")
    print(text)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        sys.exit("benchmark summary refused: " + str(error))
