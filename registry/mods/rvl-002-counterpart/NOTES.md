# RVL-002 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-002.md`, written against
`ravel-sdk`, `rvl-042-counterpart` and `rvl-321-counterpart` and nothing else. No upstream source was
opened, decompiled or read to write it; the specification is the only description of the behaviour
that was used.

The entry is a leaf of its wave's content cluster, and §1.1 says what survives as counterpart work:
the fuel derivation of §9, the food effect rule of §10, the tool grade table and dispatch rule of
§11, the boss drop rule of §12, the ground behaviour it shares with §8, and the absence rules of §6.
Everything Ravel already owns — content declaration, tag identity, configuration resolution, side
isolation, world shaping — is expressed through Ravel. Everything the two dependencies already own —
the tier and crop model, the three absence states, the ground drops and conversion, the flag table,
the recipe kinds — is consumed from them rather than rewritten.
This entry owns no shared persistence and has no dependency traffic with its sibling in the same
cluster. Those dependency facts live here rather than as callable functions returning fixed prose.
Companion absence disables recipes without unregistering the crop's block and item identities.
Crop construction and tier adjustment accept validated `ContentId` values for materials, ground
and foundation blocks, matching the dependency's typed model rather than passing unchecked strings.
Boss drops receive validated `ContentId` values and use the shared stack's zero-count emptiness
contract; ground drops and the fall rule remain the dependency's implementation.
Unconsumed spec-count exports and alias functions are omitted; catalogue counts are derived by
the generated census rather than exposed as duplicate API.

## Root workspace lint migration

The crate inherits the root workspace's strict Clippy rules. Public `Settings` groups each
dimension's deposit switches into `world_lower` and `world_outer` values of type `Deposits`,
whose `shard` and `essence` fields replace the four flat world-generation fields. Configuration
keys, defaults, flag publication and generation rules remain unchanged. Derived Debug output
reflects the grouped fields; it is diagnostic rather than a compatibility contract.

Flag publication and the shared `flags::setting_flag(namespace, key)` return typed identity errors instead
of silently omitting an invalid binding; configuration propagates publication failures. World
shaping uses the entry's already-declared identity and is infallible rather than parsing the
package constant again.

Tool attachments implement `tools::ToolModifier`, whose `offer` and `tick` methods receive
interactions and inventory ticks without short-circuiting the remaining attachments. The entry
keeps its published flag table private and exposes its five configured values through `flags`;
unpublished or malformed identities receive `{"answered": false, "refused": "reason"}` replies.
Published values receive `{"answered": true, "enabled": bool}`; caller mistakes never fail the
provider. A refused or malformed dependency reply fails this consumer without changing its flags.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side, lifecycle and ordering | implemented | `src/lib.rs` |
| §2 the two client integration shims | not carried, by the specification's own exclusion | — |
| §3 the eight settings and their bounds | implemented | `src/settings.rs` |
| §3 the five published flags | implemented, answered by this mod's `flags` service | `src/settings.rs`, `src/lib.rs` |
| §3 the two consumed flags | implemented as reads of the dependency's namespace | `src/settings.rs` |
| §4 the extension's namespace declaration | implemented as this entry's own namespace | `src/lib.rs` |
| §4 the opt-out of all three derivations | implemented | `src/crops.rs` |
| §4 registering the callbacks with the dependency | declarable only | — |
| §5 the sixth tier, four values and four adjustments | implemented | `src/tier.rs` |
| §5 the second-drop flag this tier turns off | implemented | `src/tier.rs` |
| §6 the six crops and their materials | implemented | `src/crops.rs` |
| §6 the three states of absence | implemented, through the dependency | `src/crops.rs` |
| §6 the anchor pointing, unconditional | implemented | `src/lib.rs` |
| §7.1 twenty-six blocks and their properties | implemented, claimed at the barrier | `src/census.rs`, `src/lib.rs` |
| §7.2 fifty-three items, block items first | implemented, claimed at the barrier | `src/census.rs`, `src/lib.rs` |
| §7.2 the percentage hover line | implemented, with the declared change | `src/census.rs` |
| §7.3 the creative listing and its order | declarable only, with the order carried as data | `src/census.rs` |
| §7.4 the six fluid families, thirty registrations | declarable only | `src/census.rs` |
| §7.5 the colour handler | declarable only, with the tint indices carried | `src/census.rs`, `src/tier.rs` |
| §8 the sixth-tier ground, drops and fall rule | consumed from the dependency | `rvl-321-counterpart::ground` |
| §8 conversion gated on the dependency's flag | implemented | `src/ground.rs` |
| §9 all twelve fuel values, as two derivations | implemented | `src/fuel.rs` |
| §10 the six food rows and the cumulative order | implemented | `src/foods.rs` |
| §10 the additive duration and the fixed amplifier | implemented | `src/foods.rs` |
| §10 the hover line's cache | implemented, with the declared change | `src/foods.rs` |
| §11 the six-row grade table | implemented | `src/tools.rs` |
| §11 zero durability as unbreakable | implemented | `src/tools.rs` |
| §11 the repair material resolved at first repair | declarable only; stated as a timing rule, no holder type | `src/tools.rs` |
| §11 modifier dispatch across five points and the tick | implemented | `src/tools.rs` |
| §12 the boss drop rule and its ordering | implemented, with the declared change | `src/drops.rs` |
| §13 the side split | implemented, through Ravel's own side isolation | `src/lib.rs` |
| §14 the recipe census and its two counts | implemented as data | `src/recipes.rs` |
| §14.3 the three conditions and what each removes | implemented as predicates | `src/recipes.rs` |
| §14 the recipe documents themselves | declarable only | — |
| §15 the four deposits and their parameters | implemented as data | `src/world.rs` |
| §15 feature and biome-modifier registration | declarable only, with the shaping set declared | `src/world.rs` |
| §16.1 the loot shapes and the one code path | implemented as data plus the ground's own path | `src/data.rs`, `src/ground.rs` |
| §16.2 the forty-eight tag documents | implemented, declared through Ravel's tags | `src/data.rs` |
| §16.3 the twelve language key counts | implemented as data | `src/data.rs` |
| §16.4 what survives a save and a reload | implemented | `src/data.rs`, `src/lib.rs` |
| §17 what this entry needs, as public functions | implemented | `src/lib.rs` |
| §18 the catalogue, its names and its presentation | not carried, by the specification's own exclusion | — |

