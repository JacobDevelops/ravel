# SevTech: Ages 3.2.3 — WP0 input freeze

## Delivered files and scope

Added only these repository files (no existing tracked files changed by this assignment):

- `tests/fixtures/descriptors/sevtech-ages-3.2.3.json`
- `tests/fixtures/descriptors/runtime-inputs-1.12.2.json`
- `registry/inventory/sevtech-ages-3.2.3/references.json`
- `registry/inventory/sevtech-ages-3.2.3/rights.json`
- `registry/inventory/sevtech-ages-3.2.3/graph.json`
- `registry/provenance/sevtech-ages-3.2.3.json`

The records preserve the ATM10 envelope, field vocabulary and factual identity/reference representation, with explicit
legacy acquisition and declaration-provenance extensions. Current schema/checker incompatibilities are documented below,
not hidden with fabricated launcher/runtime metadata. No tooling, defaults, existing fixtures, mappings, implementation
code or build pins were edited. ATM10 remains intact; Minecraft 26.1.2 remains the existing build default. This is
acquisition/declared-metadata evidence, not a live JDK 27 launch, native compatibility or progression-parity result.

## Publisher input and acquisition counts

Publisher: CurseForge project **268208**, file **3569915**, `SevTech_Ages_3.2.3.zip`.

- URL: `https://mediafilez.forgecdn.net/files/3569/915/SevTech_Ages_3.2.3.zip`
- Archive bytes: **21,095,019**.
- Required and observed SHA256: `f872775f7ac0996fdd1bde177a1517aecc43e9209da97406dcbbe3ac842483b8`.
- Manifest SHA256: `16f1ecb9083c720fbf7d2adc9e0ffeea2dfbe9cb2873201cf20d72a6a81edad2`.
- `modlist.html` SHA256: `6846147277dbfeaef17f84c5286a877899494f59b6ec236b4f7bc419bb2a036c`.
- Exact publisher pins: Minecraft **1.12.2**, Forge **14.23.5.2860**, pack version **3.2.3**.

| Input category                         | Expected | Fetched | Unfetched |
| -------------------------------------- | -------: | ------: | --------: |
| CurseForge project/file references     |      254 |     254 |         0 |
| Publisher-bundled override jars        |        5 |       5 |         0 |
| Total top-level mod-jar inputs         |      259 |     259 |         0 |
| Official game jars, separate from mods |        2 |       2 |         0 |
| Official version JSON                  |        1 |       1 |         0 |

All 254 reference project/file pairs are unique. Their publisher file names came from
`https://www.curseforge.com/api/v1/mods/{projectId}/files/{fileId}`. Actual CDN acquisition URLs, SHA1, SHA256, sizes,
local paths, acquisition status and individual attempt records are in `references.json`; no failed acquisition is
silently omitted. The CurseForge archive includes 254 manifest references, not 254 loaded containers. Missing `isLocked`
is represented as `null`, exactly as the current rebuild tool reads the absent publisher property.

All large inputs and evidence are ignored under `tests/fixtures/local/sevtech-ages-3.2.3/`: `source/`, `extracted/`,
`jars/`, `metadata/`, `runtime/`, `rights-evidence/`, and the local acquisition ledgers. `manifest.json`, `modlist.html`
and all overrides were extracted; there are **1,524 override files**. `extracted-entries.json` locally records sizes and
SHA256 for all 1,526 non-directory publisher entries. The original publisher archive is retained under its original
filename; an identical `${FIXTURE_ID}.zip` copy satisfies the existing inventory check's filename convention. No pack
scripts, quests, jar bytes or assets were added to tracked files.

### Five separately hashed override jars

