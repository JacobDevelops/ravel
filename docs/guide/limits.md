# What does not work yet

The mod API is experimental and may break. Beyond that, the limits below change
what you can plan today, and the guide states them because finding them by
experiment costs a day each.

**A launched game does not discover your package.** The launch path shipped today
composes a fixed set of mods in Rust source, and it will only ever run a mod this
binary was compiled from. It reads the `package.json` of each of *those* — the
manifests are compiled in beside the code, `ravel_loader::admitted` turns them
into the entries the lifecycle holds, and a `sides` list that omits the running
side parks its mod before any of its code runs. What is missing is discovery: a
package sitting in an instance directory is found by nothing, so its manifest is
read by nothing either. You can build a conforming mod and test it thoroughly as
a library. You cannot yet drop it into an instance and see it run.

Nothing in the launch verifies package integrity either. The host hashes no
source tree, because the source it would hash is the source it was compiled from;
`cargo xtask package digests --check` is where that check lives today.

**A launch reads its side, but only as far as the distribution it started from.**
`initialize` no longer asserts a side. It asks its own JVM which Minecraft
classes it can load: `net/minecraft/client/main/Main` is in the client
distribution and not in the dedicated server one, and a process that can load
neither that nor `net/minecraft/SharedConstants` refuses to start a mod set at
all rather than guessing. So a client launch is a client and a dedicated server
launch is a dedicated server, and the `sides` list your package ships decides
which of the two your mod runs on.

Two things that follow are worth planning around. **An integrated server is never
answered.** It is a server inside a client process rather than a distribution, so
nothing on a class path can name it: a single-player launch is composed as
`Side::Client`, and a mod that declares `integrated-server` and not `client` is
parked there. Declare both if your mod should run in single-player.

**And no dedicated server launch has happened.** That the server distribution
carries `SharedConstants` and not the client's entry point is Mojang's split, not
something this repository has measured — no server jar is a pinned input and none
may be tracked. The JVM harness runs the whole path on a class path built to look
like each distribution, which proves the side is read and that it decides what
loads; it is not a launched server.

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

**Three more surfaces are declared and not yet carried, for the same reason a
packet is not.** A sound, a drawing stage and an interception point are each a
declaration you can write, test and refuse against today, with nothing on the
other end of them:

- `audio` — you declare the sounds your mod owns and the events that play them,
  and `AudioDeclaration::play` answers with a `PlayRequest` describing what would
  be played. Nothing reaches audio hardware. The side rule is real and enforced:
  asking to play on a server is refused rather than skipped, so a mod that would
  have worked only in single-player finds out in its own tests.
- `render_stages` — you declare which published stages your mod contributes to
  and which mods it follows at them, and Ravel decides the order, refusing a cycle
  and a stage nothing publishes. **This is not a way to draw.** No drawing path
  exists for a mod, `docs/decisions/adg-6-configuration-and-screens.md` settled
  that Ravel's own screens are declared in data and drawn in Java, and a
  contribution today says where your drawing would go rather than putting anything
  on the display.
- `interception` — you declare that your mod answers at a published `Point`, and
  a round asks every mod in the declared order, ending at the first refusal.
  Nothing in a running game asks a round. A point is a place the host calls out
  from, so this is never a patch your mod applies; there is simply no caller yet.

Declaring against any of the three is designing against a specification that is
tested now and connected later. `world_shaping` and `server_data` are different:
what they produce — a record a world stores, files a pack holds — is real output
today, and only the game's reading of it waits on the same wiring every pack
waits on.

**A shared library your mod provides is declared and nothing reads the
declaration.** `provision` is the fourth surface in that state, and it is
different from the three above in one way worth knowing: the thing a library
declaration is usually *for* — getting one copy of a library loaded instead of
several — is not waiting on anything, because it never applies. A mod is source
your player's machine compiles, so a library two mods build on is an ordinary
Cargo dependency and Cargo resolves it once. What `provision` decides is the part
Cargo cannot see: that of the mods installed together, exactly one claims to
supply `shared_maths`, and that the version it supplies is inside every range the
others declared. `Provision::resolve` names the provider or refuses the set,
naming the library and the mods on both sides. What no launch does yet is build
one from the packages it composed — nothing today turns your `provides` into a
fact about a running game, for the same reason nothing carries your packet.

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
