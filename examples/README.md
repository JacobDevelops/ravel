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
- [`pack-module`](pack-module) — where M4-W5's pack example lives, and why it is not
  a crate in this directory.
