# Stateful mod example

A Ravel mod with the kinds of state that outlive a single call: settings a player
or a pack chooses, versioned world data that an older save migrates into, and a
service other mods may call. It is not every kind a mod can hold — a client-only
value and a packet's contract are state too, and `test-mods/client-only` and
`test-mods/networking` are where those live. The table below is the list; a
sentence counting it would be wrong the next time the list grew, which is how the
sentence this one replaced went stale.

`examples/minimal-mod` is the smallest complete mod. Read that one first; this
one is what to read when you need to remember something.

| Surface | What the mod writes | Where it is |
| --- | --- | --- |
| Settings | a `configuration.schema.json` document and `Mod::configure` | `SETTINGS` and `configure` in [`src/lib.rs`](src/lib.rs) |
| World data | a version list, a `MigrationChain`, `Mod::open_saved_data` and `Mod::save_saved_data` | `VERSIONS`, `Journal::chain` |
| A service | a `ServiceDeclaration` and `Mod::answer_service` | `entries_service`, `answer_service` |

## The rules worth taking from it

**A setting is a document, not a screen.** ADG-6 declares configuration in data
and draws every Ravel-owned screen in Java, so an author writes JSON and reads
resolved values. The three layers — the author's `default`, a pack's override,
the player's file — are resolved before `configure` is called, and a player value
that is unknown or illegal has already been dropped.

**A world data version is a promise to be able to read it.** Declaring version 1
means a world written at version 1 still opens. `MigrationChain` is how: one step
per version, run in order. A step that cannot read the shape it was handed
refuses, the world is withheld, and the mod is left holding nothing — a save that
will not open is recoverable, and a save that opened empty is not.

**A service is named by whoever answers it.** `stateful:entries` is in this mod's
namespace, so no other mod can claim it, and the request and the answer are JSON
rather than Rust types. A consumer therefore depends on the version range its
package manifest declares rather than on this mod's build.

`Cargo.toml` beside this file lists one dependency, `ravel-sdk`, which is what a
mod author's manifest lists.

## Installing it

[`package.json`](package.json) and [`provenance.json`](provenance.json) are what
make this crate a package rather than only a crate — the metadata the loader
validates before any of the code above runs. Copy the directory whole into an
instance's `ravel/mods` and `ravel-cli host build` compiles it into that
instance's host.

Its `savedDataVersions` is the one field the table above forces: a mod that owns
world data states the same version list in its manifest that
`Mod::saved_data_declaration` states in its code, and
`tests/conformance/tests/example_package.rs` compares the two. That file also
recomputes the digests, the file list, and whether `"id"` still agrees with
`#[declare_mod]`. Any edit to a declared file invalidates the manifest; refresh
it with

```console
cargo xtask package digests examples/stateful-mod
```

which rewrites the digests and nothing else, and says so when there was nothing
to do.
