#!/usr/bin/env python3
"""Bind cited grants to acquired bytes without replacing host declarations."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys
from urllib.parse import quote
from typing import Any, NoReturn
import zipfile


def refuse(message: str) -> NoReturn:
    raise SystemExit("rights evidence refused: " + message)


def digest(value: object, length: int = 64) -> re.Match[str] | bool | None:
    return isinstance(value, str) and re.fullmatch("[0-9a-f]{" + str(length) + "}", value)


def cached_bytes(cache: Path, sha256: object) -> bytes:
    if not isinstance(sha256, str) or not digest(sha256):
        refuse("missing cached response SHA256")
    path = cache / sha256
    if not path.is_file():
        refuse("missing byte-bound grant cache: " + str(path))
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != sha256:
        refuse("cached grant SHA256 differs: " + str(path))
    return data


def exact_file_grant(cache: Path, artifact: bytes, citation: dict[str, Any], declared: str) -> dict[str, Any]:
    sha1 = hashlib.sha1(artifact).hexdigest()
    receipt_path = cache / ("modrinth-" + sha1 + ".json")
    if not receipt_path.is_file():
        refuse("missing new exact-file observation: " + str(receipt_path))
    receipt = json.loads(receipt_path.read_text())
    if not (
        re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?(?:Z|\+00:00)", receipt.get("fetchedAt", ""))
        and receipt.get("endpoint") == citation["api"]
        and receipt.get("hashAlgorithm") == "sha1"
        and receipt.get("artifactHash") == sha1
    ):
        refuse("exact-file observation request identity differs")
    versions = json.loads(cached_bytes(cache, receipt.get("versionResponseSha256")))
    version = versions.get(sha1)
    if not version or not any(
        file.get("hashes", {}).get("sha1") == sha1
        and file.get("hashes", {}).get("sha512") == hashlib.sha512(artifact).hexdigest()
        for file in version.get("files", [])
    ):
        refuse("exact-file response does not identify acquired jar bytes")
    project = json.loads(cached_bytes(cache, receipt.get("projectResponseSha256")))
    if not (
        receipt.get("projectEndpoint") in ("https://api.modrinth.com/v2/project/" + str(version.get("project_id")), "https://api.modrinth.com/v2/project/" + citation["project"])
        and version.get("project_id") == project.get("id")
        and citation["project"] in (project.get("id"), project.get("slug"))
        and citation["licenseId"] == project.get("license", {}).get("id")
    ):
        refuse("exact-file project or licence declaration differs")
    expected_declaration = {"LicenseRef-All-Rights-Reserved": "All Rights Reserved"}.get(citation["licenseId"], citation["licenseId"])
    if declared != expected_declaration:
        refuse("exact-file licence does not authorize the recorded declaration")
    return receipt


def grant_text_matches(text: str, declared: str) -> bool:
    text = " ".join(re.sub(r"[^a-z0-9]+", " ", text.lower().replace("licence", "license")).split())
    normal = " ".join(re.sub(r"[^a-z0-9]+", " ", declared.lower().replace("licence", "license")).split())
    restriction_text = re.sub(r"\bcommercial or non ?commercial\b", "commercial", text)
    assets = r"\b(?:assets?|textures?|models?|sounds?|art|artwork|resources?|images?)\b"
    restrictions = (
        (r"\b(?:noncommercial|non commercial) (?:use|purposes)\b|\b(?:not for|no) commercial\b|\bcommercial (?:use )?(?:is )?(?:prohibited|forbidden|not permitted)\b", r"\b(?:nc|noncommercial|non commercial)\b"),
        (r"\bcommons clause\b", r"\bcommons clause\b"),
        (rf"(?:{assets}.{{0,80}}(?:all rights reserved|reserved)|(?:all rights reserved|reserved).{{0,80}}{assets})", rf"(?:{assets}.*reserved|reserved.*{assets})"),
    )
    if any(re.search(term, restriction_text) and not re.search(named, normal) for term, named in restrictions):
        return False
    if declared.startswith("MIT"):
        return "permission is hereby granted free of charge" in text and "without restriction" in text
    if declared.startswith("BSD-3"):
        return "redistribution and use in source and binary forms" in text and "neither the name" in text
    if declared.startswith("Apache-2"):
        return "apache license" in text and "version 2 0" in text and "grant of copyright license" in text
    if declared.startswith("MPL-2"):
        return "mozilla public license" in text and "2 0" in text
    if declared == "Unlicense":
        return "free and unencumbered software released into the public domain" in text
    if declared.startswith(("CC-", "CC ", "CC0")):
        compact = text.replace(" ", "")
        version = re.search(r"\d+\.\d+", declared)
        if not version or version[0].replace(".", "") not in compact:
            return False
        for code, term in (("NC", "noncommercial"), ("SA", "sharealike"), ("ND", "noderivatives")):
            if term in compact and code not in declared:
                return False
        terms = ["attribution"] if "BY" in declared else ["publicdomain"]
        if "NC" in declared:
            terms.append("noncommercial")
        if "SA" in declared:
            terms.append("sharealike")
        if "ND" in declared:
            terms.append("noderivatives")
        return all(term in compact for term in terms)
    if declared.startswith(("GPL", "LGPL")):
        lesser = declared.startswith("LGPL")
        version = re.search(r"\d+(?:\.\d+)?", declared)
        title = re.search(r"gnu (?:lesser )?general public license", text)
        expected_title = "gnu lesser general public license" if lesser else "gnu general public license"
        if not version or not title or title[0] != expected_title:
            return False
        number = version[0].removesuffix(".0").replace(".", " ")
        return "version " + number in text[title.end():title.end() + 100]
    if normal.startswith("all rights reserved"):
        return "all rights reserved" in text
    if declared.endswith("Terms and Conditions"):
        return set(normal.split()) <= set(text.split())
    return " " + normal + " " in " " + text + " "


def observed_host(archive: zipfile.ZipFile, host: dict[str, Any], name: str, cache: Path,
                  project_id: str | None = None) -> tuple[list[str], bool]:
    observations = {e["source"].split("!", 1)[1]: e for e in host["evidence"] if e["kind"] == "jar-entry"}
    required = {p for p in ("mcmod.info", "META-INF/MANIFEST.MF") if p in archive.namelist()}
    if not required <= observations.keys():
        refuse("incomplete host metadata observation: " + name)
    declarations, sources, logos = set(), [], False
    for member in required:
        text = archive.read(member).decode("utf-8", errors="replace")
        sources.append(text)
        if member == "mcmod.info":
            try:
                # Legacy Forge metadata permits raw line breaks inside strings.
                metadata = json.loads(text, strict=False)
            except json.JSONDecodeError:
                refuse("cannot independently parse host metadata: " + name)
            mods = metadata if isinstance(metadata, list) else metadata.get("modList", [metadata])
            for mod in mods:
                for key in ("license", "licence"):
                    if mod.get(key):
                        declarations.add(str(mod[key]).strip())
                logos |= bool(mod.get("logoFile"))
        else:
            unfolded = text.replace("\r\n", "\n").replace("\n ", "")
            for line in unfolded.splitlines():
                key, separator, value = line.partition(":")
                if separator and key.lower() in ("license", "licence", "bundle-license") and value.strip():
                    declarations.add(value.strip())
    legal = []
    for member, entry in observations.items():
        if entry.get("role") == "host-licence":
            legal.append(archive.read(member).decode("utf-8", errors="replace"))
    for entry in host["evidence"]:
        if entry["kind"] == "project-api":
            match = re.fullmatch(r"https://api\.cfwidget\.com/([0-9]+)", entry["source"])
            if not match:
                refuse("unknown project observation endpoint: " + name)
            if project_id is not None and (project_id == "0" or match[1] != project_id):
                refuse("project observation does not belong to acquired artifact: " + name)
            data = (cache.parent / ("project-" + match[1] + ".json")).read_bytes()
            if hashlib.sha256(data).hexdigest() != entry["sha256"]:
                refuse("project observation SHA256 differs: " + name)
            project = json.loads(data)
            if project.get("license", "") != entry.get("observed", ""):
                refuse("project licence observation differs: " + name)
            sources.append(data.decode("utf-8"))
    declared = host["declared"]
    if declarations and declarations != {declared}:
        refuse("host declaration differs from jar metadata: " + name)
    if declared and not declarations:
        text = "\n".join(legal).lower()
        normal = re.sub(r"[^a-z0-9]+", " ", declared.lower().replace("licence", "license")).strip()
        normalized_text = re.sub(r"\bversion\b", "", re.sub(r"[^a-z0-9]+", " ", text.replace("licence", "license")))
        normalized_text = " ".join(normalized_text.split())
        matching = normal and " " + normal + " " in " " + normalized_text + " "
        if not matching:
            if declared.startswith(("LGPL-", "GPL-")):
                matching = grant_text_matches(text, declared)
            elif declared == "Galacticraft custom licence: non-commercial use only":
                matching = "galacticraft" in text and ("non-commercial" in text or "non commercial" in text)
            elif declared.startswith("Custom ") and "copyright" in declared.lower() and "redistribution" in declared.lower():
                matching = "copyright" in text and "redistribution" in text
        if not matching:
            refuse("host licence assertion lacks independently observed declaration: " + name)
    if not declared and (declarations or legal):
        refuse("host absence assertion omits a licence observation: " + name)
    return sources, logos


LICENCE_STATUS = {
    row[0]: row[1] for row in (
        line.split("\t") for line in
        (Path(__file__).resolve().parents[1] / "checks/licence-status.tsv").read_text().splitlines()
    )
}


def code_status(family: str) -> str:
    if family not in LICENCE_STATUS:
        refuse("unrecognised licence status policy: " + family)
    return LICENCE_STATUS[family]


def verify_recorded_rights(path: Path, grants: list[str], acquisition: list[str],
                          evidence: dict[str, Any], branding: dict[str, bool]) -> None:
    records = json.loads(path.read_text())
    rows = {row["artifact"]: row for row in records["artifacts"]}
    decisions = {row.split("\t")[0]: row.split("\t")[1:] for row in grants}
    if set(rows) != decisions.keys() or len(rows) != len(records["artifacts"]):
        refuse("rights records differ from acquired artifact coverage")
    for name, (declared, family, reserved) in decisions.items():
        row = rows[name]
        if row["licence"] != {"declared": declared, "family": family, "assetsReservedSeparately": reserved == "true"}:
            refuse("recorded licence differs from byte-bound evidence: " + name)
        rights = {r["surface"]: r["status"] for r in row["rights"]}
        if set(rights) != {"code", "names", "behaviour", "branding", "textures", "models", "sounds"} or len(rights) != len(row["rights"]):
            refuse("recorded rights surface coverage differs: " + name)
        if rights.get("code") != code_status(family):
            refuse("recorded code permission differs from verified licence: " + name)
        if row["acquisition"] != dict(line.split("\t") for line in acquisition)[name]:
            refuse("recorded acquisition differs from ledger: " + name)
    if records != derived_rights(records, grants, acquisition, evidence, branding):
        refuse("recorded surface permissions, publication, source-use or totals differ from acquired evidence")


def derived_rights(original: dict[str, Any], grants: list[str], acquisition: list[str],
                   evidence: dict[str, Any], branding: dict[str, bool]) -> dict[str, Any]:
    document = json.loads(json.dumps(original))
    observations = {r["artifact"]: r for r in evidence["artifacts"].values()}
    decisions = {line.split("\t")[0]: line.split("\t")[1:] for line in grants}
    acquired = dict(line.split("\t") for line in acquisition)
    rank = {"not-applicable": 0, "cleared": 1, "permission-required": 2, "unresolved": 3, "prohibited": 4}
    for row in document["artifacts"]:
        name = row["artifact"]
        declared, family, reserved = decisions[name]
        row["licence"] = {"declared": declared, "family": family, "assetsReservedSeparately": reserved == "true"}
        row["acquisition"] = acquired[name]
        status = code_status(family)
        counts = observations[name]["surfaceEvidence"]["counts"]
        rights = {"code": status, "names": "unresolved", "behaviour": "cleared", "branding": "prohibited" if branding[name] else "not-applicable"}
        for surface in ("textures", "models", "sounds"):
            rights[surface] = "not-applicable" if counts[surface] == 0 else ("prohibited" if reserved == "true" else status)
        row["rights"] = [{"surface": r["surface"], "status": rights[r["surface"]]} for r in row["rights"]]
        row["sourceUse"] = {"cleared": "permitted", "permission-required": "conditional", "prohibited": "prohibited", "unresolved": "unresolved"}[status]
        row["publication"] = "blocked:" + max(rights.values(), key=rank.__getitem__)
    rows = document["artifacts"]
    source_use = dict(sorted(Counter(r["sourceUse"] for r in rows).items()))
    document["totals"] = {
        "acquisition": dict(sorted(Counter(r["acquisition"] for r in rows).items())),
        "artifacts": len(rows),
        "licenceFamilies": dict(sorted(Counter(r["licence"]["family"] for r in rows).items())),
        "publication": dict(sorted(Counter(r["publication"] for r in rows).items())),
        "publicationCleared": sum(r["publication"] == "cleared" for r in rows),
        "sourceUse": source_use,
        "sourceUseProhibited": source_use.get("prohibited", 0),
        "sourceUseUnresolved": source_use.get("unresolved", 0),
    }
    return document


def regenerate(path: Path, grants: list[str], acquisition: list[str],
               evidence: dict[str, Any], branding: dict[str, bool]) -> None:
    original = json.loads(path.read_text())
    document = derived_rights(original, grants, acquisition, evidence, branding)
    print("WP0 source-use correction: " + json.dumps({"before": original["totals"]["sourceUse"], "after": document["totals"]["sourceUse"]}, sort_keys=True))
    path.write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")


def reconcile_provenance(path: Path, records: dict[str, Any], evidence: dict[str, Any], rebuild: bool) -> None:
    original = json.loads(path.read_text())
    updated = json.loads(json.dumps(original))
    suffix = " Per-input records are in inventory/sevtech-ages-3.2.3/rights.json and their evidence, repository citations and search trails in rights-evidence.json; this is the strictest aggregate status, not a grant for every input."
    rank = {"not-applicable": 0, "cleared": 1, "permission-required": 2, "unresolved": 3, "prohibited": 4}
    for row in updated["rights"]:
        if row["surface"] not in ("code", "textures", "models", "sounds", "names", "branding"):
            continue
        counts = Counter(r["status"] for artifact in records["artifacts"] for r in artifact["rights"] if r["surface"] == row["surface"])
        row["status"] = max(counts, key=rank.__getitem__)
        row["basis"] = ", ".join(str(count) + " " + status for status, count in sorted(counts.items())) + "." + suffix
    counts = records["totals"]["sourceUse"]
    updated["publication"]["notes"], replacements = re.subn(
        r"permitted for \d+ inputs under a cited permissive grant, conditional for \d+, prohibited for \d+ and unresolved for \d+",
        "permitted for {permitted} inputs under a cited permissive grant, conditional for {conditional}, prohibited for {prohibited} and unresolved for {unresolved}".format(**counts),
        updated["publication"]["notes"],
    )
    if replacements != 1:
        refuse("provenance publication summary must occur exactly once")
    undeclared = [r for r in evidence["artifacts"].values() if not r["hostDeclaration"]["declared"]]
    resolved = sum(r.get("resolution", {}).get("status") == "resolved" for r in undeclared)
    for source in updated["sources"]:
        if source.get("locator", "").endswith("/rights-evidence.json"):
            source["use"], replacements = re.subn(
                r"\d+ of the \d+ jar-undeclared inputs resolved to (?:a cited grant|a byte-bound cited grant); \d+ remain (?:absent|unresolved)\.(?: Prior attempted citations remain in their resolution trails, not as grants\.)?",
                f"{resolved} of the {len(undeclared)} jar-undeclared inputs resolved to a byte-bound cited grant; {len(undeclared) - resolved} remain unresolved. Prior attempted citations remain in their resolution trails, not as grants.",
                source["use"],
            )
            if replacements != 1:
                refuse("provenance grant summary must occur exactly once")
    if rebuild:
        path.write_text(json.dumps(updated, indent=2, ensure_ascii=False) + "\n")
    elif updated != original:
        refuse("provenance rights summaries differ from acquired evidence")


def observe_artifact(name: str, record: dict[str, Any], path: str, cache: Path,
                     ledger: dict[str, list[str]] | None, rebuild: bool) -> tuple[bytes, list[str], bool]:
    artifact = Path(path).read_bytes()
    if hashlib.sha256(artifact).hexdigest() != record["artifactSha256"]:
        refuse("artifact SHA256 differs: " + name)
    with zipfile.ZipFile(path) as archive:
        host = record["hostDeclaration"]
        jar_evidence = [e for e in host["evidence"] if e["kind"] == "jar-entry"]
        if not jar_evidence:
            refuse("missing host observation: " + name)
        for entry in jar_evidence:
            prefix, member = entry["source"].split("!", 1)
            if Path(prefix).name != name or ".." in Path(prefix).parts:
                refuse("host observation artifact identity differs: " + name)
            if hashlib.sha256(archive.read(member)).hexdigest() != entry["sha256"]:
                refuse("host observation differs: " + name + "!" + member)
        sources, logos = observed_host(archive, host, name, cache, ledger[name][3] if ledger else None)
        counts = {"textures": 0, "models": 0, "sounds": 0}
        for member in archive.namelist():
            if not member.startswith("assets/") or member.endswith("/") or re.match(r"assets/[^/]+/lang/", member):
                continue
            lower = member.lower()
            if lower.endswith((".png", ".jpg", ".jpeg")):
                counts["textures"] += 1
            elif re.match(r"assets/[^/]+/(models|geo|animations)/", member):
                counts["models"] += 1
            elif lower.endswith((".ogg", ".wav", "sounds.json")):
                counts["sounds"] += 1
        if ledger and counts != record["surfaceEvidence"]["counts"]:
            if not rebuild:
                refuse("recorded surface counts differ from acquired jar: " + name)
            print("WP0 surface-count correction: " + name + " " + json.dumps({"before": record["surfaceEvidence"]["counts"], "after": counts}, sort_keys=True))
            record["surfaceEvidence"]["counts"] = counts
    return artifact, sources, logos


def repository_linked(repository: dict[str, Any], sources: list[str]) -> bool:
    # A recorded redirect name alone does not byte-bind the old repository to
    # its new owner. Redirect-only attempts remain unresolved without that proof.
    url = repository.get("url", "")
    if not re.fullmatch(r"https://github\.com/[^/]+/[^/]+", url):
        return False
    path = url.removeprefix("https://github.com/").lower()
    return any(re.search(r"https?://(?:www\.)?github\.com/" + re.escape(path) + r"(?:[/'\"#?\s]|$)", text, re.IGNORECASE) for text in sources)


def commit_path_observation(cache: Path, repository: dict[str, Any], citation: dict[str, Any], rebuild: bool) -> None:
    project = repository["url"].removeprefix("https://github.com/")
    endpoint = "https://api.github.com/repos/" + project + "/contents/" + quote(citation["path"], safe="/") + "?ref=" + repository["commit"]
    key = hashlib.sha256(endpoint.encode()).hexdigest()
    path = cache / ("github-contents-" + key + ".json")
    if not path.is_file():
        refuse("missing commit/path grant observation")
    observation = json.loads(path.read_text())
    if observation.get("endpoint") != endpoint or not re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?(?:Z|\+00:00)", observation.get("fetchedAt", "")):
        refuse("commit/path grant observation request identity differs")
    response = json.loads(cached_bytes(cache, observation.get("responseSha256")))
    if response.get("type") != "file" or response.get("path") != citation["path"] or response.get("sha") != citation["blobSha"]:
        refuse("commit/path grant observation blob identity differs")
    if rebuild:
        citation["observation"] = observation
    elif citation.get("observation") != observation:
        refuse("commit/path grant observation differs from admitted record")


def repository_grant(name: str, resolution: dict[str, Any], sources: list[str], cache: Path,
                     rebuild: bool = False) -> None:
    repository = resolution["repository"]
    citation = repository.get("licenceFile" if resolution["basis"] == "licence-file" else "readme")
    if not (digest(repository.get("commit"), 40) and citation and digest(citation.get("sha256")) and citation.get("path")):
        refuse("missing pinned grant citation: " + name)
    if resolution["basis"] == "licence-file" and not digest(citation.get("blobSha"), 40):
        refuse("missing pinned Git blob identity: " + name)
    # README citations byte-bind cached raw text, but do not independently prove
    # the recorded commit/path through the GitHub contents API.
    source = repository.get("url", "")
    if not re.fullmatch(r"https://github\.com/[^/]+/[^/]+", source):
        refuse("missing repository source: " + name)
    if repository.get("laterLicence"):
        refuse("later licence is not a pinned host grant: " + name)
    expected_url = source + "/blob/" + repository["commit"] + "/" + citation["path"]
    if citation.get("url", expected_url) != expected_url:
        refuse("cited grant repository identity differs: " + name)
    grant = cached_bytes(cache, citation["sha256"])
    if citation.get("blobSha") and hashlib.sha1(b"blob " + str(len(grant)).encode() + b"\0" + grant).hexdigest() != citation["blobSha"]:
        refuse("cited Git blob identity differs: " + name)
    if not grant_text_matches(grant.decode("utf-8", errors="replace"), resolution["declared"]):
        refuse("cited legal text does not state the recorded grant: " + name)
    if not repository_linked(repository, sources):
        refuse("artifact has no byte-bound link to cited repository: " + name)
    if resolution["basis"] == "licence-file":
        commit_path_observation(cache, repository, citation, rebuild)


def resolve_grant(name: str, record: dict[str, Any], sources: list[str], artifact: bytes, cache: Path,
                  policies: set[tuple[str, ...]], rebuild: bool) -> tuple[str, str, str]:
    host = record["hostDeclaration"]
    resolution = record.get("resolution", {})
    previous = (resolution.get("trail") or [{}])[-1].get("result", "")
    if rebuild and resolution.get("status") == "unresolved" and "WP7 correction:" in previous and "cited legal text does not state the recorded grant:" in previous:
        try:
            repository_grant(name, resolution, sources, cache, True)
        except SystemExit:
            pass  # An unsuccessful attempted citation remains unresolved.
        else:
            resolution["status"] = "resolved"
            reason = "WP7 matcher correction: leading licence title and restrictive phrasing independently verified; prior conservative rejection retained in the trail."
            resolution["note"] += " " + reason
            resolution["trail"].append({"step": "WP7 legal matcher correction", "result": reason})
    if rebuild and resolution.get("status") == "resolved" and resolution.get("basis") in ("licence-file", "readme"):
        try:
            repository_grant(name, resolution, sources, cache, True)
        except SystemExit as error:
            reason = "WP7 correction: prior resolved citation retained only as an attempted source; " + str(error) + "."
            resolution["status"] = "unresolved"
            resolution["note"] = resolution.get("note", "") + " " + reason
            resolution.setdefault("trail", []).append({"step": "WP7 byte-bound source attribution", "result": reason})
    if resolution.get("status") == "resolved":
        if host["declared"] and host["declared"] != resolution["declared"]:
            refuse("host declaration conflicts with cited grant: " + name)
        if resolution["basis"] in ("licence-file", "readme"):
            repository_grant(name, resolution, sources, cache)
        elif resolution["basis"] == "modrinth-exact-file":
            citation = resolution.get("modrinth", {})
            if citation.get("hashAlgorithm") != "sha1" or not citation.get("project") or not citation.get("licenseId") or citation.get("api") != "https://api.modrinth.com/v2/version_files":
                refuse("missing exact-file grant citation: " + name)
            observation = exact_file_grant(cache, artifact, citation, resolution["declared"])
            if rebuild:
                citation["observation"] = observation
            elif citation.get("observation") != observation:
                refuse("new exact-file observation receipt differs from admitted record: " + name)
        else:
            refuse("unknown grant basis: " + name)
        declared, family = resolution["declared"], resolution["family"]
        reserved = str(resolution["assetsReservedSeparately"]).lower()
        if (declared, family, reserved) not in policies:
            refuse("unrecognised cited licence policy: " + name)
    else:
        declared = host["declared"]
        matches = [(family, reserved) for text, family, reserved in policies if text == declared]
        if len(matches) != 1:
            refuse("ambiguous or unrecognised host licence: " + name)
        family, reserved = matches[0]
    if any(c in declared for c in "\t\r\n"):
        refuse("multiline licence declaration: " + name)
    return declared, family, reserved


def main() -> None:
    evidence_path, paths_path, families_path, grants_path, acquisition_path, cache_path, *rest = sys.argv[1:]
    cache = Path(cache_path)
    evidence = json.loads(Path(evidence_path).read_text())
    ledger = None
    rebuild = bool(rest and rest[-1] == "--rebuild")
    branding = {}
    if len(rest) >= 4:
        records_path, ledger_path, fixture_id, artifact_set_sha = rest[:4]
        ledger = {fields[1]: fields for fields in (line.split("\t") for line in Path(ledger_path).read_text().splitlines())}
        if evidence["fixture"]["id"] != fixture_id or evidence["fixture"]["artifactSet"]["sha256"] != artifact_set_sha:
            refuse("evidence fixture or artifact-set identity differs")
        if evidence["fixture"]["artifactSet"]["count"] != len(ledger):
            refuse("evidence artifact-set count differs")
        aliases = {alias: node for node, record in evidence["artifacts"].items() for alias in record["nodeIds"] if alias != node}
        if evidence.get("aliases", {}) != aliases:
            refuse("evidence graph aliases differ from node identities")
    paths = dict(line.rstrip("\n").split("\t") for line in Path(paths_path).read_text().splitlines())
    policy_rows = [tuple(line.split("\t")) for line in Path(families_path).read_text().splitlines()]
    if len({row[0] for row in policy_rows}) != len(policy_rows):
        refuse("duplicate licence policy declaration")
    policies = set(policy_rows)
    grants, acquisition, seen = [], [], set()
    for node, record in evidence["artifacts"].items():
        name = record["artifact"]
        if name not in paths or name in seen or node not in record["nodeIds"]:
            refuse("artifact coverage or graph identity differs: " + name)
        seen.add(name)
        if ledger:
            expected = "bundled-override" if ledger[name][5] == "bundled-override" else "fetched"
            if record["acquisitionStatus"] != expected:
                refuse("acquisition mode differs from authoritative ledger: " + name)
        artifact, sources, branding[name] = observe_artifact(name, record, paths[name], cache, ledger, rebuild)
        declared, family, reserved = resolve_grant(name, record, sources, artifact, cache, policies, rebuild)
        grants.append("\t".join((name, declared, family, reserved)))
        status = record["acquisitionStatus"]
        if status not in ("fetched", "bundled-override"):
            refuse("unknown acquisition status: " + name)
        acquisition.append(name + "\t" + ("bundled-override" if status == "bundled-override" else "automated-curseforge"))
    if seen != set(paths):
        refuse("evidence does not cover acquired artifacts")
    if rebuild:
        regenerate(Path(rest[0]), grants, acquisition, evidence, branding)
        Path(evidence_path).write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
    elif rest:
        verify_recorded_rights(Path(rest[0]), grants, acquisition, evidence, branding)
    if ledger is not None:
        provenance = Path(evidence_path).parents[2] / "provenance" / (fixture_id + ".json")
        reconcile_provenance(provenance, json.loads(Path(rest[0]).read_text()), evidence, rebuild)
    Path(grants_path).write_text("\n".join(sorted(grants)) + "\n")
    Path(acquisition_path).write_text("\n".join(sorted(acquisition)) + "\n")


if __name__ == "__main__":
    main()
