# RVL-060 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-060.md`, written against
`ravel-sdk` and nothing else. No upstream source was opened, decompiled or read to write it; the
specification is the only description of the behaviour that was used.

The entry is a foundation seed with no gameplay of its own and two admitted pack entries that load
only if it does. §1.1 says what survives as counterpart work and this package is that: the predicate
and validator semantics of §7, the payload shapes of §8, the split integer transport of §6.3 and the
colour arithmetic of §11, plus the nesting and grouping of §4 and the identifier derivations of §5.
Everything Ravel already owns — content declaration and the registration barrier, side isolation,
saved-data ownership, networking channels — is expressed through Ravel rather than rebuilt.
The support command is exposed through `Mod::content_declaration` before the default registration
barrier claims it. Invalid declaration constants are lifecycle refusals, not constructor panics.

Block, item and entity predicates and their candidates take parsed `ContentId` identities with
explicit namespaces; bare names are refused by the shared identity grammar. Candidates require an
identity rather than manufacturing an invalid empty default. Validator, channel-identity and
declaration failures retain typed source errors. Fluid debug output follows the nested property
groups instead of flattening their old names.
Tests retain behavioral and uniqueness checks without restating fixed-array lengths.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 the two construction-time steps | implemented, without the file work | `src/lib.rs` |
| §2 the side-appropriate registry accessor and its hard failure | implemented | `src/lib.rs` |
| §2 the client-only observers | implemented as Ravel's own side isolation | `src/lib.rs` |
| §2 the process-wide channel list | host-blocked, dropped by design | `src/lib.rs` |
| §2, §10.1 declared packs contributed on the client only | implemented on both sides, declared | `src/highlights/declarations.rs` |
| §4 the declaration handle and its before-registration error | implemented | `src/declaration.rs` |
| §4 nesting, grouping and the inert nested initialisation | implemented | `src/declaration.rs` |
| §4 the one extension point and its refusal | implemented; refusal kept, claims nothing | `src/declaration.rs` |
| §4 a variant group answering null | implemented as an empty group, declared | `src/declaration.rs` |
| §5 the two identifier derivations | implemented, patterns from a build-time input | `src/channel.rs` |
| §5 the direction split and its two handler shapes | implemented | `src/channel.rs` |
| §5 the three packet-kind shapes | implemented | `src/channel.rs` |
| §5 the optional channel's silent refusal, at channel level | implemented | `src/channel.rs` |
| §5 the six send targets | implemented | `src/channel.rs` |
| §5 the distance send comparing a square to an unsquared argument | fixed, declared | `src/channel.rs` |
| §6.1 the two directories outside the game directory | declined; a durable per-package directory instead | `src/storage.rs` |
| §6.2 the codec-described record and its defaults | implemented | `src/storage.rs` |
| §6.2 the parse-failure fallback | implemented, and made loud | `src/storage.rs` |
| §6.2 the merging write that leaves stale keys | implemented; clearing by default, declared | `src/storage.rs` |
| §6.2 the always-dirty and world-wide modes | implemented | `src/storage.rs` |
| §6.2 a non-compound codec output dropped at write time | refused at declaration time, declared | `src/storage.rs` |
| §6.2 the older envelope and its client stand-in | implemented, a copy per request, declared | `src/storage.rs` |
| §6.2 the inert data-fixer argument | not carried, deliberately | — |
| §6.3 the split integer arithmetic and the doubled backing | implemented | `src/transport.rs` |
| §6.3 the compatibility-only narrow pair | implemented, both kept | `src/transport.rs` |
| §7.1 the state predicate and its unknown-property failure | implemented | `src/matching.rs` |
| §7.1 the two matcher kinds told apart by shape | implemented | `src/matching.rs` |
| §7.1 running the predicate backwards, with the randomness | implemented, caller's source | `src/matching.rs` |
| §7.2 the block and entity predicates | implemented | `src/matching.rs` |
| §7.2 the item predicate's inverted identity test | fixed, declared | `src/matching.rs` |
| §7.2 the durability rules kept deliberately | implemented, both kept | `src/matching.rs` |
| §7.3 the lenient stored-data rules | implemented | `src/data.rs` |
| §7.4 the eleven kinds in four families | implemented | `src/validators.rs` |
| §7.4 the family check before anything else | implemented | `src/validators.rs` |
| §7.4 both list kinds rejecting an empty candidate | implemented, kept | `src/validators.rs` |
| §7.4 the floating-point lower bound | fixed to negative infinity, declared | `src/validators.rs` |
| §7.4 decoding by first-parse order | implemented; family required, declared | `src/validators.rs` |
| §7.4 the regular-expression kind | implemented over a stated subset, declared | `src/validators.rs`, `src/validators/conditions.rs` |
| §7 hanging any of this on a data-pack document | host-blocked | — |
| §8 the base recipe's four deliberate answers | implemented | `src/shapes.rs` |
| §8 the ingredient complexity flag and its inversion | implemented | `src/shapes.rs` |
| §8 the item test and the example stacks | implemented as two things | `src/shapes.rs` |
| §8 the namespace-scoped lazy registration | implemented, bounded | `src/shapes.rs` |
| §8 the optional menu payload and the pointer reset | implemented | `src/shapes.rs` |
| §8 registering a recipe kind, an ingredient kind or a screen payload | host-blocked | — |
| §9 the fluid property vocabulary | declared as a shape | `src/fluids.rs` |
| §9 the lazy client lookup and its cache | implemented | `src/fluids.rs` |
| §9 fluid registration and client drawing | host-blocked | — |
| §9 the one mixin supplying the loader's accessor | not carried; Ravel has no mixin | — |
| §10 the scissor stack's intersection rule | implemented | `src/presentation.rs` |
| §10 the pose scope | implemented | `src/presentation.rs` |
| §10 the cursor system's per-frame resolution | implemented | `src/presentation.rs` |
| §10 the screen history and its priority rule | implemented | `src/presentation.rs` |
| §10 the memory that restores a screen | implemented | `src/presentation.rs` |
| §10 the memory never being cleared | implemented; cleared on disconnect, declared | `src/presentation.rs`, `src/lib.rs` |
| §10 the widgets and the cursor shape catalogue | declarable only | — |
| §10 the four system categories and the registration point | implemented | `src/presentation.rs` |
| §10.1 the declaration shapes and the fixed lookup path | implemented | `src/highlights/declarations.rs` |
| §10.1 the required and non-required metadata rules | implemented | `src/highlights/declarations.rs` |
| §10.1 the inner failure that discards its cause | fixed, declared | `src/highlights/declarations.rs` |
| §10.2 the two document shapes and the key that parts them | implemented | `src/highlights.rs` |
| §10.2 flattening, sharing and discarding the named table | implemented | `src/highlights.rs` |
| §10.2 the exact-state lookup and the length rule | implemented | `src/highlights.rs` |
| §10.2 the fixed highlight colour | implemented in the portable draw output | `src/highlights.rs` |
| §10.2 intercepting the outline and the reload listener | host-blocked | — |
| §11 the packed layout | implemented | `src/colour.rs` |
| §11 the naming table's two first-registration-wins mechanisms | implemented | `src/colour.rs` |
| §11 the document form and the three-arm network form | implemented | `src/colour.rs` |
| §11 the animated colour on a timer | implemented; driven from the frame, declared | `src/colour.rs` |
| §12 the three declined areas | excluded; not implemented at any Ravel revision | `src/lib.rs` |
| §12 the client command | kept, claimed and run; user-invoked | `src/lib.rs` |