| Jar                                       | SHA256                                                             |
| ----------------------------------------- | ------------------------------------------------------------------ |
| Galacticraft-Planets-1.12.2-4.0.2.280.jar | `e8e8c0fe8fe5f4ad4669bdd567d4397d78e893b106a4780101796747e292102d` |
| GalacticraftCore-1.12.2-4.0.2.280.jar     | `405f0bd7fb6ccb52512a6823d4b9eb288bbb9c1ac36ceba651738a85484f2fce` |
| MicdoodleCore-1.12.2-4.0.2.280.jar        | `a8dcb698eefe4f70ee1ff920227f261434e36a582a2752a057fd692161ad125d` |
| Nutrition-1.12.2-4.3.0.jar                | `1f5f69af7944de19e4c2662c3079115d45e60f4f464ed5c04d83e6fc58d8d00f` |
| SevTweaksNPC-0.0.4.jar                    | `924b6f9cd11592bb0944dc4baaf64b826c4f21fd2ccf8628b6cb18659886339e` |

The 259-input artifact-set SHA256 is `f18dece0e1c6d22a7929753b8c662f857eb0abaed2329e2ec02fdda1a2e5207a`. It hashes UTF-8
rows `mod<TAB>filename<TAB>lowercase SHA256<LF>`, sorted by filename, matching the current artifact-set row encoding. It
excludes nested jars, game jars and runtime metadata. Both rights and graph bind to this digest, and provenance cites
it. The different counts in those records are intentional: 254 publisher references, five bundled inputs, 259 top-level
jar inputs, four nested jars, and 277 declared graph identities are not synonyms for runtime-loaded mods.

## Official Minecraft 1.12.2 inputs and JDK distinction

Version metadata URL: `https://piston-meta.mojang.com/v1/packages/832d95b9f40699d4961394dcf6cf549e65f15dc5/1.12.2.json`

| Input        | Publisher SHA1                             | Observed SHA256                                                    |            Bytes |
| ------------ | ------------------------------------------ | ------------------------------------------------------------------ | ---------------: |
| Version JSON | `832d95b9f40699d4961394dcf6cf549e65f15dc5` | `56dac2e1caa872d7e37a7b5523056cd812dca94f912ad2bd652817bee60edca4` | recorded locally |
| Client       | `0f275bc1547d01fa5f56ba34bdc87d981ee12daf` | `8ada07da5ee77dad3527bd7278fbd05ee1fc8a597813b216a871a2d7d64cc64f` |       10,180,113 |
| Server       | `886945bfb2b978778c3a0288fd7fab09d315b25f` | `fe1f9274e6dad9191bf6e6e8e36ee6ebc737f373603df0946aafcded0d53167e` |       30,222,121 |

Client and server URLs are the official `piston-data.mojang.com/v1/objects/{publisher-sha1}/client.jar` and `server.jar`
URLs in the version JSON. All three publisher SHA1s were checked against downloaded bytes; SHA256 was independently
computed. The descriptor retains Mojang's historical `jre-legacy` / Java 8 metadata, while its separate `execution`
block pins **Eclipse Temurin 27.0.0+35 / JDK 27** from `mise.toml`. This is a requested execution target, not a claim
that the publisher runtime was Java 27, and not a live-launch attestation. No Prism entry or nonexistent Java
distribution was fabricated to placate the current schema.

## Rights ledger and limitations

All **259** input jars have per-input records and host-licence evidence or an explicit absent/unknown finding.
Vocabulary matches ATM10 (`licence.family`, `sourceUse`, per-surface statuses, acquisition and publication verdicts).

| Licence family                 |    Jars |
| ------------------------------ | ------: |
| absent                         |     235 |
| weak-copyleft                  |      16 |
| noncommercial-or-noderivatives |       5 |
| custom                         |       2 |
| all-rights-reserved            |       1 |
| permissive                     |       0 |
| strong-copyleft                |       0 |
| attribution-required           |       0 |
| source-restricted              |       0 |
| reference-only                 |       0 |
| **Total**                      | **259** |

Source use: **0 permitted, 16 conditional, 6 prohibited, 237 unresolved**. Code-surface statuses: **16
permission-required, 6 prohibited, 237 unresolved**. Publication: **0 cleared**, **64 blocked:prohibited**, **195
blocked:unresolved**.

