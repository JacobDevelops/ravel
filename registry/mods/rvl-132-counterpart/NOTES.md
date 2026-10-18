# RVL-132 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-132.md`, written against
`ravel-sdk` and `rvl-210-counterpart` and nothing else. No upstream source was opened, decompiled or
read to write it; the specification is the only description of the behaviour that was used.

The entry is a bridging library with no gameplay of its own. §1.1 says what survives as counterpart
work and this package is that: the three ingredient kinds and their matching, ordering and hashing
(§6), the serialised forms (§7), the adapter table between store shapes (§8), the attachment table
(§9) and the recipe views (§11). Everything Ravel already owns — content declaration and the
registration barrier, configuration declaration and resolution, the lifecycle, the side model — is
expressed through Ravel rather than rebuilt.

## The interface vocabulary is RVL-210's, and is consumed rather than rebuilt

The wave decided that the interface vocabulary §5 lists is owned by RVL-210's counterpart, which
re-exports it at its crate root. This package declares `rvl-210-counterpart` as a dependency and
consumes every one of these from it, redeclaring none:

| What §5 names | Where it comes from |
| --- | --- |
| the ingredient-kind abstraction | `rvl_210_counterpart::quantity::QuantityKind` |
| the quantity type | `rvl_210_counterpart::quantity::Amount` |
| the match-condition set | `rvl_210_counterpart::quantity::Conditions` |
| the refusal a kind raises on stored bytes | `rvl_210_counterpart::quantity::KindError` |
| the slotless and slot-addressed storage abstractions | `rvl_210_counterpart::storage::{Store, SlottedStore}` |
| whether a call changes anything | `rvl_210_counterpart::storage::Attempt` |
| the slot a result came through | `rvl_210_counterpart::storage::Slotted` |
| the capability-constructor registry §9 declares every attachment through | `rvl_210_counterpart::capabilities::{Registry, Address, TargetClass}` |

`kinds::Ingredient` is this package's own and is an extension rather than a replacement: it is what
§6 asks of an ingredient kind beyond what RVL-210's abstraction asks of every kind — a quantity that
can be read and rebuilt, a total order, the three standard conditions, and a decomposition into
category types. Every kind here implements RVL-210's trait first.

