# RVL-210 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-210.md`, written against
`ravel-sdk` and nothing else. No upstream source was opened, decompiled or read to write it; the
specification is the only description of the behaviour that was used.

The entry is the wave's foundation seed and depends on nothing. Section 1.1 says what survives as
counterpart work once Ravel's own facilities are used for what Ravel already owns: the wire contracts
of §6 and §7, the collection and storage semantics of §10, the query grammar of §11, the shape
detector of §12, and the registration vocabulary of §5 that dependents declare against. Content
declaration and registration, settings declaration and resolution, saved-data ownership, side
isolation, screen drawing and network payload registration are Ravel's, and this package expresses
itself through them rather than rebuilding the framework that is where most of the original's 591
files go.

**This package owns the interface vocabulary of §10.** §18 raised it as the thing blocking both this
counterpart and RVL-132's, and the wave decision settles it here: `quantity::QuantityKind`,
`quantity::Conditions`, `quantity::Partitioned`, `storage::Store` and `storage::SlottedStore` are
declared in this crate and re-exported at its root. §17's "what dependents need" list is the crate
root's public surface.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side, lifecycle and the two table halves | implemented | `src/lib.rs` |
| §2 the client-only crafting helper tables as a runtime side test | implemented | `src/lib.rs` |
| §2 the three advancement-trigger hooks | implemented as declared data | `src/lib.rs` |
| §2 the load-complete latch | implemented | `src/lib.rs` |
| §3 the two settings, their scope and their grouping | implemented | `src/settings.rs` |
| §4 the sixteen declared entries and the fifteen that register | implemented | `src/declarations.rs` |
| §4 the seven storable identifiers | declarable only; carried as this package's own names, never the original's | `src/declarations.rs` |
| §5.1 the thirty-four kind vocabulary | implemented as data | `src/declarations.rs` |
| §5.1 disableable, and the refusal for one that is not | implemented | `src/declarations.rs` |
| §5.1 kind-at-a-time registration in declaration order | implemented | `src/declarations.rs` |
| §5.1 the client companion as a declaration-time split | implemented | `src/declarations.rs` |
| §5.2 the three-value scope vocabulary and its consequence | implemented | `src/settings.rs` |
| §5.3 runtime read and write through the command tree | implemented, with the declared change | `src/settings.rs`, `src/lib.rs` |
| §6 the field-ordering rule | implemented | `src/transport.rs` |
| §6 the type-to-encoder table and both halves of its loading | implemented | `src/transport.rs` |
| §6 the thirteen message kinds and the position base | implemented as data | `src/transport.rs` |
| §6 the three delivery targets and the fifty-block radius | implemented | `src/transport.rs` |
| §6 the portable encoder list the original enumerates | declarable only; four kinds, named here rather than transcribed | `src/transport.rs` |
| §7 the value channel and all seven readers | implemented | `src/channel.rs` |
| §8 the slot-range vocabulary and shift-click transfer | implemented | `src/menus.rs` |
| §8 the store, its change mark and its change counter | implemented, with the declared change | `src/menus.rs` |
| §8 the indexed wrapper answering without a scan | implemented | `src/menus.rs` |
| §8 the widget toolkit and everything drawn | not carried, by §18's own exclusion | — |
| §9 the hard-coded account list and the outbound fetch | excluded; deliberately not reproduced, by the specification's own instruction | — |
| §9 the version-check service and the analytics beacon | not carried; both are dormant at the pinned revision | — |
| §10 the kind, its conditions, its matcher and its serializer | implemented, and owned here | `src/quantity.rs` |
| §10 list, chain, collapsed, summing view and classified | implemented | `src/quantity.rs` |
| §10 the partition restriction | implemented as one rule in one place, with the declared change | `src/quantity.rs` |
| §10 the storage interfaces, slotted and slotless | declared here and implemented by dependents | `src/storage.rs` |
| §10 the move and its conditional raise | implemented | `src/storage.rs` |
| §10 the difference tracker | implemented | `src/storage.rs` |
| §11 the twenty-two handlers in the published order | implemented, with the declared change | `src/query.rs` |
| §11 evaluation as a set of matches | implemented | `src/query.rs` |
| §11 navigation over the parsed form | implemented | `src/query.rs` |
| §12 the detector, its constraints and its failure detail | implemented, with the declared change | `src/shapes.rs` |
| §13 annotated field persistence and the type table | implemented | `src/persistence.rs` |
| §13 the per-world counters and their guarantee | implemented, through Ravel's saved-data ownership | `src/persistence.rs`, `src/lib.rs` |
| §14 the dumping service and its seven producers | implemented | `src/dumps.rs` |
| §14 the manual engine | not carried; §14 itself says a counterpart draws it in Ravel if at all | — |
| §15 the seven subcommands, the two that act | implemented | `src/commands.rs`, `src/lib.rs` |
| §15 the seven subcommands, the five that are declarable only | declarable only | `src/commands.rs`, `src/lib.rs` |
| §15 the permission levels | not carried; the specification records them as unread | — |
| §16 the access widening and the one bytecode patch | declarable only | — |
| §17 the capability-attachment registry | implemented | `src/capabilities.rs` |
| §17 block-entity lookup, stores, fluids and the side questions | answered by `menus::insert_first`, `Store::count_of`, `storage::fill` and `Tank::drain`; block lookup calls the supplied closure directly and side questions use SDK `Side` matching/predicates, with no package lookup or side API | `src/menus.rs`, `src/storage.rs`, `ravel_sdk::Side` |
| §17 the single-use tank and the mutable box | implemented | `src/storage.rs` |
| §17 the game-test harness | not carried; it is not in the shipped artifact's path | — |

