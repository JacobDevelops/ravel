# RVL-321 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-321.md`, written against
`ravel-sdk` and `rvl-042-counterpart`, with `num-traits` for checked numeric conversions. No upstream
source was opened, decompiled or read to write it; the specification is the only description of the
behaviour that was used.

The entry is the cluster scenario's subject, and §1.1 says what survives as counterpart work: the
decision rules and the portable arithmetic of §5, §7, §8, §9, §10, §11, §12.1 and §14. Everything
Ravel already owns — content declaration, configuration resolution, saved-data ownership, payload
declaration, side isolation — is expressed through Ravel. Everything the shared library already owns
— the persistence envelope, the flag table, the withheld-seed rule — is consumed from
`rvl-042-counterpart` rather than rewritten.

The catalyst-use count and growth-pulse interval declare an upper bound of `u32::MAX`, matching
their runtime representation. Configuration resolution refuses larger values before typed reading;
the maximum representable value remains valid.

Flag publication and `settings::flag_id` return the shared library's typed identity error instead
of silently omitting an invalid binding; configuration propagates publication failures. World
shaping and machine envelopes reuse the entry's already-declared identity, so neither operation
reparses the package constant or reports an impossible identity failure.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side, lifecycle and ordering | implemented | `src/lib.rs`, `src/registries.rs` |
| §3 the twenty settings | implemented | `src/settings.rs` |
| §3 the fourteen published flags | implemented, through the shared library | `src/settings.rs` |
| §4 the three registries and every refusal | implemented | `src/registries.rs` |
| §4 the crop registration window | implemented | `src/registries.rs` |
| §4 extension discovery | implemented, through declared services | `src/lib.rs` |
| §5 tier, crop kind and crop, with all defaults | implemented | `src/model.rs` |
| §6 registration order and identifier shape | implemented | `src/registries.rs` |
| §6 the registered content itself | declared as names only | `src/lib.rs` |
| §7 the four growth gates | implemented | `src/growth.rs` |
| §7 the two fertilisers | implemented, less the sapling branch | `src/growth.rs` |
| §7 the pulse block and its delay | implemented | `src/growth.rs` |
| §7 the armour set aura | implemented | `src/growth.rs` |
| §8 the second-drop arithmetic and both drop rules | implemented | `src/harvest.rs` |
| §8.4 the machine's withheld seed | implemented, through the shared library | `src/harvest.rs` |
| §9 moisture, drops and conversion | implemented | `src/ground.rs` |
| §9 the near-water test | implemented, reimplemented from positions | `src/ground.rs` |
| §10 the eight recipe kinds' matching | implemented | `src/recipes.rs` |
| §10 the kinds' registration and serialisers | declarable only | — |
| §10.4 the three ingredient kinds | implemented as predicates | `src/recipes.rs` |
| §10.5 the four conditions | implemented as predicates | `src/recipes.rs` |
| §11 the derivation of three recipes per crop | implemented | `src/recipes.rs` |
| §11 the contribution point | declarable only | — |
| §12.1 the upgrade table and its acceptance | implemented, with the declared change | `src/machines.rs` |
| §12.2 the fuel and energy model | implemented | `src/machines.rs` |
| §12.3 every machine's persisted fields | implemented as one versioned envelope each | `src/machines.rs`, `src/lib.rs` |
| §12.4 both rituals | implemented over a supplied ring | `src/machines.rs` |
| §12.5 the harvesting sweep and output fill | implemented, with the declared change | `src/machines.rs` |
| §12.6 the summoning attempt | implemented, with two declared changes | `src/machines.rs` |
| §12.7 the reservoir predicate | implemented | `src/cache.rs`, `src/machines.rs` |
| §12.8 the fitting rules | implemented | `src/machines.rs` |
| §13 the five item-level data kinds | implemented | `src/cache.rs` |
| §13 the four payloads | implemented; declared and encoded through the SDK | `src/cache.rs` |
| §13 the reload signal that drives them | implemented | `src/lib.rs` |
| §13 the other of the two signals | declarable only | `src/lib.rs` |
| §13 the ingredient cache and the colour map | implemented | `src/cache.rs` |
| §14 the three deposits and the substitution | implemented as data plus one predicate | `src/world.rs` |
| §14 feature and biome-modifier registration | declarable only, with the shaping set declared | `src/world.rs` |
| §15 the three states of a crop | implemented | `src/lib.rs` |
| §16 what dependents need | implemented as public functions | every module |
| §17 the catalogue and the presentation set | not carried, by the specification's own exclusion | — |

