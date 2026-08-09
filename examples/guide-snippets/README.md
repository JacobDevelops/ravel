# Guide snippets

Every Rust block in [`docs/guide/`](../../docs/guide) is a copy of a run of lines
in [`src/lib.rs`](src/lib.rs). This crate is where they compile.

It is not a mod anyone would install. It exists because markdown compiles nowhere
and the guide is prose full of code, so without it the guide's snippets would be
a claim nothing recomputes.

Two things make it worth having rather than pasting the code into a doctest:

- It lists `ravel-sdk` and nothing else, like every other example, so a block that
  only compiles with the workspace in scope fails here. That difference is not
  theoretical — the guide was written because a documented example turned out to
  need `serde_json`, which a mod author cannot name.
- Its tests drive each snippet rather than only building it. The content snippet
  claims the entries it declared through a real `Registrar`, the client-only
  snippet builds nothing on a server side, the saved-data snippet carries a
  version-1 payload across its own migration, and the settings snippet resolves
  to the author's default. A block that compiles and does something other than
  what the prose beside it says is the failure a compile check cannot see.

`tests/conformance/tests/guide_snippets.rs` holds the guide to this crate: a
block that drifts from the module it was copied from fails the gate.
