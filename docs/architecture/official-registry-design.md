# The official Ravel registry — design

Status: design proposal. This describes the shape the *public, distributable*
Ravel registry should take. It is not the private compatibility workspace, which
is migration-time scaffolding; the relationship between the two, and which
directories are temporary, are stated in "The migration boundary" below.

This document is a target to build toward, not a record of what exists. Where it
names a mechanism the loader does not yet have, that is a dependency to satisfy,
not a claim it is present.

**Authority and scope.** `SPEC.md` is the contract; this doc is subordinate to
it. `SPEC.md §12.3` places the registry at **post-initial / M13 scope** and
records a *preferred shape* plus three candidate mechanisms whose **selection is
a deferred M13 decision gate**. This document elaborates that preferred shape; it
does **not** select a mechanism and does not change M0–M10. Two constraints from
§12.3 govern everything below and are repeated so this doc cannot drift from them:

- **The registry is an index over author-owned release records, not the
  authoritative store of the artifacts.** Authors keep and serve their own
  release artifacts; the registry indexes, verifies, and points at them. The
  content-addressed `artifacts/` view below is the *addressing scheme and an
  optional cache/mirror*, never the sole authoritative copy — losing the registry
  must not lose a release.
- **Publisher identity is portable and author-controlled**, so losing a registry
  account does not invalidate published releases. Releases are immutable and
  addressed by the content hash the package manifest already requires, and
  moderation acts at the index, not the distribution layer.

Where the layout below shows an `artifacts/` store, read it as "content-addressed
addressing, which the registry may cache and mirror but authors own" — not as an
authoritative upload silo. Selecting the concrete distribution/identity mechanism
(a Go-module-style transparency log, TUF+Sigstore, or another) is left to the
M13 gate exactly as §12.3 records.

---

## 1. What the registry is for

The registry is where a Ravel user or a pack author finds, resolves, and
downloads native Rust mods, and where the loader verifies one is what it
claims before it runs. It answers four questions and nothing else:

1. **Discovery** — what mods and packs exist, at what versions, for which
   Minecraft and Ravel versions, and on which sides.
2. **Resolution** — given a set of wanted mods (or a pack), which exact versions
   satisfy their mutual compatibility and dependency constraints.
3. **Integrity** — the artifact you downloaded is byte-for-byte the one the
   registry indexed, bound by digest.
4. **Provenance and rights** — every published mod and pack carries a resolved
   provenance record covering code, behaviour, assets, names, branding,
   permissions, attribution, and licence obligations, because
   `docs/architecture/public-private-boundary.md` requires it and
   "reimplementation in Rust does not itself clear those rights."

It is deliberately **not** a build system, a general package host for arbitrary
code, or a social platform. It indexes signed, provenance-cleared Ravel mods and
the packs that compose them.

## 2. What a published unit is

The publishable unit is a **mod**: a versioned artifact plus the manifest that
`schemas/package.schema.json` (`urn:ravel:schema:package:1`) already validates.
("Package" is the format term — a mod is distributed *as* a package, and the
manifest schema keeps its `package` name; the registry's user-facing unit and its
index directory are `mods/`.) The manifest's required fields are the registry's
source of truth for identity (`id`, `version`), `compatibility` (`ravel`,
`minecraft`), `sides`, `authors`, `license`, `source`, and `provenance`. The
registry adds no parallel metadata format: it indexes what the manifest already
declares and refuses a mod whose manifest does not validate.

A mod `id` is a Ravel-native id. It is **not** an upstream mod id. The private
workspace's `RVL-NNN` canonical ids are internal slot numbers and do not appear in
the public registry; a published mod chooses its own name from the lexicon the
rewrite-integrity gate holds it to, and the registry rejects any mod whose
manifest or artifact carries an upstream identifier (the same U1/N1 rule the
private `checks/verify-counterparts.sh` enforces, run here as an admission gate).

The other publishable unit is a **pack** (§5): a composition of pinned mods plus
pack-owned data. Mods and packs sit in sibling index trees (`mods/` and `packs/`)
and share the artifact store and the admission gate.

## 3. Directory / storage layout

Two trees, and the boundary between them is the whole point of this section. The
**registry tree** is official and permanent — it is what ships and what a mirror
serves. The **working tree** is gitignored, local to the rewrite machine, and
never ships; it holds the original sources we translate from. Section 7.1
explains the working tree in full; it is shown here so that "the layout" is not
mistaken for the registry tree alone.

The registry tree is content-addressed at the artifact layer and human-navigable
at the index layer.

