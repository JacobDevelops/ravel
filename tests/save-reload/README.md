# Save and reload tests

World persistence and saved-data migration are tested from
[`tests/conformance/tests/integration/saved_data.rs`](../conformance/tests/integration/saved_data.rs),
against the world fixtures in
[`tests/fixtures/public/worlds/`](../fixtures/public/worlds/).

They are there rather than here because this directory sits outside the
workspace `members` globs. A crate in it would need a `members` entry and a
`ravel-xtask` scaffold entry to buy nothing, since the saved-data tests compose
the same crates every other conformance test does.
