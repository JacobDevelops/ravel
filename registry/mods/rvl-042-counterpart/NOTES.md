# RVL-042 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-042.md`, written against
`ravel-sdk`, with `num-traits` preserving primitive floating-point cast semantics under the
workspace's strict lints. No upstream source was opened, decompiled or read to write it; the
specification is the only description of the behaviour that was used.

The entry is a shared library with no gameplay of its own, and §14 of the specification says what its
dependents actually observe. What is carried here is what §1.1 identifies as counterpart work: the
decision rules and the portable arithmetic. Everything Ravel already owns — content declaration,
configuration resolution, tag identity, saved-data ownership, side isolation — is expressed through
Ravel rather than rebuilt.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side and lifecycle | implemented | `src/lib.rs` |
| §3 configuration | implemented | `src/settings.rs` |
| §4 flag mechanism | implemented | `src/flags.rs` |
| §4 data-pack condition | declarable only | `src/flags.rs` |
| §5 tag selection | implemented, with the declared behaviour change | `src/selection.rs` |
| §5.2 the record file | implemented; logic implemented, the file left to the host | `src/selection.rs` |
| §6 registered content | declared as names only | `src/lib.rs` |
| §6 the sound event | implemented; declared and asked for through the SDK | `src/lib.rs` |
| §6.1 the five recipe kinds | declared as names only | `src/lib.rs` |
| §6.2 the two wire defects | declarable only; not reached | — |
| §7 persistence | implemented | `src/inventory.rs` |
| §8 recipe view and load events | declarable only | — |
| §9 published events, the one that is published here | implemented | `src/arithmetic.rs` |
| §9 published events, the other three | declarable only | `src/arithmetic.rs` |
| §10 reusable entries | implemented | `src/reusable.rs` |
| §11 derived arithmetic | implemented | `src/arithmetic.rs` |
| §12 client presentation | gating only | `src/settings.rs` |
| §13 the command | declared as a name only | `src/lib.rs` |

## Behaviour changes, each declared rather than incidental

- **Selection honours the preference order (§5.3).** The original folds its comparator over a
  sequence of one, so the configured order is never read and the tag's first member wins. Here the
  order is honoured, unlisted namespaces sort after every listed one, and ties break on the full
  identifier. Member iteration order is deliberately not reproduced: it is the host's and is not
  reproducible across runs, so an answer built on it would be arbitrary and is then frozen by the
  record. This makes an inert setting live, so no parity test is written against the original here.
  The change is narrow: a tag the unification path claims never reaches the rule, and a tag already
  recorded keeps its recorded answer.
- **An unreadable record still selects (§5.2).** The original leaves its table empty and stops
  selecting, so every tag-resulted recipe silently has no result until the file is fixed.
  `Selection::read_or_select` selects instead.
- **A reference that bound to nothing is diagnosable (§4).** The bind-once rule is reproduced
  exactly; `FlagReference::permanently_disabled` reports the case the original neither logs nor
  refuses.
- **A direction view defaults to permitting (§7).** The original's wrapped view answers "may this go
  here" as false with no insert predicate set, where the unwrapped store answers true. Both permit
  here.
- **An opacity from a ratio is computed in floating point (§11).** The original truncates the ratio
  to an integer first, so it can only answer zero or the maximum.
- **A saved slot count is bounded (§7).** The specification states grow-never-reduce and no ceiling,
  and a saved form is state an operator edits by hand, so an unbounded count is untrusted input: a
  payload declaring two to the fortieth slots ends the process in the allocator, which a test of the
  unbounded code proved rather than argued. `inventory::MAXIMUM_SLOTS` is a declared ceiling, far
  above any store a dependent builds, and a count above it is counted as a skipped entry under §7's
  own partial-load tolerance rather than allocated. The ceiling is a number to raise, not a rule to
  rewrite, if a dependent ever needs more slots. Bounding by the entry count present in the payload
  was rejected: a sparse store legitimately saves a large count beside few entries, so that rule
  would refuse the round trip §7 requires.
- **A bare null is read as no representative (§5.2).** The specification names two spellings, the
  empty string and the word null in any casing, both of which are strings. A hand-edited record can
  carry a JSON `null` instead, and refusing the whole document for it would discard every other
  recorded answer, so it is read as the same "no representative" and the tag is chosen again.
- **A direction's predicate is consulted in both directions (§7).** §7 says each direction's view
  consults its direction-aware predicate before delegating, and it is consulted before an extraction
  as well as before an insertion. An empty slot is taken from by nobody.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision. None is stubbed: the
package declares the shape where a shape is declarable and carries no pretend implementation.

