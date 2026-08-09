# Examples

Mods and packs written against nothing but the public surface. An example's
manifest is a mod author's manifest, which is what makes these crates worth
compiling in the workspace rather than pasting into documentation: a facade that
has drifted stops building here first.

- [`minimal-mod`](minimal-mod) — the smallest complete mod, and M4-W1's
  compile-pass/fail suite for the author boundary.
- [`content-mod`](content-mod) — a mod that adds content: registry entries, tags,
  resources and the pack M4-W2's generator produces from them.
- `pack-module` — M4-W5, once `ravel-pack` declares and validates a pack.