`kinds::Flags` and `kinds::Truth` are the two condition flavours of §6.1 and both wrap or reduce to
RVL-210's `Conditions`; `Ingredient::ignoring_quantity_conditions` is the one place the two flavours
meet the vocabulary's own set, so a caller writing one filter for all three kinds has one method.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 the lifecycle, and the two orderings that are load-bearing | implemented, as a refusal | `src/lib.rs` |
| §2 the two alternative-set serialisers, at registry construction | implemented, and runnable from the table | `src/forms.rs`, `src/lib.rs` |
| §2 the three kinds, at registry population | implemented | `src/kinds.rs`, `src/lib.rs` |
| §2 common setup asserting all three resolved and failing loudly | implemented | `src/kinds.rs` |
| §2 subscribing **every** registered kind, a dependent's included | implemented, as a contract | `src/kinds.rs` |
| §2 the adapter table at the cross-mod message point | implemented | `src/lib.rs`, `src/storage.rs` |
| §2 the two compatibility bundles at the compatibility point | implemented | `src/capabilities.rs`, `src/lib.rs` |
| §2 clearing the recipe caches when a server stops | implemented, and on reload too | `src/lib.rs`, `src/cache.rs` |
| §3 four of the six settings | implemented; three implemented, one carried-but-inert | `src/settings.rs` |
| §3 the two abort settings changing what loading does | implemented | `src/lib.rs`, `src/recipes.rs` |
| §3 the two outbound-network settings | not shipped, deliberately | `src/settings.rs` |
| §3 the ignore list rebuilding the comparator on every load | implemented | `src/settings.rs`, `src/lib.rs` |
| §4 the three ingredient kinds and their registered names | implemented, names coined | `src/kinds.rs` |
| §4 the seven category types and their roles | implemented | `src/kinds.rs` |
| §4 registering nothing else anywhere | implemented, as an empty claim | `src/lib.rs` |
| §5 the vocabulary this entry compiles against | consumed from RVL-210 | — |
| §6.1 the two condition flavours and their arithmetic | implemented, one declared change | `src/kinds.rs` |
| §6.1 the three standard conditions per kind | implemented | `src/kinds.rs` |
| §6.2 the quantity, the maxima and the rebuild | implemented, two declared changes | `src/kinds.rs` |
| §6.3 the three hashes and the three total orders | implemented, one declared change | `src/kinds.rs` |
| §6.4 the four-step data comparison and the ignore list | implemented, one declared change | `src/data.rs` |
| §7 the six serialised forms and the count escape | implemented, spelling from a build-time input | `src/forms.rs` |
| §7 the charge decoder's 32-bit tolerance | implemented | `src/forms.rs` |
| §7 every other decoder refusing a tag of the wrong shape | implemented | `src/forms.rs` |
| §8 the twelve adapters, four shapes on three classes | implemented, each one adapting what an object publishes | `src/storage.rs` |
| §8 the combined item store | implemented | `src/storage.rs` |
| §8 all-or-nothing extraction by prototype | implemented | `src/storage.rs` |
| §8 a missing capability yielding an empty store | implemented | `src/storage.rs` |
| §8 the phantom slot on a slotless source | not carried, by §8's own instruction | `src/storage.rs` |
| §8 the charge adapter's narrowing at the host boundary | implemented, one declared change | `src/storage.rs` |
| §9 the vanilla bundle's 46 attachments | implemented | `src/capabilities.rs` |
| §9 the third-party bundle's one attachment | implemented, target named by the caller | `src/capabilities.rs` |
| §9 the delegation rule for the three entity attachments | implemented | `src/capabilities.rs` |
| §10 the work rules for all three station kinds | implemented | `src/readings.rs` |
| §10 the heat rules for all three readings | implemented | `src/readings.rs` |
| §10 obtaining the smelting burn timer | host-blocked | — |
| §10 obtaining the brewing fuel count and brewable test | host-blocked | — |
| §11 the general station view, its grid and its two simulations | implemented, one declared change | `src/recipes.rs`, `src/recipes/selection.rs` |
| §11 the brewing view's simulation | implemented, its starting value inferred | `src/recipes/selection.rs` |
| §11 the eight recipe-view attachments bound to their views | implemented, two flag columns inferred | `src/recipes.rs`, `src/lib.rs` |
| §11 the third cache and the base it hangs off | not carried, by §14's own instruction | `src/cache.rs` |
| §11.1 the brewing closure and its cap | implemented, cap as a setting | `src/recipes/selection.rs` |
| §12 the list-valued read-modify-write cycle and its two rules | implemented | `src/holdings.rs` |
| §12 the container item's ceiling and what it accepts | implemented | `src/holdings.rs` |
| §12 the pouch's weight-driven validity and cap | implemented, weight host-blocked | `src/holdings.rs` |
| §12 the slot count taken from the current contents | implemented | `src/holdings.rs` |
| §13.1 the fifteen access-widening entries | host-blocked, counted as data | `src/census.rs` |
| §13.2 the shipped class in the host's own namespace | not carried, blocked outright | `src/census.rs` |
| §14 the caches that hold anything, and their keys | implemented | `src/cache.rs` |
| §14 invalidation | implemented, one declared change | `src/cache.rs` |
| §15 all eleven defects | excluded; none reproduced | throughout |
| §16 what the four dependents observe | implemented, and counted | `src/census.rs` |
| §17 the excluded material | not carried, by the specification's own exclusion | — |

## One compatibility-required value this package refuses to invent

