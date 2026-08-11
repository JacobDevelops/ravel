# What does not work yet

The mod API is experimental and may break. Beyond that, the limits below change
what you can plan today, and the guide states them because finding them by
experiment costs a day each.

**A launched game finds your package and cannot run it.** The launch path shipped
today composes a fixed set of mods in Rust source, and it will only ever run a mod
this binary was compiled from. It reads the `package.json` of each of *those* — the
manifests are compiled in beside the code, `ravel_loader::admitted` turns them
into the entries the lifecycle holds, and a `sides` list that omits the running
side parks its mod before any of its code runs.

What a launch does now do is discover. `ravel_loader::discover` reads
`ravel/mods` under the game directory, and every package installed there is
located, its manifest validated against the package schema, and its declared
source files and provenance record verified against the digests it records. That
is where launch-time integrity verification lives; `cargo xtask package digests
--check` is the same check at packaging time.

The leg between the two is now built and is not yet wired to a launch.
`ravel-build` takes what discovery found, generates a Cargo workspace naming
every installed package, and compiles it into the host library the JVM loads —
on your machine, from your source, into the build cache Ravel shares per
machine. Your mod says what to construct by writing `ravel_sdk::export_mod!` once
at the root of its crate; a package that does not fails the build naming itself
rather than yielding a host quietly one mod smaller.

What no code does yet is call that on your behalf during a launch. Nothing in the
launch path builds a host, and nothing chooses between the host a build produced
and the statically composed one the launcher stages today. So the shipped launch
still behaves as above: an installed package this build was not compiled from
stops the launch and is named, and an installed package whose bytes do not match
its manifest stops it too. You can build a conforming mod, test it thoroughly as
a library, and install it into an instance to have it validated and compiled into
a host. You cannot yet drop it into an instance and see it run.

One thing to plan around while that is true: a generated workspace supplies
`ravel-sdk` and nothing else. A mod whose `Cargo.toml` writes
`serde.workspace = true` builds in this repository and does not build once
installed, because the generated workspace declares no such dependency to
inherit. Name your own dependencies with explicit versions.

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
tests — all of that is real and all of it is checked. The receiving half is
further along than "declared": `ravel_loader::deliver_packet` is written and
tested, and it is what refuses a delivery claiming the wrong side, a direction
that may not be received here, and an owner that is absent or not reacting,
before handing the rest to `Mod::receive_packet`. What does not exist is
transport. No wire adapter is written and no launch path moves a byte between a
client and a server, so `encode_from` hands you bytes with nothing to hand them
to, and nothing in a running game ever builds the `PacketDelivery` that delivery
path is waiting for. `ROADMAP.md` puts the wire adapters after the abstractions,
and this is what "after" currently means: a declared packet is a specification of
what your mod will send, tested today and carried later — with the loader-side
half of the arrival already built. Design against it if you like; do not plan a
feature whose value arrives when the packet does.

**Two more surfaces are declared and not carried at all, for the same reason a
packet is not, and a third is now partly carried.** A sound and a drawing stage
are each a declaration you can write, test and refuse against today with nothing
on the other end; interception has an end now, for one of its three points:

- `audio` — you declare the sounds your mod owns and the events that play them,
  and `AudioDeclaration::play` answers with a `PlayRequest` describing what would
  be played. Nothing reaches audio hardware. The side rule is real and enforced:
  asking to play on a server is refused rather than skipped, so a mod that would
  have worked only in single-player finds out in its own tests. The bytes you
  hand `AudioDeclaration::entry` stay in the declaration: `ravel-pack` never
  reads an `AudioDeclaration`, so no pack file carries a sound because you
  declared one. Getting the file into a pack is a separate
  `ContentDeclaration::resource` call with the same bytes, and nothing checks
  that the two agree.
- `render_stages` — you declare which published stages your mod contributes to
  and which mods it follows at them, and Ravel decides the order, refusing a cycle
  and a stage nothing publishes. **This is not a way to draw.** No drawing path
  exists for a mod, `docs/decisions/adg-6-configuration-and-screens.md` settled
  that Ravel's own screens are declared in data and drawn in Java, and a
  contribution today says where your drawing would go rather than putting anything
  on the display.
- `interception` — **partly carried, and the first of these four to be.** You
  declare the points you answer at with `interception_declaration` and answer
  with `intercept`; the host orders the set, refuses one it cannot ask, and asks
  each mod in turn until the first refusal. One point is reached in a running
  game: `world_is_saved`, where refusing withholds *Ravel's* world write for that
  pass — not Minecraft's save, which is not what that seam does. What was
  withheld is offered again at the next save. `block_is_broken` and
  `item_is_used` are still declarable-only: no hook resolves them in the pinned
  version data, so nothing asks there yet.

  Until this landed the surface was missing *both* ends, and this guide said only
  that no caller existed. There was also no way for a mod to register an answer
  at all — `Points` took a closure, and a host holds its mods as trait objects
  and cannot get a closure out of one. The declaration is what replaced it.

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
