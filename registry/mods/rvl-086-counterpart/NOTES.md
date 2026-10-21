# RVL-086 counterpart

Shipped settings failures are retained until `Constructed`, not panicked away during
construction. Typed cost reads refuse values outside their declared ranges; packet
adoption retains its specified clamp. Malformed deciding-settings payloads preserve
`InvalidPacket`, allowing the loader to reject delivery while keeping the owner running.
The declared attractor-toggle contract has no entry-level handler here and returns
`Unsupported`, as do undeclared contracts; this does not add toggle gameplay.

An original Rust implementation of `specifications/m6-foundation/RVL-086.md`, written against
`ravel-sdk` and `rvl-042-counterpart` and nothing else. No upstream source was opened, decompiled or
read to write it; the specification is the only description of the behaviour that was used.

The entry is the cluster's leaf, so the question its own §1.1 asks is what it consumes and what it
leaves behind in a world. What it leaves behind is one boolean per attractor item and ordinary item
damage, and nothing else: `Entry::saved_data_declaration` answers nothing, and a test says so.
What survives as counterpart work is §6 through §8 and §11 through §13, which is where most of the
code and most of the tests are.

Flag publication returns the shared library's typed identity error instead of silently omitting
an invalid binding. Rebuilding the configured entry propagates that failure.
Shared stacks, repair constructors, projectile acceptance callbacks and optional crop seeds use
validated `ContentId` identities. Zero count alone denotes an empty stack. Settings, deciding
payloads and loose attractor targets derive grouped Debug diagnostics without changing configuration
keys or wire layout. Repair refusals are named `RepairRefused` and implement `Error`; material
worth reads the existing repair `Context` rather than separate callbacks. Tag declaration failures
retain their typed sources through `Error::source` instead of flattening them into text.
Generated census kinds and setting sides use the SDK's canonical string spellings,
not Debug variant names. Repair-discount tests independently turn each shipped-on
switch off and carry distinct digging/shearing values through configuration and wire reads.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side, lifecycle and the registration order | implemented | `src/lib.rs`, `src/census.rs` |
| §2 the client-only guard as a declared side | implemented | `src/lib.rs` |
| §2 §7's one-time startup parse | not reproduced, by design | `src/overrides.rs` |
| §3 the twenty-eight settings and their defaults | implemented | `src/settings.rs` |
| §3.5 the unsynchronised scope | corrected and synchronised | `src/settings.rs` |
| §4 the seventeen flags and their binding | implemented, through the shared library | `src/settings.rs` |
| §4 what a flag actually gates | implemented as the creative grouping's gating | `src/census.rs` |
| §4 data-pack conditioning of the recipe documents | declarable only | — |
| §5.1 seventeen blocks, sixteen of them tinted | implemented as cardinality and colour | `src/census.rs` |
| §5.2 fifty-nine items and the figures §17 admits | implemented as declared data | `src/census.rs` |
| §5.3 the two tiers' and four materials' balance | not carried, by §17's own exclusion | — |
| §5.3 the mismatched armour repair ingredient | corrected | `src/census.rs` |
| §5.4 the creative grouping's order and gating | implemented | `src/census.rs`, `src/lib.rs` |
| §5.4 the grouping's title, as a declared translation key | declared | `src/census.rs`, `src/lib.rs` |
| §5.5 the one item-level data value | implemented | `src/attractor.rs` |
| §5.6 the recipe kind and its empty encodings | implemented | `src/repair.rs` |
| §5.7 the tag memberships whose members this package owns | declared | `src/census.rs` |
| §5.7 the loot and recipe document bodies | declarable only | — |
| §5.7 the memberships naming host or third-party entries | not carried, by §17's own exclusion | — |
| §6.1 reading the grid, in slot order | implemented | `src/repair.rs` |
| §6.2 the four branches of what a material is worth | implemented | `src/repair.rs` |
| §6.3 the craft loop and every refusal | implemented | `src/repair.rs` |
| §6.4 both truncations, and the structural curse test | implemented | `src/repair.rs` |
| §6 matching and assembling as one computation | implemented | `src/repair.rs` |
| §7 the override table, its parse and its two lookups | implemented, with three changes | `src/overrides.rs` |
| §8 the item-matching expression | implemented | `src/matching.rs` |
| §9 the persisted inventory | implemented as nothing world-level | `src/lib.rs`, `src/attractor.rs` |
| §10.1 the one payload, with the missing checks | implemented, with the declared change | `src/attractor.rs` |
| §10.1 the client press and the screen test | implemented as a capability | `src/attractor.rs` |
| §10.2 the use-in-hand flip, its sound and its success | implemented | `src/attractor.rs` |
| §10.2 the sweep, its exclusions and its box | implemented, server-side only | `src/attractor.rs` |
| §10.3 the food effect durations | implemented, accumulator reproduced | `src/effects.rs` |
| §10.3 applying a timed effect | declarable only | — |
| §11 the nearly-destroyed refusal and its seven surfaces | implemented | `src/refusal.rs` |
| §11 the raised priority across other mods | declarable only | `src/lib.rs` |
| §12 the ineffective-tool rule | implemented, typed damage source | `src/refusal.rs` |
| §12 the build-without-consumption exemption | implemented over all four effects | `src/refusal.rs` |
| §12 the tooltip line rewriting | declarable only | — |
| §13 the right-click crop harvest and its withholding | implemented | `src/harvest.rs` |
| §14.1 what the two informational lines would say | implemented | `src/tooltip.rs` |
| §14.1 inserting and recolouring a tooltip line | declarable only | — |
| §14.2 the per-block colour | implemented as declared data | `src/census.rs` |
| §14.2 the tinting itself | declarable only | — |
| §14.3 the night-vision upkeep | implemented, with two declared changes | `src/effects.rs` |
| §14.3 the model layer and its textures | declarable only | — |
| §15 the accessory-slot integration | not carried, by §17's own exclusion | — |
| §16 what this entry needs from RVL-042 | consumed, not rewritten | every module |

