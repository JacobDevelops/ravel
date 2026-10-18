# RVL-025 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-025.md`, written against
`ravel-sdk` and against `rvl-120-counterpart`, which §5 and §13 name as the owner of two of the
kept component identifiers and of the typed map the payloads travel in. No upstream source was opened,
decompiled or read to write it; the specification is the only description of the behaviour that was used.

The entry is two things at once, and §16 says which half the dependents need: three admitted
counterparts declare this entry as a required dependency, and what they depend on is the published
interface surface of §15 rather than the items. A counterpart that shipped §5 to §12 and not §15
would satisfy none of them, so §15 is carried here as a first-class module and not as an appendix.
Everything Ravel already owns — content declaration and the registration barrier, configuration
resolution, saved-data ownership, side isolation, packet contracts and their framing — is expressed
through Ravel rather than rebuilt, which is §1.1's disposition.

The Rust matching API represents inclusion versus exclusion with `VerdictMode`
in `Modes::verdict`; the saved `exclusion` boolean and the `Modes` debug display
retain their original form. Machine arithmetic retains primitive single-precision
conversion semantics, and signed packet fields retain their original bit patterns.

Configuration reads return missing-key and wrong-kind errors rather than silently
substituting shipped defaults, including integer values outside the reader's range.
Saved stack scalar fields and match-rule flags default only when absent and refuse
present malformed values; saved and wire stack counts refuse overflow instead of
clamping. Saved carrier selections must name a valid slot, rather than being clamped.
Both declared server commands reject client-side invocations before reading or
mutating held state. Surface, matching, pattern, and carrier errors preserve wrapped
refusals through `Error::source`.
Surface registration owners are validated but self-asserted `ModId`s: `ServiceRequest`
does not authenticate the caller. Factory kinds, channel identifiers, and dimension
identifiers are validated `ContentId`s whose namespace must match the supplied owner.
Missing, wrong-typed, malformed, and foreign-namespace identifiers are refused before
mutation; an omitted or empty owner cannot claim an empty-namespace factory. Optional
module and dimension scalar fields default only when absent; malformed present fields
are refused without registration or replacement of an existing dimension.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side, lifecycle and the deferred phase | implemented | `src/lib.rs` |
| §2 the server-started signal | implemented at the nearest published point, declared | `src/lib.rs` |
| §2 the client-only registrations and observers | implemented as Ravel's own side isolation | `src/lib.rs` |
| §3 the four settings, their defaults and ranges | implemented | `src/settings.rs` |
| §3 the empty second document | excluded; not declared, deliberately | `src/settings.rs` |
| §3 the split between values read once and values read on every use | implemented uniformly, declared | `src/settings.rs` |
| §4 the registration census | implemented as counts | `src/census.rs` |
| §4 the two claimed empty groups | excluded; claimed by nothing | `src/census.rs` |
| §4 the creative grouping and its one exception | implemented | `src/census.rs` |
| §4 the five records and their tolerant or strict decoding | implemented, persisted through Ravel's saved-data seam | `src/data.rs`, `src/lib.rs` |
| §4 the record registered twice | implemented as one property of the record | `src/data.rs` |
| §5 the shipped document counts | implemented as counts | `src/census.rs` |
| §5 the five document defects | declarable only; four not shipped, one kept as a number, each declared | `src/census.rs` |
| §5 the machine's loot rule and its four kept components | implemented, the two shared names taken from RVL-120 | `src/machines.rs` |
| §6 the three ore blocks and their shared behaviour | implemented | `src/world.rs` |
| §6 the drop rule | implemented | `src/world.rs` |
| §6 the four placements and their effective numbers | implemented | `src/world.rs` |
| §6 the two placement modifiers | implemented, without the kind-reporting defect | `src/world.rs` |
| §6 the particle burst | declared as numbers only | `src/world.rs` |
| §7 the two slots and both insertion tests | implemented | `src/machines.rs` |
| §7 the cycle, the five ticks and the one payment | implemented | `src/machines.rs` |
| §7 the energy discount and its truncation | implemented, pinned by boundary and mid-range values | `src/machines.rs` |
| §7 the re-read ceiling and the re-read output | implemented | `src/machines.rs` |
| §7 the four capabilities | declared as names | `src/machines.rs` |
| §8 the verdict and its four steps | implemented | `src/matching.rs` |
| §8.1 the deduplicating insertion and two of the three filling routes | implemented | `src/matching.rs` |
| §8.1 the third filling route | declarable only | `src/matching.rs` |
| §8.1 the removal defect | corrected, declared | `src/matching.rs` |
| §8.1 the tooltip expansion | implemented, bounded | `src/matching.rs` |
| §9 the five-by-four grid and the output position | implemented | `src/shapes.rs` |
| §9 evaluation and the linear recipe scan | implemented, with the ceiling disclosed | `src/shapes.rs` |
| §9 the three readbacks | implemented | `src/shapes.rs` |
| §9 the disabled sub-grid predicate | corrected, declared | `src/shapes.rs` |
| §9 the menu quirk | implemented | `src/shapes.rs` |
| §10 the six slots, the two extractions and the cycle | implemented | `src/storage.rs` |
| §10 the item swap | implemented, written to the interacting hand, declared | `src/storage.rs` |
| §10 the latent failure in the replacement path | closed, declared | `src/storage.rs` |
| §11 the two modes and the mode switch | implemented | `src/tools.rs` |
| §11 the selection rules and the dimension refusal | implemented | `src/tools.rs` |
| §11 nothing here sets the stored position | implemented; reproduced as a gap, held to a test | `src/tools.rs` |
| §12 the facing lookup and the tenth-tick provider call | implemented, both defects fixed | `src/values.rs` |
| §12 the mode byte and its toggle | implemented | `src/values.rs` |
| §12 the synthesised energy provider | implemented, read once | `src/values.rs` |
| §12 the split between the two sides | implemented | `src/values.rs`, `src/lib.rs` |
| §13 the six payloads and their framing | implemented and dispatched from `receive_packet` | `src/network.rs`, `src/lib.rs` |
| §13 the five validation rules | implemented | `src/network.rs` |
| §13 the three findings a counterpart must act on | implemented; all three acted on, declared | `src/network.rs` |
| §13 the typed map the payloads travel in | implemented; the shared library's, encoded by calling it | `src/values.rs`, `src/network.rs` |
| §14 the two commands and the two key bindings | implemented; claimed at the barrier and run from `run_command` | `src/commands.rs`, `src/lib.rs` |
| §14 the leftover hand read | refused instead, declared | `src/commands.rs` |
| §14 the one reply encoding and its bound | implemented | `src/commands.rs` |
| §14 the ordering service, its five ranks and its counter | implemented, published across mods | `src/ordering.rs`, `src/services.rs`, `src/lib.rs` |
| §15 the published obligations and what each one is here | implemented | `src/capabilities.rs` |
| §15 the surface a dependent registers into | implemented; published across mods through Ravel's service seam | `src/services.rs`, `src/lib.rs` |
| §15 the registration window and every stated refusal | implemented | `src/capabilities.rs`, `src/capabilities/primitives.rs` |
| §15 the two unimplemented subsystems | declarable only; published and declared unimplemented | `src/capabilities.rs` |
| §15 the two render helpers | not carried, by §18's own exclusion | — |
| §16 the ordering service, the material tag and the intermediates | implemented and reachable by a dependent | `src/ordering.rs`, `src/machines.rs`, `src/census.rs`, `src/lib.rs` |
| §17 the recipe browser | declarable only | `src/network.rs` |
| §17 the guidebook | host-blocked | — |
| §18 screens, renderers and the host's types | not carried, by the specification's own exclusion | — |