```text
# === REGISTRY TREE — official, permanent, this is what ships ===
registry/
  index/                      # the queryable catalogue (small, cacheable, signed)
    mods/                     # one mod each (see §2)
      <mod-id>/
        meta.json             # id, display name, description, authors, license,
                              #   homepage, all published versions (summary)
        <version>/
          manifest.json       # the validated package manifest (schema v1)
          provenance.json     # resolved provenance: code/behaviour/assets/names/
                              #   branding/permissions/attribution/licence
          artifact.json       # digest, size, download locator, signature
    packs/                    # pack compositions (see §5)
      <pack-id>/
        meta.json             # id, display name, authors, license, versions
        <version>/
          pack.json           # the validated pack manifest (schema urn:ravel:schema:pack:1):
                              #   pinned members + declared overrides + compatibility
          provenance.json     # provenance for the pack's OWN authored data AND
                              #   the composition/distribution permission
          overrides.json      # manifest of the pack-owned data bundle (config/
                              #   script/quest/resource/branding), by digest
          artifact.json       # digest, size, locator, signature of the override bundle
    index.json                # top-level: schema version, generated-at, the digest
                              #   of every mods/*/meta.json AND packs/*/meta.json
                              #   (Merkle root)
    signing/
      index.sig               # detached signature over index.json
      keys/                   # published public keys + rotation record
  artifacts/                  # content-addressed addressing (registry cache/mirror;
                              #   authors own the authoritative copies — see header)
    <sha256[0:2]>/<sha256>    # every artifact — a mod artifact OR a pack's
                              #   override bundle — addressed by its own digest.
                              #   The registry may cache/mirror these; the
                              #   author-owned release is the source of truth.
                              #   A pack stores references to member artifacts,
                              #   never copies of them.
  schemas/                    # copies of the schemas an index entry is validated to
```

```text
# === WORKING TREE — gitignored, local only, NEVER shipped (see §7.1) ===
# This is where the original sources we translate from live. Nothing here is
# tracked by git, indexed, signed, or served. It is the *input* to the rewrite,
# deleted-forward rather than published. Reserved by .gitignore so it has a
# place to land without ever being committed.
tests/fixtures/local/            # (in the public repo; the one authorized home
                                 #  for pinned material, per the boundary doc)
  atm11-0.2.1/                   # the pinned pack as acquired: mod JARs, the pack
                                 #   archive, ATM-owned overrides
  decompiled-mods/
    <jarstem>/                   # decompiled/extracted source a reader reads to
                                 #   write a behaviour specification
# (private workspace, also gitignored:)
  artifacts/  extracted/  scratch/   # acquisition downloads, unpacked trees,
                                     #   per-run scratch
```

Design properties, each with the reason:

- **The two trees never mix, and a check enforces it.** Original sources live only
  in the gitignored working tree; nothing there is ever copied into the registry
  tree. The public repo's `ravel:verify-scaffold` sweep and the private
  workspace's `verify-boundary.sh` fail the gate on any tracked file that carries
  those contents or even names the private workspace — so the separation is
  enforced, not merely intended (see §7.1).
- **The index points at author-owned releases (SPEC §12.3).** The authoritative
  copy of a release is the author's own; the registry is an index over release
  records plus an optional cache/mirror. Losing the registry, or an author's
  registry account, must not invalidate or delete a published release — publisher
  identity is portable and author-controlled.
- **Content-addressed artifacts.** An artifact's address *is* its SHA-256, so the
  integrity check is "does what I downloaded hash to the address I resolved,"
  and no host — registry or mirror — can substitute content without changing the
  address. This holds for a mod artifact and for a pack's override bundle alike; a
  pack references its members by their existing digests and stores no member bytes
  of its own. The manifest's `source`/`provenance` and the index's `artifact.json`
  both bind that same digest.
- **The index is a signed Merkle tree.** `index.json` carries the digest of every
  mod and pack `meta.json`, and each `meta.json` carries the digest of every
  version's manifest (`manifest.json` or `pack.json`), `provenance.json`, and
  `artifact.json`. A client verifies one signature over the root and then trusts
  any subtree by digest, so partial mirrors and offline caches are safe.
- **Per-version immutability.** A `<id>/<version>/` directory — mod or pack —
  is written once and never edited. A fix is a new version, never a rewrite of a
  published one — the same discipline the loader's mod identity depends on.
  Yanking is a flag in `meta.json`, not a deletion (the artifact stays addressable
  so existing installs still resolve).
- **Provenance travels with the version, not the mod.** Rights can differ
  between versions (a relicense, a new asset), so `provenance.json` is per-version
  and a version whose provenance is unresolved is not indexed. For a pack this
  covers both its authored data and its composition-distribution permission (§5.4).