## Behaviour changes, each declared rather than incidental

- **The deciding settings are the server's and travel to a client (§3.5).** The original puts every
  setting that decides a recipe result or a server-side refusal into a per-installation file that a
  joining client never receives, so two sides can disagree about a repair cost or an ineffective
  list. `settings::Decisive` is a server-to-client payload carrying all twenty-six, and
  `Settings::adopt` takes them over whatever the client resolved for itself. The two presentation
  settings stay the player's own. The specification says in as many words that this is the one place
  where matching the original would be the wrong call.
  Repair material policies and tool-shape discounts are grouped in the Rust settings API;
  the deciding payload's field order is unchanged.
- **The one payload bounds its index and checks ownership (§10.1).** The original indexes the
  sender's open container with an integer the sender chose, unchecked, and asks only whether what it
  found is an attractor. `Toggle::apply` refuses a container larger than a declared ceiling, refuses
  an index outside the slot list, and refuses a slot the sender does not own, all before the slot is
  read. Nothing is changed on any refusal. The specification records this as not optional.
- **A non-positive repair multiplier is refused when the table is read (§6.3, §7).** The original
  compares a material's worth against zero for exact equality, so a negative multiplier passes that
  test and accumulates as a negative contribution — a configuration-driven way to hand a player back
  a tool more damaged than the one they laid in the grid. `overrides::Table::parse` rejects any
  multiplier that is not finite and above zero, and records the rejection beside the line that caused
  it.
- **The override table resolves lazily (§2, §7).** The original resolves every identifier in the
  table once at startup and caches the result for the process, so a later registration or a data
  reload never reaches it while a tag named in the same table does. Selectors are kept as written
  here and matched when a repair asks, so both follow a reload. Written order is preserved exactly,
  because a lookup returning the first matching row makes that order observable.
- **A selector with no colon takes the host namespace (§7).** The original treats it as a syntax
  error. The namespace is supplied by the caller rather than written here, because the host's own
  namespace is not this package's to name.
- **Each flag reads the switch it is named for (§4).** The original binds the flag named for the
  light-emitting block to the watering-tool switch, so turning the watering tools off removes the
  light block from the creative grouping and turning the light block off does nothing. Two tests here
  are written directly against that pair.
- **Each tier's armour is repaired by its own tier's material (§5.3).** The original declares the
  upper tier's armour repairable with a different tier's metal entirely, not the mineral its own
  tools are made of, which makes §6.2's default-material branch behave differently for that armour
  than a player expects.
