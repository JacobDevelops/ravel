# Stateful mod example

A configurable journal with versioned world data and an entries service.
Start with [`minimal-mod`](../minimal-mod) for a smaller complete example.

| Surface | What the mod writes | Where it is |
| --- | --- | --- |
| Settings | a `configuration.schema.json` document and `Mod::configure` | `SETTINGS` and `configure` in [`src/lib.rs`](src/lib.rs) |
| World data | a version list, a `MigrationChain`, `Mod::open_saved_data` and `Mod::save_saved_data` | `VERSIONS`, `Journal::chain` |
| A service | a `ServiceDeclaration` and `Mod::answer_service` | `entries_service`, `answer_service` |

## The rules worth taking from it

Settings are JSON declarations. `configure` receives defaults resolved with
pack overrides and valid player values.

Each declared world-data version must remain readable. `MigrationChain` upgrades
older payloads in order; an unreadable payload refuses the world without
partially changing the journal.

The provider owns the `stateful:entries` service name. Requests and answers are
JSON, so consumers need a package dependency rather than this mod's Rust types.

## Installing it

[`package.json`](package.json) and [`provenance.json`](provenance.json) are what
make this crate a package rather than only a crate — the metadata the loader
validates before any of the code above runs. Copy the directory whole into an
instance's `ravel/mods` and `ravel-cli host build` compiles it into that
instance's host.

Its `savedDataVersions` is the one field the table above forces: a mod that owns
world data states the same version list in its manifest that
`Mod::saved_data_declaration` states in its code, and
`tests/conformance/tests/integration/example_package.rs` compares the two. That file also
recomputes the digests, the file list, and whether `"id"` still agrees with
`#[declare_mod]`. Any edit to a declared file invalidates the manifest; refresh
it with

```console
cargo xtask package digests examples/stateful-mod
```

which rewrites the digests and nothing else, and says so when there was nothing
to do.
