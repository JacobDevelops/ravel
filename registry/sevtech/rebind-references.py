#!/usr/bin/env python3
"""Regenerate only WP0 references-byte binding, never legacy graph declarations."""
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True

SPEC = importlib.util.spec_from_file_location("verify_wp0", Path(__file__).with_name("verify-wp0.py"))
WP0 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WP0)


def rebind(root: Path) -> None:
    fixture = WP0.FIXTURE
    inventory = root / "registry/inventory" / fixture
    refs_path = inventory / "references.json"
    evidence_path = inventory / "graph-evidence.json"
    refs, graph, evidence, rights = [WP0.load(inventory / (name + ".json"))
                                    for name in ("references", "graph", "graph-evidence", "rights")]
    descriptor = WP0.load(root / "tests/fixtures/descriptors" / (fixture + ".json"))
    WP0.require(descriptor["id"] == fixture, "descriptor fixture identity differs")
    for name, data in (("references", refs), ("graph", graph), ("evidence", evidence), ("rights", rights)):
        WP0.require(data["fixture"]["id"] == fixture, name + " fixture identity differs")
    previous = evidence["fixture"]["references"]["sha256"]
    current = WP0.digest(refs_path)
    WP0.require(current == "eddb27d0cbcb6be2eb02a6895e960d8ad2af9d7e1394cbe379d594142b9be9f4",
                "references bytes differ from approved frozen input; rebinding refused")
    evidence["fixture"]["references"]["sha256"] = current
    # This producer regenerates an input binding only. The separate WP0 gate
    # still validates all graph declarations, classifications and components.
    WP0.validate_bindings(root, refs_path, refs, graph, evidence, rights, descriptor)
    original = evidence_path.read_bytes()
    old_field = ('"sha256": "' + previous + '"').encode()
    new_field = ('"sha256": "' + current + '"').encode()
    WP0.require(original.count(old_field) == 1, "references binding is not uniquely patchable")
    updated = original.replace(old_field, new_field, 1)
    WP0.require(json.loads(updated) == evidence, "rebinding would change another field")
    evidence_path.write_bytes(updated)
    print("Regenerated input binding fixture.references.sha256: " + previous + " -> " + current)
    print("NOT-YET-MET: reproducible legacy graph derivation (mcmod.info / @Mod extraction)")


if __name__ == "__main__":
    try:
        rebind(Path(__file__).resolve().parents[2])
    except (ValueError, KeyError, TypeError, OSError) as error:
        print("WP0 references rebinding failed: " + str(error), file=sys.stderr)
        sys.exit(1)
