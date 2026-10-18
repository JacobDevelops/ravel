#!/usr/bin/env python3
"""Offline static WP0 rights gate: vocabulary, policy, derived statuses and totals."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

SURFACES = ("code", "textures", "models", "sounds", "names", "branding", "behaviour")
SOURCE_USE = {"cleared": "permitted", "permission-required": "conditional",
              "prohibited": "prohibited", "unresolved": "unresolved"}
RANK = {"not-applicable": 0, "cleared": 1, "permission-required": 2, "unresolved": 3, "prohibited": 4}


def refuse(message):
    raise SystemExit("static rights refused: " + message)


def table(path, width):
    rows = []
    for number, line in enumerate(path.read_text().splitlines(), 1):
        fields = line.split("\t")
        if len(fields) != width:
            refuse(f"{path.name}:{number} has {len(fields)} fields, expected {width}")
        rows.append(fields)
    return rows


def policies(root, fixture_id):
    combined = {}
    paths = (root / "registry/checks/licence-families.tsv",
             root / "registry/inventory" / fixture_id / "licence-families.tsv")
    for path in paths:
        if not path.is_file():
            continue
        for declared, family, reserved in table(path, 3):
            if reserved not in ("true", "false"):
                refuse("reservation is not true/false: " + declared)
            if declared in combined:
                refuse("licence declaration has more than one policy: " + (declared or "(none declared)"))
            combined[declared] = (family, reserved)
    return combined


def load_vocabulary(root):
    schema = json.loads((root / "schemas/provenance.schema.json").read_text())
    item = schema["properties"]["rights"]["items"]["properties"]
    statuses, surfaces = set(item["status"]["enum"]), set(item["surface"]["enum"])
    if not set(SURFACES) <= surfaces:
        refuse("public schema lacks a required surface")
    family_status = {}
    for family, status, reason in table(root / "registry/checks/licence-status.tsv", 3):
        if family in family_status or status not in statuses or not reason.strip():
            refuse("licence-status row is duplicate, unknown or unexplained: " + family)
        family_status[family] = status
    return statuses, family_status


def load_inputs(root, fixture_id):
    inventory = root / "registry/inventory" / fixture_id
    ledger = json.loads((inventory / "references.json").read_text())
    rights = json.loads((inventory / "rights.json").read_text())
    if ledger["fixture"]["id"] != fixture_id or rights["fixture"]["id"] != fixture_id:
        refuse("fixture identity differs")
    if rights.get("inventoryKind") != "rights-triage":
        refuse("rights inventory kind differs")
    inputs = {}
    for record in ledger["acquiredArtifacts"] + ledger["bundledArtifacts"]:
        if record["artifact"] in inputs:
            refuse("duplicate ledger artifact: " + record["artifact"])
        inputs[record["artifact"]] = record
    rows = "".join(sorted("mod\t" + n + "\t" + r["sha256"] + "\n" for n, r in inputs.items()))
    binding = rights["fixture"]["artifactSet"]
    expected = {**binding, "count": len(inputs), "sha256": hashlib.sha256(rows.encode()).hexdigest()}
    if binding != expected:
        refuse("rights artifact-set binding differs from the ledger")
    return inputs, rights


def validate_records(rights, inputs, statuses, family_status, policy):
    names = [row["artifact"] for row in rights["artifacts"]]
    if len(names) != len(set(names)) or set(names) != set(inputs):
        refuse("rights records do not cover each ledger artifact exactly once")
    for row in rights["artifacts"]:
        validate_record(row, inputs[row["artifact"]], statuses, family_status, policy)


def validate_record(row, record, statuses, family_status, policy):
    name = row["artifact"]
    bundled = "archiveEntry" in record
    reference = None if bundled else {"projectId": record["projectId"], "fileId": record["fileId"]}
    acquisition = "bundled-override" if bundled else "automated-curseforge"
    if row["class"] != "mod" or row["reference"] != reference or row["acquisition"] != acquisition:
        refuse("class, reference or acquisition differs from the ledger: " + name)
    surface_rows = row["rights"]
    got = {r["surface"]: r["status"] for r in surface_rows}
    if len(surface_rows) != len(SURFACES) or set(got) != set(SURFACES):
        refuse("record does not cover every surface exactly once: " + name)
    if not set(got.values()) <= statuses:
        refuse("status outside the public vocabulary: " + name)
    licence = row["licence"]
    reserved_value = licence["assetsReservedSeparately"]
    if not isinstance(reserved_value, bool):
        refuse("reservation is not boolean: " + name)
    reserved = "true" if reserved_value else "false"
    if policy.get(licence["declared"]) != (licence["family"], reserved):
        refuse("licence tuple has no recorded policy: " + name)
    code = family_status.get(licence["family"])
    if code is None or got["code"] != code:
        refuse("code status differs from the family decision: " + name)
    validate_derived(row, got, code, reserved, name)


def validate_derived(row, got, code, reserved, name):
    asset = "prohibited" if reserved == "true" else code
    for surface in ("textures", "models", "sounds"):
        if got[surface] not in ("not-applicable", asset):
            refuse(f"{surface} status differs from code/reservation: {name}")
    if got["names"] != "unresolved" or got["behaviour"] != "cleared":
        refuse("inherited names/behaviour rule differs: " + name)
    if got["branding"] not in ("prohibited", "not-applicable"):
        refuse("branding status differs from the logo rule: " + name)
    if row["sourceUse"] != SOURCE_USE[code]:
        refuse("sourceUse differs from code status: " + name)
    worst = max(got.values(), key=RANK.__getitem__)
    publication = "cleared" if RANK[worst] <= RANK["cleared"] else "blocked:" + worst
    if row["publication"] != publication:
        refuse("publication differs from the worst surface: " + name)


def validate_totals(rights):
    records = rights["artifacts"]
    source_use = dict(sorted(Counter(r["sourceUse"] for r in records).items()))
    totals = {
        "acquisition": dict(sorted(Counter(r["acquisition"] for r in records).items())),
        "artifacts": len(records),
        "licenceFamilies": dict(sorted(Counter(r["licence"]["family"] for r in records).items())),
        "publication": dict(sorted(Counter(r["publication"] for r in records).items())),
        "publicationCleared": sum(r["publication"] == "cleared" for r in records),
        "sourceUse": source_use,
        "sourceUseProhibited": source_use.get("prohibited", 0),
        "sourceUseUnresolved": source_use.get("unresolved", 0),
    }
    if rights["totals"] != totals:
        refuse("totals differ from the records")


def main():
    root, fixture_id = Path(sys.argv[1]).resolve(), sys.argv[2]
    statuses, family_status = load_vocabulary(root)
    policy = policies(root, fixture_id)
    inputs, rights = load_inputs(root, fixture_id)
    validate_records(rights, inputs, statuses, family_status, policy)
    validate_totals(rights)
    print(f"WP0 static rights: {len(rights['artifacts'])} records match vocabulary, policy, derived statuses and totals; "
          "asset counts and grant bytes are --fixture only")


if __name__ == "__main__":
    main()
