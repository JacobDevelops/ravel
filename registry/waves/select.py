#!/usr/bin/env python3
"""Selects the M6 foundation wave and writes waves/m6-foundation.json.

This is the producer. It is not the checker: checks/verify-wave.sh re-reads the
inventory and holds the written manifest to it without running any of this code,
so a rule that is wrong here is a rule the checker never learns.

Every admission below is a stated predicate, and the manifest records which
predicate admitted each entry. The selection frame itself -- excluding
unresolved and prohibited source use, requiring dependency closure, splitting
the wave into a foundation set and one gameplay cluster -- is M6-W1's brief and
is not rederived here.
"""

import hashlib
import json
import os
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURE = os.environ.get("RAVEL_FIXTURE_ID", "atm10-tts-2.0.2")
INV = ROOT / "inventory" / FIXTURE
OUT = ROOT / "waves" / "m6-foundation.json"

ADMISSIBLE_SOURCE_USE = ("permitted", "conditional")
GAMEPLAY_OWNERS = ("client-presentation", "content-data", "integration-runtime")
SEED_OWNER = "sdk-platform"
SEED_MIN_FOUNDATION_VALUE = 3


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def closure(seeds, deps):
    seen, stack = set(), list(seeds)
    while stack:
        node = stack.pop()
        if node in seen:
            continue
        seen.add(node)
        stack.extend(deps[node])
    return seen