Evidence includes `mcmod.info`, manifests and host legal notices, with exact legal-entry SHA256. Dependency/doc-only
licence text is not treated as a grant for the containing mod. Direct CurseForge project-page attempts were HTTP 403;
all **254** cfwidget project responses were fetched successfully, but all their licence fields were blank. Those
response hashes and responses are local. Thus `absent` means no host grant located in the inspected legal
metadata/project response, not a claim that no licence exists anywhere. Missing grants remain unresolved, not assumed
permissive. Asset-presence observations are bounded top-level archive-name evidence, not copied assets or a complete
runtime resource inventory. Pack scripts/progression material are not implicitly licensed by mod licences. The
fixture-level publication status is private-only and unreviewed.

## Declared legacy dependency graph

The graph includes all 259 top-level inputs and four nested jars. It records **277 nodes, 746 declaration edges (481
required, 265 optional), 360 platform edges, 176 clusters, largest cluster 41**. It extracts **269 class-level Forge
@Mod annotations** and **252 mcmod.info entries**. Each edge carries jar path, precise source location, raw declaration,
ordering, side and literal version constraint. Both duplicate declaration sources remain visible instead of silently
merging claims.

Ten input jars explicitly have no declared dependencies. Seven `mcmod.info` files needed non-strict JSON
control-character recovery; original strict parse errors remain in their per-jar issue records. Three required target
identities are unresolved (`ivtoolkit`, `bnbgamingcore`, `micdoodlecore`); these are declaration-resolution gaps, **not
evidence that their jars are absent or runtime loading fails**. Literal versions are not interpreted as a successful
version-range match. Runtime-dependent Forge metadata flags are retained, not simulated.

Class constant pools/class-level annotation attributes were read; method bodies were skipped by attribute length, not
decompiled, disassembled or interpreted. The graph conservatively marks `restrictedContentRead: true` for opening
compiled class bytes and explicitly explains its narrow scope. Provenance records that access rather than pretending a
pure ZIP-table-only process. No tracked generation/scaffolding script was introduced; the throwaway graph generator was
removed.

## Validation exercised

Commands ran from `ravel`. Exit status is stated explicitly. Schema/checker rejection is reported, not bypassed by
changing existing tooling.

| Command                                                                                                                                                            | Result                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `target/debug/ravel-xtask fixture verify tests/fixtures/descriptors/sevtech-ages-3.2.3.json tests/fixtures/local/sevtech-ages-3.2.3/source/SevTech_Ages_3.2.3.zip` | **exit 0** — `Verified 254 references for Minecraft 1.12.2 with forge-14.23.5.2860.` Existing binary used; no concurrent source rebuild was attempted.                                                                                                        |
| `mise exec -- check-jsonschema --schemafile schemas/fixture.schema.json tests/fixtures/descriptors/sevtech-ages-3.2.3.json`                                        | **exit 1** — `$.manifest.modLoader`: Forge pin rejected by NeoForge-only regex.                                                                                                                                                                               |
| `mise exec -- check-jsonschema --schemafile schemas/runtime-inputs.schema.json tests/fixtures/descriptors/runtime-inputs-1.12.2.json`                              | **exit 1** — root `execution` unexpected, `prism` and `platforms` missing, `minecraft.downloads` unexpected.                                                                                                                                                  |
| `mise exec -- check-jsonschema --schemafile schemas/provenance.schema.json registry/provenance/sevtech-ages-3.2.3.json`                                            | **exit 0**, including final graph-access source record — `ok -- validation done`.                                                                                                                                                                             |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-inventory.sh --fixture`                                                          | **exit 0** — `inventory reconciles: 254/254 pinned references, fixture digests bound`.                                                                                                                                                                        |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-graph.sh`                                                                        | **exit 1** — acquired artifact tree missing: `prism-approot/instances/sevtech-ages-3.2.3/minecraft/mods`.                                                                                                                                                     |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-rights.sh --fixture`                                                             | **exit 1** — same Prism artifact-tree prerequisite.                                                                                                                                                                                                           |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-rights.sh --offline`                                                             | **exit 1** — global decision-table families absent from this fixture are rejected as stale: attribution-required, permissive, reference-only, source-restricted, strong-copyleft. All actually used statuses/surfaces are valid; independently checked below. |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-provenance.sh --fixture`                                                         | **exit 1** — same Prism artifact-tree prerequisite.                                                                                                                                                                                                           |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-provenance.sh --offline`                                                         | **exit 1** — acquired-byte check explicitly skipped, then missing `registry/inventory/sevtech-ages-3.2.3/surfaces.json`. Surfaces/plans are not WP0 target files.                                                                                             |