## Behaviour changes, each declared rather than incidental

- **The entry allocates from its own counter table once per world opening (§13).** The specification
  gives the entry a table of named counters that hand out monotonically increasing integers unique
  within a world and survive a restart, and names no allocation the entry itself makes. Nothing else
  in this package allocates, so in a launched game the table stayed empty for the whole of a world's
  life and the guarantee it owes was demonstrated only in tests: every session wrote the same bytes,
  and a save that restored nothing was indistinguishable from one that restored everything. Opening a
  world now takes the next value of `persistence::OPENINGS`, after the stored counts are read and
  only when reading them succeeded. That is the entry using its own mechanism rather than a new one,
  and it makes the surviving sequence observable in the world the cluster scenario saves and reloads.

- **Neither half of §9 is reproduced.** The five hard-coded account identifiers are other people's
  identity and the server-start fetch is unannounced outbound network access from a library a pack
  author installed for its collection types. The specification states this divergence and its reason,
  and this package carries neither. What survives is the delivery shape the effect used:
  `transport::Target::Nearby` and `Target::COSMETIC_RADIUS`, so a server owner who wants the effect
  has the mechanism without the list.
- **A runtime settings change is not written back to its file (§5.3).** The specification records the
  write-back path as unread. `settings::Framework::write` changes the in-memory value, which is the
  behaviour that was read, and writes no file. A counterpart that guessed would be writing a file on a
  guess.
- **`<=` is matched whole while the handler order is kept (§11).** The published order tries
  less-than before less-than-or-equal and first-match wins, which taken to the letter leaves `<=`
  unreachable, since `<` matches its first character. `query`'s comparison handlers match their whole
  operator and are still tried in the published order, so the two places the order is genuinely
  observable — `.length` beating a named child, and `!=` beating logical not — behave as the
  specification states and both are tested.
- **The detector's traversal order is stated rather than inferred, and it has no tie to break
  (§12).** §12 records the search order as unread. `shapes::detect` walks the six face neighbours in
  the order `shapes::NEIGHBOURS` names, breadth first from the start, and a region is what that walk
  reaches. §12's "largest connected region" is read here as the one the player is standing in: the
  walk is anchored at the start, so it never sees a second region and no tie can arise. A parity run
  against the original should expect a disagreement wherever the original searches for candidate
  regions instead, which §12 does not say it does.