Numeric conversions use checked `num-traits::ToPrimitive` results with explicit rounding and
out-of-range handling to preserve truncation, saturation and NaN semantics. Entity interaction attributes now live in `FluidProperties::entity`;
defaults and the flat debug representation remain unchanged.

## Four compatibility-required spellings this package refuses to invent

§14 names four spellings a dependent or a pack author writes, and states that none is transcribed in
the specification. Nothing here guesses one. Each is read from a build-time input, each has a
placeholder the tests pin the surrounding **shape** with, and each is an error rather than a default
when the build supplies nothing — because a package that has to agree with an original cannot fall
back to a placeholder and still claim to agree.

- **The channel and payload suffixes (§5).** `channel::Suffixes::declared` reads
  `RVL_060_COUNTERPART_CHANNEL_SUFFIX` and `RVL_060_COUNTERPART_PAYLOAD_SUFFIX` as patterns carrying
  `{version}` and `{packet}`, and refuses a pattern that does not carry its placeholder, because a
  suffix that ignores its input would make every version, or every packet, spell one identifier.
- **The pack descriptor key (§10.1).** `highlights::Spellings::declared` reads
  `RVL_060_COUNTERPART_PACK_DESCRIPTOR_KEY`.
- **The highlight directory and the key that parts the two document shapes (§10.2).** The same type
  reads `RVL_060_COUNTERPART_HIGHLIGHT_DIRECTORY` and `RVL_060_COUNTERPART_HIGHLIGHT_LINES_KEY`, and
  refuses a directory and a lines key that are the same word.