- **The data-pack condition (§4).** No conditioned data-pack loading is public.
  `flags::condition_declaration` writes the condition down as data and nothing evaluates it at load.
- **Forcing another mod's declarations to exist (§4).** Nothing public lets a mod do that, so a
  dependent's flags are published by their owner into `Shared::flags_mut` before anything asks, and
  the ordering guarantee is met by declaration order rather than by discovery.
- **The record file (§5.2).** No public surface gives a mod arbitrary files in the configuration
  directory. `Selection::read` and `Selection::write` carry the flat map, the skipped pointer key,
  the two absent spellings and the one self-heal rewrite; the host owns the file.
- **The registered content (§6).** Ravel's registry has four kinds — block, item, recipe and command
  — so the item-level data value, the ingredient kind and the condition kind have no key to claim.
  The sound event is not among them: `ravel_sdk::audio` declares a sound entry and the event that
  plays it, so `Shared::audio_declaration` declares both and `Shared::announce_sweep` turns §11's
  "a sound plays once if anything was harvested" into a request the host drains through
  `Mod::take_play_requests`. The SDK's own caveat stands: the crossing is tested on both sides
  against a stand-in game and no launched game has played one. The five recipe kinds and the command
  are claimed as names; what a recipe kind matches, how it transfers components or damage, and its
  wire format are host work. Both wire-format
  defects of §6.2 are therefore not reached, and the symmetric encoding the specification asks for is
  not written here.
- **The recipe view and both load events (§8).** Ravel publishes no recipe-assembly interception
  point and no recipe view.
- **Three of the four events (§9).** Nothing public observes item destruction, and §8's two events
  are host-blocked with it. The cancellable harvest event is portable and is implemented:
  `arithmetic::sweep_harvest` asks its listener at each position and a cancel stops that position
  alone.
- **The command (§13).** The name is claimed; the energy store it would fill is host-blocked.

## Two smaller shapes worth stating

- **The two configuration documents are one declaration.** Ravel resolves one document per mod, so
  the client/common split is carried by each setting's side. Nothing is synchronised to a joining
  client, which is what the specification requires.
- **The preference order ships empty.** The original's default names five namespaces belonging to
  other projects. Naming them would carry their identity into this package; an empty order leaves
  every namespace unlisted, which the rule orders by identifier.
- **The numeric view over getter and setter pairs (§11) is not carried.** It exists to adapt a host
  menu surface, and there is no such surface to adapt.
- **§11's two ordering rules are carried as portable logic.** `arithmetic::Family::ORDER` is the
  fixed order a combination tool's use-on interaction tries its three families in, and
  `arithmetic::combination_use_on` stops at the first that applies, so two families that both apply
  are decided by the order rather than by the caller. `arithmetic::soak` wets every soil block in the
  cube to maximum and advances a growth step on a growable block whose own draw falls under the
  declared probability, which makes the probability per block. What maximum wetness is and what a
  step advances to are the host's; the rule decides which positions get one.

## Evidence

Every rule with a number, an ordering, a refusal or a format has an inline test. The counts, the
bind-once rule, the selection order, the grow-never-reduce load, the skipped entry, the multiple-of
refusal, the plus-one on remaining uses, the truncated half-extent, the floating-point opacity, the
self-heal setting deciding whether a lost entry is dropped, a direction refusing an extraction, a
hostile slot count being skipped rather than allocated, the fixed family order and the per-block
growth draw are all held to an assertion. `checks/verify-counterpart-build.sh` runs them.

Three were proved by mutation rather than by reading: removing the self-heal gate, ignoring the
direction predicate and lifting the slot ceiling each turned a passing test red, and the last one
ended the process in the allocator rather than failing an assertion.

`src/dumps.rs` renders three documents into `generated/` out of the same functions the tests hold:
`generated/geometry.txt` is `arithmetic::half_extent`, `sweep_positions`, `radial_positions` and
`weapon_radius` walked over declared side lengths one to thirteen, so the truncation an even side
suffers is a row rather than a sentence; `generated/reusable.txt` is `reusable::Reusable` walked over
declared use counts zero to eight, carrying the unlimited case, the one-below durability, what a
fresh and a spent entry read as, and the charge probability at each preserving level zero to four;
and `generated/settings.json` is the packaged declaration parsed, with each setting's resolving sides
computed through `settings::resolved_on` rather than restated. Nothing is transcribed: every figure
comes back from a call. `the_generated_documents_regenerate_byte_for_byte` rebuilds all three and
fails on any drift from the bytes on disk, and writes them instead when
`RAVEL_REGENERATE_GENERATED` is set, which is how they were produced.
