#!/usr/bin/env python3
"""Summarize private trials without pooling distributions; reports are regenerable."""
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


def process_metrics(measurements: list[dict[str, str]], end: int,
                    ticks_per_second: int) -> dict[str, float]:
    metrics = sampled_metrics(measurements, {"rss_kib": ("rss_mib", 1 / 1024)}, end)
    metrics["rss_mib.peak"] = max(float(row["peak_rss_kib"]) / 1024 for row in measurements)
    ordered = sorted((row for row in measurements if int(row["monotonic_ns"]) <= end),
                     key=lambda row: int(row["monotonic_ns"]))
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
    require(type(count) is int and count > 0, "missing or invalid artifact count provenance")
    fixture = settings.get("fixture")
    require(isinstance(fixture, str) and bool(fixture), "missing fixture provenance")
    return {"agentSha256": agent, "options": dict(options), "artifactCount": count,
            "fixture": fixture}


def runtime_cohort(directory: Path) -> dict[str, Any]:
    command = json.loads((directory / "command.json").read_text())
    provenance = command.get("provenance", {})
    version = provenance.get("javaVersion")
    require(isinstance(version, str) and bool(version), "missing actual Java version provenance")
    argv = command.get("argv")
    require(isinstance(argv, list) and argv and all(isinstance(value, str) for value in argv),
            "missing effective launch command")
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
    candidate = provenance.get("candidateSha256")
    require(command.get("instance") != "ravel" or
            isinstance(candidate, str) and re.fullmatch(r"[0-9a-f]{64}", candidate),
            "missing candidate build provenance")
    return {"javaVersion": version, "candidateSha256": candidate, "classpathSha256": hashes,
            "argv": [value.replace(str(directory.resolve()), "<trial>") for value in argv]}


def summarize_trial(directory: Path) -> dict[str, Any]:
    result = json.loads((directory / "result.json").read_text())
    require(result.get("complete") is True and (directory / "complete").is_file() and
            not (directory / "error.txt").exists(), "trial is incomplete")
    require(result["instance"] in ("original", "cleanroom", "ravel") and
            result["cache"] in ("cold-data-cache", "warm-filesystem"), "invalid trial grouping")
    protocol = workload_protocol(json.loads((directory / "settings.json").read_text()))
    events = rows(directory, "events.csv")
    completion = [int(row["monotonic_ns"]) for row in events if row["event"] == "benchmark_complete"]
    require(len(completion) == 1, "missing or duplicate completion event")
    stops = [int(row["monotonic_ns"]) for row in events if row["event"] == "shutdown_requested"]
    if not stops:
        stops = [int(row["monotonic_ns"]) for row in events if row["event"] == "scene_end" and
                 row["detail"].split(";", 1)[0] == "loaded_chunk_stress"]
    require(len(stops) == 1 and stops[0] <= completion[0], "missing or invalid measurement endpoint")
    end = stops[0]
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
    frequency = int(result.get("clock_ticks_per_second", os.sysconf("SC_CLK_TCK")))
    metrics.update(process_metrics(rows(directory, "process.csv"), end, frequency))
    metrics.update(sampled_metrics(rows(directory, "gpu.csv"),
                                   {"used_vram_mib": ("gpu_device_vram_mib", 1)}, end))
    metrics.update(sampled_metrics(rows(directory, "jvm.csv"), {
        "heap_used_bytes": ("heap_used_mib", 1 / 1048576),
        "heap_committed_bytes": ("heap_committed_mib", 1 / 1048576),
        "loaded_classes": ("loaded_classes", 1),
    }, end))
    pauses = gc_pauses((directory / "gc.log").read_text())
    counts["gc_pauses"] = len(pauses)
    metrics["gc_pause_total_ms"] = sum(pauses)
    for percentile in ("p50", "p95", "p99"):
        metrics["gc_pause." + percentile + "_ms"] = distribution(pauses)[percentile] if pauses else None
    metrics["gc_pause_max_ms"] = max(pauses) if pauses else None
    return {"trial": result["trial"], "instance": result["instance"], "cache": result["cache"],
            "clock_ticks_per_second": frequency, "sample_counts": counts, "metrics": metrics,
            "workload_protocol": protocol,
            "runtime_cohort": runtime_cohort(directory),
            "startup_phases": startup_phases((directory / "game.log").read_text()),
            "steady_state_scope": {"window": "measurement_final30s", "scene": "loaded_chunk_stress",
                                   "includes": "Final 30 seconds before shutdown_requested or loaded_chunk_stress scene_end; excludes saving"}}


def aggregate(trials: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups = defaultdict(list)
    protocol = None
    for trial in trials:
        current = workload_protocol(trial.get("workload_protocol"))
        require(protocol is None or current == protocol,
                "workload protocols differ across supplied trials")
        protocol = current
        groups[trial["instance"], trial["cache"]].append(trial)
    require(bool(groups), "no completed trials supplied")
    output = []
    for (instance, cache), members in sorted(groups.items()):
        require(len(members) >= 3, "need at least three completed trials per instance/cache group")
        require(len({member["trial"] for member in members}) == len(members), "duplicate trial identity")
        cohort = members[0].get("runtime_cohort")
        require(isinstance(cohort, dict) and bool(cohort) and
                all(member.get("runtime_cohort") == cohort for member in members),
                "missing or differing runtime/build cohorts within instance/cache group")
        keys = set(members[0]["metrics"])
        require(all(set(member["metrics"]) == keys for member in members),
                "trial metric populations differ; cannot aggregate missing distributions")
        metrics = {}
        for key in sorted(keys):
            values = [member["metrics"][key] for member in members if member["metrics"][key] is not None]
            percentiles = distribution(values) if len(values) >= 3 else dict.fromkeys(("p50", "p95", "p99"))
            metrics[key] = {"trial_samples": len(values), **percentiles}
        output.append({"instance": instance, "cache": cache, "trial_count": len(members),
                       "workload_protocol": protocol, "runtime_cohort": cohort, "metrics": metrics})
    return output


def display(value: float | None) -> str:
    return "n/a" if value is None else format(value, ".4g")


def markdown(trials: list[dict[str, Any]], groups: list[dict[str, Any]]) -> str:
    lines = ["# Benchmark summary", "", "Nearest-rank quantiles; no interpolation or pooled trial samples.",
             "Aggregate p50/p95/p99 describe trial-level metrics, not a pooled frame/tick distribution.",
             "Metrics with fewer than three nonmissing trial values have no aggregate quantiles (n/a).",
             "Steady state is the final 30 seconds before shutdown_requested (or loaded_chunk_stress scene_end), excluding saving.",
             "CPU 100% equals one logical CPU; VRAM is whole-device usage, not per-process attribution.",
             "Cold-data-cache labels describe advisory file eviction, not a proven globally cold host.", ""]
    for group in groups:
        lines += ["## " + group["instance"] + " / " + group["cache"], "",
                  "Completed trials: " + str(group["trial_count"]), "",
                  "| Metric | Trial samples | Trial-metric p50 | p95 | p99 |", "|---|---:|---:|---:|---:|"]
        for key, value in group["metrics"].items():
            cells = [str(value["trial_samples"])] + [display(value[p]) for p in ("p50", "p95", "p99")]
            lines.append("| " + key + " | " + " | ".join(cells) + " |")
        members = [trial for trial in trials if trial["instance"] == group["instance"] and trial["cache"] == group["cache"]]
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
    directories = args.trials or sorted((LOCAL / "runs").iterdir())
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
