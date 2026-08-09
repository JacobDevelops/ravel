# ADG-6 — Configuration and UI

Status: accepted before M3-W3. Configuration is declared in data, the screen is
drawn in Java, and Rust supplies the values. No new measurement was taken for
this gate, and "Evidence" below says what decided it instead and why that is not
a thing a measurement produces. Amended when the schema edits landed, to record
the `packOverride` encoding and one open question the gate did not settle; the
decision is unchanged and both are under "What has to change, precisely". Amended
again after M3-W8's ADR-conformance audit, to describe the per-frame screen seam
the shipped hook inventory actually has; the decision is unchanged there too and
the amendment is at "Amendment: the extraction hook is the screen seam".
[ADG-7](adg-7-world-persistence.md) reuses this decision's Ravel-owned screen
mechanism for persistence refusal, but persistence itself is not an ADG-6 concern.

## Decision

A mod declares its configuration in a data file validated by
[`configuration.schema.json`](../../schemas/configuration.schema.json). Ravel
generates the settings screen from that declaration and draws it in Java. Rust
supplies the declaration and the current values, and receives back what the
player changed. **Ravel does not render from Rust**, and that holds for every
screen Ravel owns, M3-W5's mod list and dependency diagnostics included.

A pack states configuration **per setting**, not per file. The whole-file
override the pack manifest currently implies is refused, for reasons under "The
two override models" below, and `packOverride` on the mod author's declaration
is what decides who has the last word on each setting.

## Context

`SPEC.md` §4.1 says the player "edits supported configuration through the
in-game UI", §4.2 says the mod author "does not need to write Java", and M3's
exit criteria require "a generated configuration screen". Those three together
fix the shape of the problem: a screen has to exist, an author must not write it
in Java, and something has to turn a Rust mod's declaration into pixels.

Two mechanisms could do that. Either Ravel declares configuration as data and
Java draws it, or a mod draws its own screen from Rust. The second is the one
that has to be tested against what the accepted ADRs actually provide, because
it is the one that needs a capability none of them names.

`ROADMAP.md` puts ADG-6 before M3-W3 and lists its exit as "file/UI edits
round-trip; validation, sides, restart and pack overrides pass". The last of
those four is where the repository contradicts itself, which is the second half
of this record.

## Evidence

**This gate was decided without a new spike.** The deciding fact is that one
candidate needs a capability no accepted ADR provides, and no benchmark produces
a missing capability: measuring a mechanism that does not exist would mean
building it first, which is the decision rather than evidence for it.
[ADG-2](adg-2-mod-execution.md) is the precedent — it decided the distribution
property at M3 against measurements taken at M1 for a different question, and
recorded that "what decides it is a property no measurement here was addressed
to". The same holds here.

A reviewer is entitled to challenge that.
[`README.md`](README.md) states the bar as "a measurement exists, a rejection is
explicit, and the limits are written down", and this record satisfies the second
and third and not the first. The honest position is that the bar is written for
gates choosing between mechanisms that all exist, which ADG-1, ADG-3 and ADG-4
were, and that ADG-2 and this gate are the two where it does not fit. If M3-W8
disagrees, the remedy is not a benchmark of a Rust-drawn screen — it is to say
that ADG-6 should have been deferred until one existed to measure, which is a
scheduling judgement about M3-W3 rather than a fact about the mechanism.

What follows is drawn from measurements taken for other gates and from the
26.1.2 targets resolved at M3-W0.

- **The boundary runs one way.** [ADG-1](adg-1-jvm-entry.md) enters the game as a
  premain agent, rewrites hook targets, and binds callbacks to Rust through an
  exported JNI-mangled symbol. Java calls Rust. Nothing in that record, in
  [ADG-3](adg-3-bindings.md), or in the generator provides a call in the other
  direction, and
  [`m3-hook-targets.md`](../architecture/m3-hook-targets.md) states the
  consequence directly: rendering a screen from Rust "needs Ravel to call **into**
  Minecraft, and no accepted ADR provides that".
