# Counterpart package template

The skeleton a new counterpart starts from. It is derived from the twelve sealed packages under
`mods/`, not designed ahead of them: every file below is a file all twelve carry, every key below is
a key all twelve set, and nothing here is a field the three counterpart checks do not already read
or hash.

The authoring order is `records/counterpart-checklist.md`. This file is the shape; that one is the
sequence.

## The files

A counterpart package is a directory `mods/rvl-NNN-counterpart/` holding exactly these files and no
others. `checks/verify-counterpart-packages.sh` fails on a file that is neither listed in
`package.json` nor one of the two records the manifest points at, so an extra file is a finding
rather than an oversight.

| Path | What it is |
| --- | --- |
| `Cargo.toml` | The crate, named for the package directory. |
| `package.json` | The public package manifest, with a sha256 per source file. |
| `provenance.json` | What was read to write this package, and the rights that clears. |
| `aliases.json` | Save-compatible spellings this package keeps, or none. |
| `assets-provenance.json` | What art, sound and texture ships. Normally none. |
| `NOTES.md` | The section table and the declared behaviour changes. |
| `src/lib.rs` | The entry: `impl Mod`, `export_mod!`, and the module list. |
| `src/*.rs` | One module per area the specification separates. |

Adding a module means adding its `sha256` row to `package.json` in the same change. The digest check
recomputes every row, and an unlisted file fails on its own rule.

## `Cargo.toml`

```toml
[package]
name = "rvl-NNN-counterpart"
version = "0.1.0"
edition = "2024"
license = "MIT"

[dependencies]
ravel-sdk.workspace = true
```

`ravel-sdk` is the whole of what a counterpart depends on, except where the specification says the
entry loads only if another wave entry does. A dependency on a sibling counterpart is declared here
and recorded in `provenance.json` under the `counterpart` category, which is what RVL-002 and
RVL-086 do.

## `package.json`

Every path under `source.files` is hashed and compared by
`checks/verify-counterpart-packages.sh`, and `provenance.sha256` is hashed the same way. The list is
sorted by path.

```json
{
  "schemaVersion": 1,
  "id": "rvl_nnn_counterpart",
  "version": "0.1.0",
  "displayName": "RVL-NNN Counterpart",
  "description": "Ravel counterpart for RVL-NNN: what the entry does, in the words of the specification.",
  "compatibility": {
    "ravel": ">=0.0.0, <1.0.0",
    "minecraft": ["1.21.1"]
  },
  "sides": ["client", "integrated-server", "dedicated-server"],
  "configurationSchemas": [],
  "authors": [{ "name": "Ravel registry maintainers" }],
  "license": "MIT",
  "source": {
    "repository": "urn:ravel-registry:source",
    "files": [
      { "path": "Cargo.toml", "sha256": "…" },
      { "path": "NOTES.md", "sha256": "…" },
      { "path": "aliases.json", "sha256": "…" },
      { "path": "assets-provenance.json", "sha256": "…" },
      { "path": "src/lib.rs", "sha256": "…" }
    ]
  },
  "provenance": {
    "record": "provenance.json",
    "sha256": "…"
  }
}
```

`sides` states the sides the specification puts the entry on, not the three by default. `id` is the
crate name with underscores, and it is the same string `MOD_ID` carries in `src/lib.rs`; the entry's
own test asserts they agree.

The digests are written last, after every other file is final. From the repository root, refresh
the manifest and then check it with the package producer:

```console
mise exec -- cargo run -q -p ravel-xtask -- package digests registry/mods/rvl-NNN-counterpart
mise exec -- cargo run -q -p ravel-xtask -- package digests registry/mods/rvl-NNN-counterpart --check
```

## `provenance.json`

What was read to write this package. The sealed packages use five categories — `specification`,
`metadata`, `published-api`, `counterpart` and `compiled-artifact` — and every one of them declares
`implementationMethod: "specification-implemented"`, because the specification is the only
description of the behaviour that was used.

The two categories `checks/verify-provenance.sh` keys on are `source-code` and `compiled-artifact`,
and they belong to the wave-level record at `provenance/m6-foundation/RVL-NNN.json`, not here: that
check requires a `black-box` record to carry a `compiled-artifact` source with a digest and no
`source-code` source, and any other method to carry a `source-code` source with a digest. A
counterpart package's own record is hashed by
`checks/verify-counterpart-packages.sh` and read by nothing else, so the wave record is where the
digest-bearing source belongs and this one names what the implementer actually had in hand.

```json
{
  "schemaVersion": 1,
  "subject": {
    "kind": "ravel-package",
    "id": "rvl_nnn_counterpart",
    "version": "0.1.0"
  },
  "implementationMethod": "specification-implemented",
  "sources": [
    {
      "category": "specification",
      "locator": "specifications/m6-foundation/RVL-NNN.md",
      "use": "The behaviour specification this package implements. Nothing upstream was opened, decompiled or read to write this package.",
      "producedBy": "M6-W3"
    },
    {
      "category": "metadata",
      "locator": "provenance/m6-foundation/RVL-NNN.json",
      "revision": "sha256 …",
      "use": "The rights record the specification was written under, which clears code and behaviour and leaves names unresolved.",
      "producedBy": "M6-W3"
    },
    {
      "category": "published-api",
      "locator": "ravel-sdk",
      "use": "The whole of what this package names. It lists no other Ravel crate.",
      "producedBy": "Ravel"
    }
  ],
  "rights": [
    { "surface": "code", "status": "cleared" },
    { "surface": "behaviour", "status": "cleared" },
    { "surface": "names", "status": "not-applicable" },
    { "surface": "textures", "status": "not-applicable" },
    { "surface": "models", "status": "not-applicable" },
    { "surface": "sounds", "status": "not-applicable" },
    { "surface": "branding", "status": "not-applicable" }
  ]
}
```