## Behaviour changes, each declared rather than incidental

- **The boss draw comes from the level's source (§12).** The original draws from a process-wide
  random source, so its wither-class drop is not reproducible from a world seed and is not
  synchronised with anything else in the world's generation or tick. `drops::on_death` takes the draw
  as a parameter, which the caller supplies from the level. That makes the drop seed-reproducible,
  which is strictly better for testing, and it disagrees with the original on any replay comparison.
- **The percentage is formatted rather than concatenated (§7.2).** The original multiplies the stored
  fraction by a hundred and concatenates a percent sign, so a default of 0.35 renders as a
  seventeen-digit expansion. `census::chance_text` formats the value. A pixel-level presentation test
  should expect the disagreement, and the specification records that it is a defect rather than a
  rounding choice.
- **The food hover line is not cached (§10).** The original builds the line once per tier and holds it
  for the process lifetime, so a settings change mid-session leaves the display stale while the
  applied duration follows the new value. `foods::duration_text` recomputes, which the specification
  itself calls the right trade. It disagrees with the original under a mid-session change and agrees
  everywhere else.
- **A duration that would overflow saturates (§10).** The original adds without bound. `foods::extended`
  saturates, because a wrapped sum would come back as a *shorter* effect than the one already running,
  which inverts the rule the section exists to state. Nothing reachable in normal play differs: the
  configured maximum is one second short of the type's own.
- **Declared defaults and typed setting errors have separate paths (§3).** Configuration resolution
  supplies shipped defaults and rejects values outside the document's bounds before typed reading,
  so a chance above one or a scale count above sixty-four cannot reach the drop rule.
  `Settings::read` propagates missing-key and wrong-kind errors instead of replacing them with defaults.
  Packaged defaults are fallible too: construction retains a parse/read refusal and reports it at
  Constructed rather than panicking before the loader can attribute the failure.
- **An ore drop count is bounded against its declared span (§15, §16.1).** `Deposit::dropped` clamps
  the roll it is handed, so a source answering anything at all still yields a count inside the two-to-
  five or three-to-five range the loot document declares.
- **Cross-mod flags travel through the SDK service seam (§8, §11).** This entry requires rvl-321's
  `flags` service and resolves its two consumed flags during service wiring. Ground conversion and
  tool enchantability receive those booleans, never this entry's unrelated local flag table.
  Missing or malformed service answers refuse wiring rather than silently reading an unpublished flag.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision. None is stubbed.