§7 makes the count-escape field's spelling a compatibility requirement and §17 records that it is
obtainable only from the pinned artifact. Nothing here guesses it. `forms::EscapeField::declared`
reads `RVL_132_COUNTERPART_COUNT_ESCAPE_FIELD` and refuses an absent or blank value, because a
package that has to agree with the original cannot fall back to a placeholder and still claim to
agree. `forms::EscapeField::placeholder` carries this package's own spelling and is what the tests
pin the surrounding **format** with: the clamp at 99, the escape field beside it, the reader that
requires it, and a form written under one spelling refusing to read under another.

The three registered kind names and the seven category-type names are **not** read from a build-time
input, and the difference is §4's own: it states that a counterpart which need not be save-compatible
picks its own. §17 records that the names surface is unresolved and that a counterpart's alias
declaration is empty, and `aliases.json` here is empty. `kinds::STACKABLE`, `PORTION` and `SCALAR`
are coined in this package's namespace rather than the game's, for the reason §13.2 gives about the
other direction: a Ravel package declares its own names and never the host's.

## Behaviour changes, each declared rather than incidental

- **Item and fluid identities are validated content identifiers (§6–7).** `Stack::of` and
  `Portion::of` take SDK `ContentId` values. A zero count or volume alone denotes absence;
  default empty values still carry valid `minecraft:air` and `minecraft:empty` identities.
  Serialised identities remain text, but the package boundary enforces Ravel's `ContentId`
  grammar, which is stricter than the game's namespace grammar. Decoders retain the rejected
  spelling and typed `ContentIdError`, including for empty strings at zero quantity, rather than
  interpreting them as empty slots. Rebuilding an empty stack or portion at a positive quantity
  is refused. Ordering retains the lexical serialized `namespace:path` spelling without allocation.
  Bucket temperature callbacks receive the validated fluid identity directly.

- **The charge kind compares and orders at full width (§6.2, defects three and four).** The original
  narrows both 64-bit values to 32 bits before comparing, so two charges differing only above the low
  32 bits compare equal, and its ordering returns the 32-bit difference of two 64-bit values, which
  overflows and can report a larger value as smaller. The kind's own maximum is the full 64-bit
  range, so both values are reachable. `Scalar::matches` compares and `Scalar::compare` compares
  rather than subtracting; a test pins a pair that a narrowing comparison calls equal.
- **Removing a condition is and-not for both flavours (§6.1, defect five).** The original's boolean
  removal returns false when the two conditions agree and the original otherwise, so removing a
  condition it does not have leaves the condition set and removal is not the inverse of addition.
  `Truth::remove` is the boolean and-not, which is what `Flags::remove` does with bits.
- **Rebuilding an empty portion at a non-zero quantity is refused (§6.2, defect six).** The original
  answers a portion of the game's default fluid, so a caller that meant "this much of nothing"
  silently receives water. `Fluid::with_quantity` refuses. Rebuilding at zero still yields the empty
  portion, which the original also does.
- **Rebuilding never aliases (§6.2).** The original returns the same instance when the quantity is
  unchanged and a copy otherwise, so a caller that mutates a returned instance mutates the original
  in the unchanged case. These are values and there is nothing to mutate through, so the aliasing is
  eliminated by construction. §6.2 states the divergence is safe because no caller can distinguish a
  copy from an alias except by mutating one.
- **Both kinds order by name (§6.3, defect nine).** The item kind's order falls back to the game's
  numeric item index upstream while the fluid kind's falls back to a registered name. A numeric index
  is not stable across differing mod sets, so an order built on it is stable within one session and
  not between two, and a dependent that persists a sorted order sees it change under an unrelated mod
  being added. Both order by name here, which also makes the two kinds consistent with each other.
- **Data with no natural order is compared structurally (§6.4, defect seven).** The original falls
  back to an identity hash, so structurally equal data compares unequal after a copy and items that
  should stack or match do not, non-deterministically. `data::Value` has a total order over every
  shape it admits, including a decimal that is not a number, so step four has no unordered case to
  fall through to and the defect cannot be expressed. A decimal is ordered by `f64::total_cmp` and
  digested by its bits, so the order and the digest agree on a value that is not a number rather than
  making every comparison of one answer unequal for ever.
