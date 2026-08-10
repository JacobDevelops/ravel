# What "conforming" is checked to mean

These are the criteria the repository's conformance suite actually enforces. A
mod that satisfies them is conforming; a mod that reads well and violates one of
them is not.

**The manifest.** Validates against `schemas/package.schema.json`, refuses every
undefined property at every level, declares at least one source file, and every
declared file's digest matches the bytes on disk. Provenance record present and
its digest matches.

**One dependency.** The crate's `Cargo.toml` names `ravel-sdk` and nothing else
from the Ravel workspace. This is checked, not advised: the boundary suite reads
every example manifest and fails when a second entry appears, and the
compile-fail cases in
[`examples/minimal-mod/src/boundary.rs`](../../examples/minimal-mod/src/boundary.rs)
prove the forbidden crates are unreachable rather than merely unused.

**Content.** Every entry claimed in the registration window and nowhere else. No
entry claimed twice by one declaration. No two declarations of one mod producing
the same generated file. No mod supplying one resource path twice. Tag members of
the registry the tag groups. Generation byte-identical across repeats and across
declaration orders, every generated document legal JSON ending in exactly one
newline.

**Sides.** A mod declaring one side runs no code on the others — not "hides its
effects", runs nothing. Client-only state is not built on a server side.

**Packets.** Refused at every truncation length, refused with trailing bytes,
refused across a version change, refused across a contract change, refused on the
wrong side before any reading code runs.

**World data.** A created world reloads with the state it saved. Every byte a save
emits is admissible again unchanged. An older world names every version it steps
through. A world from a newer build cannot open. A world holding nothing you own
initializes fresh. A refused world never reaches the mod that owns its data, and
every refusal is decided before any mod runs.

**Settings.** A mod that declares settings and never reads them is refused rather
than ignored.

**Services.** A provider starts before its consumer. A service nothing answers
names the mod that wanted it and starts nothing. Mutual requirement is refused
with both named.

**Diagnostics.** No two refusals read the same way, and no published message names
a filesystem path or a credential. A message your mod wrote is treated as
untrusted text and has its paths and secrets stripped before publication. Do not
put a path in a refusal detail expecting a player to see it.

## The pitfalls these catch

- **Claiming outside `register`.** Refused as late. Hold no registrar.
- **Declaring settings without implementing `configure`.** Refused.
- **Reading a saved payload as though it were current.** Silently wrong in a
  running game. Apply the chain across `SavedDataPlan::steps`.
- **Declaring a saved-data version with no step into it.** The whole build is
  refused, before any payload is read, rather than surprising a player later.
- **Disagreeing with yourself about versions.** `savedDataVersions` in the
  manifest and `saved_data_declaration` in the code are two statements of one
  fact.
- **A stale digest.** Any edit to a declared source file invalidates the
  manifest. `cargo xtask package digests` refreshes them.
- **Uppercase hex, or an uppercase mod id.** Both refused.
- **Assuming everything in the package is integrity-checked.** Only
  `source.files` and the provenance record are. List your schemas and resources
  there too.
- **Reaching for `serde_json`.** It is not a dependency of your crate and cannot
  be named from it. Call the method on the value, or name `ravel_sdk::JsonValue`.
- **Naming a tag member the owning mod never declared.** Refused even when that
  mod is present, at pack validation.
- **A crate that never writes `export_mod!`.** Your own tests construct your mod
  by name and pass; the host built on a player's machine cannot, because nothing
  in your crate says which value to construct. It is caught — the build refuses
  and names your crate — but it is caught on the player's machine rather than in
  your suite, so write the line when you write the type.
- **Expecting a rendering path.** There is none.
- **Expecting registries beyond block, item, recipe, command.** There are none.
- **A manifest `id` that differs from `#[declare_mod(id = …)]`.** *Nothing checks
  this for your package.* The loader takes identity, sides and saved-data
  versions from the manifest; your content namespace, service names and packet
  ids come from the attribute. A mismatch produces a mod whose content is
  namespaced under a name the loader never heard of, and no check reports it.
  Keep the two strings identical. The repository recomputes the agreement for its
  own `examples/minimal-mod`, so the model you copy is at least correct; nothing
  extends that to your crate.