- **Registering the extension callbacks with the dependency (§4).** Ravel publishes no equivalent of
  another package's registration callback. What it does publish is `ravel_sdk::services`, and the
  dependency answers one — `rvl_321_counterpart:registries` — which a tier, a crop kind, a modifier or
  a charge type is contributed through. This entry's §5 and §6 are therefore ordinary declaration
  against Ravel's own registration window: `Entry::register` claims at the barrier and `Entry::adjust`
  is the second pass, and the ordering guarantee comes from Ravel's load order rather than from the
  dependency's §2. That is the trade the specification names, and it is recorded here rather than
  worked around.
- **The six fluid families (§7.4).** Ravel publishes no fluid, fluid-type or fluid-block declaration,
  so all thirty registrations are blocked. Nothing else in the entry depends on them: no recipe
  consumes them, no crop needs them, and they do not appear in the reload path. `census` carries the
  shared property set, the per-family tint arrangement and the bucket's stack limit, because those are
  what a parity run measures. The buckets' crafting remainder is the shared library's `reusable`
  behaviour and needs nothing new.
- **The creative listing (§7.3).** There is no listing registry to claim. The fixed order is carried as
  `census::LISTING` and the one section that is guarded is `census::guarded`, so the arrangement a
  flag-disabled item drops out of is expressible; what is missing is the listing itself.
- **The colour handler (§7.5).** Client-only tinting has no published seam. The tier colour is
  `tier::DISPLAY_COLOUR` and the bucket's tint index is `census::BUCKET_TINT_INDEX`; nothing renders.
- **The world feature and the biome modifiers (§15).** Ravel publishes no feature and no biome
  modifier, so `world::DEPOSITS` is declared data. Every rule here is expressible in the host's own
  declarative vocabulary, so unlike the dependency's §14 no portable predicate is needed at all; the
  double biome gate is `world::admits` because both tests are observable. What Ravel does publish is
  `world_shaping::WorldShaping`, and `world::shaping` describes this entry in that declaration.
  The published host does not admit this set or enforce saved-world removal refusals.
- **The recipe documents (§14).** Ravel claims a recipe kind as a registry entry and decides nothing
  about what one does. This entry owns no kind at all — every recipe it ships is data against a kind
  one of its two dependencies claims — so there is nothing here to claim and nothing to serialise. The
  census, the conditions and what each condition removes are all portable and are all implemented; one
  document shape is written out, `ground::tilling_document`, because §8 turns on it.
- **The two third-party integration shims (§2).** The specification excludes them beyond their gating
  and states that they add no behaviour. The commented-out third integration is dead code at the
  reviewed revision and is not carried; §14.4's consequence of shipping its data anyway *is* carried,
  as the difference between `recipes::shipped` and `recipes::loaded`.

## Two shapes worth stating

- **Blocks and items are claimed, not merely counted.** Ravel's registry has four kinds and two of them
  fit this entry's census, so the twenty-six blocks and fifty-three items are real entries in
  `Entry::declaration` and reach the barrier through `claim_all`. The order §7.2 makes observable falls
  out of that: every block is claimed before any item, and `census::block_item_paths` puts the twenty
  block items at the front of the item registry ahead of the thirty-three plain ones. A parity run that
  compares order and not just membership sees it.
- **Nothing this entry declares travels to a joining client.** Every setting is declared common and
  tests check directly that the declaration carries no client-bound settings, which preserves §2's arrangement: a client
  and a server that disagree about §3 disagree silently, and this entry ships no payload that would fix
  that.

## Gaps found in the dependencies' exports

Neither was worked around by copying logic; both are listed here as work for those packages.

- **`rvl_321_counterpart::model::Derived` has no constructor for the full opt-out.** `Derived::default`
  turns all three derivations *on*, and `Derived::narrowed_by` needs a second value to narrow against,
  so `crops::OPT_OUT` is written as a struct literal naming all three fields. A `Derived::none()` would
  say what §4's opt-out means in one place, and an extension that opts out of all three is exactly the
  case that section describes.
- **`rvl_321_counterpart::registries::Registries` has no way to register a crop from outside a
  window.** `register_crop` is public and `Registries::open_crop_window` gates it, but the contribution service
  refuses a crop in as many words — "a crop is registered inside the window rather than through the
  service" — so an extension that declares crops has no published path to that entry's crop table at
  all. This package therefore holds its own six crops in `Entry` and does not contribute them. A
  service argument shape for a crop, or an explicit statement that a dependent keeps its own, would
  close it; today the two surfaces disagree.
- **Flag tables are mod-owned.** rvl-321's flag service checks publication before answering, preserving
  the distinction between a missing flag (`answered: false`) and a published false value
  (`answered: true`, `enabled: false`).

