#!/usr/bin/env python3
"""Summarize the successful matrix and persisted llvm-cov report."""

import json
from pathlib import Path

root = Path(__file__).resolve().parents[2]
output = root / "target/ravel-jvm"
versions = [line.removeprefix("tested: Minecraft ") for line in
            (output / "tested-versions.txt").read_text().splitlines()
            if line.startswith("tested: Minecraft ")]
counts = []
for version in sorted(versions):
    result = json.loads((output / f"tests-{version}.json").read_text())
    if result["exit"] != 0:
        raise SystemExit(f"Minecraft {version} has no successful test result")
    counts.append(f"{version}:{result['tests']}")
coverage = json.loads((root / "target/gate/coverage.json").read_text())
percent = coverage["data"][0]["totals"]["lines"]["percent"]
print(f"ravel:verify tests=[{','.join(counts)}] coverage={percent:.2f}% exit=0")