- **A crossing costs about ten nanoseconds and a field read about nine.** ADG-1
  measures a hook crossing at 10.02 ns p50 through an agent-rewritten call site
  and a generated hook class. [ADG-4](adg-4-state-ownership.md) measures a
  Java-authoritative flow at 8.61 ns p50 and prices a field read at about 8.6 ns
  reached by calling the native method directly. ADG-4's "Limits of this
  evidence" is explicit that these are different paths, that neither is the bare
  boundary, and that they must not be added. Both are used here as orders of
  magnitude, and neither is summed with the other.
- **The screen API is the part of the game that already moved.** 26.1.2 has no
  `Screen.render`. Screens implement `Renderable` and draw through
  `extractRenderState(Lnet/minecraft/client/gui/GuiGraphicsExtractor;IIF)V`, and
  `Screen.init(II)V` is `public final` so a hook there has to be call
  interception rather than an override; the overridable one is `protected void
  init()`. Those facts come from `m3-hook-targets.md`, resolved against the
  pinned client jar by two independent passes.
- **`packOverride` has no semantics and no consumer.** The enum
  `allowed | locked | required` appears exactly once in the repository, in
  `configuration.schema.json` itself. No document defines the three values, and
  no crate reads them.

## Why the screen is Java

**The alternative is not harder, it is undecided.** Drawing from Rust is not a
new row in `hooks.toml`; it is a second boundary running the other way, with its
own lifetime rules, its own exception behaviour, and its own unsafe surface, and
no accepted ADR selects a mechanism for it. This gate is not entitled to pick
one in passing. Declaring configuration in data needs nothing that does not
already exist: the declaration crosses as data, and the screen is Java drawing
Java.

**The traffic differs in kind, not by a factor.** A data-declared screen crosses
the boundary when it opens and when it saves — one batched call each way, which
is what ADG-4 asks for when it says the answer to boundary traffic is fewer
crossings. A Rust-drawn screen crosses per widget per frame, in the direction
that does not exist. The figures above do not condemn per-frame crossing on
their own; at 10 ns a crossing, a screen redrawing at sixty frames a second
would have to be doing a great deal before the boundary showed up. What condemns
it is that the count is unbounded in the number of widgets rather than fixed at
two, and that a settings screen is exactly the surface where a mod set's
per-widget traffic multiplies by the number of mods installed. `SPEC.md` §1 asks
for the pinned progression "without hundreds of resident Java mods", §6.2 caps
steady-state RSS at 8 GiB, and §6.5 names "hook and boundary overhead" as one of
the things re-examined if the contract fails. A design whose crossing count is
O(widgets × frames) is the wrong side of that even before anyone measures it.

This paragraph prices per-frame crossing rather than forbidding it, and that
reading is the one that survives: Ravel's own inventory carries one per-frame
screen observation, `screen.extract-render-state`, and what is refused above is
the unbounded per-widget factor and the direction, not the frame. See "Amendment:
the extraction hook is the screen seam".

**A declaration is version-agnostic; a drawn screen is not.** ADG-4's amendment
argues that an opaque handle is version-agnostic by construction because its
shape is identical in every Minecraft version, while a copy's field layout is
version-specific and moves on every game update. The parallel holds here, and it
holds with evidence rather than by analogy. A declared setting names a key, a
value schema, a default, a side, and a restart flag — nothing in that sentence is
a Minecraft symbol, so a game update cannot move it. A Rust-drawn screen names
widget classes, layout constants, and the render-state extraction path, all of
which are version-specific. The evidence that this is not hypothetical is the
third bullet above: `Screen.render` is gone in 26.1.2 and `init(II)V` is final.
The drawing surface is the surface that already changed. Of the two designs, the
declared one is the one that does not accumulate version debt.