- **The counterpart's own reader requires the count escape (§7, defect eleven).** The original writes
  a clamped count that a reader unaware of the extra field accepts without error, which is silent
  data loss rather than a decode failure for any count above 99. This writer supplies the field at
  the cap as well as above it, so a form carrying a count of 99 with no escape field is malformed and
  `forms::stack_from_with` refuses it. An older reader still sees 99, which is what compatibility
  costs.
- **The slotless source wrapped as a host store has no phantom slot (§8).** The original reports one
  more slot than it holds so the last index always reads empty and can be inserted into, because a
  host store interface has no insert-anywhere operation. RVL-210's `Store` has a slotless insert, so
  there is nothing for a phantom slot to work around; §8 says in as many words that a counterpart
  with a slotless insert should not have one.
- **The charge boundary saturates and says that it did (§8, defect ten).** The original narrows every
  quantity to 32 bits at the host boundary through a checked cast helper whose behaviour above the
  range the specification marks unknown. `storage::narrowed` saturates at `HOST_CHARGE_CEILING` and
  answers whether it did, and `storage::HostCharge` carries that as a flag a caller can read, so the
  kind admitting 64-bit charges no adapter can carry is a disclosed ceiling rather than a surprise.
- **The brewing view's input-count check is the range it was meant to be (§11, defect one).** The
  original's predicate is a disjunction of "at least two" and "at most four", which is true for every
  value, so every input count is admitted including zero and one and a caller relying on the check is
  not protected. `recipes::Counts` is the conjunction; a test holds the disjunction's shape apart from
  it so that a later "simplification" back to it fails rather than passes.
- **The brewing closure's cap is a setting (§11.1).** The original hard-codes 7500, which §11.1 calls
  a disclosed ceiling and a guess at where a legitimate pack ends and a cycle begins; a pack with many
  potion-adding mods could reach it legitimately. The cap stays, because the closure is genuinely
  unbounded under a badly-behaved package, and `settings::CLOSURE_CAP` is where a pack raises it. The
  setting is itself bounded at `recipes::MAX_CLOSURE_CAP`, because a cap of no ceiling would put the
  unbounded closure back, and a cap of zero would refuse a pack with any brewing at all.
- **Every cache kept here is invalidated on a data-pack reload as well as on a shutdown (§14, defect
  eight).** The original clears the station cache when a server stops and the brewing closure never, so
  on an integrated server a reload that changes a recipe leaves every station view answering from the
  old collection until the world is left, and the whole brewing list stale for the life of the
  process. `cache::Caches::invalidate` clears both at both points. §14 calls this the single most
  useful divergence in the document.
- **`diagnostics.log_unequal_data` is carried but inert.** It is declared, resolved and readable as
  `Settings::log_unequal_data`, and nothing reads it. Ravel's SDK publishes no logging facility, so
  there is no honest way to implement "log every unequal comparison" here; declaring the setting and
  quietly doing nothing under it would be worse than saying so. The two settings §3 makes change what
  loading does are wired: `compatibility.abort_on_failure` at `Entry::load_compatibility`, which
  records the failure in `Entry::bundle_failures` and carries on at the shipped default, and
  `recipes.abort_on_invalid` at `Station::enumerate`.
- **The brewing view's running result starts as the first input (§11, inferred).** §11 says the
  simulation "mixes the given inputs in order, each against the running result" without saying what
  the running result is before the first mix. Starting from the first input is the only reading under
  which the first mix has both operands.
- **Two columns of the recipe-view binding table are inferred (§11).** §11 states that only the
  cutting block checks its output and leaves the per-station drop-empty flag and count predicate
  unstated. `recipes::bound` drops empty slots for the crafting table alone, because it is the one
  station handed a grid with empty positions in it, and gives each station the count range of its own
  slots. Both are inferences from the station shapes rather than readings of the document.
