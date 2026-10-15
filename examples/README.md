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
The two below are not packages and cannot be installed.

- [`pack-module`](pack-module) — where M4-W5's pack example lives, and why it is not
  a crate in this directory.
- [`guide-snippets`](guide-snippets) — every Rust block in
  [`docs/guide/`](../docs/guide), compiled and driven so the author's
  documentation cannot drift from what the SDK does.