- **The eleven validator kind names and their field names (§7.4).** `validators::Vocabulary::declared`
  reads `RVL_060_COUNTERPART_VALIDATOR_KINDS`, `RVL_060_COUNTERPART_VALIDATOR_FIELDS`,
  `RVL_060_COUNTERPART_VALIDATOR_BOUNDS` and `RVL_060_COUNTERPART_VALIDATOR_DISCRIMINATOR`, and
  refuses kind names that are not distinct, family gates that collide, or a discriminator that is one
  of the gates. The field names for the non-default kinds and the two bound names are this package's
  own decision rather than carried spellings; the specification names the eleven kinds and "their
  field names", and the four family gates are the ones a decode branches on.

The shared spelling configuration and pack contributions live in
`src/highlights/declarations.rs`; `highlights` retains their public paths alongside outline
geometry. The whole-value pattern engine lives in `src/validators/conditions.rs`, distinct from
the public block/entity/item predicates in `matching`; `validators` retains its public pattern
types and integrates them through parsing and whole-value matching.

## Behaviour changes, each declared rather than incidental

Nine of these are §13's own table. The rest are this package's, and each says why.

- **Declared packs are contributed on both sides (§2, §10.1).** The original's pack-finder asks the
  event which kind is wanted, so the mechanism could contribute a server-facing pack; but both the
  scan and the listener registration happen only on the client branch, so a dependent shipping data
  through it silently has none on a dedicated server. `highlights::contribute` takes the side and
  answers the same on all three.
- **A distance send is compared against the square of its argument (§5).** The original compares a
  squared distance against the unsquared argument, so a caller asking for sixteen blocks reaches
  players within four. Whether existing callers were compensating is unknown; none was read.
- **A saved record clears before writing (§6.2).** The original merges into whatever the host handed
  it and clears only under an opt-in mode, so a record that lost a field keeps the stale key forever.
  `Record::merging` keeps the original rule for a dependent that asks for it.
- **The item predicate's identity test means what it says (§7.2).** The original returns false when
  the stack *is* the declared item, so it can only ever hold for a stack that is not what it names.
  No parity test is written against the original here.
- **An absent floating-point lower bound is negative infinity (§7.4).** The original substitutes the
  smallest *positive* representable value, so every negative value and zero fail a range that
  declared only an upper bound.
- **A validator must name its family where context does not supply one (§7.4).** The original tries
  the four families in a fixed order and takes the first that parses. Which orderings collide is
  unknown and was not tested, so `Validator::decode` refuses rather than resolving by luck.
- **Screen state is cleared on disconnect (§10).** The original never clears the map, so reopening a
  screen after joining a different world restores the previous world's. `Entry::on_disconnect` is
  the caller, and its trigger is declarable only because Ravel publishes no disconnect event.