- **The index is a signed Merkle tree.** `index.json` carries the digest of every
  mod and pack `meta.json`, and each `meta.json` carries the digest of every
  version's manifest (`manifest.json` or `pack.json`), `provenance.json`, and
  `artifact.json`. A client verifies one signature over the root and then trusts
  any subtree by digest, so partial mirrors and offline caches are safe.
- **Per-version immutability.** A `<id>/<version>/` directory — mod or pack —
  is written once and never edited. A fix is a new version, never a rewrite of a
  published one — the same discipline the loader's mod identity depends on.
  Yanking is a flag in `meta.json`, not a deletion (the artifact stays addressable
  so existing installs still resolve).
- **Provenance travels with the version, not the mod.** Rights can differ
  between versions (a relicense, a new asset), so `provenance.json` is per-version
  and a version whose provenance is unresolved is not indexed. For a pack this
  covers both its authored data and its composition-distribution permission (§5.4).

## 4. Resolution model

The loader resolves a wanted set against the index alone (no artifact download
needed to resolve):

1. Read `index.json`, verify its signature, walk to each wanted `meta.json` by
   digest.
2. For each candidate version, filter by `compatibility.ravel`,
   `compatibility.minecraft`, and required `sides`.
3. Solve the dependency constraints (semver ranges over Ravel-native ids) to a
   single version per mod, or report the conflict with the constraint that
   caused it.
4. Only then fetch artifacts by digest and verify each against its address.

Dependencies are expressed in the manifest over Ravel-native ids, so the resolver
never sees an upstream name. A mod that depends on a capability the SDK
provides declares no dependency at all — the platform is not a mod (this is
the M7 "an entry that reduces to nothing is a result" rule surfacing in the
public model: absorbed behaviour ships zero mods).

## 5. Packs (modpack compositions)

A **mod** is one mod. A **pack** is a curated composition: a pinned set of exact
mod versions plus the pack's own authored data — config overrides, scripts, quest
definitions, menu and branding assets — that no individual mod carries. ATM11 is
the case in point: the mods exist on their own, and the pack is the selection
*plus* the overrides layered on top. The registry models the pack as a
first-class, separately-published unit, because it is the thing a user actually
installs and because its rights profile is different from any mod it contains.

### 5.1 What a pack is (and is not)

- A pack **ships no Rust of its own.** Its "behaviour" is *selection* (which mod
  at which exact version) and *data* (pack-owned overrides). So it is not held to
  `#![forbid(unsafe_code)]` — it has no code — but it **is** held to provenance
  and rewrite-integrity over its data.
- A pack **pins, it does not range.** Where a mod declares dependency *ranges*, a
  pack declares an exact version per member, so an install is reproducible: the
  same pack manifest resolves to the same artifacts by digest every time. This is
  a lockfile, signed and indexed.
- A pack **may override a member it does not own.** A config or script override
  changes how a referenced mod behaves without modifying or republishing that mod.
  The override is pack-owned data; the mod is untouched and still resolves to its
  own digest.

### 5.2 Storage layout

The `packs/` index tree and the shared `artifacts/` store are shown in §3.
Members are referenced by `(mod-id, exact-version)`, and the resolver reaches each
member's own `manifest.json`/`artifact.json` by the digests already in the index —
the pack stores references, never copies of member artifacts. The pack-owned
override bundle is a content-addressed blob in the same `artifacts/` store as
everything else.

### 5.3 A pack manifest (new schema, `urn:ravel:schema:pack:1`)

Additive to the existing package schema rather than a change to it:

- `schemaVersion`, `id` (a pack id, its own namespace, still no upstream name),
  `version`, `authors`, `license`.
