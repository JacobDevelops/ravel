#!/usr/bin/env python3
"""Routes every inventory entry to the host capability it demands.

ROADMAP.md M7-W1 says the routing count does not exist, and that until it does
"the 235 figure is an upper bound on packages rather than an estimate of work".
This computes it.

The question it answers is not "how much code does this entry have". It is
"which host surfaces would have to exist before this entry could run at all",
because a counterpart written against a capability the SDK does not have is a
library that cannot be exercised. Each entry's own surface record is the
evidence; nothing here reads a jar or a source tree.

What this is not: a judgement of how much of an entry Ravel's platform absorbs.
That needs a reader per entry, which is M7-W1's other half. This routes demand,
which is mechanical, and it says so in its output rather than implying more.
"""

import json
from typing import Any
import sys

from wave_common import FIXTURE, ROOT, write_json

# Which SDK module answers a demand, and whether that module can carry it into a
# running game today. The second column is the whole point: a module that only
# lets a mod *declare* something is not a module that makes it happen, and
# docs/guide/limits.md in the public repository is where each of these is
# stated to an author.
CAPABILITIES = {
    "content-behaviour": ("content", False),
    "recipes": ("no module", False),
    "loot": ("no module", False),
    "advancements": ("no module", False),
    "worldgen": ("world_shaping", False),
    "screens": ("no module", False),
    "tags": ("tags", False),
    "resources": ("resources", False),
    "shaders": ("render_stages", False),
    "audio": ("audio", False),
    "network": ("network", False),
    "persistence": ("save", False),
    "interception": ("interception", True),
    "services": ("services, provision", False),
    "config": ("config", False),
}


def demands(record: dict[str, Any]) -> set[str]:
    """The capabilities one entry needs, from its own surface record."""
    data = record.get("data", {})
    resources = record.get("resources", {})
    indicative = record.get("indicative", {})
    integration = record.get("integration", {})

    wanted = set()

    # Every catalogued mod ships code that runs against the game. Nothing in the
    # inventory distinguishes a mod whose classes only register data from one
    # whose classes implement a block, so this is charged to every entry that
    # has class files at all, and the limit is disclosed rather than guessed at.
    if indicative.get("classFiles", 0) > 0:
        wanted.add("content-behaviour")
        wanted.add("config")

    for counts, fields in (
        (data, {
            "recipes": "recipes", "lootTables": "loot", "advancements": "advancements",
            "worldgen": "worldgen", "tags": "tags",
        }),
        (resources, {"models": "resources", "textures": "resources", "shaders": "shaders", "sounds": "audio"}),
        (indicative, {"networkingClasses": "network", "persistenceClasses": "persistence"}),
        (integration, {"mixinConfigs": "interception", "accessTransformers": "interception", "serviceProviders": "services"}),
    ):
        wanted.update(capability for field, capability in fields.items() if counts.get(field, 0) > 0)

    # A client-side mod with models is drawing something, and nothing in the SDK
    # draws a screen. This is the closest the inventory gets to naming that.
    if record.get("side", {}).get("layout") == "client-leaning" and resources.get("models", 0) > 0:
        wanted.add("screens")

    return wanted


def main() -> int:
    surfaces = json.loads((ROOT / f"inventory/{FIXTURE}/surfaces.json").read_text())
    plans = json.loads((ROOT / f"inventory/{FIXTURE}/plans.json").read_text())

    by_artifact = {plan["artifact"]: plan for plan in plans["plans"]}

    entries = []
    demand_counts = {name: 0 for name in CAPABILITIES}

    for record in surfaces["artifacts"]:
        plan = by_artifact.get(record["artifact"])
        if plan is None:
            print(f"no plan for {record['artifact']}", file=sys.stderr)
            return 1

        wanted = demands(record)
        for name in wanted:
            demand_counts[name] += 1

        blocking = sorted(n for n in wanted if not CAPABILITIES[n][1])
        entries.append(
            {
                "canonicalId": plan["canonicalId"],
                "method": plan["method"]["name"],
                "sourceUse": plan["sourceUse"],
                "demands": sorted(wanted),
                "blockedOn": blocking,
                "runnableToday": not blocking,
            }
        )

    entries.sort(key=lambda entry: entry["canonicalId"])
    runnable = sum(1 for entry in entries if entry["runnableToday"])

    out = {
        "schemaVersion": 1,
        "inventoryKind": FIXTURE,
        "derivation": "computed by waves/route.py from surfaces.json and plans.json",
        "totals": {
            "entries": len(entries),
            "runnableToday": runnable,
            "blocked": len(entries) - runnable,
            "demandByCapability": dict(
                sorted(demand_counts.items(), key=lambda item: -item[1])
            ),
        },
        "capabilities": {
            name: {"module": module, "reachesARunningGame": reaches}
            for name, (module, reaches) in CAPABILITIES.items()
        },
        "limits": [
            "It routes demand and does not measure absorption: how much of an entry"
            " Ravel's platform already answers needs a reader per entry, which this"
            " does not attempt.",
            "Content behaviour is charged to every entry carrying class files,"
            " because the inventory does not distinguish a class that registers data"
            " from one that implements a block. That makes the content-behaviour"
            " figure an upper bound.",
            "content-behaviour has a module and still reads False. A mod can now"
    " declare a block or an item, describe how it behaves, and have the host"
    " register it in a launched game — but a counterpart needs blocks that do"
    " something, and a block whose only behaviour is hardness, blast"
    " resistance, tool requirement and random ticking is not that. The flag"
    " flips when the surface carries behaviour a mod writes rather than values"
    " it picks.",
    "A capability's reachesARunningGame verdict is written here by hand from"
            " what docs/guide/limits.md states in the public repository. It is a"
            " claim about the host at the time of writing and goes stale when the"
            " host gains a surface.",
            "It reads the inventory's own counts and opens no artifact.",
        ],
        "entries": entries,
    }

    path = ROOT / "waves/routing.json"
    write_json(path, out)

    print(f"routed {len(entries)} entries; {runnable} could run today, {len(entries) - runnable} could not")
    for name, count in out["totals"]["demandByCapability"].items():
        module, reaches = CAPABILITIES[name]
        mark = "reaches a running game" if reaches else "declaration only"
        print(f"  {count:>3} entries need {name:<18} ({module}, {mark})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
