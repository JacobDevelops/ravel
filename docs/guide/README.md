# Writing a Ravel mod

Ravel is a mod loader and SDK for Minecraft: Java Edition. A Ravel mod is
a Rust crate. It is distributed as source and compiled on the machine that
installs it, so there is no ABI to match, no binary to build per platform, and no
Java to write.

**Know this before you read the rest: a launched game finds your package and
cannot run it.** Discovery is real — a launch reads `ravel/mods`, validates each
installed manifest against the package schema, and verifies the digests it
records. What it cannot do is admit what it found: the mods a launch runs are the
ones this binary was compiled from, so an installed package the composition does
not name refuses the launch rather than joining it. A mod you write can be built,
tested and held to every criterion in this guide as a library, and cannot be
dropped into an instance and watched run. Nor can it send a declared packet;
only the 1.12.2 adapter delivers inbound custom payloads. Both limits, and the rest, are in
[What does not work yet](limits.md); read that document before you plan anything,
not after.

This directory is the author's documentation. It is the only part of `docs/` that
is written for someone outside this repository; everything else here records
decisions and boundaries for the people building Ravel itself.

Read in this order:

1. [Your first mod](your-first-mod.md) — the crate, the one dependency, and the
   mod trait.
2. [The package manifest](package-manifest.md) — the metadata without which a mod
   cannot be admitted.
3. [Declaring content](content.md) — blocks, items, recipes, commands, tags,
   resources, and the pack Ravel generates from them.
4. [The lifecycle, events, sides and packets](running.md) — what happens while a
   mod runs.
5. [Settings, world data and services](state.md) — the state that outlives a call.
6. [What "conforming" is checked to mean](conforming.md) — the acceptance
   criteria, and the mistakes they catch.
7. [What does not work yet](limits.md) — read this before you plan anything.

The API reference is the `ravel-sdk` rustdoc. Build it with

```console
cargo doc -p ravel-sdk --no-deps --open
```

Every module there states the rule it enforces and carries an example that
compiles as a doctest, so an example in the rustdoc is an example that still
works. The guide does not restate them; it says which module answers which
question.

The mods under [`examples/`](../../examples) are the worked versions of
everything below and are compiled as workspace members, so a facade that drifted
stops building there first. Read
[`examples/minimal-mod`](../../examples/minimal-mod) before writing anything: it
is the smallest complete mod and the only one that carries the package files a
mod is distributed with.

Every Rust block in this guide is a copy of a module in
[`examples/guide-snippets`](../../examples/guide-snippets), which depends on
`ravel-sdk` and nothing else and is compiled and driven by the test suite. A
block that stopped compiling, or that drifted from the crate it was copied from,
fails the repository's own gate. The blocks show code rather than whole files, so
when you want the imports a snippet needs, read the module it came from.