def main():
    plans_path, rights_path = INV / "plans.json", INV / "rights.json"
    plans = json.loads(plans_path.read_text())
    rights = json.loads(rights_path.read_text())

    by_id = {p["canonicalId"]: p for p in plans["plans"]}
    deps = {k: list(v["dependencies"]["entries"]) for k, v in by_id.items()}
    admissible = {
        k for k, v in by_id.items() if v["sourceUse"] in ADMISSIBLE_SOURCE_USE
    }

    # Rule F1. A foundation seed is an sdk-platform entry that at least two other
    # entries depend on, and whose rights position is settled. The lane alone is
    # not enough: its owner rule also admits any code-only layout, so most of the
    # lane has no dependent at all and is a leaf wearing a library's name.
    seeds = sorted(
        k
        for k in admissible
        if by_id[k]["owner"] == SEED_OWNER
        and by_id[k]["rubric"]["foundationValue"] >= SEED_MIN_FOUNDATION_VALUE
    )
    # Rule F2. A wave is dependency-closed, so anything a seed needs joins it.
    foundation = closure(seeds, deps)

    # Rule C1. A cluster candidate is a pair of admissible gameplay-lane entries
    # that share a dependency, taken with its closure, with every member
    # admissible. Two entries hanging off one library is the smallest shape that
    # is a cluster rather than a leaf and its prerequisite.
    candidates = []
    gameplay = sorted(k for k in admissible if by_id[k]["owner"] in GAMEPLAY_OWNERS)
    for i, a in enumerate(gameplay):
        for b in gameplay[i + 1 :]:
            if not set(deps[a]) & set(deps[b]):
                continue
            members = closure({a, b}, deps)
            if not members <= admissible:
                continue
            candidates.append((frozenset(members), (a, b)))

    # Rule C2. Order candidates by cost, then by what they exercise, then by
    # risk, then by id. Smallest first: entries the foundation set already pays
    # for are free, so cost is the count of members outside it. Ties break to
    # the candidate spanning the most owner lanes, because a cluster that
    # crosses server data, client presentation and runtime tests more of the
    # authoring process than one that does not; then to the lowest summed
    # riskScore over the new entries; then to the lowest leaf ids, which is
    # arbitrary and is there only so the answer is single-valued.
    def order(entry):
        members, leaves = entry
        new = sorted(members - foundation)
        return (
            len(new),
            -len({by_id[m]["owner"] for m in members}),
            sum(by_id[m]["riskScore"] for m in new),
            leaves,
        )

    if not candidates:
        print("no admissible gameplay cluster", file=sys.stderr)
        return 1
    cluster_members, cluster_leaves = min(candidates, key=order)

    def admissions(cid):
        out = []
        if cid in foundation:
            if cid in seeds:
                out.append({"part": "foundation", "rule": "foundation-seed"})
            else:
                out.append(
                    {
                        "part": "foundation",
                        "rule": "foundation-closure",
                        "requiredBy": sorted(
                            k for k in foundation if cid in deps[k]
                        ),
                    }
                )
        if cid in cluster_members:
            if cid in cluster_leaves:
                out.append({"part": "cluster", "rule": "cluster-leaf"})
            else:
                out.append(
                    {
                        "part": "cluster",
                        "rule": "cluster-closure",
                        "requiredBy": sorted(
                            k for k in cluster_members if cid in deps[k]
                        ),
                    }
                )
        return out

    wave = sorted(foundation | cluster_members)
    manifest = {
        "schemaVersion": 1,
        "manifestKind": "foundation-wave",
        "wave": "m6-foundation",
        "fixture": plans["fixture"],
        "inputs": {
            "plans": {"path": f"inventory/{FIXTURE}/plans.json", "sha256": digest(plans_path)},
            "rights": {"path": f"inventory/{FIXTURE}/rights.json", "sha256": digest(rights_path)},
        },
        "producedBy": "waves/select.py",
        "verifiedBy": "checks/verify-wave.sh",
        "rules": {
            "R0-rights": (
                "An entry whose sourceUse is prohibited or unresolved is excluded"
                " outright. A counterpart whose rights position is unresolved is"
                " not foundation work."
            ),
            "R1-closure": (
                "Each part is dependency-closed: if an entry is in a part, every"
                " id in its dependencies is in that part."
            ),
            "F1-seed": (
                "A foundation seed is an admissible entry owned by sdk-platform"
                " whose rubric foundationValue is 3 or more, which is two or more"
                " dependent entries. This rule is exhaustive: every entry in the"
                " inventory satisfying it is in the foundation part."
            ),
            "F2-closure": "Anything a foundation seed depends on joins the foundation part.",
            "C1-candidate": (
                "A cluster candidate is a pair of admissible entries in the"
                " client-presentation, content-data or integration-runtime lanes"
                " that share a dependency, closed over dependencies, with every"
                " member admissible."
            ),
            "C2-choice": (
                "Among candidates, fewest members outside the foundation set"
                " wins; then most distinct owner lanes spanned; then lowest"
                " summed riskScore over the new members; then lowest leaf ids."
            ),
        },
        "arithmetic": {
            "plans": len(by_id),
            "excludedByRights": len(by_id) - len(admissible),
            "admissible": len(admissible),
            "sdkPlatformLane": sum(
                1 for v in by_id.values() if v["owner"] == SEED_OWNER
            ),
            "sdkPlatformAdmissible": sum(
                1 for k in admissible if by_id[k]["owner"] == SEED_OWNER
            ),
            "foundationSeeds": len(seeds),
            "foundation": len(foundation),
            "clusterCandidates": len({c[0] for c in candidates}),
            "cluster": len(cluster_members),
            "clusterOutsideFoundation": len(cluster_members - foundation),
            "wave": len(wave),
        },
        "scenario": {
            "part": "cluster",
            "leaves": sorted(cluster_leaves),
            "prerequisites": sorted(cluster_members - set(cluster_leaves)),
            "shape": (
                "Two gameplay entries in different owner lanes over a shared"
                " runtime entry and a shared library, so M6-W4's fresh play,"
                " save, exit and reload run exercises server-owned content and"
                " client presentation against one prerequisite pair rather than"
                " against a single leaf."
            ),
        },
        "entries": [
            {
                "canonicalId": cid,
                "owner": by_id[cid]["owner"],
                "sourceUse": by_id[cid]["sourceUse"],
                "method": by_id[cid]["method"]["name"],
                "rank": by_id[cid]["rank"],
                "riskScore": by_id[cid]["riskScore"],
                "foundationValue": by_id[cid]["rubric"]["foundationValue"],
                "dependencies": sorted(deps[cid]),
                "tests": by_id[cid]["tests"],
                "admission": admissions(cid),
            }
            for cid in wave
        ],
    }

    # rights.json is read so the producer cannot claim a rights position the
    # triage does not hold; the checker repeats this join from the files.
    triage = {a["artifact"]: a["sourceUse"] for a in rights["artifacts"]}
    for cid in wave:
        if triage[by_id[cid]["artifact"]] != by_id[cid]["sourceUse"]:
            print(f"plans and rights disagree on {cid}", file=sys.stderr)
            return 1

    OUT.parent.mkdir(exist_ok=True)
    OUT.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
