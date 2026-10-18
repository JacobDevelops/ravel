#!/usr/bin/env python3
"""Validate frozen WP0 declarations, not a rerun of the legacy extractor."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import zipfile

FIXTURE = "sevtech-ages-3.2.3"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load(path):
    return json.loads(path.read_text())


def unique(rows, key, label):
    result = {row[key]: row for row in rows}
    require(len(result) == len(rows), "duplicate " + label)
    return result


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(root, fixture, mode):
    require(fixture == FIXTURE, "WP0 phase requires fixture " + FIXTURE)
    require(mode in ("--offline", "--fixture"), "invalid WP0 mode")
    inventory = root / "registry/inventory" / fixture
    refs_path = inventory / "references.json"
    refs, graph, evidence, rights = [load(inventory / (name + ".json"))
                                    for name in ("references", "graph", "graph-evidence", "rights")]
    descriptor = load(root / "tests/fixtures/descriptors" / (fixture + ".json"))
    require(descriptor["id"] == fixture, "descriptor fixture identity differs")
    for name, data in (("references", refs), ("graph", graph), ("evidence", evidence), ("rights", rights)):
        require(data["fixture"]["id"] == fixture, name + " fixture identity differs")
    inputs = validate_bindings(root, refs_path, refs, graph, evidence, rights, descriptor)
    top = validate_artifacts(root, fixture, mode, inputs, evidence, refs["acquiredArtifacts"])
    nodes = validate_nodes(graph, evidence, top)
    validate_declarations(graph, evidence, top)
    adjacency, blocked = validate_edges(graph, evidence, top, nodes)
    validate_components(graph, evidence, nodes, adjacency, blocked)
    validate_frozen_pins(root, inventory, graph, evidence)


def validate_frozen_pins(root, inventory, graph, evidence):
    pins = load(root / "registry/sevtech/evidence-pins.json")
    require(pins["fixture"] == FIXTURE, "frozen evidence pin fixture differs")
    require(pins["edges"] == len(graph["edges"]) == len(evidence["edges"]) == 743,
            "frozen declaration edge count differs")
    require(pins["excludedDeclarations"] == len(evidence["excludedDeclarations"]) == 3,
            "frozen excluded declaration count differs")
    require(set(pins["sha256"]) == {"references", "graph", "graph-evidence"}, "frozen evidence pin population differs")
    for name, expected in pins["sha256"].items():
        require(digest(inventory / (name + ".json")) == expected, "frozen evidence bytes differ: " + name)


def validate_bindings(root, refs_path, refs, graph, evidence, rights, descriptor):
    for field, pin in (("archive", descriptor["source"]), ("manifest", descriptor["manifest"])):
        require(refs["fixture"][field]["sha256"] == pin["sha256"], field + " descriptor hash differs")
    require(refs["fixture"]["minecraft"] == descriptor["manifest"]["minecraft"], "Minecraft pin differs")
    require(refs["fixture"]["modLoader"] == descriptor["manifest"]["modLoader"], "loader pin differs")
    references = refs["references"]
    acquired, bundled = refs["acquiredArtifacts"], refs["bundledArtifacts"]
    require((len(references), len(acquired), len(bundled)) == (254, 254, 5), "WP0 reference/bundled counts differ")
    require(refs["totals"] == {"references": 254, "bundledJars": 5, "jarInputs": 259,
                            "fetchedReferences": 254, "unfetchedReferences": 0,
                            "fetchedBundledJars": 5, "loadedContainers": None}, "reference totals/runtime-container boundary differs")
    require(refs["fixture"]["expectedReferenceCount"] == descriptor["manifest"]["expectedReferenceCount"] == 254,
            "pinned reference denominator differs")
    require(graph["fixture"]["pinnedReferences"] == evidence["fixture"]["pinnedReferences"] == 254
            and evidence["fixture"]["bundledJars"] == 5, "graph input denominator differs")
    require([row["ordinal"] for row in references] == list(range(254)), "reference ordinals differ")
    pairs = lambda rows: Counter((row["projectId"], row["fileId"]) for row in rows)
    require(pairs(references) == pairs(acquired) and len(pairs(references)) == 254, "reference/acquisition identities differ")
    inputs = unique(acquired + bundled, "artifact", "input artifact")
    for name, row in inputs.items():
        require(Path(name).name == name and not any(c in name for c in "\t\r\n"), "invalid artifact name")
        require(row["status"] == "fetched" and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]), "invalid acquired artifact hash/status: " + name)
    rows = sorted("mod\t" + name + "\t" + row["sha256"] + "\n" for name, row in inputs.items())
    artifact_sha = hashlib.sha256("".join(rows).encode()).hexdigest()
    for name, data in (("graph", graph), ("evidence", evidence), ("rights", rights)):
        binding = data["fixture"]["artifactSet"]
        require(binding["count"] == 259 and binding["sha256"] == artifact_sha, name + " artifact-set binding differs")
    require(evidence["fixture"]["references"]["path"] == str(refs_path.relative_to(root)), "evidence references path differs")
    require(evidence["fixture"]["references"]["sha256"] == digest(refs_path), "evidence references hash differs")
    rights_by_name = unique(rights["artifacts"], "artifact", "rights artifact")
    require(set(rights_by_name) == set(inputs), "rights/input artifact identities differ")
    for name, row in rights_by_name.items():
        if "projectId" in inputs[name]:
            require(row["reference"] == {key: inputs[name][key] for key in ("projectId", "fileId")}, "rights reference identity differs: " + name)
    return inputs


def validate_artifacts(root, fixture, mode, inputs, evidence, acquired):
    top = unique([row for row in evidence["artifacts"] if not row["nested"]], "artifact", "evidence artifact")
    nested = [row for row in evidence["artifacts"] if row["nested"]]
    require(set(top) == set(inputs), "graph evidence/input artifact identities differ")
    require(len(nested) == 4 and len({(row["artifact"], row["nestedEntry"]) for row in nested}) == 4, "nested artifact count/identity differs")
    for name, row in top.items():
        require(row["sha256"] == inputs[name]["sha256"], "graph evidence artifact hash differs: " + name)
        if name in {item["artifact"] for item in acquired}:
            require((row["projectId"], row["fileId"]) == (inputs[name]["projectId"], inputs[name]["fileId"]), "graph evidence reference identity differs: " + name)
    for row in nested:
        require(row["artifact"] in inputs and not row["declaredModIds"], "nested identity differs")
        require(re.fullmatch(r"[0-9a-f]{64}", row["sha256"]), "invalid nested hash")
        if mode == "--fixture":
            raw = inputs[row["artifact"]]["path"].replace("${FIXTURE_ID}", fixture)
            path = (root / raw).resolve()
            require(path.is_relative_to((root / "tests/fixtures/local" / fixture).resolve()), "nested host escapes fixture")
            with zipfile.ZipFile(path) as jar:
                require(hashlib.sha256(jar.read(row["nestedEntry"])).hexdigest() == row["sha256"], "nested artifact hash differs")
    return top


def validate_nodes(graph, evidence, top):
    nodes = unique(graph["nodes"], "id", "graph node")
    node_evidence = unique(evidence["nodes"], "id", "evidence node")
    require(len(nodes) == 273 and nodes.keys() == node_evidence.keys(), "declared node identities differ")
    expected_nodes = set()
    for name, row in top.items():
        expected_nodes.update(row["declaredModIds"] or ["artifact:" + name])
    require(set(nodes) == expected_nodes, "artifact declarations/node identities differ")
    for identity, node in nodes.items():
        require(node["artifact"] in top, "node artifact missing")
        declared = top[node["artifact"]]["declaredModIds"]
        require((node["kind"] == "mod" and identity in declared) or
                (node["kind"] == "opaque-artifact" and not declared and identity == "artifact:" + node["artifact"]), "node kind/identity differs")
        sources = node_evidence[identity]["sources"]
        require(bool(sources), "node declaration source missing")
        require(node["artifact"] in {source["artifact"] for source in sources}, "node primary artifact source missing")
        for source in sources:
            require(source["artifact"] in top and bool(source["location"]), "invalid node source")
            require(identity in top[source["artifact"]]["declaredModIds"] or identity == "artifact:" + source["artifact"], "node source identity differs")
    return nodes


def validate_declarations(graph, evidence, top):
    require(len(graph["edges"]) == len(evidence["edges"]), "edge evidence count differs")
    declarations = evidence["edges"] + evidence["excludedDeclarations"]
    for declaration in evidence["excludedDeclarations"]:
        require(declaration["to"] == "*" and declaration["type"] == "optional"
                and declaration["reason"] == "wildcard-ordering-not-a-dependency"
                and declaration["ordering"] in {"BEFORE", "AFTER"}, "invalid wildcard exclusion")
    declaration_counts = Counter()
    for declaration in declarations:
        source = declaration["source"]
        require(source["artifact"] in top and declaration["from"] in top[source["artifact"]]["declaredModIds"]
                and bool(source["location"]) and bool(declaration["rawDeclaration"]), "invalid declaration owner/source")
        require(declaration["ordering"] in {"BEFORE", "AFTER", "NONE"}, "invalid declaration ordering")
        require(declaration["semantics"] in {"required-presence", "ordering-only-if-present"}, "invalid declaration semantics")
        declaration_counts[source["artifact"]] += 1
    for name, row in top.items():
        count = declaration_counts[name]
        require(row["declaredDependencyCount"] == count, "artifact declaration count differs: " + name)
        require(row["dependencyStatus"] == ("declared-dependencies" if count else "no-declared-dependencies"), "artifact declaration status differs: " + name)
    require(evidence["totals"]["excludedDeclarations"] == len(evidence["excludedDeclarations"]), "excluded declaration total differs")


def validate_edges(graph, evidence, top, nodes):
    platforms = {"minecraft", "forge", "fml", "java", "neoforge"}
    adjacency = {identity: set() for identity in nodes}
    blocked = set()
    for edge, declaration in zip(graph["edges"], evidence["edges"]):
        require(all(edge[key] == declaration[key] for key in ("from", "to", "type", "versionRange", "side")), "edge evidence alignment differs")
        require(edge["from"] in nodes and edge["type"] in {"required", "optional"} and edge["side"] in {"BOTH", "CLIENT", "SERVER"}, "invalid edge vocabulary/owner")
        source = declaration["source"]
        require(source["artifact"] in top and edge["from"] in top[source["artifact"]]["declaredModIds"] and bool(source["location"]) and bool(declaration["rawDeclaration"]), "edge declaration source differs")
        platform = edge["to"].lower() in platforms
        if platform:
            require(declaration["platformId"] == edge["to"].lower(), "canonical platform identity differs")
        resolution = "platform" if platform else "internal" if edge["to"] in nodes else "absent"
        classification = "blocked:required-dependency-absent-from-pack" if resolution == "absent" and edge["type"] == "required" else "resolved"
        require(edge["resolution"] == resolution and edge["classification"] == classification, "edge resolution/classification differs")
        if classification != "resolved":
            blocked.add(edge["from"])
        if resolution == "internal" and edge["type"] == "required":
            adjacency[edge["from"]].add(edge["to"])
            adjacency[edge["to"]].add(edge["from"])
    return adjacency, blocked


def validate_components(graph, evidence, nodes, adjacency, blocked):
    components, unseen = [], set(nodes)
    while unseen:
        pending, component = [unseen.pop()], set()
        while pending:
            identity = pending.pop()
            if identity not in component:
                component.add(identity)
                pending.extend(adjacency[identity] - component)
        unseen -= component
        components.append(frozenset(component))
    clusters = graph["clusters"]
    unique(clusters, "id", "cluster")
    require(Counter(frozenset(row["members"]) for row in clusters) == Counter(components), "cluster partition/connectivity differs")
    require(all(row["size"] == len(row["members"]) == len(set(row["members"])) for row in clusters), "cluster size differs")
    for identity, node in nodes.items():
        classification = "blocked:no-declared-dependency-metadata" if node["kind"] == "opaque-artifact" else "blocked:required-dependency-absent-from-pack" if identity in blocked else "resolved"
        require(node["classification"] == classification, "node classification differs")
    totals = {"nodes": len(nodes), "edges": len(graph["edges"]), "clusters": len(clusters),
              "largestCluster": max(map(len, components)), "blockedEdges": sum(edge["classification"] != "resolved" for edge in graph["edges"]),
              "blockedNodes": sum(node["classification"] != "resolved" for node in nodes.values())}
    require(graph["totals"] == totals, "graph totals differ")
    for key, value in totals.items():
        require(evidence["totals"][key] == value, "graph evidence total differs: " + key)
    for key, value in (("inputJars", 259), ("nestedJars", 4), ("requiredEdges", sum(edge["type"] == "required" for edge in graph["edges"])),
                       ("optionalEdges", sum(edge["type"] == "optional" for edge in graph["edges"])), ("platformEdges", sum(edge["resolution"] == "platform" for edge in graph["edges"]))):
        require(evidence["totals"][key] == value, "graph evidence total differs: " + key)


def public_collision_decisions(root):
    decisions = {}
    selected = set()
    for relative in ("registry/checks/public-word-collisions.tsv",
                     "registry/sevtech/public-word-collisions.tsv"):
        for line in (root / relative).read_text().splitlines():
            fields = line.split("\t", 1)
            require(len(fields) == 2 and bool(fields[0]) and bool(fields[1].strip()), "invalid public collision decision")
            identity = fields[0].lower()
            require(identity not in decisions, "duplicate public collision decision: " + identity)
            decisions[identity] = fields[1]
            if relative.startswith("registry/sevtech/"):
                selected.add(identity)
    return decisions, selected


def validate_public_boundary(root, identities, tracked_paths=None):
    """Apply historical whole-word/path refusal to the selected declared ids."""
    if tracked_paths is None:
        command = (["jj", "--no-pager", "file", "list", "-T", 'path ++ "\\0"']
                   if (root / ".jj").is_dir() else ["git", "ls-files", "-z"])
        result = subprocess.run(command, cwd=root, capture_output=True, check=True, timeout=60)
        tracked_paths = result.stdout.decode().split("\0")
    public = [path for path in tracked_paths if path and not path.startswith("registry/")]
    require(bool(public) and bool(identities), "public boundary has no paths or graph identities")
    decisions, selected = public_collision_decisions(root)
    patterns = {identity.lower(): re.compile(
        r"(?<![A-Za-z0-9_])" + re.escape(identity) + r"(?![A-Za-z0-9_])", re.IGNORECASE | re.ASCII)
                for identity in identities}
    observed = set()
    for raw in public:
        path = root / raw
        content = path.read_bytes().decode("latin1") if path.is_file() else ""
        for identity, pattern in patterns.items():
            if pattern.search(raw) or pattern.search(content):
                observed.add(identity)
    require(selected <= patterns.keys(), "selected collision decision is not a graph identity: "
            + ", ".join(sorted(selected - patterns.keys())))
    require(selected <= observed, "selected collision decision has no public occurrence: "
            + ", ".join(sorted(selected - observed)))
    require(not observed - decisions.keys(),
            "inventory identifier outside registry without recorded collision decision: "
            + ", ".join(sorted(observed - decisions.keys())))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("offline", "fixture"))
    parser.add_argument("--fixture-id", default=FIXTURE)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[2]
        validate(root, args.fixture_id, "--" + args.mode)
        graph = load(root / "registry/inventory" / args.fixture_id / "graph.json")
        validate_public_boundary(root, [node["id"] for node in graph["nodes"]])
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print("WP0 validation failed: " + str(error), file=sys.stderr)
        return 1
    print("WP0 declaration evidence bound: 254 references + 5 bundled = 259 input jars; 4 nested jars; 273 declared identities (not runtime containers)")
    print("WP0 offline structural evidence only; acquisition was NOT rerun" if args.mode == "offline" else "WP0 local nested artifact bytes verified; acquisition/rights checks are separate required gate steps")
    for obligation in ("reproducible legacy graph derivation (mcmod.info / @Mod extraction)",
                       "surfaces inventory", "implementation plans", "observed reads reconciliation"):
        print("NOT-YET-MET: " + obligation)
    return 0


if __name__ == "__main__":
    sys.exit(main())