## Behaviour changes, each declared rather than incidental

- **One configuration document, not two (§3).** The original declares a common document and adds no
  setting to it, so it resolves empty. An empty document a pack author can find, edit and be ignored
  by is worse than no document, so only the populated one is declared.
- **All four settings are resolved together (§3).** The original reads the buffer capacity and the
  accept rate once, when the machine's block entity is built, and the other two on every use, so
  editing either of the first two leaves a loaded machine unchanged until it is reloaded. All four
  are read together here. The divergence is in one direction only: this counterpart is more
  responsive, never less.
- **Every setting is declared `common` (§3).** Ravel's four sides carry no value meaning "either
  server", and the machine runs on the integrated server as well as on a standalone one. Declaring
  either server name would make the settings unresolvable in single player. Nothing is synchronised
  to a joining client either way, because Ravel synchronises no setting at all.
- **The queues are emptied at the lifecycle step rather than on a signal (§14).** Ravel's event set
  is closed and has no server-started member, so the clear happens when runtime hooks become active,
  which is the nearest published point. On a client the queues are empty and the clear costs nothing.
- **The removal loop is the correct one (§8.1).** The original scans by index and removes in place
  without adjusting the index, so two adjacent matching entries lose only the first. Insertion
  deduplicates on the same predicate, so the defect is unreachable through the shipped paths; a test
  here places the state directly and requires both to go.
