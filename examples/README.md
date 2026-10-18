# Examples

Mods and packs written against nothing but the public surface. An example's
manifest is a mod author's manifest, which is what makes these crates worth
compiling in the workspace rather than pasting into documentation: a facade that
has drifted stops building here first.

- [`minimal-mod`](minimal-mod) — the smallest complete mod, M4-W1's
  compile-pass/fail suite for the author boundary, and the package manifest and
  provenance record that make a crate a mod.
- [`content-mod`](content-mod) — a mod that adds content: registry entries, tags,
  resources and the pack M4-W2's generator produces from them.
- [`stateful-mod`](stateful-mod) — a mod with settings, versioned world data and a
  service other mods call.

Those three ship a `package.json` and a `provenance.json`, so each is a package
rather than only a crate: copy the directory whole into an instance's
`ravel/mods`, then run `cargo make ravel:host-build <instance> <cache> <minecraft>`
to compile it into that instance's host.
The guide snippets are not an installable package:

- [`guide-snippets`](guide-snippets) — every Rust block in
  [`docs/guide/`](../docs/guide), compiled and driven so the author's
  documentation cannot drift from what the SDK does.

Pack authors use
[`crates/ravel-pack/examples/example_pack.rs`](../crates/ravel-pack/examples/example_pack.rs).
It declares a two-mod pack with exact versions, configuration, a recipe, a texture
and progression, validates its closures, then writes the pack:

```console
cargo run -p ravel-pack --example example_pack -- <output directory>
```

That example lives with `ravel-pack` because the crates here model mod-author
manifests and depend only on `ravel-sdk`; pack declarations have a different owner.