- **Diagonal contact is not connection (§12).** Two blocks touching only at an edge or a corner are
  two regions. §12 does not say, and this is the reading that makes "connected region" mean what a
  player building a multiblock sees.
- **A quantity total saturates rather than wrapping (§10).** Every sum in `quantity` and `menus`
  saturates. A wrapped total comes back *smaller* than one entry alone holds, which inverts the
  question every one of these collections exists to answer.
- **The store notifies its owner by a mark the owner reads, not by calling it (§8).** §8 lists a
  store that notifies its owner on change and a change counter dependents poll. `menus::Store` carries
  both as read state: `dirty()`, which the owner clears once it has saved, and `changes()`. A store
  owned by a data component cannot hold a reference back to its owner to call, and a callback would
  cost the store its `Clone`, `Debug` and `Eq`. The owner reads the mark after it mutates, which is
  the same information one step later.

- **A multi-category classified collection reads every partition (§10).** §10 says the two
  partitioning variants differ in whether a lookup may intersect partitions. `quantity::Partitioned`
  publishes one category function, so a lookup through a `Partitioning::Multiple` kind has no second
  partition to name and reads them all rather than guessing at an intersection. The answer is the same
  either way; what differs is the cost, and `Lookup::used_partitions` says which happened. Naming
  several partitions would need a wider trait than §17's dependents consume.

- **The value channel is emptied on a resource reload (§7).** §6 registers a resource reload request
  and §7's channel holds values a screen is reading. `Entry::observe` clears the channel on
  `GameEvent::ResourcesReloaded`, so a screen re-reads rather than showing values from before the
  reload. Nothing in the specification says the original does this, and the alternative is a screen
  holding stale values it cannot tell are stale.
- **The seventh reader is inferred (§7).** §7 states seven readers and names six: the whole number,
  the truth value, the text, the display-text list, the raw one and the single display text. The
  seventh here is `Channel::fraction`, a second numeric reader following the same absent-value rule as
  the first two. It is named as inferred rather than presented as read.

## Bounds against hostile input

Every decoded count and every index a caller supplies is bounded, and each bound is a published
constant with a test that reaches it: `transport::MAX_LEVELS` and `MAX_FIELDS`,
`channel::MAX_IDENTIFIERS`, which bounds a send as well as an allocation because the identifiers
arrive from the server, `query::MAX_LENGTH`, `MAX_STEPS`, `MAX_DEPTH` and `MAX_MATCHES`, the last
bounding an evaluation's match set because the wildcard and union forms are many-valued and the
parser's bounds do not bind that, `shapes::MAX_VISITED`,
`persistence::MAX_COUNTERS` and `MAX_FIELDS`, `capabilities::MAX_ATTACHMENTS`, and
`quantity::CONDITION_SLOTS`. A list index and a slice bound are resolved against the list's real
length, a negative element counts from the end and one past either end matches nothing, a counter at
the end of its range hands out nothing rather than repeating, and a non-finite delivery radius reaches
nobody.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision. None is stubbed.

- **Twenty-eight of the thirty-four registration kinds (§5.1).** Ravel's registry has four kinds —
  block, item, recipe and command — and nothing this entry declares is a block, an item or a recipe.
  So the whole of `declarations::KINDS` is a vocabulary a dependent
  declares against and this package holds the three rules over, and `Entry::declaration` claims the
  command tree alone. `declarations::CLAIMED_KINDS` says so in one line. The three rules — the
  refusal, the order, the companion split — are what a dependent observes and are all implemented.
- **The two particles, the four data components, the three triggers, the menu, the loot modifier and
  the capability declaration (§4).** Each is real in `declarations::own_declarations` and none reaches
  a Ravel registry, for the reason above. The counts, the order and the seven storable ones are all
  tested.
- **The host intrusion surface (§16).** The 117 lines of access widening and the one client-side
  bytecode patch are not counterpart-implementable, and §16 says so itself: several exist to let the
  original's own widget toolkit and manual engine draw, which a Ravel counterpart does not do. Ravel's
  own interception points are the equivalent reach, and which of the 117 survive the filter is the
  counterpart's design rather than this specification's, so nothing here claims a number.