- **The night-vision upkeep removes only what it granted (§14.3).** The original clears whatever
  night vision the wearer holds when they take the piece off, so a potion the player drank for
  disappears with it. `effects::Upkeep` tracks its own grants and leaves a wearer it never granted
  alone.
- **The upkeep is keyed on the persistent player identifier (§9, §14.3).** The original keys on the
  profile name concatenated with whether the wearer is the local player, which collides across a name
  change. Nothing incorrect could reach a save either way, because the list never outlives the
  process; the identifier is used because it is the one that is actually an identity.
- **The attractor's sweep runs on the server alone (§10.2).** The original runs the scan on both
  sides and performs the move on the server only, so the client scan produces no effect. It is run
  once here.
- **The accessory screen is recognised by capability (§10.1).** The original compares the screen's
  class name as a string. `attractor::Screen` carries the answer as a value the caller supplies from a
  real query.
- **A whole-namespace selector is refused however it is spelled (§7).** The table means to refuse a
  selector naming a whole namespace, and `tag:*` spells one: it names the namespace `tag` rather than
  a tag. `overrides::selector` refuses every `Expression::Namespace`, so the shape cannot be admitted
  by its spelling.
- **The deciding payload is adopted on a client alone (§3.5).** `Entry::receive_packet` refuses a
  frame that reaches a server, so a hostile or misrouted frame cannot rewrite the settings the server
  is the one to decide.
- **A malformed packaged document refuses the entry rather than defaulting it (§3).** Both packaged
  documents are parsed at the constructed step and a failure is returned there, so the mod cannot
  launch declaring no settings and silently running every deciding value at its default.
- **A wearer past the upkeep's ceiling is reported truncated, not gone (§14.3).** Reporting the
  surplus as having stopped wearing stripped the effect from them every tick. `Change::Truncated`
  says how many the tick did not look at, and the ceiling is `effects::MAXIMUM_WEARERS` grants.
- **The observed signals are kept to a ceiling (§2).** A server observes one signal a tick for as
  long as it runs, so `Entry::observed` keeps the most recent sixty-four and `Entry::seen` keeps the
  count whole.

Three things the specification records as read behaviour are reproduced rather than corrected, and
each is named here so the choice is not silent:

- **The toggle sound's pitch is chosen from the value being left (§10.1, §10.2).** The specification
  marks the intent unknown, and the two pitches are equally arbitrary, so there is nothing to correct
  towards.
- **The food effect durations share one accumulator (§10.3).** An earlier effect the eater already
  holds extends a later effect they do not hold at all. It is almost certainly unintended, but giving
  each effect its own accumulator changes every duration the entry produces, and the observable
  consequence of reproducing it is longer buffs rather than a defect.
- **The toggle payload's slot index is a fixed-width field (§10.1).** §10.1 states the body as one
  variable-length integer. Ravel's packet writer publishes no variable-length integer, so
  `Toggle::write` uses the fixed-width thirty-two-bit field. Both halves of this package agree on it
  and nothing outside it reads the frame, so the divergence is in the encoding alone.

## Two of §4's counts cannot both hold

§4 states seventeen flags, fourteen switches, three flags without a switch of their own, and three
switches without a flag. Those cannot all hold at once: three switches carrying no flag leaves eleven
switches bound, and eleven switches cannot give fourteen flags one each. `settings::FLAG_BINDINGS`
honours the two counts that are checkable against the rest of the specification — seventeen flags and
three unbound switches — so six flags share another flag's switch rather than three. Which four
switches carry a second flag is **inferred**, and `census::ITEM_GROUPS` is what makes the choice
observable: it is the division that sums to fifty-nine, uses each of the seventeen flags, and leaves
exactly two entries gated on two flags at once as §4 states.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision. None is stubbed.

- **Everything drawn (§14.1, §14.2, §12, §14.3).** Ravel publishes no tooltip-assembly, model,
  rendering or colour-handler surface. What the two informational lines would say is implemented in
  `src/tooltip.rs`, the per-block colour is declared data in `src/census.rs`, and the insertion,
  recolouring, rewriting, the armour model layer and its two textures are not reachable.
- **Applying a timed effect (§10.3, §14.3).** Nothing public applies a timed effect to an entity. The
  duration arithmetic and the grant bookkeeping are both here and both testable without it.