**It puts the version-specific part where the version-specific parts live.**
`ROADMAP.md` already scopes `ravel-config` to "schemas, validation, side/restart
flags, layered overrides" and explicitly excludes "generated UI widgets or
version details". This decision is what that exclusion was written for.

## Rejected

- **A screen drawn from Rust.** Rejected on the three grounds above: it needs a
  Rust-to-Java capability no ADR provides, its boundary traffic scales with
  widgets and frames rather than with opens and saves, and it binds every mod to
  a client API that demonstrably moves between versions. If Ravel ever needs to
  call into the game for another reason, that is a new gate with its own
  measurement, and this decision should be revisited then rather than assumed.
- **Configuration declared in Java.** A mod author writing a Java screen or a
  Java config class is refused by `SPEC.md` §4.2 before any of the above
  applies.
- **Files only, with no in-game editing.** Simplest, and it needs no screen at
  all, but `SPEC.md` §4.1 and M3's exit criteria both require the player to edit
  configuration in game.
- **A hand-written Java screen per mod, shipped by the author.** This keeps the
  drawing in Java and so escapes the boundary argument, but it puts Java back in
  the author's hands and gives up the generation the roadmap asks for.

## The two override models

The repository currently carries two incompatible readings of "pack override".
Only one of them is checked in: `configuration.schema.json` declares the
per-setting policy, and the whole-file model is a convention the manifest is
read through rather than a constraint the manifest states. That asymmetry is
what makes this resolvable at all, and it is why the section below defines
semantics rather than choosing between two that already exist.

[`pack.schema.json`](../../schemas/pack.schema.json) gives a pack a
`configuration` array of `{owner, path, sha256}`. Read as the modded-Minecraft
ecosystem reads it — a pack ships the config files it wants — that is a
whole-file override: the pack replaces an owner's configuration entirely, and
every setting in that file is frozen by the pack's hash.

`configuration.schema.json` puts `packOverride: allowed | locked | required` on
each individual setting. That is a per-setting policy, declared by the mod
author, about what a pack may do to that one value.

These cannot both be the model. A whole-file override cannot honour a
per-setting policy, because a replaced file replaces the settings the author
marked `allowed` along with the ones marked `locked`, and nothing in the pack
manifest records which of the two it meant.

**Per-setting wins.** The whole-file model serves the pack author's habit and
nobody else. It is what the existing ecosystem's tooling produces, and it is
convenient for one workflow — configure a test instance, copy the files out —
but every property it has past that point is a cost. It freezes settings the
pack has no opinion about, including the client-side and per-machine ones a
player has every right to change, and it gives the in-game UI nothing to say
beyond "this file belongs to the pack". It goes stale silently: a mod that adds
a setting in a later version finds the pack's file missing the key, with no way
to distinguish "the pack chose the old default" from "the pack has not seen this
setting". And it is at odds with the surface `SPEC.md` §4.1 promises the player,
which is editing configuration in game, not discovering that a pack has taken
the whole file.

Per-setting override loses the copy-the-files workflow and keeps everything
else. A pack states values for the settings it actually cares about; every other
setting keeps the author's default and stays the player's. The UI can show, per
row, whether the pack set this value and whether the player may change it. A mod
update adds settings the pack never mentioned and they arrive at their defaults,
which is the correct behaviour and needs no pack revision.

Nothing about reproducibility is given up. The pack still points at a file, and
the file still carries a `sha256`, so the pack is pinned exactly as it is today.
What changes is what that file contains.

### What has to change, precisely

This ADR changes no schema. M3-W3 makes these edits.

1. `pack.schema.json`'s `configuration` items keep their shape — `owner`,
   `path`, `sha256` — and gain a `description` stating that the referenced
   document is a **per-setting override document for that owner**, not a
   replacement configuration file. The structural change is zero; the content
   contract, which the manifest never stated, is what is being fixed.