An initial rights-check invocation before the rights subagent landed exited 1 with missing `rights.json`; the final
`--fixture` invocation above supersedes that transient result. The graph checker stops before reading its committed
graph, so its recorded failure is independent of graph generation timing.

Additional Python assertions exercised through Eval passed: archive expected SHA256; Mojang metadata/client/server
publisher SHA1; 254 unique project/file pairs; all 259 top-level jar SHA1/SHA256s recomputed; exact 259-record
rights/input equality; graph coverage of all 259 inputs plus four nested jars; per-edge jar/location provenance on all
746 edges; 277 nodes and complete cluster membership; rights status/surface vocabulary read from the public schema;
licence tally recomputation; shared rights/graph artifact-set digest and provenance digest binding. These are bounded
input/inventory integrity checks, not replacements for the blocked legacy registry recomputation gate. No live launch,
JVM compatibility test, broad build/test suite or performance check was run.

## Exact WP7 schema and checker changes required

Line numbers below are from the current files read during WP0. Existing files were not edited; no fake Prism tree,
launcher logs, metadata index or shaderpack was created.

| Field / contract                                        | Path:line                                                                             | Required change                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `manifest.modLoader.pattern`                            | `schemas/fixture.schema.json:115`                                                     | Admit the `forge-14.23.5.2860` family alongside existing `neoforge-*` values; preserve both historical workloads. The Rust fixture verifier already accepts the honest Forge pin.                                                                                                                                                                                                                        |
| Required root `prism`, `platforms`                      | `schemas/runtime-inputs.schema.json:7`                                                | Make launcher/platform acquisition conditional or introduce a precise game-runtime descriptor variant; official game-input pins do not establish Prism installation or per-platform Java downloads.                                                                                                                                                                                                      |
| Root `execution`                                        | `schemas/runtime-inputs.schema.json:6-8`                                              | Define the explicit execution-target object (JDK27 vendor/version/pin source/purpose) while retaining strict extra-property validation. Do not overwrite Mojang's historical Java8 baseline.                                                                                                                                                                                                             |
| `minecraft.downloads.client` / `.server`                | `schemas/runtime-inputs.schema.json:19-31`                                            | Define official client/server hashed artifact records containing URL, SHA1, SHA256 and size; require these for this game-input variant. Current Minecraft-object strictness rejects `downloads`.                                                                                                                                                                                                         |
| Platform-specific runtime shape                         | `schemas/runtime-inputs.schema.json:80-96`                                            | Keep existing two-platform launcher checks where applicable, but do not require/fabricate Linux and macOS launcher/Java artifacts for the official legacy jar-input variant.                                                                                                                                                                                                                             |
| Acquired artifact tree/import/index/shaderpack contract | `registry/checks/artifact-set.sh:35-76`                                               | Support pinned standalone reference jars plus publisher-bundled override jars as an acquisition mode. Current code assumes a Prism instance root/import log, `mods/.index`, shaderpacks/resourcepacks/config/kubejs directories, exact index-to-jar population, and at least one shaderpack. Preserve the byte-bound artifact-set encoding; do not create fake launcher evidence for legacy acquisition. |
| Legacy identity/dependency reader                       | `registry/checks/modmeta.py:58,125-131`; `registry/checks/mod-metadata.sh:4-6`        | Add `mcmod.info` and exact class-level @Mod annotation metadata accounting alongside modern TOML/jarjar. Preserve required/optional/ordering, side, raw versions, source provenance, nested inputs and explicit missing declarations. Current modern-only recomputation cannot reproduce the new legacy graph.                                                                                           |
| Graph node/edge equality                                | `registry/checks/verify-graph.sh:44-60`                                               | Validate legacy declaration/source fields and ordering semantics rather than comparing enriched edges/nodes to an exact modern-only field reconstruction that necessarily strips them. Retain per-edge provenance.                                                                                                                                                                                       |
| Global licence decision table vs selected fixture       | `registry/checks/verify-rights.sh:45-57`                                              | Validate every observed family against the policy table without requiring every globally supported family to occur in every selected fixture. The reverse subset check at 55-57 falsely treats valid global policy rows as stale for SevTech.                                                                                                                                                            |
| Host licence evidence model                             | `registry/checks/verify-rights.sh:79-92`; `registry/checks/rights-model.sh`           | Accept evidence-bearing legacy mcmod/manifest/legal-notice observations and explicit unresolved grants; do not equate absent modern mods.toml licence metadata with a contradictory legacy observation. Acquisition classes need `bundled-override`.                                                                                                                                                     |
| Provenance inventory prerequisites                      | `registry/checks/verify-provenance.sh:64-71`                                          | Make phases/required inventory records explicit: WP0 has references/rights/graph but no surfaces/plans. Full post-WP0 verification should still require genuinely produced surfaces/plans, not placeholder files.                                                                                                                                                                                        |
| Metadata access vs implementation method                | `registry/checks/verify-provenance.sh:257-301`; `registry/checks/derivation-reads.sh` | Distinguish constant-pool/class-annotation metadata access from reading method/source implementation. Register exact legacy `derivation.reads` vocabulary and its policy. Current blanket `metadata-only => restrictedContentRead false` cannot describe explicitly declared annotation-only binary access. Do not remove the graph's honest access declaration merely to pass.                          |

