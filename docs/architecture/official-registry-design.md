# The official Ravel registry — design

Status: design proposal. This describes the shape the public Ravel registry
should take.

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
claims before it runs. It answers three questions and nothing else:

1. **Discovery** — what mods and packs exist, at what versions, for which
   Minecraft and Ravel versions, and on which sides.
2. **Resolution** — given a set of wanted mods (or a pack), which exact versions
   satisfy their mutual compatibility and dependency constraints.
3. **Integrity** — the artifact you downloaded is byte-for-byte the one the
   registry indexed, bound by digest.

It is deliberately **not** a build system, a general package host for arbitrary
code, or a social platform. It indexes signed Ravel mods and
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

A mod `id` identifies the published Ravel mod.

The other publishable unit is a **pack** (§5): a composition of pinned mods plus
pack-owned data. Mods and packs sit in sibling index trees (`mods/` and `packs/`)
and share the artifact store and the admission gate.

## 3. Directory / storage layout

The registry tree is what ships and what a mirror serves.

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
          provenance.json     # per-version package provenance metadata
          artifact.json       # digest, size, download locator, signature
    packs/                    # pack compositions (see §5)
      <pack-id>/
        meta.json             # id, display name, authors, license, versions
        <version>/
          pack.json           # the validated pack manifest (schema urn:ravel:schema:pack:1):
                              #   pinned members + declared overrides + compatibility
          provenance.json     # per-version pack provenance metadata
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

Design properties, each with the reason:

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
- **Provenance travels with the version, not the mod.** Per-version package
  metadata is digest-bound alongside the manifest and artifact.

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
installs.

### 5.1 What a pack is (and is not)

- A pack **ships no Rust of its own.** Its "behaviour" is *selection* (which mod
  at which exact version) and *data* (pack-owned overrides). So it is not held to
  `#![forbid(unsafe_code)]` — it has no code.
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

- `schemaVersion`, `id` (a pack id, its own namespace),
  `version`, `authors`, `license`.
- `compatibility`: the `ravel` and `minecraft` the pack targets (the resolver
  checks each member's compatibility is a superset).
- `members`: an array of `{ id, version, digest }` — exact pins, digest-bound.
- `overrides`: an array of `{ target, path, kind }` where `target` is a member id
  (or the pack itself), `kind` is `config` / `script` / `quest` / `resource` /
  `branding`, and `path` is where the override lands. Overrides carry no member
  code — only data.
- `provenance`: per-version metadata bound to the pack's authored data digest.

### 5.4 What this deliberately keeps out of the pack

- **No member code in the pack artifact.** Overrides are data only; a pack cannot
  smuggle a patched mod as an "override." The admission gate rejects an override
  bundle containing executable artifacts.
- **No re-pinning of the platform.** SDK-absorbed behaviour is not a member;
  a pack lists mods, and the platform is not a mod (§4).

## 6. Admission gate (what the registry runs before it indexes a version)

A submission is indexed only if all pass:

1. **Manifest validates** against `urn:ravel:schema:package:1` (mods) or
   `urn:ravel:schema:pack:1` (packs).
2. **Artifact digest** matches the manifest's declared source digest; for a pack,
   every `members[].digest` resolves to an already-indexed mod version and the
   override bundle matches its declared digest.
3. **Signature** by a registered author key.
4. **`#![forbid(unsafe_code)]`** and the workspace lint bar hold on the artifact's
   source (the registry indexes mods that meet the same safety contract the
   loader guarantees).

## 7. What this design does not settle

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

These are deferred deliberately; the core claim of this doc is the shape:
content-addressed artifacts, a signed Merkle index, per-version immutable
metadata, and integrity and safety admission checks.