- **A pack contribution failure names its cause (§10.1).** The original's inner registration catches
  its failure, discards the cause, and logs a message that does not say what went wrong. Every
  `highlights::Failure` carries both who and why, and the scan still carries on past it.
- **Colour animation is a function of the frame it is drawn in (§11, §12).** The original runs it on
  a ten-thread pool created at class load and never shut down, mutating shared state the drawing
  thread reads without synchronisation. `colour::Animated::at` removes the pool and the race
  together, and two reads on one frame agree.
- **A group with no declaration is empty rather than absent (§4).** The original's grouped-by-value
  accessor answers null for a value nothing was declared under, with no empty-collection fallback.
- **A parse failure is reported rather than only logged (§6.2).** The fallback to the default is kept
  — refusing would cost a world its state — and `storage::Loaded::recovery` makes the failure
  something a caller has to look at.
- **A codec whose output is not a compound is refused at declaration time (§6.2).** The original logs
  and silently drops the write. A codec's output shape is knowable before any world exists.
- **The client stand-in is a copy per request (§6.2).** The original's is a single shared instance,
  so two client worlds share one mutable record and each sees the other's writes.
- **The regular-expression validator supports a stated subset (§7.4).** The original delegates to the
  platform's engine. This package supports literals, `.`, a `[...]` class with ranges and a leading
  `^` negation, the repetitions `*`, `+` and `?`, and `\` escaping one of this subset's own
  metacharacters, and refuses anything else at decode rather than accepting a pattern it would match
  differently. `\d`, `\w` and `\s` name classes this subset does not have, so each is refused rather
  than read as its letter, which would match a pack author's `\d+` against the letter `d`. Matching is anchored at both
  ends either way, which is what the whole-value rule means. Matching also carries an explicit step
  budget, because a backtracking matcher is exponential in the worst case and the pattern arrives
  from a document; past the budget the answer is false rather than a hang. The upgrade path is a
  matcher that does not backtrack.
- **A fluid is not declared as a block (§4, §9).** The one extension point keeps its refusal and
  claims nothing: Ravel has no fluid registry, and claiming a fluid as a block would refuse a
  dependent that also declares a block of that name for a duplicate it did not write.
  `Registry::declare_special` hands back the identifier the entry would carry.
- **The ingredient's item test is not its example list (§8).** §8 lists the test and the stacks it
  matches as two things a dependent supplies. `Ingredient::admitting` declares the test and
  `Ingredient::matching` the display stacks, and an ingredient with no declared test answers nothing
  rather than admitting nothing, because a complex ingredient's answer is its dependent's.
- **A name past the reader's ceiling is refused by the writer (§11).** `Value::to_wire` refuses a name
  over `MAX_NAME_BYTES` instead of writing a packet the peer at the other end refuses to decode.
- **A registration says which of its two failures happened (§11).** `Names::register` answers `Placed`
  rather than a boolean, so a palette past the ceiling is not read as a name already taken.
- **A derived identifier Ravel cannot express is a packaging failure (§5).** The contract carries the
  derived payload identifier itself rather than a repaired spelling of it. Two suffix patterns that
  differ derive two identifiers, so a rename that collided them into one route is impossible by
  construction and an illegal spelling is `ChannelError::Identity` at packaging time.
- **Every collection and index is bounded (§4 to §11).** Naming tables, wide-value stores, validator
  depth and width, tag comparison depth and width, the fluid lookup cache, target chains, packet
  bodies, channel kinds, cursor regions, clip depth, remembered screens, system facts, declared packs,
  outline segments and ingredient registrations all have a stated ceiling and refuse past it. A
  ceiling forgets one entry or refuses one addition; none empties a collection to make room. Documents
  and payloads here are pack authors' and peers', so none of their sizes is this package's to trust.
  The specification does not require this and every ceiling is a constant with an upgrade path.

## Declined outright, and why

§12's three, none of them stubbed and none reachable from this package. `DECLINED` names them.

- **Direct memory and reflection access.** Ravel forbids unsafe code in every maintained crate with
  no exception, and the enumeration-extension helper the original builds on it is marked obsolete
  there. A dependent needing an extensible enumeration is a Ravel feature request.
- **Outbound network requests.** A library any dependent can use to reach an arbitrary address from
  the game process is a capability decision and belongs to Ravel's own permission surface.
- **A background timer pool.** Removed along with the data race it caused; see the colour change
  above.

§6.1's two directories are a fourth decline, and the specification calls it that rather than
host-blocked: a package writing into the user's home on its own initiative is outside what Ravel's
sandbox should permit. `storage::DurableStorage` joins a host-supplied root to a validated
single-segment package name, touches no file, reads no environment variable, and knows nothing about
the operating system. It writes no explanatory file either. Any dependent that had recorded an
absolute path would break; none is known to, and none was read.

## Declarable only, and why

Each needs a seam Ravel does not publish at the reviewed revision. None is stubbed.

- **Hanging the predicates and validators on a document (§7).** Ravel publishes no data-pack document
  loading, so §7.1 to §7.4 are portable logic over this package's own declared types and cannot be
  made live. Every rule is implemented and tested; nothing loads one from a pack.
- **Recipe kinds, ingredient kinds and screen-opening payloads (§8).** Ravel publishes none of the
  three registrations. The portable halves — the base recipe's four answers, the complexity
  inversion, the lazy namespace registration and the optional payload — are implemented.
- **Fluid registration and client fluid drawing (§9).** Ravel publishes neither. The property
  vocabulary is a declared shape and the lazy lookup rule is implemented, because that rule is
  observable and portable. The one mixin the original ships is not carried at all: Ravel has no
  mixin mechanism and the safety boundary is why.
- **Intercepting a block's outline, and a client reload listener (§10.2).** Ravel publishes neither,
  so nothing here draws. The loading, the flattening, the sharing and the lookup are all here.
- **Widgets and the cursor shape catalogue (§10).** Ravel-owned screens are drawn in Java, so the
  widget material is declared rather than implemented. `presentation::Cursors` decides which region
  the pointer is in and the shape is a name a dependent supplies.
- **Carrying a packet between a client and a server (§5).** `ravel_sdk::network` publishes the
  contract, the framing and the side rule and states plainly that nothing carries a mod's packet yet.
  `channel::Channel::contract` produces a real contract whose name is the derived payload identifier,
  so what Ravel routes on and what a peer derives are one string rather than two that can disagree.
- **The disconnect that clears the screen memory (§10).** Ravel publishes no disconnect event. The
  rule is implemented in `Entry::on_disconnect` and the caller is the host's.
- **The clipboard the support report is placed on (§12).** Ravel publishes no clipboard. The command
  itself is not declarable only: it is claimed at the registration barrier as a command and run
  through `Mod::run_command`, which produces the text and holds it for `Entry::take_report`. Only the
  placing is the host's. What is kept either way is that the command is explicitly user-invoked and
  sends nothing.

## Two shapes worth stating

- **The named-outline table cannot outlive its load.** §10.2 says a name is resolvable only during the
  load that defined it. `highlights::load` builds the table as a local and drops it at the end, so
  nothing in the returned `Outlines` can name an outline: the rule is the type's shape rather than a
  discipline a later edit could forget.
- **The handle contract is a before-and-after rule, not an object.** Ravel's barrier claims
  identifiers and hands back no object, so `declaration::Handle::resolve` yields the identifier and
  refuses before initialisation. That refusal is the part §4 makes observable, and it is what a
  dependent that asked too early actually sees.

## A gap found in the published surface

- **`ravel_sdk::save::SavedDataError` has no arm for a version that is not there.**
  `SavedDataVersion::new` answers an `Option`, so `storage::Record::declaration` has to name
  `NoVersionsDeclared` for a constant that cannot be zero. It is the nearest true statement rather
  than the right one. A checked constructor returning the module's own error type, or a
  `ZeroVersion` arm, would close it. The same gap is recorded by the RVL-120 counterpart, which is
  worth saying: two independent packages reached it from different directions.

## Evidence

Portable rules have inline tests. Among them: the packed layout's channel order and its round trip,
both naming mechanisms winning on the first registration and the table's ceiling, an animated colour agreeing
with itself within a frame and returning after six sixty-degree steps, a grey having no hue to turn,
the three network arms and their discriminators, the default arm carrying no body, a value followed
by more reading only its own bytes, five refusals off the wire and a lying length prefix refused
before anything is read, a name at the ceiling travelling and one past it refused by the writer as
well as by the reader, a full table and a name already taken being two different answers, the
document form's stated spellings, a six-digit spelling being opaque, and a named colour travelling
as its name so two ends animate from their own tables.

The wide store: the doubled backing hidden from the index, the whole signed range surviving, a value
over sixteen bits not being its low half, the low half masked rather than sign-extended, the high
half carrying the sign, each value keeping its own pair, the narrow pair still available, and four
refusals including a size past the ceiling refused before anything is allocated.

The stored-data rules: a null expectation matching anything, a null candidate failing, numeric
widening across widths, a number and a text that spells it not matching, different non-numeric kinds
failing, a compound naming keys and ignoring the rest, nesting compared the same way at every level,
a list matching in any order with repeats, an element nothing matches failing, an empty expected
list requiring an empty candidate, the three primitive arrays admitted, strict mode admitting no
widening, the empty rule, a tree deeper than the ceiling answering false rather than exhausting the
stack, and one wider than the ceiling answering false rather than walking the product of two widths.

The validators: eleven kinds in four families with the stated counts, one default per family, the
four gates and the discriminator distinct, the placeholder vocabulary naming eleven distinct kinds,
the declared vocabulary absent rather than defaulted, six ways a supplied vocabulary is refused,
every kind checking the family first, the five text kinds, a pattern matching the whole value, the
stated subset with six refusals, an escaped metacharacter, six escapes naming a class the subset
does not have, a negated class, a hostile pattern answering false on the budget, both list kinds
rejecting an empty candidate, ordered matching at the same length, universal matching, the three
arrays admitted, a number compared at the candidate's own width, the negative-infinity lower bound
at four values, both bounds inclusive, partial against exact compounds, a missing key failing both,
a context family decoding its default kind, no family named being refused, a named kind winning over
context, all eleven decoding from a document naming them, a range reading each bound by name, six
document refusals, and depth and width ceilings refused before the stack and before the allocation.

The matching: every named property satisfied, an undeclared property failing the whole predicate, an
empty predicate holding, exact and range matchers, unparseable spellings on either side, both bounds
inclusive, the two kinds told apart by shape, a round trip through that shape, two refusals, the
backwards construction, a two-bounded range picking only its two bounds and reproducibly from a
seed, a single bound contributed and an unbounded range contributing nothing, a zero seed still
producing a sequence, five definition refusals, the block predicate's four tests, a stored test
needing a block entity, the state default, the entity predicate's kind and five tests, a target test
against a non-targeting entity, a bounded target chain, the item predicate holding for what it
names, the durability rules kept deliberately at both ends, an inclusive count bound, and every
predicate exposing its stored tag.

Declaring content: the handle's identifier, the before-registration error and the
after-registration answer, nesting forwarding and remembering, an empty group,
variant grouping reaching the group and the root, an empty variant, the inert nested initialisation,
exactly one special registry with every other ask refused and nothing claimed at the block barrier
for it, a duplicate declaration refused and the same name in two registries admitted, an illegal
path refused, the declaration the barrier claims from, and the group ceiling.

The channel: the effective identifier, the payload identifier building on it, two versions
differing, the patterns absent rather than defaulted, three pattern refusals, a supplied pattern
changing both derivations, a derived identifier Ravel cannot express refused rather than renamed,
the direction split and which handler takes the sender, the dataless shape, the three body shapes, a
duplicate kind refused, the Ravel contract carrying the derived name and direction, an optional
channel's silent refusal, a non-optional channel not checking at either kind, a sixteen-block send
reaching sixteen blocks, a negative distance reaching nobody, all six targets, chunk rather than
position, four send refusals, a body past the ceiling refused before it reaches a peer, and the kind
ceiling.

The storage: the durable path, a trailing separator, seven hostile package names refused, a
non-compound default refused before any world exists, record names bounded at both ends, absence
reading the default without being a failure, a parse failure reported as well as recovered, a stored
record read whole, clearing leaving no stale key, merging keeping the original rule, two write
refusals, always-dirty, world-wide scope, one declared version, a server side reading the world's
record, a client world reading the stand-in, two client worlds getting two copies, and an envelope
with no stand-in handing a client nothing.

The shapes, fluids, presentation and highlights: the base recipe's four answers and a dependent
overriding all four, complexity defaulting to true with the loader told the inverse, the example
stacks, lazy namespace creation, two namespaces keeping their own, an empty namespace, one
registration per identifier, the namespace ceiling, a provider with and without a serialiser, the
pointer reset, the item test being the dependent's own rather than the example list; the fluid
vocabulary, a record registered after the first look never seen and one
registered before it held, one registration per fluid, the registration ceiling, and lookups of
absent names bounded by that same ceiling with a read-through past it; the clip intersection,
popping restoring, disjoint clips enclosing nothing, the clip ceiling, pose composition, cursor
resolution with the later region winning, the priority rule, a screen restored, a wrong-kind state
replaced, a fresh screen, the disconnect clearing, the memory ceiling forgetting one screen rather
than all of them, four built-in categories with a registration point, the report order and its
omissions, a dependent's category printed once however far apart its facts were added, an empty
report, and the fact ceiling; the three pack spellings absent rather than defaulted and two
refusals, both declaration shapes, five declaration refusals, the fixed lookup path and the fallback
key, both server sides contributing, a missing kind skipped silently, required packs hidden and
always-on, non-required packs reading their own metadata or falling back, a declared description
used, a failure naming its cause with the scan carrying on, the declared-pack ceiling, nine floats
per segment, the two document shapes told apart with identical outlines sharing one array, two
different outlines not sharing, a name unresolvable outside its load, the exact-state lookup, a
block's own outline winning, a partial array skipped, a dangling mapping dropped, five document
refusals, an empty load, the shipped colour, and the flattening order.

And the library itself: the declared identity, the one command claimed at the barrier and nothing
else, the command running and producing the report once for the host to place, a command this
package did not claim refused, the accessor each side installs, the hard failure before the registry
set exists, the durable directory named at construction, client-only state not built on a server,
the disconnect clearing on a client and costing a server nothing, the whole lifecycle, every event
recorded, a channel declared beside the package, the one command reading what the game knows, the
three declined areas named once each, and this package's own fourteen sources naming nothing that
would implement one of them.

The derived documents: `src/dumps.rs` renders three tables from the same functions the rest of the
crate calls, and `generated/` holds what they render. `generated/validators.json` is the decoding
shape, one row per kind with its spelling, its argument field, the gate its family is recognised
through and whether it is that family's default; `generated/transport.txt` is every single-bit
magnitude, its negation and the three values the sign handling turns on, each written and read back
through a store of one so the row carries the two halves the store actually keeps; and
`generated/colour.txt` walks the packed word one bit at a time, reading each into its quartet and
packing it again. One test, `the_generated_documents_regenerate_byte_for_byte`, rebuilds all three
and compares them against the bytes on disk, so the tree fails the moment it disagrees with the
crate; setting `RAVEL_REGENERATE_GENERATED` writes them instead, and that is how they were produced.

`checks/verify-counterpart-build.sh` runs the counterpart tests, including numeric conversion edge cases.

Pack records are non-exhaustive. Dependents start with `Declaring::new(namespace)` and fill its
declarations and discovered resources; `Declaration::named` checks a bare pack name and
`Declaration::decode` reads descriptor data. Scan results come from `contribute`, with
`Contribution::default()` available for an empty result.