The xtask default descriptor/archive constants in `tools/ravel-xtask/src/commands.rs:5-6` are ATM10-specific, but
explicit descriptor and archive arguments work and were used successfully. This is not a blocker to WP0 fixture
verification. Registry `RAVEL_FIXTURE_ID` selection itself works (`registry/checks/public-repo.sh:23`); remaining
failures are acquisition/model/phase assumptions, not a silently ignored fixture selector.

## Follow-up: graph.json reshaped to the ATM10 contract

`registry/inventory/sevtech-ages-3.2.3/graph.json` now has exactly the ATM10 graph shape: top-level
`clusters, derivation, edges, fixture, inventoryKind, nodes, schemaVersion, totals`; node keys
`artifact, classification, displayName, id, kind, license, loader, version`; edge keys
`classification, from, resolution, side, to, type, versionRange`; cluster keys `id, members, size`; fixture
`artifactSet{count,sha256}, id, pinnedReferences`; derivation `decompiled, producedBy, reads, restrictedContentRead`;
and the six ATM10 totals. It uses the same conventions: keys sorted, 2-space indent, nodes sorted by id, edges sorted by
(from, to, type), members sorted, `cluster-NNN` ids, and clusters made from required internal/nested connectivity. Size:
**252,256 bytes**, with no local paths.

The ATM10 edge object has no field for a source, so source evidence moved to the new sibling file
`registry/inventory/sevtech-ages-3.2.3/graph-evidence.json` (**805,336 bytes**, under 1 MiB, no `tests/fixtures/local`
or home paths). It contains:

- `edges[i]`, which matches graph `edges[i]` by index and repeats from/to/type/versionRange/side so the pairing can be
  checked. Each entry adds declarationField, ordering, rawDeclaration, semantics, platformId and
  `source{artifact, [nestedEntry], location}`. The location is either an `mcmod.info#/N` entry or a
  `Class.class#RuntimeVisibleAnnotations/…/@…Mod/dependencies/N` annotation attribute.
- `excludedDeclarations`, which holds 3 wildcard `after:*`/`before:*` ordering rules. They depend on no mod identity,
  and ATM10 has no `resolution` value for them, so they are no longer graph edges.
- Per-jar `artifacts` for all 259 inputs and 4 nested jars: SHA256, origin, mcmod.info/@Mod declarations, strict-JSON
  issues and `dependencyStatus`. This includes 14 jars with no declared dependencies: 10 top-level and 4 nested.
- Per-node `sources`, the full original derivation notes, and extended totals.

Changes in substance from the first version:

- **Loader:** the ATM10 vocabulary is `javafml`, `lowcodefml` and empty. Legacy 1.12 jars declare no per-artifact
  modLoader, so `loader` is `""` on every node and `"forge"` is not used. **Schema gap:** the graph format has no value
  for "legacy Forge, loader implied by pack pin `forge-14.23.5.2860`". WP7 should either admit a value such as `forge`
  or document empty as "not declared by the artifact".
- **Nested jars without identity:** in ATM10 a nested jar becomes a node only when it declares a mod id. The 4 SevTech
  nested jars declare none. Their earlier opaque nodes also had local paths baked into their ids, so they were dropped
  from the graph. They are still listed in the evidence `artifacts`.
- **Totals:** 273 nodes (269 mod, 4 opaque-artifact), 743 edges (481 required, 262 optional; 360 platform, 283 internal,
  100 absent), 172 clusters, largest 41. There are 3 blocked edges (`galacticraftcore→micdoodlecore`,
  `reccomplex→ivtoolkit`, `triumph→bnbgamingcore`, all required-dependency-absent-from-pack declaration-resolution gaps)
  and 7 blocked nodes.
- The artifact-set binding is unchanged (`f18dece0…207a`). `producedBy` names the untracked one-off extraction, and
  `restrictedContentRead` stays `true`.
- The provenance graph source now cites both files and was rechecked against the provenance schema.

Revalidation:

- `check-jsonschema` on the provenance file: **exit 0**.
- `verify-inventory.sh --fixture`: **exit 0**.
- `verify-graph.sh`: still **exit 1**, at the same Prism acquired-tree prerequisite, before it reads the graph.

Eval assertions passed:

- The key sets for top level, node, edge, fixture and totals equal ATM10's, and the orderings match.
- The artifact-set SHA256 was recomputed from the 259 jars and matches rights.
- Every input jar appears as a node `artifact`, and node ids are unique.
- Every node and edge carries a resolved or `blocked:` verdict, and no required absent edge is marked resolved.
- Clusters partition the nodes, sizes match, no required internal/nested edge crosses a cluster boundary, and a BFS walk
  confirms each cluster is connected.
- Evidence edges match graph edges by index on all five shared fields, and every source names an input artifact and a
  location.
- Neither file contains a local path.

**Update:** `references.json` and the provenance locators are now templated (next section). `rights.json` is left to
SevTechRights, which is removing its local paths in parallel.

## Follow-up: `${FIXTURE_ID}` placeholders in references and provenance

`registry/inventory/sevtech-ages-3.2.3/references.json` now writes every local path as
`tests/fixtures/local/${FIXTURE_ID}/…`. That covers 262 placeholder paths: the existing archive and manifest pair, 254
`jars/` paths and 5 bundled `extracted/overrides/mods/` paths. This is the convention ATM10 `references.json` and
`rebuild-references.sh:43-47` use. The file is now 194,328 bytes. The only literal fixture id left is `fixture.id`.

In `registry/provenance/sevtech-ages-3.2.3.json`, the 3 source locators now read:

- `tests/fixtures/local/${FIXTURE_ID}/source/${FIXTURE_ID}.zip`. This is ATM10's archive filename convention, which
  `verify-inventory.sh:17` also uses. The byte-identical local copy has SHA256 `f872775f…83b8`, the same as the
  publisher's `SevTech_Ages_3.2.3.zip`.
- `…/extracted/manifest.json`
- `…/extracted/modlist.html`

No `tests/fixtures/local/sevtech-ages-3.2.3/` string remains in references, provenance, graph or graph-evidence. The
`tests/fixtures/local/` prefix is kept, as in ATM10, followed by the placeholder.

Revalidation:

- `check-jsonschema` on the provenance file: **exit 0**.
- `verify-inventory.sh --fixture`: **exit 0** (`254/254 pinned references, fixture digests bound`).
- `verify-inventory.sh --offline`: **exit 0** (provenance schema validated; acquired reconciliation skipped).
- Eval: all 265 placeholder paths (262 + 3) resolve to existing files after substituting `${FIXTURE_ID}`, and all 259
  jar SHA256s recomputed through the resolved paths match the record.