## Behaviour changes, each declared rather than incidental

- **The sixth upgrade is accepted after the fifth (§12.1).** The original orders upgrades by the tier
  value they borrow, and the sixth borrows the fifth tier's, so a machine holding the fifth refuses
  the sixth outright. `machines::accepts` orders by the upgrade's own position in the table. Every
  other multiplier in that table says the sixth is a genuine step up.
- **The output fill carries its remainder (§12.5).** The original writes a partially merged stack
  into the first empty slot at its original count, so the merged portion is created a second time.
  `machines::fill_outputs` tracks the remainder. Reproducing the original would make the machine an
  item duplicator. The slot order itself — empty first, matching stack later — is reproduced exactly,
  because that order is observable and is not the defect.
- **A summoning draws over a half-open range and accepts its final attempt (§12.6).** The original
  draws inclusively of the upper bound, so the last index folds one row beyond the square, and
  decrements its attempt counter before testing it, so an unobstructed position found on the
  twentieth try is thrown away. Both are corrected and both are tested.
- **The enchanting compatibility test is inverted back (§10.2).** The original refuses a craft when
  the target carries a different enchantment the host reports as *compatible*, which permits exactly
  the pairs that may not coexist. `recipes::Enchanting::apply` refuses on incompatibility. A parity
  test against the original disagrees on every target that already carries any compatible
  enchantment, and that is expected.
- **The eight-station ritual's document encoding is symmetric (§10.1).** The original's document
  getter hands back the interleaved eight while its document reader refuses more than four, so a
  recipe read from a data pack cannot be written back to one. `EightStation::encoded_ingredients`
  writes the four its reader accepts, and `EightStation::interleaved` keeps the observable reading
  order separately. The network path was already symmetric and is unchanged.
- **The energy amount is written in one spelling (§12.3).** The original writes it in the store's own
  form in three machines and as a plain integer in two. Both produce an integer, so the saved shape
  was already the same; one shape is written here and the divergence is only in the source.
- **Charge containers are ordered totally (§17).** The specification records that the comparator the
  original uses when distributing charge across an inventory answers on a nullness test of whichever
  operand it is handed, which is not a consistent ordering. Nothing here sorts containers with an
  inconsistent comparator: `cache::Cache` is a sorted map and every ordering in this package is
  total.
- **A saved upgrade position outside the table reads back as no upgrade (§12.3).** The original
  stores the upgrade as a table position and reads it back unchecked. `machines::Upgrade::at` is the
  checked lookup and `Persisted::load` filters through it, so a payload naming a seventh upgrade
  loses the upgrade rather than running a machine on multipliers nothing declares.
- **A saved payload that is missing a field reads back as the shipped value (§12.3).**
  `machines::Persisted::load` and `machines::Energy::load` keep the shipped capacity rather than
  refusing, which matches the partial-load tolerance the shared library's envelope already applies to
  the inventory beside them.
- **A missing or malformed machine table refuses world opening (§12.3).** Field defaults apply only
  inside recognized machine objects. A missing `machines` member, a non-object table or a non-object
  machine entry refuses the entire table before replacing in-memory state. An invalid entry's refusal
  names its machine, so malformed input cannot silently become defaults and overwrite the prior envelope.
- **Content references are validated identities.** Crafted entries and tags, charge creatures and
  modifier identities carry `ContentId`; contributions refuse malformed identifiers before registration.
  Machine recipe ticks use named `RecipeStep` inputs and a `Redstone` mode rather than positional
  timing and gating flags.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision. None is stubbed.

- **Registration of everything but a recipe kind (§6).** Ravel's registry has four kinds — block,
  item, recipe and command. The eight recipe kinds are claimed as names in `Entry::declaration`. The
  block entity kinds, menu kinds, item-level data kinds, ingredient kinds, condition kinds, armour
  materials, the creative grouping, the world feature and the biome modifiers have no key to claim,
  so the counts of §6 are not registered anywhere. What is implemented is the ordering §6 makes
  observable: `registries::registered_order` produces every crop block, then every material, then
  every seed, in the sorted order and in this entry's namespace.