- **The list-of-lists helper of defect two is not ported.** It is dead code with a boundary error and
  §15 says not to port it.
- **The two outbound-network settings are not reproduced (§3).** Both are on by default upstream, and
  the update check fetches a document whose address is assembled from the running game version. Ravel
  does not ship code that contacts a third-party service by default. `settings::DECLINED` names them
  so the absence is a declaration rather than an omission.
- **The three server-scoped settings are declared `common` rather than `dedicated-server` (§3).**
  Ravel has no "any server" side, and `SettingSide::DedicatedServer` applies to that side alone. This
  package declares `integrated-server` as a supported side, so declaring them the way §3's server
  document reads would put them on no screen in single-player: a player could never change the
  matching rule their own world runs under. This is the same reasoning RVL-120's counterpart recorded
  for its own three.
- **The two orderings §2 calls load-bearing are refusals rather than empty answers.** §2 says a
  dependent that asks for a storage adapter during its own setup has no entry to read.
  `Entry::install_adapters` refuses before common setup and `Entry::register_kinds` refuses before the
  serialisers are installed, so an out-of-order caller is told rather than handed an empty table.
- **A simulated extraction from a list-valued item touches nothing (§12).** The original splits the
  entry in place and writes the remainder back only when not simulating, which means a simulated
  extraction mutates the in-memory copy it read and discards it — safe only because the copy is
  re-read each call. `holdings::Listing::extract` answers the same number by a route that has nothing
  to discard. Nothing outside can tell the two apart; it is recorded because the specification calls
  the original's version out.
- **Every decoded count, depth and length is bounded.** Component maps, component value depth, tag
  depth and tag entry counts, candidate sets, shelf slots, list-valued item entries, station snapshot
  slots, the kind registry, the station cache, the converted collection
  and the ignore list all have a published ceiling and refuse past it before anything allocates or
  recurses. The original states none of these; a stored form and a resolved setting are both untrusted
  input.

## Declarable only, and why

Each needs a seam Ravel does not publish at the reviewed revision. None is stubbed. §13.3 states the
blocked set and `census::BLOCKED_VALUES` and `census::BLOCKED_PLACEMENT` carry it as data.

- **The smelting station's raw burn timer (§10, §13.3).** The heat rule itself is implemented and
  tested: the freezing point plus the raw remaining burn ticks, no maximum, and §10's own note that
  this is a tick count read as a temperature and not a physical quantity. What is blocked is reading
  the timer, which sits behind the access widening of §13.1. `readings::Smelting::remaining_ticks` is
  the shape a Ravel hook would fill and `Smelting::heat` answers nothing while nothing fills it.
- **The brewing station's fuel count and its brewable test (§10, §13.3).** Both sit behind the same
  widening. `readings::Brewing::work` implements the rule — work exists when the contents are
  brewable, and proceeds when the fuel count is above zero — and answers nothing until both values
  arrive.
- **The carrying pouch's weight arithmetic (§12, §13.3).** The two widened fields on the pouch and the
  one on its contents are what the weight rule reads. `holdings::Weight` is the shape a hook would
  supply; `Pouch::admits` and `Pouch::ceiling` implement the two rules over it and answer nothing
  without it. The rest of §12 — the read-modify-write cycle, both insertion rules, the extraction
  rule, the ceiling of 64 and the slot count taken from the contents — is implemented and does not
  need it.
- **The access widening of §13.1 itself.** Ravel does not widen host access and the safety boundary's
  exception table is empty. The fifteen entries are counted in `census::WIDENING_GROUPS` and the
  arithmetic behind the fifteen is a test.
- **The host-namespace placement of §13.2.** Blocked outright and needing no replacement: §13.2
  establishes that the placement obtains nothing, gets no package-private access, and carries no
  behaviour. The recipe list that class computes is specified from what it computes and is implemented
  in `recipes::Closure`.
