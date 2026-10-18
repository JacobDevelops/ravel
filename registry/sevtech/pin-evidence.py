#!/usr/bin/env python3
"""Record frozen WP0 evidence bytes; this does not derive legacy graph declarations."""
import hashlib
import json
from pathlib import Path
import sys

REFERENCES_SHA256 = "eddb27d0cbcb6be2eb02a6895e960d8ad2af9d7e1394cbe379d594142b9be9f4"
FIXTURE = "sevtech-ages-3.2.3"


def produce(root):
    inventory = root / "registry/inventory" / FIXTURE
    documents = {name: (inventory / (name + ".json")).read_bytes()
                 for name in ("references", "graph", "graph-evidence")}
    if hashlib.sha256(documents["references"]).hexdigest() != REFERENCES_SHA256:
        raise ValueError("references bytes differ from approved frozen input")
    decoded = {name: json.loads(data) for name, data in documents.items()}
    if any(data["fixture"]["id"] != FIXTURE for data in decoded.values()):
        raise ValueError("frozen evidence fixture identity differs")
    graph, evidence = decoded["graph"], decoded["graph-evidence"]
    if (len(graph["edges"]), len(evidence["edges"]), len(evidence["excludedDeclarations"])) != (743, 743, 3):
        raise ValueError("frozen declaration edge/exclusion counts differ")
    pins = {"fixture": FIXTURE, "edges": 743, "excludedDeclarations": 3,
            "sha256": {name: hashlib.sha256(data).hexdigest() for name, data in documents.items()}}
    path = root / "registry/sevtech/evidence-pins.json"
    if path.exists() and json.loads(path.read_text()) != pins:
        raise ValueError("existing frozen evidence pins differ; refusing to bless changed evidence")
    path.write_text(json.dumps(pins, indent=2) + "\n")
    print("Recorded frozen WP0 input hashes and declaration counts: " + str(path.relative_to(root)))
    print("NOT-YET-MET: reproducible legacy graph derivation (mcmod.info / @Mod extraction)")


if __name__ == "__main__":
    try:
        produce(Path(__file__).resolve().parents[2])
    except (ValueError, KeyError, TypeError, OSError) as error:
        print("WP0 evidence pinning failed: " + str(error), file=sys.stderr)
        sys.exit(1)
