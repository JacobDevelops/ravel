# What does not work yet

The mod API is experimental and may break. Beyond that, the limits below change
what you can plan today, and the guide states them because finding them by
experiment costs a day each.

**A launched game does not discover your package.** The launch path shipped today
composes a fixed set of mods in Rust source; it reads no `package.json`, so
nothing about your manifest — including its `sides` — has any effect in a running
game. `crates/ravel-composed-host/src/lib.rs` says so itself, under a section
headed "Open obligation: this host reads no manifest": both lifecycle entries are
written by hand there and the sides each mod runs on are constants in that file.
The mapping from a validated package set to lifecycle entries exists and is
production code — `ravel_loader::admitted` does read `manifest.sides` — but
nothing calls it in a launch, because that host discovers no package and
validates none. You can build a conforming mod and test it thoroughly as a
library. You cannot yet drop it into an instance and see it run.

**A mod cannot send a packet.** You can declare a `PacketContract`, implement
`Packet`, encode a value, decode it back, and prove every refusal in your own
tests — all of that is real and all of it is checked. What does not exist is the
leg after encoding. No wire adapter is written; no launch path moves a byte
between a client and a server on a mod's behalf; `encode_from` hands you bytes
and there is nothing to hand them to, and the receiving half — `Mod::receive_packet`
— has no caller in a running game either. `ROADMAP.md` puts the wire adapters
after the abstractions, and this is what "after" currently means: a declared
packet is a specification of what your mod will send, tested today and carried
later. Design against it if you like; do not plan a feature whose value arrives
when the packet does.

**Configuration composition is fixed too.** The same launch path hardcodes which
mods' settings are resolved, against a constant pack configuration path. A
settings document from a package it does not know about is not read.

**Only `source.files` and the provenance record are integrity-checked.** Package
verification hashes exactly those and nothing else, so a `configurationSchemas`
entry or a `resources` entry that is not *also* listed in `source.files` ships
unverified. This is not a bug — `source.files` is the declared set, and the
manifest is explicit about it — but "declared in the manifest" and "protected"
are different sets, and an author will reasonably assume they are one. List
anything you want protected twice.

**Packaging tooling is one subcommand, not a workflow.** `cargo xtask package
digests` refreshes a manifest's digests, and `--check` verifies them. Nothing
else: no scaffold for a new mod crate, no command that writes a first
`package.json` or `provenance.json` for you, no publish step. Copy
[`examples/minimal-mod`](../../examples/minimal-mod) and edit it. `SPEC.md` §4.2
promises the author publishes through supported tooling; that subcommand is the
sharpest part of it and the rest is still open.

**Nothing is published.** `ravel-sdk` is not on a registry, so a mod crate
outside this repository depends on it by path against a checkout, and there is no
version of it to declare a range against. `compatibility.ravel` in your manifest
is still meaningful — the loader checks it — but the SDK you compiled against
came from a working copy.

**Nothing checks that your manifest `id` and your `#[declare_mod(id = …)]`
agree.** See the last pitfall in
[What "conforming" is checked to mean](conforming.md).

**There is no command that validates your manifest.** The schemas under
`schemas/` are the authority and they are ordinary JSON Schema 2020-12, so any
validator that speaks that dialect checks a `package.json`, a `provenance.json`
or a configuration document — Python's `jsonschema`, `ajv`, `check-jsonschema`,
whichever you already have. Ravel ships no wrapper around one. See
[The package manifest](package-manifest.md).

**Writing a packet field can panic; reading one cannot.** The reading half
refuses everything the bytes do not say, including a length prefix that runs past
the end of the buffer, and that refusal is explicitly not a panic. The writing
half is not symmetrical: `PacketWriter::text` and `PacketWriter::bytes_field`
panic on a field of 4 GiB or more, because the length prefix on the wire is a
32-bit count and there is no larger one to write. Nothing a decoded packet can
produce reaches it — the prefix that framed the field was itself 32 bits — so
this is not a panic a remote peer can cause. It is one your own mod can, by
handing over a field it built that large. Both methods document it.

Nothing here affects whether a mod is conforming. It affects when you will see it
run.