- **Attaching a capability to an object the host owns (§1.1).** §9's table is what a dependent
  observes and it is built and installed here, into RVL-210's capability-constructor registry, exactly
  as §9 describes. What is Ravel's rather than portable is the act of attaching: the loader decides
  what a capability on a host object means. Nothing here pretends to perform the attachment.
- **Naming the third-party crafting-station package (§9).** A Ravel package declares its own names and
  never another's. `capabilities::Bundle::third_party` takes the target from its caller, which is also
  what makes the bundle present only when that package is, and an enabled bundle naming nothing is
  refused rather than silently empty.

## Gaps found in the published surface

The wave assigned the §5 interface vocabulary to RVL-210's counterpart. Four of the names §5 covers
are declared here instead, because that package publishes no counterpart of them: `forms::Candidate`,
`forms::CandidateSet`, `recipes::Definition` and `recipes::Input`, checked against
`rvl-210-counterpart`'s crate-root re-exports and its `quantity` module. The "shared serialiser
table" §2 installs is likewise the local `forms::Serialisers`. The ownership decision is unmet rather
than reversed: if RVL-210 later publishes them, these five are what this package deletes.

- **RVL-210's counterpart publishes no freezing-point constant.** §10 calls the offset every heat
  reading is stated from "a constant the vocabulary publishes", and the vocabulary's owner does not
  publish one. `readings::FREEZING` is declared here so the value sits in one place rather than being
  written into three rules. If RVL-210 later publishes it, this constant is the one line to delete.
- **`ravel_sdk::config::ValueSchema` enforces scalars only.** §3's ignore list is a list of component
  kind names and the schema subset refuses `{"type": "array"}` rather than ignoring it, which is the
  right call for a keyword nothing enforces. The list is therefore declared as a bounded string and
  read comma-separated by `settings::ignored_from`, with both a character ceiling and a name-count
  ceiling. A `string-array` kind, or an `items` keyword, would let the declaration say what it means.

## Evidence

Every rule with a number, an ordering, a refusal or a format has an inline test.

The data comparison: identical maps short-circuiting, a copy of a structural value comparing equal
where an identity hash would not, an ignored kind dropped from both the comparison and the digest, a
side holding no ignored kind not filtered at all, differing key sets compared as sorted name lists in
both directions, a differing name deciding before any value is read, shared values deciding when the
names agree, the order total across all six shapes, a decimal that is not a number sitting in the
order and digesting alike for the same bits, depth measured and refused one past the ceiling and
admitted at it, a map refused one past the component ceiling and admitted at it, a repeated name not
counting twice, and an ignore list refused past its own ceiling.

The three kinds: seven category types across the three with three primary and three quantitative, every
category naming a slot its kind understands, projection onto each category and onto nothing else,
flag testing answering yes for any flag rather than all, flag removal as the inverse of addition, the
truth flavour's and-not removal in all four cases, its or and its equality test, the charge kind's
ignoring-quantity being its match-anything, ignoring-quantity dropping only the quantitative axis for
both flag kinds, two stacks alike but for count matching under three different sets, matching
instances digesting alike, an ignored component making two stacks match and digest alike, the stack
order at all four of its levels, both kinds ordering by name, the portion order, the charge order at a
width a 32-bit difference overflows, two charges differing above the low 32 bits not matching, the
three stated maxima, rebuilding refused past each maximum and admitted at it, an unchanged rebuild,
rebuilding a portion at zero, and rebuilding an empty portion refused.

The registry: the three registered in order, a duplicate refused, common setup subscribing a
dependent's kind as well as the three, wiring twice subscribing nothing further, an unresolved kind
failing loudly with nothing subscribed, and the registry's own ceiling.