- **The manual engine (§14).** An XML book with conditional sections and claimable rewards needs a
  drawn screen. Ravel draws its own, and §14 says a counterpart treats the manual as Ravel-drawn if it
  is built at all. The demonstration menu is declared and the subcommand that opens it is in the tree;
  nothing paginates.
- **Five of the seven subcommands' own effects (§15).** Two act on state this entry holds and
  `Entry::run_command` dispatches them: the settings one reads and writes through
  `settings::Framework`, and the reload one empties the value channel. The other five do not act
  here. What `version`, `diagnostic` and `manual` answer is Ravel's chat surface and there is none
  published. `dump` writes registry documents to a destination Ravel does not publish, and this
  entry holds no registry of its own to serialise, so `dumps::Producers::write_all` is the piece that
  exists and the writing is not. `ignite` acts on a level entity through a seam Ravel does not
  publish. The tree shape, its seven names, which two are inherited, the arity of each and the two that
  act are implemented and tested.

- **The slotted storage form (§10).** `storage::SlottedStore` is vocabulary this package owns and
  publishes for RVL-132, which implements it over its own stores. Nothing in this package is a
  slotted store, so nothing here implements it. The guarantee §10 says the slotted form adds — that a
  result names the slot it came from — is held to a test against a fixture store in `storage.rs`.
- **The game-test harness (§17).** It is in RVL-132's build and not in the shipped artifact's critical
  path, and Ravel publishes no equivalent.

## Two shapes worth stating

- **The transport's levels are declared, not discovered.** Rust has no inheritance, so §6's "walk the
  concrete message type upward through its ancestors" is `transport::Shape`, a list of levels a
  dependent writes down. That is a structural change and it is the reason the ordering rule can be
  held to a test at all: `Shape::wire_order` is the rule, and a peer that assembles the same fields
  into a different level order is proved to read a different message.
- **Nothing this package registers is storable by a world.** §4's seven storable identifiers are the
  ones the provenance record leaves open, and none of them reaches a registry here, so no world can
  store one. The saved data this package does own is the counters of §13, under one version, and it
  round-trips through Ravel's own saved-data ownership.

## Evidence