- **The sub-grid predicate answers (§9).** The original demands that every stored position be inside
  the three-by-three sub-grid, and since the stored list is always twenty-one long the fourth
  position fails by construction, so the predicate is false for every card ever written. The
  predicate here asks the question of the filled input positions only. A parity test against the
  original disagrees here, on purpose.
- **The placement modifiers report their own kind (§6).** The original reports the standard biome
  check, so anything serialising a placed feature back out loses the dimension test.
- **The carrier is written to the interacting hand (§10).** The original writes the replacement into
  the currently selected hotbar index, which misplaces the carrier whenever the two differ. The
  original contains a commented-out hand-based write, so the divergence was noticed there and left
  unresolved.
- **The replacement path tests the slot (§10).** The original casts the selected slot's contents to
  the carrier-support declaration without testing it and relies on the insertion path for safety.
  Any route that writes the record directly reaches that cast, and §13's wholesale payload is one.
  The test is here and the payload's own validation closes the route as well.
- **The wholesale payload validates contents, not only kind (§13).** The original's guard is a class
  test on both ends, so a client may send a match item whose record is arbitrary. The record is
  decoded and held to its own rules before the replacement happens.
- **A readout request is bounded by distance (§13).** The original checks chunk load alone, so any
  player may read any loaded position. The squared distance is compared at 64 bits, so nothing takes
  a root of a client's numbers and no pair of coordinates wraps back into range.
- **The components payload copies one named record (§13).** The original copies the whole component
  map of an attacker-chosen stack onto the held item.
- **Wire refusals retain their cause (§13).** Oversized fields report `WireError::TooLarge`,
  while unknown readout tags and shared-library value failures report `InvalidValue` with the
  original reason. `ReadoutAnswer::new` validates and encodes the shared values before an answer
  can be sent; its values are read-only, so an overlong text cannot become an empty body in transit.
  Sender and receiver expose the same canonical values: duplicate names retain their first
  position and final value, and energy amounts take the shared map's clamp before publication.
- **The tooltip expansion is bounded (§8.1).** The original expands every stored tag to its members
  on the client for every hover, unbounded. The expansion stops at a declared ceiling and reports how
  many lines it left out. That is a number to raise, or a tooltip that pages; it is not a rule to
  rewrite.
- **The cycle command refuses an unset hand (§14).** The original reads the player's active hand in a
  way that can be unset and discards the result on the line before using it, which is a leftover
  rather than a behaviour.
- **The four record decoders are bounded (§4, §8, §9, §10).** A saved document is a file an operator
  edits by hand and a payload is bytes a client sent, so every list is bounded before it is
  allocated, every name is bounded before it is kept, and a carrier's stored selection must
  name one of its six slots. The ceilings are declared numbers to raise, far above any store this package
  builds. Bounding by what a payload claims rather than by a fixed ceiling was rejected: a claimed
  count is the attacker's number.
- **The infusion factor is read out of §3, not out of the section §7 cites (§7).** §7 says the
  discount uses "the machine's own infusion factor as RVL-120 defines it (RVL-120 §6)", and RVL-120
  §6 defines an owner, an identity, a security channel and a redstone mode and no factor at all. The
  factor here is the machine's own completed steps over §3's "upgrade steps to full infusion",
  clamped into zero to one, which is the only ceiling either specification states. It is a reading of
  §3 standing in for a citation that does not resolve, not a number invented to fill the gap, and the
  boundary values §7 does state — full price at zero, half at one — are what pin it.
- **The discount is read off the machine and not off its subject (§7).** The machine carries its own
  infusion level, written from its own record; slot one's level is read only at the completion step,
  where §7 puts it. A test drives two machines through `Machine::tick` with the same subject and
  different own levels, because a test that called the free functions could not see which stack the
  cycle reads.