The forms: every width round-tripping, a width the form does not name refused, a number outside its
width refused rather than wrapped, a tag refused one past the depth ceiling and admitted at it, a tag
refused past the entry ceiling, a stack under the cap round-tripping without the escape field, a stack
above it writing a clamped count and the true one, a stack of exactly the cap still carrying the
field, a clamped count with no field refused rather than read as the cap, a field of another spelling
not read, the spelling refused for absence and for blankness and agreeing with whatever this build
supplied, the placeholder being this package's own, the empty stack and the empty portion each writing
an empty compound, the item decoder refusing anything that is not a compound, a stack's data
round-tripping across four shapes, a decimal component surviving by its bits, a named item with no
count refused, the fluid decoder refusing the wrong shape, a charge round-tripping across the wide
range, the charge decoder widening the older 32-bit form and refusing every other shape and a negative
value, a flag condition round-tripping including the top bit through the signed form, a condition
decoder refusing the wrong shape, a truth condition as a byte of one or zero with a third value
refused, both kinds reading and writing through the vocabulary's own methods, a candidate and a
candidate set round-tripping in order, a candidate missing either field refused, the candidate set's
ceiling at both ends, a set that is not a list refused, the serialiser table taking exactly two
contributions and refusing a second run, a name resolved out of the table handing back something that
runs and round-trips, the single-candidate serialiser round-tripping an empty set, a dependent's
serialiser filed under a name but resolving to nothing this package would run, a name the table does
not hold resolving to nothing, a dependent's serialiser not colliding, and a data map and a value
depth refused at the read.

The adapters: twelve installed with no repeats, the shapes naming their kinds, an adapter over the
shape its object publishes handing back that store, an item adapter over both halves building the
combined store under either item shape, an adapter whose half the object does not publish answering
the empty store while the other shape does not, every one of the twelve answering the empty store over
an object publishing nothing, an adapter of one shape refusing another shape's store, an empty store for a
missing capability answering nothing on every method, a shelf capacity as the sum of its slots, a
shelf refused past the slot ceiling and admitted at it, insertion filling a matching slot before an
empty one, insertion capped per slot, a simulated insertion answering the same number and changing
nothing, extraction accumulating across slots, a partial match committing nothing, a partial
extraction available for a caller that wants one, a remainder left where one slot holds more than
asked, a simulated extraction committing nothing, extraction of nothing taking nothing, the filtered
walk, a pool folding one entry per distinct instance and reporting its total, a pool capped by the
whole store's capacity, pool extraction all-or-nothing, the combined store reading the slotless half
and indexing the slotted one, falling back to the slotted half, answering the empty answer over
neither half, a wrapped slotless source reporting no phantom slot and none at all when empty, a
wrapped source still taking an insertion, the charge boundary saturating and saying so at three
values, an insertion above the host ceiling saturating rather than wrapping, a transfer inside the
range not marked saturated, a simulated transfer changing nothing, and the fluid kind through the same
adapters.

The attachments: 46 in six capability kinds with the six counts and their sum, six kinds named and no
others, three from the vocabulary and three from the platform, no attachment made twice, the two
by-class attachments being the only broad ones, eighteen item-class item stores, all three stores on
all three carriers and one on the composting block, the whole bundle installing, a disabled bundle
installing nothing, the third-party bundle making one, an enabled third-party bundle naming nothing
refused, a bundle refused after the registry is baked, both bundles reaching one registry, an entity
holding an item that publishes no store publishing none either, an entity holding nothing, operations
forwarding to the held item's store, an operation on an entity publishing nothing answering the empty
answer, the store appearing and disappearing as the held item changes, and a caller reading across a
change seeing the new item.

The readings: work existing only when a recipe yields something, an empty input slot and an absent one
being no work, a recipe assembling to nothing being no work, proceeding when burning or holding fuel
that burns and not on fuel that does not or an empty fuel slot, running needing both, the smelting
heat as the freezing point plus the tick count with its minimum, absent maximum and default, the same
reading answering nothing where no hook supplies the count, a brewing station's work needing both
blocked values and answering with them, a campfire's work on any non-empty slot and its lit test, its
heat reading the lit value and carrying it as a maximum, its snapshot bounded at the slot ceiling, a
bucket reading its fluid's declared temperature, an empty bucket and a fluid declaring none reading
the freezing point.