- **The recipe kinds' serialisers (§10).** No recipe kind is registrable, so no serialiser is either.
  The matching and assembly rules are portable and are all here; the wire and document formats are
  host work, and only the one divergence above is recorded about them.
- **The contribution point for the derived recipes (§11).** RVL-042 §8 already records that Ravel
  publishes no recipe-assembly interception point. `recipes::derive` is the whole function from a
  crop to its three recipe declarations; nothing carries them into a recipe collection.
- **What still cannot be said about the cache's two signals (§13).** One of the two is published:
  `GameEvent::ResourcesReloaded` is "the game finished reloading resource or data packs", so
  `Entry::observe` rebuilds the colour map from the last declared set and empties the ingredient
  index on it, and §13's "cleared and rebuilt on each reload" is implemented rather than declared.
  Three things around it are not. The event does not distinguish a server data load from a
  client-side tag update, and §13's rebuild happens only for the former; nothing says "the tags were
  updated" separately at all; and nothing pushes a payload to a joining player, so what refills the
  index after a reload is still a caller. The four payload contracts themselves are real and both
  directions round-trip.
- **The world feature and the biome modifiers (§14).** Ravel publishes no feature and no biome
  modifier, so `world::DEPOSITS` is declared data and `world::place` is the one predicate the third
  deposit needs. Nothing generates. What Ravel does publish is `world_shaping::WorldShaping`, the
  set of mods a world was shaped by, and `world::shaping` declares this entry into it: a world this
  entry deposited into is refused once the entry is gone, rather than opening and generating
  differently from the chunks already on disk. That is the half of §14 that is not blocked, and it
  is the half that costs a player a world.
- **The strong fertiliser's sapling branch (§7).** It reaches a tree generator through the second of
  the two widened host members named in §1, and there is no equivalent. The other branch of the same
  item is implemented. The first widened member, the near-water test, *is* reimplemented, in
  `ground::near_water`.
- **Menus and slots (§12.8).** The fitting bench's acceptance and add-remove rules are here as
  portable logic over a declared modifier set. The bench itself is a menu and the four material slots
  name four specific crops' materials by identity, both of which are host surfaces.

## The seam §4's discovery was written onto

`ravel_sdk::services` publishes what the original's annotation scan was for. A service is namespaced
by the mod that answers it, the loader reads every declaration before anything runs, orders providers
ahead of consumers and refuses a set whose wiring does not resolve. So `Entry` provides one service,
`rvl_321_counterpart:registries`, and an extension contributes a tier, a crop kind, a modifier or a
charge type by calling it; the answer says whether the contribution was registered and renders the
refusal when it was not. The ordering §4 makes observable falls out of the loader rather than being
arranged here: this entry's own tiers and kinds are registered in `Entry::new`, and every
contribution arrives after them because a provider is composed first.

Two parts of §4 stay outside that seam and neither is stubbed. A crop is registered inside the
window that opens during block registration, which is a moment in this entry's own callback order
rather than a call an extension can make whenever it likes, so `registries::register_blocks` is
still where a crop enters. And the original's *installation* — a write that bypasses the published
surface — has no equivalent and needs none.

## Gaps found in RVL-042's exports

Two functions that §8.4 and §12 lean on are not reachable from that package's crate root. Neither was
worked around by copying the logic; both are listed here as work for RVL-042 rather than edited into
it.

- **`arithmetic::withhold_one_seed` is private.** §8.4 says the machine's harvest and the shared
  library's sweeping harvest should share one implementation. `harvest::machine_harvest` reaches it
  by calling `arithmetic::sweep_harvest` with a radius of zero and reading the drops out of the
  report, which does share the implementation but also computes a tool damage and a sound this
  caller has no use for. A public `withhold_one_seed(Mature) -> Vec<Stack>` would be the honest
  signature for both callers.
- **`inventory::Store` exposes no iterator over its slots.** `machines::fill_outputs` walks slots by
  index through `Store::slot`, `Store::limit` and `Store::place`, which is correct but is two reads
  and a write per slot where one pass would do. A `slots()` iterator would let a fill be written once.

- **`inventory::Store::place` does not hold a stack to the slot's limit.** A count above
  `Store::limit(slot)` is written as given, so every caller has to check what the store already
  knows. `fill_outputs` does check, and leaves a slot found already over its limit exactly as it
  was rather than growing it.

## Two smaller shapes worth stating