All seven surfaces are stated. A surface left out is a surface nothing decides. `names` is
`not-applicable` because a counterpart carries none of the original's names; if it keeps one for
save compatibility, that spelling is declared in `aliases.json` and evidenced in the specification,
and this surface says so instead.

## `aliases.json`

Empty unless the specification states a save-compatibility requirement. The shape is enforced
exactly by `alias_shape_valid` in `checks/verify-counterparts.sh`: three keys and no more, a
canonical id matching `RVL-[0-9]{3}` and agreeing with the directory name, and unique sources and
targets.

```json
{
  "schemaVersion": 1,
  "canonicalId": "RVL-NNN",
  "mappings": []
}
```

A mapping is admitted only when the specification carries the line verbatim:

```text
save-compatible alias: <source> -> <target>
```

Both sides match `^[a-z][a-z0-9]*(?:[-_.][a-z0-9]+)*:[a-z0-9][a-z0-9_./-]*$`. A proved mapping's
`source` is the one place a counterpart may carry an upstream spelling, and the exemption is
granted only after the whole declaration validates, so a malformed `aliases.json` loses the
exemption and the source then fails rule U1.

## `assets-provenance.json`

```json
{
  "schemaVersion": 1,
  "subject": "RVL-NNN",
  "assets": [],
  "notes": "This package ships no art, no sound and no texture. Nothing upstream is tracked or shipped and nothing was rewritten from reference, so there is no asset to record."
}
```

An entry that declares a sound through `ravel_sdk::audio` still records no asset: the declaration
carries bytes the package supplies and never reads, which is a fact for `NOTES.md` rather than an
asset with provenance.

## `NOTES.md`

The reviewer's entry point. Its two load-bearing parts are the section table and the declared
behaviour changes; everything else is a heading that earns its place or is left out.

```markdown
# RVL-NNN counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-NNN.md`, written against
`ravel-sdk` and nothing else. No upstream source was opened, decompiled or read to write it.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side and lifecycle | implemented | `src/lib.rs` |
| §4 the mechanism | declarable only | — |
| §6 registered content | declared as names only | `src/lib.rs` |

## Behaviour changes, each declared rather than incidental

- **A one-line statement of the change (§N).** What the original does, what this does instead, and
  why the difference is required rather than convenient. A change with no reason is a defect.

## Declarable only, and why

- **The area (§N).** The seam Ravel does not publish at the reviewed revision. None is stubbed: the
  package declares the shape where a shape is declarable and carries no pretend implementation.

## Evidence

Every rule with a number, an ordering, a refusal or a format has an inline test.
`checks/verify-counterpart-build.sh` runs them. The ones proved by mutation rather than by reading
are named here, each with the failure the mutation produced.
```

Every row of the section table resolves to one of four verdicts, and the four are what the evidence
schema counts:

- **implemented** — the rule runs here. Counts as exercised.
- **implemented, with the declared behaviour change** — likewise, and the change is in the list
  below the table.
- **declarable only** — no Ravel seam exists, the shape is declared, nothing pretends. Counts as
  declarable-only.
- **not reached** — the specification excludes it or marks it not carried. Outside the denominator
  entirely, and the reason is stated in the row.

Optional headings the sealed packages add when they have something to say: compatibility-required
spellings this package refuses to invent, declined outright, gaps found in the published surface,
bounds against hostile input, host blockers.

## `src/lib.rs`

```rust
#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-NNN.
//!
//! `NOTES.md` beside this file says which sections of the specification are
//! carried here and which are left declarable only because Ravel publishes no
//! seam for them.

use ravel_sdk::content::ContentDeclaration;
use ravel_sdk::prelude::*;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_nnn_counterpart";

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    declaration: Result<ContentDeclaration, ModError>,
}

impl Entry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            declaration: Ok(ContentDeclaration::new(Self::declared_id().clone())),
        }
    }
}

impl Default for Entry {
    fn default() -> Self {
        Self::new()
    }
}

#[declare_mod(id = "rvl_nnn_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        self.declaration
            .as_ref()
            .map(|declaration| Some(std::borrow::Cow::Borrowed(declaration)))
            .map_err(Clone::clone)
    }
}

export_mod!(Entry::new());

#[cfg(test)]
mod tests {
    use super::{MOD_ID, __ravel_entry};
    use ravel_sdk::prelude::*;

    #[test]
    fn the_host_builds_this_package_under_its_declared_identity() {
        assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
    }
}
```

`export_mod!` takes either a value or a closure over the side, depending on whether the entry needs
to know its side to construct. Both forms are in use: the value form where the entry is
side-agnostic, `export_mod!(|side| Entry::new(side))` where it is not.

Only the trait methods the entry actually implements are written. A method left at its default is
left out, not overridden with a body that does nothing — that is one of the two review defects the
checklist names.

## Sealing

Every file above is written before anything is sealed, and the digests are refreshed last. Then the
package is committed by path, so a shared working copy does not sweep up other work:

```console
jj commit mods/rvl-NNN-counterpart specifications/m6-foundation/RVL-NNN.md \
  provenance/m6-foundation/RVL-NNN.json checks/counterpart-lexicon.txt
```