The recipe views: enumeration keeping only this kind's ordinary convertible recipes, an empty world,
the grid always three by three and filled in order, mixed kinds refused, empty inputs refused after
the optional filter, a view that keeps empty slots, a count outside the range refused with the
disjunction's shape held apart from it, more inputs than the grid holds refused, a simulation
answering the matching result and answering nothing without one, output checking needing exactly one
named instance and picking the recipe matching both halves, the first recipe winning without it, the
brewing closure walking out from the water bottle, each result being a four-position recipe producing
nothing in the first, a repeated result not added again, an empty or refused output dropped, a cycle
refused at the cap rather than running forever, the cap bounded at both ends, the candidate sets
matching the ingredient and the input and not a third item, a grid matching only a recipe
of the same length, and a set admitting several alternatives.

The item stores inside items: an empty container publishing no slots, the slot count taken from the
contents and going stale in both directions, a list refused past the entry ceiling and admitted at it,
an insertion past it appending nothing, an insertion into the slot past the end appending and one
beyond that not, a merge capped per slot, the whole input coming back for another item and for
differing data, a simulated insertion writing nothing back, an extraction splitting and writing the
remainder back, one that empties the slot removing it, a simulated extraction leaving the list exactly
as it was, an extraction taking no more than the slot holds, an extraction from an absent slot and of
nothing, the container's ceiling and what it accepts, a pouch with no supplied weight answering
nothing on all three, a pouch admitting an item only when the weight takes at least one across three
cases, its insertion capped by weight, a pouch with room to spare still capped at 64, and a weightless
item admitted by nothing.

The caches: a station collection converted once per key, the dimension being part of the key, the
station ceiling, the brewing closure derived once, a refusing derivation holding nothing and being
tried again, a reload invalidating both where the original invalidates neither, a shutdown doing the
same, and a fresh set being empty.

The settings: the document parsing and naming five settings in order, neither outbound setting being a
key under any of five spellings, four of six plus one added, the three server-scoped ones declared
`common` and the diagnostic one `client`, the shipped values, an empty list, a list trimmed sorted and
deduplicated, a list refused past either ceiling and admitted at the name ceiling, the comparator
rebuilt from the values in force, and the closure rebuilt from the cap in force including two out-of-
range caps falling back.

The entry: claiming nothing at the registration barrier, construction installing the two serialisers
and nothing else, registering the kinds before the serialisers refused, the adapter table before
common setup refused, registration registering three kinds and wiring them and installing twelve
adapters, a dependent's kind wired too, the vanilla bundle installed and the registry baked, the
third-party bundle making 47, a resolved ignore list reaching both data-carrying kinds, a resource
reload invalidating every cache, a tick invalidating nothing, a server stopping invalidating
everything, the settings document declared, the lifecycle steps kept in order, and three distinct late
lifecycle points.

The census: the widening groups adding up to fifteen two ways, the shipped sources against the harness
and companions, the shipped resources, four blocked values and one blocked placement, the recount
naming a set in both directions with no entry on two sides at once, four dependents observing five
things, and what this package carries agreeing with what the census counts.

The generated documents: `generated/capabilities.txt` is the vanilla bundle's 46 attachments as
`capabilities::vanilla()` tables them, `generated/storage.txt` is the twelve adapters as
`storage::table()` builds them, and `generated/census.json` is the counts of `census` with the sums
and products this package derives from them, including the per-capability attachment counts folded
out of the same table. `dumps::documents()` renders all three from those functions and nothing else, and
`the_generated_documents_regenerate_byte_for_byte` rebuilds each one and compares it against the
bytes in the package, so a table that changes without its document fails. Setting
`RAVEL_REGENERATE_GENERATED` makes that test write the documents instead of comparing them, which is
how they are produced.

`checks/verify-counterpart-build.sh` runs them: 278 tests.