- **The two configuration groupings are one declaration.** Ravel resolves one document per mod, so
  the general and world groupings are carried by the key prefix. Every setting is declared common and
  `settings::travels_to_clients` is held to answering false, which is what preserves the
  specification's requirement that nothing is synchronised to a joining client.
- **No name in this package is a catalogue name.** §17 excludes the catalogue and says every one of
  its names is an upstream identifier. The six tiers and two crop kinds `model::shipped_tiers` and
  `model::shipped_kinds` produce are numbered and generic, and the crop catalogue is empty: a
  dependent supplies its own crops through §4's registration window, which is the machinery §17 says
  a counterpart needs.
- **The alias declaration is empty.** §12.3 states in as many words that no save-compatible
  identifier mapping is specified by the document.
- **Workspace lint integration preserves the portable rules.** Crop component registration
  switches live in `Crop::registration`; optional drops, equipment properties and deposit switches
  are grouped in `Settings::drops`, `Settings::gear` and `Settings::deposits`. Their diagnostic
  `Debug` output retains the original flat field names and order. Each independent boolean
  retains its original default and configuration key. Float-to-integer conversions explicitly
  truncate before checked `num-traits::ToPrimitive` conversion and handle out-of-range and NaN
  results to preserve saturation; the first-tier produce count uses equivalent integer ceiling
  division. Machine payload saving moves the
  inventory into the same JSON field rather than cloning it through serialization.
- **The second-drop override is an `Option`, not a -1 sentinel.** RVL-321 §5/§8 describe a crop's
  own chance at or below -1 as deferring to its tier. `Crop::second_drop_chance` is `None` to defer
  and `Some(chance)` for an explicit override, including `Some(0.0)` for no second drop; a dependent
  that writes `Some(-1.0)` is giving an explicit chance, which clamps to zero. No wire, dump, service
  or settings path carries this chance, so no serialisation point converts back to -1.

## Evidence

Every rule with a number, an ordering, a refusal or a format has an inline test: the twenty settings
and their bounds, the fourteen flags, the crop-path collision across namespaces, the global
first-wins creature claim, the closed registration window, the stable ascending sort, materials
before seeds, the two opacity constants, the absent-versus-explicit second-drop override, all four growth gates, the exemption
by identity, the truncate-before-multiply delay, the per-block aura draw, the 0.2-against-0.1 second
drop, the three independent draws, the even-tier bump, the withheld seed, the instantaneous wetting,
the quarter-chance ground drop, the moisture carried across a conversion, the exact ring assignment,
the enchanting level and both refusals, the order-sensitive emptying match, all six upgrade rows and
the acceptance rule, the burn caps, the unclamped capacity, the strictly-greater harvester
completion, the three charge amounts, the wrapping cursor over every facing, the empty-slot-first
fill and its remainder, the ritual latch, the crowd limit, the final summoning attempt, the fitting
rules over a target slot above the bench's own, the acceptance rules gating the add path, the
experience cap, the offset clamp, all four payload contracts and their round trips, and every
deposit parameter. Added with the review's findings: a thousand ticks leaving the entry exactly as
they found it, a reload rebuilding the colour map from the last declared set rather than
accumulating, a machine's upgrade and all four of its multipliers surviving a save and a reopen
through the envelope, a saved position no upgrade sits at reading back as none, an output slot
filled to its own limit and an over-limit slot stepped over, a world refused once the entry that
deposited into it is gone, a contribution refused through the service coming back named, the
fourteen flags pinned by name rather than by count, and two tiers sharing one ground block refusing
to convert into each other. `checks/verify-counterpart-build.sh` runs them: 154 tests. The added one is
`the_generated_documents_regenerate_byte_for_byte`, which rebuilds every document under
`generated/` from the crate's own functions and compares it against the bytes on disk, so a table
that moves without the artefacts moving with it fails rather than drifts. Three documents are
written there: `generated/recipes.json`, the three recipes `recipes::derive` produces for each of
the twelve crops the six shipped tiers and two shipped kinds join; `generated/machines.txt`, the
operation time, fuel usage, capacity, reach, swept side and swept area every one of the five
machines runs at with no upgrade and with each of the six; and `generated/world.txt`, the three
deposits' parameters and `world::place` read at both bound states against five draws. Setting
`RAVEL_REGENERATE_GENERATED` makes the same test write them instead of comparing.
