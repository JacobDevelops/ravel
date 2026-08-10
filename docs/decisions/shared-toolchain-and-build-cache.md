# The toolchain and the build cache are shared per machine

Status: accepted before M7-W0, for every Ravel installation that compiles a
player's mods. It is not an architecture decision gate and carries no ADG number:
`SPEC.md` §15 owns those, and its unused numbers are reserved for subjects this
record is not about. It settles a question ADG-2 left open rather than reopening
what ADG-2 decided.

## The problem this record exists for

ADG-2 decided that mods ship as Rust source and are compiled on the player's
machine, and `SPEC.md` §3.4 accepts the costs in the singular: "a Rust toolchain
of about 196 MB on the player's machine, a build cache that has to stay resident
for a rebuild to be warm". Neither says *how many* of each a machine has.

That is not a detail. A player who runs several instances — a modpack, a second
modpack, a test instance — either pays those costs once or pays them per
instance, and nothing written down decided which.

## Decision

**One toolchain and one build cache per machine, not per instance.**

The toolchain is pinned to an exact release, so two instances on the same Ravel
version want a byte-identical toolchain and a second copy is waste with no
compensating property.

The build cache is shared because Cargo's cache key already captures everything
that could make two instances' artifacts legitimately differ — crate source,
version, features, profile, target triple and toolchain. Two instances that would
produce identical artifacts reuse them; two that would not, do not collide,
because their keys differ. Sharing is therefore not a compromise between
correctness and cost, which is what makes it the right default rather than a
tuning choice.

## What decides it is disk, not time

The intuition is that sharing saves compile time, and
[`evidence/m3/pack-compile.json`](../../evidence/m3/pack-compile.json) says
otherwise. On the Linux reference host, a 256-mod pack builds cold in about 32
seconds and rebuilds warm in about 1.6 seconds. A cold build per instance is an
annoyance, not a wall.

The disk figures are the real argument. That same pack leaves a 754 MB build
cache, and the toolchain is 196 MB. Per instance, that is roughly 950 MB
duplicated for nothing; across four instances it is most of four gigabytes to
hold four copies of identical bytes.

Recording that distinction matters more than the decision it supports. If a later
measurement shows cold compile times far worse than these — and the evidence
itself warns that its mods are generated and alike, so a real pack is harder —
the time argument strengthens and the conclusion does not change. If disk becomes
free, the conclusion is weaker than it looks here.

## Consequences

- Two instances compiling at once serialise on Cargo's lock over the shared
  directory. That is a slowdown, not a corruption, and it is the price of the
  decision rather than a defect in it.
- The shared cache grows without bound. Nothing here bounds it, and something
  must: a size ceiling and an eviction policy are owed before a player who
  installs many packs discovers the growth for us. That obligation is stated and
  deliberately not built, because the shape it should take depends on numbers
  nobody has yet.
- A corrupted cache now breaks every instance rather than one. The mitigation is
  a property the cache already has: it is wholly reconstructible from source, so
  deleting it is always safe and costs only time. Any repair path may therefore
  be "delete it", and no repair path needs to be cleverer than that.
- This governs Ravel compiling a *player's* mods. It says nothing about this
  repository's own builds, where the standing rule against setting or overriding
  `CARGO_TARGET_DIR` is unaffected.

## Limits of this record

The figures come from generated mods that resemble each other, built on one
sixteen-thread Linux reference host. The evidence record says so itself. A real
pack's mods differ in size and dependency shape, so the cold figure is optimistic
and the disk figure is the more transferable of the two.

It decides nothing about macOS, where the platform expansion phase re-measures.

It does not decide where on disk either lives, beyond "not inside an instance".
The path belongs with whatever owns installation layout, and picking one here
would bind a decision this record has no evidence about.

It does not decide what happens when two instances want different Ravel versions,
and therefore different pinned toolchains. Cargo's keys keep their *caches* apart
correctly, but two toolchains is two toolchains, and whether both stay resident
or the older is evicted is undecided.