- `compatibility`: the `ravel` and `minecraft` the pack targets (the resolver
  checks each member's compatibility is a superset).
- `members`: an array of `{ id, version, digest }` — exact pins, digest-bound.
- `overrides`: an array of `{ target, path, kind }` where `target` is a member id
  (or the pack itself), `kind` is `config` / `script` / `quest` / `resource` /
  `branding`, and `path` is where the override lands. Overrides carry no member
  code — only data.
- `provenance`: as for a mod, but covering the pack's authored data and — the part
  unique to a pack — **the right to distribute this composition.** A pack that
  references mods it did not author needs distribution permission for the *set*,
  not merely clearance of each part.

### 5.4 Why the composition needs its own provenance

This is the ATM11 lesson made general. The private workspace already treats
ATM-authored overrides as a distinct category — "counted and never named," with a
per-mod flag for whether the pack overrides that mod — precisely because pack data
has a different owner and a different licence from the mods it sits on. So a pack's
`provenance.json` resolves two things a mod's does not:

1. **The authored data**: configs, scripts, quests, branding the pack ships — its
   own rights surface (code/behaviour/assets/names/branding/permissions/
   attribution/licence), same schema, different owner.
2. **The composition right**: permission to distribute *this selection* of these
   mods together. Curating a set can carry obligations (e.g. a member's licence
   may permit redistribution only under conditions) that no single member's
   clearance settles.

A pack whose composition right or authored-data provenance is `unresolved` is not
indexed — the same publication bar as a mod, applied to the layer above it.

### 5.5 What this deliberately keeps out of the pack

- **No member code in the pack artifact.** Overrides are data only; a pack cannot
  smuggle a patched mod as an "override." The admission gate rejects an override
  bundle containing executable artifacts.
- **No re-pinning of the platform.** SDK-absorbed behaviour is not a member;
  a pack lists mods, and the platform is not a mod (§4).

## 6. Admission gate (what the registry runs before it indexes a version)

A submission is indexed only if all pass, mirroring the private workspace's gate
so the public bar is the same one the counterparts were built under:

1. **Manifest validates** against `urn:ravel:schema:package:1` (mods) or
   `urn:ravel:schema:pack:1` (packs).
2. **Artifact digest** matches the manifest's declared source digest; for a pack,
   every `members[].digest` resolves to an already-indexed mod version and the
   override bundle matches its declared digest.
3. **Rewrite-integrity**: no upstream identifier in the manifest or artifact
   (U1/N1 over the published identifier set; the registry ships the lexicon and
   the normalization it holds names to).
4. **Provenance resolved**: every surface in `provenance.json` is `cleared`,
   `not-applicable`, or carries an explicit, reviewed permission — never
   `unresolved`. **This is the gate the private workspace cannot pass today**,
   because names stay `unresolved` there; clearing it is the act of publication.
5. **Signature** by a registered author key.
6. **`#![forbid(unsafe_code)]`** and the workspace lint bar hold on the artifact's
   source (the registry indexes mods that meet the same safety contract the
   loader guarantees).

## 7. The migration boundary — what is temporary

The private compatibility workspace — the separate repository beside this one
that `docs/architecture/public-private-boundary.md` describes without naming it,
for the reason given there — is **migration scaffolding**, not a prototype of
this registry. During the rewrite of the pinned pack it holds directories that
must **not** appear in, or map one-to-one onto, the official registry:

| private (temporary) | role during migration | fate at publication |
| --- | --- | --- |
| `inventory/atm11-0.2.1/` | the rights-aware work ledger (references, graph, surfaces, rights, plans) derived from the pinned fixture | **stays private forever.** It records upstream identifiers, which the public registry must never hold. |
| `waves/`, `routing.json` | dependency-ordered work scheduling for the rewrite | **discarded.** Scheduling is a migration concern; the registry indexes finished mods, not work order. |
| `specifications/` | reader-authored behaviour specs, one per entry | **stays private.** They are derived from reading upstream source; they inform the rewrite but are not shipped. |
| `provenance/<wave>/RVL-NNN.json` | clean-room provenance keyed by internal slot id | **transformed, not copied.** The per-version `provenance.json` in the public registry is re-keyed to the public mod id and must reach `cleared`/permissioned on the `names` surface first. |
| `counterparts/rvl-NNN-counterpart/` | the Rust mods under internal ids, with `aliases.json`, `NOTES.md` (scaffolding), scrubbed metadata | **renamed and re-homed.** Each becomes a public mod under its chosen Ravel-native id; `NOTES.md` and `aliases.json` are migration artifacts and do not ship. `aliases.json` save-compat mappings inform migration tooling, then are dropped. |
| `checks/` | the private gate (rewrite-integrity, provenance, reads-observation) | **the source of the public admission gate.** The rules move to the registry's admission service; the fixture-reading halves stay private. |
| `RVL-NNN` canonical ids | opaque internal slot numbers | **never public.** The public id is chosen from the lexicon; the mapping RVL-NNN→public-id lives only in the private workspace. |
| the pinned pack's ATM-owned overrides (configs, KubeJS scripts, quests, menus, branding) | during migration these are **counted and never named** — a per-mod flag records whether the pack overrides a given mod; the override *contents* are never tracked | **not reproducible by Ravel and not shipped by Ravel.** A Ravel pack that reconstructs an equivalent experience is a *new composition* under §5 with its own authored overrides and its own composition-distribution permission — it does not copy or redistribute the ATM-owned data. The private flag informs which members need override behaviour; the data itself stays out of both repositories. |

The one tree that legitimately crosses is `proposals/` (SDK capability gaps),
which the boundary doc already routes to the public repo via M6-W2. Everything
else in the private workspace either stays private or is transformed — never
copied — into the public shape above.

Two temp-directory conventions used *during* migration and expected to disappear:

- **`specifications/m6-foundation/`** is currently the single spec home only
  because `checks/verify-counterparts.sh` hardcodes that path. That is an M6-era
  artifact: the foundation wave is frozen at seven entries, and M7 entries are
  being filed there as an expedient. A per-wave layout
  (`specifications/<wave>/`) with the check scanning across waves is the correct
  private shape; either way, none of it ships.
- **`scratch/`, `artifacts/`, `extracted/`** are ignored acquisition scratch in
  the private workspace and have no public counterpart.

### 7.1 Gitignored working directories (the original material we translate from)

This is the working tree §3 shows alongside the registry tree — expanded here
with the reasons and the enforcement. The rewrite needs the original mods on disk
to read from, but that material is rights-restricted and must never be tracked in
*either* repository — not the public registry and not the private workspace. It
lives in **gitignored** working
directories that exist only on the machine doing the rewrite, are never
advertised in any index, and never become a registry entry. They are genuinely
useful — indispensable, while the rewrites are in progress — but they are working
state, not deliverables.

Where they live (all under the public repo's ignored `tests/fixtures/local/`,
which the boundary doc already reserves as the one place authorized pinned
material may be written):

| gitignored path | what it holds | why it never ships |
| --- | --- | --- |
| `tests/fixtures/local/atm11-0.2.1/` | the pinned pack as acquired: the mod JARs, the pack archive, ATM-owned overrides | official/third-party binaries and ATM-authored data; tracking any of it would cross the licence boundary |
| `tests/fixtures/local/decompiled-mods/<jarstem>/` | decompiled/extracted source a reader reads to write a behaviour specification | source-derived; the *specification* is tracked (privately), the source it was read from is not |
| private workspace `artifacts/`, `extracted/`, `scratch/` | acquisition downloads, unpacked trees, and per-run scratch | intermediate acquisition state; reserved by `.gitignore` so it has an ignored place to land |

Two rules keep these from leaking, and both are enforced rather than asserted:

- **The public repo never names the private workspace or its fixture contents.**
  `ravel:verify-scaffold` sweeps every tracked file for that name and for any
  manifest whose path dependency resolves outside the checkout, and fails the gate
  on either. So a stray reference to one of these directories from tracked code is
  caught, not trusted to reviewer vigilance.
- **The private workspace tracks identifiers, never contents.** Its
  `checks/verify-boundary.sh` refuses the file kinds and path shapes that would
  carry pack contents even if a path slipped through, and the reads-observation
  check confirms the tooling only ever *lists and counts* these archives (plus a
  four-byte type probe) rather than reading their bodies into anything tracked.

The migration relationship, stated once: a reader opens the gitignored original,
writes a tracked (private) behaviour specification that names nothing upstream, an
implementer writes the Rust mod from that specification alone, and the original is
never copied forward. The gitignored directory is the *input* to that process and
the first thing that would betray it if it were ever tracked — which is exactly
why it is ignored and swept for, not merely left out of the registry layout.

## 8. What this design does not settle

The largest deferral is set by `SPEC.md §12.3`, not here: **the concrete
distribution and publisher-identity mechanism is an M13 decision gate.** SPEC
records three candidates — a Go-module-style design over an append-only
transparency log, TUF + Sigstore, and an AT-Protocol-style design (noted there as
weakening the author-owned-store property) — and this doc selects none of them.
Everything below is subordinate to that gate:

- **Distribution mechanism and publisher identity**: deferred to the M13
  candidates above. The requirements are fixed by §12.3 (author-owned releases,
  portable identity, immutable content-hash addressing, moderation at the index);
  the mechanism is not.
- **Transport and hosting** (static object store vs. an API service): the layout
  above works as flat signed files behind any static host; an API is an
  optimization, not a requirement.
- **Federation/mirroring protocol**: the content-addressed + Merkle-signed design
  makes mirrors safe, but the discovery protocol between mirrors is unspecified.
- **The public id lexicon's governance**: who admits a new name, and how
  collisions between independent authors are arbitrated.

These are deferred deliberately; the core claim of this doc is the *shape*
(content-addressed artifacts, signed Merkle index, per-version immutable
provenance, an admission gate equal to the private rewrite-integrity bar) and the
migration boundary (what is temporary and what is transformed rather than copied).