179 inline tests, and every rule with a number, an ordering, a refusal or a format has one: both
settings client-scoped under one grouping with both defaults off, the three-value scope vocabulary
and which side resolves each, a runtime write taking effect and a non-changeable setting refusing
one, a write of the wrong kind refused, the thirty-four kind count with each named once, a kind
outside the vocabulary refused at declaration, the raise for a declaration marked
not disableable, a switched-off declaration never registered, one kind finished before the next
begins, declaration order inside a kind, a companion loading on a client and on neither server, the
sixteen declared entries with fifteen registering and the seven storable ones counted, the wire
order most-derived-first and ascending within a level, that order proved not to be declaration
order, a round trip through it, two peers in different orders reading different messages, no field
name and no framing in the bytes, every truncation of a message refused, a wrong-kind value refused,
the portable codecs present before common setup and the platform ones absent, a second platform load
answering false, thirteen message kinds with the position base outside them, the position base's
three fields following a dependent's own, all three delivery targets including fifty blocks exactly
and 50.1 outside it, a non-finite radius reaching nobody, all seven channel readers on what was
sent, the numeric and truth readers collapsing absent and wrong-kind, the text and list readers
keeping them apart, the raw and single display-text readers collapsing them, the allocator stopping
at its limit, a send for an identifier past that limit refused, the slot roles and every shift-click
target including an output slot, nothing ever inserted into an output-only run, the change counter
on every mutation including a clear, a write past the end counting as nothing, the index following
every write and leaving no entry behind, a saturating total, the same collection answering
differently under different conditions, one entry per addition against one per instance, a zero
amount entering no collection, a summing view copying nothing, taking never yielding more than is
held, a partitioned lookup reading one partition, a wider query reading all eight entries and a
multi-category kind reading all eight even where the query would permit a partition, the superset
rule alone, a chain at both ends, a filtered iterator, a kind round trip and four refusals of a
stored form, the matcher and hasher agreeing under every set, a move bounded by three things, a move
from an empty source and into a full destination, the reduced completion when a destination accepts
less than it promised, the inconsistency raised only when the recovery also fails, a simulated call
changing nothing, a slotted result naming its slot and a slotted store answering which slot a real
insertion went through where the slotless form answers only the amount, the inconsistency naming
what the simulation promised where a concurrent drain made the real extraction short, a difference
folding repeated entries and reporting a partial change as an amount, a tank refusing a second
instance and bounded by its capacity, the twenty-two handlers in the published order, `.length`
beating a named child and the bracket form reaching a child really called that, `!=` beating logical
not and logical not still reachable, every navigation form, a negative element and one past either
end, a slice past the end and a backwards slice, a union reporting each node once, every comparison
form, all three logical forms, four parse errors naming where, every published bound reached, a
nesting of negations and one of conjunctions each stopping the parser at the published depth rather
than after every form is on the stack, a re-rooting step answering once for the whole step where re-
rooting inside the loop would multiply the match set, an evaluation past the published match bound
refused, a wildcard walked through so the steps after it are answered about too, navigation
answering without a document and saying nothing where data decides, a parent step off the root, the
detector's start refusal, a diagonal not connecting, every size and count constraint reporting what
it found, the cube extent, the hollow variant naming the block inside, every failure message, a walk
at the edge of the coordinate space, both persistence tables and their loading, a field type the
table lacks refused, a name-keyed format proved by adding a field ahead of another, a missing stored
field refused, a marked field with no value refused rather than written as null, counters increasing
and never repeating, two counters kept apart, a counter surviving a save and a load, a stored
counter that is not a number refused rather than restarted and the world holding it refusing to
open, a stored set larger than the limit refused rather than read short, a world that stored nothing
opening on no counters rather than the last one's, a counter at the end of its range, seven
producers each named once, a producer collected twice, a producer writing nothing
named with nothing rather than an empty registry, seven subcommands with five own and two inherited,
the arity of each, the settings subcommand writing the setting it names and refusing an undeclared
name and a value of another kind, the reload subcommand emptying the channel, a listener installed
on first registration of each of the four classes, the registry raising after baking and holding
nothing, specific against broad addressing,
every platform codec naming a kind the codec table really has, an insertion that would overflow a
slot refused rather than losing the overflow and the next slot taking it instead, the crafting
helpers on a client only, the load-complete latch, the producer service and a repeated contribution,
the counters through a save and reopen, and a resource reload emptying the channel while a tick does
not. The command tree as the whole of the barrier claim is covered by the real-loader integration
tests in `../../../tests/conformance/tests/integration/registry_lifecycle.rs`, alone and alongside
every counterpart on each side.

One of the 179 is `dumps::tests::the_generated_documents_regenerate_byte_for_byte`, which is
not about a rule but about the three documents under `generated/`. `dumps::documents` renders them
out of the functions that already derive their content — `declarations::own_declarations` with its
registration order and per-side companion lists for `generated/census.json`, `commands::SUBCOMMANDS`
with `commands::arity` and `commands::inherited` for `generated/commands.txt`, and
`settings::own_framework` with `Scope::resolved_by` and `Framework::document` for
`generated/settings.json` — and the test rebuilds all three and compares them byte for byte against
the tree, so a table that changed without its document changing fails. Setting
`RAVEL_REGENERATE_GENERATED` makes the same test write the documents instead of comparing them,
which is the only way they are produced: there is no binary and no build script. Nothing in them is
transcribed: `generated/census.json`'s registration list is the sixteen declared entries regrouped
by the vocabulary rather than the declaration list retyped, and every count in it is a `len()` of
what the crate produced rather than a number written beside it.

## Host blockers

Nothing here has been run inside a game. Every claim above is a claim about this package's own code
against the specification, tested at the seams `ravel-sdk` publishes. What a running game would decide
and this cannot: whether Ravel's registration barrier fires where §2 needs it, whether an interception
point exists for the reach §16 counts, what a subcommand prints, and whether the transport's bytes
reach a peer at all.