- **The two ordering queues are bounded, and so is the number of them (§14).** A dependent enqueues
  from a block entity, so a queue grows with the world; a dimension name is a caller's string, so the
  key space grows with what callers name. One rank of one dimension holds at most a declared ceiling
  and the map holds at most a declared number of dimensions, both refusing past it and saying so. A
  dimension that empties is dropped at the drain.
  Queued work is published as `OrderingEntry`, distinct from the loader-owned `Entry`.
- **A server tick drains every dimension, not one (§14).** §14 puts the drain after a server-side
  *level* tick and Ravel's event set carries no dimension with its server tick. Draining one named
  dimension would leave every other dimension's queue to grow until its ceiling refused, which is a
  loss no dependent can see, so the tick drains all of them and advances the counter once.
- **The wholesale payload's two kinds are not interchangeable (§13).** §13 states the guard as "both
  must be either the match item or the handheld carrier", which reads as two independent tests. The
  guard here also refuses a match item sent onto a held carrier and the reverse, because a payload
  that names one kind and lands on the other is a client changing which object it is editing. The
  stricter reading is the one in force and a test pins it.
- **An unreadable setting is reported rather than only defaulted (§3).** A key that is present but
  is not a whole number, or is outside its declared range, is answered with the shipped default *and*
  named in a list of what went unread. An absent key is not reported, because this build declares
  four settings and an absent key is a value nobody wrote. The declaration refuses a bad value before
  a resolved configuration exists, so this stays defence in depth; what it stops being is silent.
- **The energy amounts a readout reports are 64 bits wide (§12).** §12 calls them long values and
  they are carried at that width rather than narrowed to fit a 32-bit member.
- **The two ordering clauses for a generation rule are read as one (§15).** §15 says a rule goes
  ahead of every rule added later and behind that dependent's own earlier ones. The first clause
  already fixes the answer, so rules apply in the order they were added and nothing here reorders.
  Both clauses are held to the same test, so a reader who expected the second to mean something
  separate can see which reading is in force.
- **An instruction's parameter is checked against its declared kind (§15).** §15 says every parameter
  is evaluated to the declared type before the body runs, and a body's reads are only total if
  something checks that. A parameter of the wrong kind is refused beside the two refusals §15 names.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision. None is stubbed: the
package declares the shape where a shape is declarable and carries no pretend implementation.

- **The registration groups Ravel has no key for (§4).** Ravel's registry has four kinds — block,
  item, recipe and command. Block entity kinds, menu kinds, item data component kinds, the per-owner
  attachment, the two placement modifier kinds and the creative grouping have no key to claim, so
  they are carried as the census counts and the behaviour that depends on them is written against
  this package's own types.
- **The shipped documents themselves (§5, §6).** The recipes, advancements, loot tables, tag
  documents and world-generation documents are counted and their behaviour is stated; the documents
  are not written, because the world-generation schema is a Minecraft version's shape and the
  identifiers a save-compatible document would need are the surface the rights record leaves
  unresolved.
- **The particle burst (§6).** Client-only presentation. The counts and the scale are declared and
  nothing here draws.
- **Screens, menus and renderers (§9, §10, §12, §18).** Ravel draws its own screens and §18 excludes
  them. What is carried is which values reach a screen and which side computes them, which is the
  part §12 makes load-bearing.
- **The third filling route for a match rule (§8.1).** A dependent fills a rule programmatically, and
  the published methods are what it calls; nothing here is that dependent.
- **The transport a command travels on (§13, §14, §18).** The transport belongs to the shared
  library this entry depends on and §18 excludes that library's behaviour from this specification.
  What this package owns is stated and carried: which commands it files, on which side, what a
  reply's element type is, the bound each element is held to, and which values a readout collects in
  which order. The typed map those payloads travel in is *not* in this list: it is RVL-120's, and it
  is encoded by calling `rvl_120_counterpart::values`, so the wire shape has one owner rather than
  two. That call is why the manifest lists `rvl_120_counterpart` as a required dependency.
- **The discovery half of the inter-mod message (§15).** The display-module data registry and the
  cabling surface are reached upstream through an inter-mod message rather than a capability. Ravel's
  service seam carries the registration half — a dependent registers a module, an instruction, a
  channel, a data factory, a dimension or a generation rule, and enqueues into §14's ordering service,
  through a declared service with a declared provider — and it has no equivalent for discovery, where
  a mod asks who is out there. The registrations and their refusals are carried; the discovery is not.