2. A new `configuration-override.schema.json` defines that document: a
   `schemaVersion`, an `owner` matching the manifest entry, and a map from
   setting `key` to value. Pack validation checks each value against the owning
   setting's `valueSchema`.
3. `configuration.schema.json`'s `packOverride` gains `"default": "allowed"` and
   a `description` carrying the semantics it does not currently have:
   - `allowed` — a pack may state a value; the player has the last word and may
     change it in game.
   - `locked` — a pack may state a value; where it did, the pack has the last
     word and the UI shows the value as fixed by the pack rather than editable.
   - `required` — a pack must state a value, and the pack has the last word.
     Pack validation refuses a pack whose override document omits it.

`required` is the only one of the three that is an obligation on the pack rather
than a permission, and it is the only one that can fail validation. That is
deliberate: an author who marks a setting `required` is saying the value is not
theirs to default, which is a statement pack validation can check and a screen
cannot.

**One description, not three.** This record originally asked for "a `description`
per enum member", which draft 2020-12 cannot express on a bare `enum` — doing it
literally means replacing the `enum` with `oneOf: [{const, description}, …]`.
That was rejected on implementation, and the rejection is recorded here rather
than left as a silent divergence. It would be the only construct of its kind in
either schema, where `side`, `relationship`, `role` and `enabledSides` are all
bare enums; and it degrades what a bad value reports, from "must be one of
allowed, locked, required" to "must match exactly one schema in oneOf", in a
document whose whole purpose is refusing bad input. The three semantics above are
carried verbatim in one combined `description`. Nothing about the meaning
changed, only the encoding.

### Open: may a pack lock a client-side setting?

**This gate does not decide it, and M3-W3 will meet it in its first test of a
locked setting.** The schema admits `side: "client"` together with
`packOverride: "locked"`, and nothing here says whether that is legal.

It is a real choice rather than a limitation. `side` and `packOverride` are
siblings inside the same `setting` object, so unlike the cross-document rules
this one *is* expressible in the schema: an `if`/`then` could refuse the pairing
outright. Both readings follow from what is already written and this record picks
neither:

- A pack legitimately locks a client setting for its own coherence — a pack that
  needs a particular client-side rendering toggle to keep its visuals consistent
  — and "Consequences" below, in saying a client setting is "not a pack's
  business to lock on a server", means only that a *server* cannot impose one on
  a connecting client.
- Locking a client setting is exactly the pack-clobbers-player-preferences
  behaviour the per-setting model was chosen to prevent, and the schema should
  refuse the pairing.

**The reversible direction is to refuse, and the schema currently permits.**
Strict-to-permissive later costs nothing: documents that were refused start being
accepted, and nothing already written breaks. Permissive-to-strict later breaks
every declaration that used the pairing. This is the same asymmetry ADG-2 turned
on — a compatibility surface cannot be withdrawn once things depend on it — and
it runs the same way here.

The schema permits the pairing anyway, because whether a pack may override a
player's client-side choice is a product decision about player agency and
accessibility rather than a schema detail, and it is not this gate's to take. The
cost of that is worth stating plainly: the question gets more expensive to answer
the longer it stays open, and it only gets more expensive in the direction of
refusing.

## Consequences

- Configuration resolves in three layers: the author's `default`, then the
  pack's override document, then the player's value. The player layer is
  suppressed for a setting whose `packOverride` is `locked` or `required` and
  which the pack actually set. `ravel-config` owns this and stays free of
  widgets and version details.
- The screen is a third artifact in the game process, beside the agent jar and
  the native library, and **not inside the agent jar**. It names game types and
  so cannot be compiled without the client jar, which the agent jar deliberately
  can be; and it would sit on the wrong side of the class loader delegation
  M2-W5 established, where a system-loader class does not resolve
  `net.minecraft.*`. ADG-1 carries this as "Amendment: what the agent jar
  carries", which upholds its restriction on the jar and corrects its count from
  two artifacts to three. How the artifact reaches a loader that resolves game
  types is unresolved there and is M3-W3's to answer.