- **Five of §11's seven surfaces, and the priority raise.** Ravel's interception publishes three
  points, of which this entry answers at two. The other five surfaces have no point to answer at, and
  ordering across mods is expressed as "this mod after that one" rather than as a raised priority, so
  a counterpart cannot declare itself first. The predicate is complete and the seven surfaces and
  their differing shape lists are all in `src/refusal.rs`.
- **The recipe kind's serialisers (§5.6).** The kind is claimed as a name. Both its encodings carry
  nothing, which `repair::encoding` states and a test checks, but nothing registers a serialiser.
- **The loot and recipe document bodies (§5.7).** Seventeen loot documents and about seventy recipe
  documents are data a pack ships, and their bodies are shapes a game version decides rather than
  something this package writes; Ravel's own declaration surface says as much where it publishes the
  supplied-resource escape hatch. The flag conditioning that gates them is RVL-042's mechanism rather
  than something this package can apply to a document it does not write.
- **The tag memberships whose members are not this package's (§5.7).** Membership documents in the
  host's namespace, the conventional cross-mod namespace and the accessory dependency's name entries
  §17 keeps out of the specification, and so do §5.3's two "cannot drop" block tags, whose members are
  host blocks. A membership needs a member, so those are not declared. The memberships whose members
  this package owns are declared rather than described: `census::declare_tags` puts this entry's
  tools in RVL-042's three tool-shape tags and the two upper-tier sweeping shapes in this package's
  own item tags, and `Entry::declaration` declares the grouping's title as a translation key.
- **The accessory-slot integration (§15).** Ravel publishes no accessory-slot surface and the
  integration is with a mod outside this wave. None of it is reachable in a pack without that mod, so
  §9's round trip is unaffected by its absence.

## What RVL-042 does not yet export

- **The withheld-seed rule on its own.** §13's withholding is that entry's rule, and the only way to
  reach it is `arithmetic::sweep_harvest` at a radius of nought over a single-position offer, which is
  what `harvest::harvest` does. A `withhold_one_seed` on the public surface would say what is meant.
- **A crop-seed helper taking a block.** §16 names one, and what is published is the `Mature` value a
  caller assembles. That is workable — this package supplies the seed for both crop kinds the same
  way, which is why §13's two kinds take one path here and the kind is not carried — but the
  identification itself stays the caller's.

- **A variable-length integer on the packet writer.** §10.1's body is one, and the writer publishes
  fixed-width fields alone.
- **A tag-membership predicate.** Three settings here and the override table all need "is this entry
  in this tag", and every one of them takes it as a closure from the caller. `selection::Selection`
  answers a different question. Nothing is missing that blocks the work; the closure is the seam.

## What is written into `generated/`

Three documents, each a rendering of a table this crate already computes and none of them written by
hand. `src/dumps.rs` builds them and one test,
`dumps::tests::the_generated_documents_regenerate_byte_for_byte`, rebuilds every one from the same
functions and compares it against the bytes on disk, so a table that moves and a document that does
not is a failing test rather than a stale file. Setting `RAVEL_REGENERATE_GENERATED` in the
environment makes that test write the documents instead of comparing them, which is how they are
produced; there is no build script and no binary.

- `generated/census.json` — `census::claims`, `census::grouping`, `census::item_shapes` and
  `census::tint`: every identifier claimed in claim order, every creative-grouping entry with the
  flags gating it, the tool shape of each of the fifty-nine items, and the packed colour of each of
  the sixteen tinted variants.
- `generated/settings.txt` — the packaged declaration `settings::declaration` parses, one row per
  setting: its side, whether it is deciding or presentation, whether it needs a relaunch, its
  packaged default, and the flags `settings::FLAG_BINDINGS` binds to it.
- `generated/refusal.txt` — `Surface::watches` over `Surface::ALL` and `Shape::ALL`, which is the
  seven surfaces and their differing shape lists as a matrix.

## The names

`aliases.json` declares no mapping. The specification's §17 records that the names surface is
unresolved in the rights record, so no save-compatible identifier mapping is specified and none is
claimed here. §5.1's instruction to keep sixteen blocks rather than one is about cardinality, and
`census::claims` keeps them sixteen with generated names of this package's own.