- **The recipe browser (§17).** Compiled against a browser's own types, inactive without it, and the
  browser is not in scope. The payload the transfer would send is declared and framed; nothing
  collects a slot view.

## Host-blocked

- **The guidebook (§17).** The guidebook is a third-party mod outside the inventory. §17 says a
  counterpart records the entry names without opening a book, and that is what happens: one item is
  registered and no entry is written. The seventeen guidebook documents are not shipped, and the one
  of them that is also a data document is the reason §5's thirteen and §17's seventeen overlap by
  exactly one.

## Two smaller shapes worth stating

- **This package ships no carrier-compatible and no cyclable item.** §16 says so and §15 calls it a
  gap rather than a rule: the carrier of §10 and the two key bindings of §14 are both untestable
  against real content inside this package alone. `Group::testable_alone` says which group that is,
  and the tests drive the carrier with stacks that declare support the way a dependent's item would.
- **The five ranks are published and unused.** §14 says the rank meanings are documented upstream
  only as a comment naming five device kinds, none of which is in this entry. They are numbered here
  and named for their position, because naming them after devices this package does not have would
  be a name with nothing behind it.

## Evidence

Every rule with a number, an ordering, a refusal or a format has an inline test, and
`checks/verify-counterpart-build.sh` runs them. The 211 assertions cover the verdict order and all
four mode flags, the deduplicating insertion and the corrected removal, the five-by-four grid and the
corrected sub-grid predicate, the five-tick cycle with its single payment at the start, the truncated
energy discount at both boundaries and at a mid-range factor, the effective placement table and the
one declared figure that differs from it, the two modifiers partitioning every level, the drop rule
under silk touch and fortune and explosion decay, the carrier's cycle at one occupied slot and at
none, the readout's tenth-tick call and its unchanged mode for an unsupported one, the ordering
service's rank order and both halves of the re-entrancy rule, every payload's round trip, and all
three of §13's findings.

Four were proved by making the counterpart's own rule fail rather than by reading it: a carrier
record carrying an unsupported item is built directly and required to refuse at the swap, a
coordinate pair at both extremes is required to stay out of range rather than wrapping into it, a
list one entry past its ceiling is required to refuse before anything is allocated, and a card whose
fourth position is filled is required to fail the sub-grid predicate that the corrected one otherwise
passes.

The nine assertions that carry the seams — the service declaration, the two answered services, the
saved-data round trip, the two claimed commands, the packet dispatch, the every-dimension drain and
the machine's own discount — were each proved the same way, by breaking the thing they name and
requiring a red suite. Reverting the discount to slot one's level fails three, draining only the
default dimension fails two, declaring no service fails one, dropping the packet route fails three,
claiming the commands as items fails two, saving nothing fails one, letting a move land where
nothing receives or a stale overlay overwrite a newer one fails one each, and the cabling group
reporting itself implemented fails two, the assertion that names it and the regenerated document.
A tenth, the §15 registrars reached through the seam, fails when the module action is renamed out of
the surface service.

The package also ships three generated documents under `generated/`, each rendered by `src/dumps.rs`
from the module that owns the figures rather than transcribed: `census.json` from `src/census.rs`
(the registered items and blocks in registration order, the creative grouping and its one exclusion,
§4's and §5's fifteen counts, and the five document defects with the one that is kept),
`capabilities.json` from `src/capabilities.rs` (§15's ten published obligations in the order the
section states them, whether each is implemented and whether it is testable inside this package
alone, and the five built-in data kinds), and `network.txt` from
`src/network.rs` (§13's six packet contracts in the table's own order, with each contract's
identifier, direction and version read back off the declared contract). The 211th assertion,
`dumps::tests::the_generated_documents_regenerate_byte_for_byte`, rebuilds all three from the crate
and compares them against the bytes on disk, so a figure that changes in a module and not in the tree
fails the suite; setting `RAVEL_REGENERATE_GENERATED` makes the same test write them instead, which
is how they are produced.

What no test here reaches is what every counterpart's host-blocked list declares: nothing has run
inside a game. The side claims come from Ravel's own side isolation and from the declared sides, and
the parity claims are claims about what the specification states rather than measurements against a
running original.

Instruction declarations use `Instruction::new(name, output)` and
`Parameter::new(kind, required)`; their public fields remain configurable. `Program::default()`
starts an empty runtime state. These records are non-exhaustive so consumers need not name every
field when the declaration vocabulary grows.