- The screen hooks `Screen.init()V` to populate, `Screen.tick()V` if it needs
  ticking, `Minecraft.setScreen` to observe navigation, and
  `TitleScreen.createNormalMenuOptions(II)I` for the entry point in the main
  menu. The inventory also carries `screen.extract-render-state`, which is this
  version's per-frame seam and an observation point rather than a drawing one:
  Ravel's own screens omit that override and every widget is drawn in Java.
  **Ravel never draws** is what the bullet was written to protect and it is
  undisturbed. See "Amendment: the extraction hook is the screen seam".
- Two batched boundary crossings are all this design *needs* per screen session,
  both in the direction ADG-1 provides. Opening reads one owner's declaration and
  current values; saving hands back the settings that changed. Neither of them
  scales with widgets. The per-frame observation the inventory carries is a
  separate crossing and the amendment below is where it is accounted for.
- `restartRequired` is reported by the screen, not enforced by it. A changed
  restart-required setting is written and takes effect on the next launch, and
  the UI has to say so at the moment of the edit rather than at the next launch.
- `side` decides where a setting is editable at all. A `dedicated-server`
  setting has no client screen, and a `client` setting is not a pack's business
  to lock on a server.
- Validation runs against `valueSchema` when the file is loaded and again when
  the screen saves. A rejected value fails the edit and leaves the previous
  value in place; it does not fail the world. That is distinct from M2-W7's
  fail-closed path, which is about a mod graph, not a setting.
- M3-W5's mod list and dependency diagnostics are the same mechanism. They are
  Java screens fed by data from Rust, and `SPEC.md` §9.2's list of things the UI
  must report before world access is a data contract rather than a rendering
  one.

## Amendment: the extraction hook is the screen seam

Amended after M3-W8's ADR-conformance audit, on the shipped inventory rather than
on an argument. Two sentences in "Consequences" described a screen surface this
repository does not have. One said the omission of `extractRenderState` from the
hook inventory was the decision. The other said that no crossing happened per
frame or per widget. **Both are withdrawn as design properties, and the decision
they were written to protect is untouched: Ravel never draws.**

What the inventory says instead: `hooks.toml` declares `screen.extract-render-state`
on `Screen.extractRenderState(Lnet/minecraft/client/gui/GuiGraphicsExtractor;IIF)V`,
its own section comment gives the reason — "26.1.2 has no Screen.render; screens
draw through render-state extraction, so extractRenderState is the per-frame
seam" — and the entry is not decorative. The generator emits one native export per
inventory entry, so the agent rewriting that method is a crossing, and the launch
recorded in
[`linux-process-matrix.json`](../../evidence/m3/linux-process-matrix.json) rewrote
it inside the official client along with the other three `Screen` targets. The ADR
was describing what it thought it had asked for; the inventory has said otherwise
since M3-W0.

### Why the hook stays and the prose changes

There is no `Screen.render` in 26.1.2, which is the third bullet of "Evidence"
above and the reason this version was interesting in the first place. Extraction
is therefore the only point at which a screen can be observed while a frame is
being built, and `screen.init`, `screen.init-sized`, `screen.tick` and
`screen.set` do not stand in for it: none of them fires during a frame. Removing
the entry would not move drawing into Java, because drawing is already in Java —
it would remove per-frame screen observation from the inventory altogether. The
alternative to this hook in this version is no such seam at all, so the record is
what was wrong and the record is what is corrected.

### What crosses, and what still does not

The entry is an **observation** point. A rewritten `Screen` crosses into Rust once
per frame while it is the open screen, because the generator emits one export per
entry and the agent rewrites the method carrying it. That is per frame and per
screen. It is not per widget: it does not multiply by the settings a mod declares
or by the number of mods installed, which is the factor "The traffic differs in
kind, not by a factor" above actually objects to, and that objection survives
intact.