## Evidence

The crate root exposes the public modules and owns `Entry`, its loader callbacks, declaration,
and dependency-flag resolution. The entry's private-state regressions live in its inline `tests`
module. `crops::availability` names the dependency's three-way availability result explicitly.
Each deposit switch and published flag is tested with only its own configured value inverted,
including the default-off fertiliser flag.

Every rule with a number, an ordering, a refusal or a format has an inline test: the eight settings
and each of their bounds, the five published flags pinned by name and namespace, the two consumed
flags proved to belong to another namespace and never published here, the tier sitting exactly one
above the dependency's highest, the display colour at full opacity, the second-drop flag turned off
whatever the tier arrived holding, the accelerant switch copied rather than read live, the six crops
with four naming one companion each, all three absence states kept apart including the third reached
through the own-tag crop, the development escape hatch, the anchor pointing surviving an unobtainable
crop, the block groups adding to twenty-six and the block items to twenty, the thirty-three plain
items named by group, the thirty fluid registrations across four registries, the block items at the
front of the item registry in the block registry's own insertion order, the ore experience range, the
listing order and its one guarded section, the formatted percentage, all twelve fuel constants against
the derivation, a block burning exactly what its nine items would, a tier outside one to six refused
rather than extrapolated, the six food rows and both nutrition steps, a tier granting the first *n*
effects, the fixed amplifier at every tier, six foods stacking to six times the duration, a lower-tier
food refreshing a higher one's shared effects at the same strength, a saturating duration, the
zero-padded minutes rendering with no upper bound, all six tool grades, zero durability as unbreakable
at the top two grades, the top grade sharing the fifth's modifier tier and adding one slot,
every modifier seeing every interaction after one has acted, the base break result discarded, the game
rule tested before any draw, the certainty of the dragon-class count and its zero switch, a repeated
death dropping again, the same seeded draw producing the same drop, the silk-touch and quarter-chance
ground drops including a draw far above a hundred, unconditional fall damage with the block surviving,
conversion gated on the dependency's flag and on none of this entry's five, the shipped and loaded
recipe counts differing by exactly seventy-seven, every crop document's conditions and what each one
removes, the default installation loading no crafting recipe of the six, the compression family
agreeing with the fuel arithmetic, the awakening's component transfer, the four deposits two to a
dimension, the shard deposits rarer than the essence ones at both, identical absolute height bounds in
two unlike dimensions, each switch gating exactly its own deposit, the double biome gate declining on
either test, an ore drop bounded inside its range for any roll, nineteen loot documents and one code
path, forty-eight tag documents with the two mirrored families stating each fact twice, the twelve
language key counts including the two stale and the two over-length, no saved-data declaration and no
envelope, a thousand ticks leaving the entry exactly as they found it, five reloads changing nothing,
and a world reloaded without a companion mod keeping its identifiers.

Three documents are rendered out of these tables by `src/dumps.rs` and kept under `generated/`:
`generated/census.json` walks the block and item registries in their own order, with the group table, the
derived counts and the listing's five sections; `generated/foods.json` applies the cumulative effect rule at
every declared tier; and `generated/fuel.txt` is the twelve burn times taken from the two derivations rather
than from the constants they reproduce. `the_generated_documents_regenerate_byte_for_byte` rebuilds all three
and compares them against the bytes on disk, so a table that moved without its document failing is not a
state this package can be in; setting `RAVEL_REGENERATE_GENERATED` makes the same test write them.

`checks/verify-counterpart-build.sh` runs the counterpart unit suites. The root
`tests/conformance/tests/integration/registry_lifecycle.rs` suite drives all twelve
counterparts through the real loader registration barrier and service admission, both
with only each entry's required dependencies and as a full manifest-resolved set, on client, integrated-server and
dedicated-server sides. It compares manifest saved-data versions with each host's
declaration, checks running states and declared key ownership, and pins independent
per-kind and total registration censuses, and requires world-access readiness. The combined
round asserts declared after-dependency order against the driven host sequence.
The minimal rounds include the transitive mandatory manifest dependencies: this leaf cannot
honestly start alone while requiring the crop entry's flag service. The configured-service
regression drives real loader configuration and startup with all four boolean combinations;
the leaf's SDK-only unit regression also plants contradictory local constants before service
calls, proving that only the configured dependency's answers control the consumed flags.

## Public contract consistency

Reload counts are exposed as `reloads_taken`. Shaping sets describe declarations only; the published
host does not enforce saved-world removal refusal.
