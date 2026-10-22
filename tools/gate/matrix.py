#!/usr/bin/env python3
"""Run version-isolated suites and publish only completed version verdicts."""

from contextlib import nullcontext
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
import os
from pathlib import Path
import re
import subprocess
from threading import Lock
import time

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "target/ravel-jvm"
GENERATION = Lock()


def test_version(version: str, eligibility: str, workers: int) -> dict[str, str | int | float]:
    started = time.monotonic()
    env = dict(os.environ, RAVEL_MINECRAFT_VERSION=version,
               CARGO_TARGET_DIR=str(ROOT / "target/matrix" / version))
    threads = str(max(1, len(os.sched_getaffinity(0)) // workers))
    env.setdefault("CARGO_BUILD_JOBS", threads)
    env.setdefault("NEXTEST_TEST_THREADS", threads)
    env["CARGO_TERM_COLOR"] = "never"
    log = OUTPUT / f"tests-{version}.log"
    record = OUTPUT / f"tests-{version}.json"
    record.unlink(missing_ok=True)
    commands = version_commands(eligibility)
    status = 0
    with log.open("w") as output:
        for command in commands:
            output.write("+ " + " ".join(command) + "\n")
            output.flush()
            lock = GENERATION if "ravel-xtask" in command and "--exclude" not in command else nullcontext()
            with lock:
                status = subprocess.run(command, env=env, stdout=output,
                                        stderr=subprocess.STDOUT).returncode
            if status:
                break
    counts = [int(count) for count in re.findall(r"Summary\s+\[[^\]]+\]\s+(\d+) tests? run:", log.read_text())]
    result = {"version": version, "exit": status, "tests": sum(counts),
              "seconds": round(time.monotonic() - started, 2), "log": str(log.relative_to(ROOT))}
    record.write_text(json.dumps(result) + "\n")
    print(f"Minecraft {version}: exit={status} tests={result['tests']} seconds={result['seconds']} log={result['log']}", flush=True)
    if status:
        print(log.read_text(), flush=True)
    return result


def version_commands(eligibility: str) -> list[list[str]]:
    if eligibility != "live":
        return [["cargo", "build", "--locked", "-p", "ravel-minecraft-adapter"]]
    return [
        ["cargo", "nextest", "run", "--locked", "--workspace", "--exclude", "ravel-xtask",
         "--exclude", "ravel-minecraft-generate", "--all-features"],
        ["cargo", "nextest", "run", "--locked", "-p", "ravel-composed-host", "--no-default-features"],
        ["cargo", "nextest", "run", "--locked", "-p", "ravel-xtask",
         "-p", "ravel-minecraft-generate", "--all-features"],
    ]


def main() -> None:
    os.chdir(ROOT)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    tested = OUTPUT / "tested-versions.txt"
    tested.write_text("")
    versions = []
    for directory in sorted(Path("minecraft/versions").iterdir()):
        if not directory.is_dir():
            continue
        version = directory.name
        eligibility = subprocess.check_output(
            ["cargo", "run", "--quiet", "--locked", "-p", "ravel-xtask", "--", "version-matrix", version],
            text=True).strip()
        if eligibility not in ("live", "generation-only"):
            verdict = f"SKIPPED: Minecraft {version} adapter tests were NOT run; {eligibility}"
            print(verdict, flush=True)
            with tested.open("a") as output:
                output.write(verdict + "\n")
            continue
        versions.append((version, eligibility))
    workers = max(1, len(versions))
    failed = False
    covered = False
    with ThreadPoolExecutor(max_workers=workers) as executor:
        futures = {executor.submit(test_version, version, eligibility, workers): (version, eligibility)
                   for version, eligibility in versions}
        for future in as_completed(futures):
            result = future.result()
            version, eligibility = futures[future]
            failed |= result["exit"] != 0
            if result["exit"]:
                continue
            live = eligibility == "live"
            covered |= live
            verdict = f"tested: Minecraft {version}" if live else f"built: Minecraft {version}; adapter tests not yet run"
            with tested.open("a") as output:
                output.write(verdict + "\n")
    if failed or not covered:
        raise SystemExit("version matrix failed or proved no live version")


if __name__ == "__main__":
    main()
