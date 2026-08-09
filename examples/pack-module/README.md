# Pack example

M4-W5's pack example is
[`crates/ravel-pack/examples/example_pack.rs`](../../crates/ravel-pack/examples/example_pack.rs).
Run it with

```console
cargo run -p ravel-pack --example example_pack -- <a directory to write into>
```

and it declares a two-mod pack, pins both members at exact versions, states the
pack's configuration layer, supplies a recipe and refers to a texture, declares a
progression with a gate, validates all four closures, and writes the pack.

## Why it is not a crate in this directory

Every crate under `examples/` declares `ravel-sdk` and nothing else, and
`tests/conformance/tests/sdk_boundary.rs` enforces that, because these are the model
manifests a **mod** author reads. A pack author is a different role with a different
dependency: `SPEC.md` §4.3 has them selecting mods, pinning versions and validating
closure, which is `ravel-pack`. Putting `ravel-pack` into a manifest this directory
presents as an author's would teach the wrong dependency and would mean widening a
boundary check to accommodate the example — the opposite of what the check is for.

`SPEC.md` §3.6's second pack-authoring layer, a compiled Rust module for pack
behaviour that cannot be expressed declaratively, is a mod-shaped crate and would
belong here. Nothing needs one yet, so this directory holds no crate.
