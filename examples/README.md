# Examples

Mods and packs written against nothing but the public surface. An example's
manifest is a mod author's manifest, which is what makes these crates worth
compiling in the workspace rather than pasting into documentation: a facade that
has drifted stops building here first.

- [`minimal-mod`](minimal-mod) — the smallest complete mod, and M4-W1's
  compile-pass/fail suite for the author boundary.
- `content-mod` — M4-W2, once registries, tags, resources and generated data have
  public APIs.
- `pack-module` — M4-W5, once `ravel-pack` declares and validates a pack.