**Ravel never draws, and it is checkable rather than asserted.** No crate names a
drawing call — `drawString`, `GuiGraphics`, `renderBackground` and `blit(` appear
nowhere under `crates/`. Every widget is built and drawn in Java under
`jvm/screen/src/dev/ravel/jvm/screen/`, the game's font does the wrapping, and
`RavelSettingsScreen` deliberately declares no `extractRenderState` of its own —
its class comment records that this is why `AbstractSliderButton` is subclassed
and `AbstractButton`, which forces an extraction override, is not. So Ravel's own
screens do not even contribute the one per-frame crossing above; a rewritten
*game* screen does. Configuration is still declared in data, and the two batched
crossings per screen session are still the only ones this design needs.

### What it costs is unmeasured, and stays that way here

No M3 record prices this crossing. There is no per-frame figure in
`profiling-overhead.json`, `linux-process-matrix.json` or any other M3 record, and
the nearest numbers in the repository are ADG-1's 10.02 ns hook crossing and
ADG-4's 8.61 ns Java-authoritative flow, both taken through other call sites for
other questions and neither addressed to a frame. "Limits of this evidence" below
still says nobody has measured a per-frame crossing, and that is still true after
this amendment. What changed is that the repository now *has* one, not that it has
a number for it. Anyone who wants the number should measure it rather than
multiply 10.02 ns by a frame rate, which is arithmetic across paths that ADG-4's
"Limits of this evidence" expressly refuses.

### Why this is an amendment rather than a code change

The stale sentences are the defect. Changing code to match them would have deleted
the only screen observation the version admits, on the authority of a sentence
nobody had checked against `hooks.toml` — and `hooks.toml` is re-pinned by
ceremony, which is a further reason a prose error is not a licence to edit it.
`verify_decision_hook_claims` in `tools/ravel-xtask/src/lib.rs` now fails
`ravel-verify-scaffold` if a decision record denies a hook the inventory declares,
so this particular pair cannot drift apart again in silence.

## Limits of this evidence

- **No measurement was taken for this gate**, and the record above says why. The
  nanosecond figures are ADG-1's and ADG-4's, taken for other flows against
  other questions, and neither measured a screen or a configuration load. They
  bound the cost of a crossing; they do not predict the cost of this design.
- The argument against a Rust-drawn screen is a missing capability and a moving
  API, not a number. Nobody has measured a per-frame crossing, and if the
  capability is ever built for another reason the arithmetic should be redone
  rather than inherited from here. That sentence is now load-bearing in a second
  way: the amendment above records that the inventory carries a per-frame
  observation crossing today, and no record prices it either.
- The 26.1.2 screen targets were resolved against the pinned jar and, when this
  was written, were not in `hooks.toml` and had never run. M3-W0 added them and
  M3-W3 ran them: the launch recorded in
  [`evidence/m3/class-loader-identity.json`](../../evidence/m3/class-loader-identity.json)
  rewrote 10 classes and 24 methods in the official client, which reached its
  title screen. That record reports counts rather than names, so it does not say
  which of the targets those were. What remains unexercised is not the inventory
  but the screen the screen targets are for.
- One version. The claim that a declaration outlasts a drawn screen rests on the
  observation that 26.1.2's screen API already moved; it does not say how the
  next version moves, and a declaration format is only version-agnostic for as
  long as nothing in it starts naming game types.
- The three-layer model and the `packOverride` semantics defined above are
  written, not tested. M3-W3's round-trip test is what proves them, and the
  first thing it should try is a pack that locks a setting a player then tries
  to change.
- Whether pack authors accept losing the copy-the-files workflow is a product
  judgement, not a measured one. It is the one place this decision trades a real
  convenience for a property, and it is stated here so that the trade is visible
  if it turns out to be the wrong one.
