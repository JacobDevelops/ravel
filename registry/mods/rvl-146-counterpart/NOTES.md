# RVL-146 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-146.md`, written against
`ravel-sdk` and nothing else. No upstream source was opened, decompiled or read to write it; the
specification is the only description of the behaviour that was used.

The entry is a foundation seed: a toolbox with no gameplay of its own, no content in any world, and
three pack entries that load only if it does. §1.1 says what survives as counterpart work and this
package is that: the argument wire format of §5.1, the search state machine of §6, the ingredient
arithmetic of §7, the colour arithmetic of §8 and the highlight lifetime rules of §9, plus the
declarable kinds and identifier composition of §4 and the codec shapes of §10. Everything Ravel
already owns — content declaration and the registration barrier, side isolation, saved-data
ownership, networking contracts — is expressed through Ravel rather than rebuilt.

The recipe API names its operations explicitly: `Search::mark_inputs_changed` records an input
mutation and `Search::search` performs a search. Channel and registration path refusals expose
their `ContentIdError` through `std::error::Error::source`; leaf refusals have no source.
The quantity API names its matcher, portion and codec refusal `quantity::QuantityRefused`.
Declined and host-blocked areas are documented below rather than exported as unused spec-only
arrays. Tests exercise registration output and Ravel's stage API directly instead of keeping
test-only public kind-mapping and stage-side helpers.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 the process-wide slot guarded by a language assertion | fixed, uniqueness is per composition through the loader's one member per mod identity | `src/lib.rs` |
| §2 the accessor for the running server | implemented, absent on every path | `src/channel.rs` |
| §2 the client-only overlay subscription | implemented as Ravel's own side isolation | `src/lib.rs` |
| §2 the entry point that registers nothing of its own | implemented | `src/lib.rs` |
| §4 the six declarable kinds and the identifier composition | implemented | `src/registration.rs` |
| §4 every declared block also getting an item | fixed, the item form is optional | `src/registration.rs` |
| §4 the fixed registration order | not carried; this package walks its own stated order | `src/registration.rs` |
| §4 capability registration exposed and never wired | fixed, wiring made unnecessary and bounded | `src/registration.rs` |
| §4 the class lookup keyed by name string | fixed, keyed by type identity, one direction | `src/registration.rs` |
| §4 registering block-entity types and data components | host-blocked, named and ordered | `src/registration.rs` |
| §5 the channel, its declaration order and its direction flag | implemented | `src/channel.rs` |
| §5 the four send targets and the exclusion | implemented | `src/channel.rs` |
| §5 a radius send with no running server | fixed, answered rather than faulted | `src/channel.rs` |
| §5 the payload constructed empty and filled | not carried; values are immutable | `src/transport.rs` |
| §5 carrying a payload to the other end | host-blocked; the contract is real | `src/channel.rs` |
| §5.1 the wire shape: name, flag, count, tag and encoding | implemented | `src/transport.rs` |
| §5.1 the eight argument types | implemented | `src/transport.rs` |
| §5.1 the tag numbering as an enumeration's declaration order | pinned as data from a build input | `src/transport.rs` |
| §5.1 the signed-byte count that wraps past 127 | fixed, bounded variable-length count | `src/transport.rs` |
| §5.1 the tag used as an unchecked table index | fixed, range-checked and refused | `src/transport.rs` |
| §5.1 a decode a hostile peer buys cheaply | fixed, the decoded size is bounded | `src/transport.rs` |
| §5.1 the compound arm that may store nothing | fixed by construction | `src/transport.rs` |
| §5.1 the blind cast at the argument accessor | fixed, no cast exists | `src/transport.rs` |
| §5.1 an action looked up on a menu or a screen | implemented | `src/menus.rs` |
| §5.1 a missing action name ignored | kept, and made answerable | `src/menus.rs` |
| §5.1 arguments validated before dispatch | fixed, each action declares its shape | `src/menus.rs` |
| §6 the four pieces of state and the grace period | implemented | `src/recipes.rs` |
| §6 the fast path leaving the changed flag alone | implemented, kept deliberately | `src/recipes.rs` |
| §6 a missing level treated as a searched-and-empty list | fixed, nothing is concluded | `src/recipes.rs` |
| §6 the linear scan | implemented; the caller's, behind a seam | `src/recipes.rs` |
| §6.1 the nested document and its two entries | implemented | `src/persistence.rs` |
| §6.1 the value read from the outer document | fixed, read where it was written | `src/persistence.rs` |
| §6.1 the load that returns when there is no level | fixed, a level is not consulted | `src/persistence.rs` |
| §6.1 the catch of every throwable | fixed, one narrow failure | `src/persistence.rs` |
| §6.1 the saved memory as this package's world data | implemented through Ravel's envelope; malformed tables and unreadable memories refuse open or save, preserving prior data | `src/lib.rs` |
| §7 the declared form, its two spellings and its count default | implemented, spellings from a build input | `src/quantity.rs` |
| §7 the matcher and count, in two flavours | implemented as a sum type | `src/quantity.rs` |
| §7 the min-of-available-and-outstanding rule | implemented exactly | `src/quantity.rs` |
| §7 consuming mutating both values | fixed, immutable, results returned | `src/quantity.rs` |
| §7 the declared bound and the wire bound disagreeing | fixed, one bound in both directions | `src/quantity.rs` |
| §7 the untyped candidate, its cast and its predicate | fixed, removed together | `src/quantity.rs` |
| §8 the four channels, the three packings, the truncation | implemented | `src/colour.rs` |
| §8 equality and the hash on the packed value | implemented | `src/colour.rs` |
| §8 a bare integer triple meaning byte intensities | fixed, the two forms have distinct names | `src/colour.rs` |
| §9 the entries, their equality and their predicate | implemented | `src/highlights.rs` |
| §9 expiry evicting an entry merely equal to the expired one | fixed, one structure with full keys | `src/highlights.rs` |
| §9 expiry running only while drawing, and never on leaving | fixed, expiry on a tick, cleared on leaving | `src/highlights.rs`, `src/lib.rs` |
| §9 lifetimes against the wall clock | fixed, a monotonic reading and a duration | `src/highlights.rs` |
| §9 drawing a wireframe box in world space | host-blocked; the stage is declared | `src/highlights.rs` |
| §10 the pair and its two spellings | implemented, spellings from a build input | `src/data.rs` |
| §10 the map's unstable encode order | fixed, sorted on encode | `src/data.rs` |
| §10 the list wire form's unbounded declared size | fixed, bounded at both ends | `src/data.rs` |
| §10 the optional and compound wire forms | implemented | `src/data.rs` |
| §10 the data-component type builder | declarable only; declarable, the two forms named and paired | `src/data.rs` |
| §11 the reflection kit and the memory helper | declined outright, nothing stubbed | `src/lib.rs` |
| §12 the clamp and the rotation | implemented | `src/tools.rs` |
| §12 the predicate named for the inverse of its result | fixed, named for what it returns | `src/tools.rs` |
| §12 its string overload throwing on a malformed identifier | fixed, absent rather than a failure | `src/tools.rs` |
| §12 the loaded-package test and its two-list fallback | implemented as the loader's one answer | `src/tools.rs` |
| §12 the name-matching class hash strategy | declined; §4 keys by identity | `src/tools.rs` |

Channel packing uses checked `num-traits::ToPrimitive` conversion after explicit truncation
and saturation. Dump rendering writes directly into its output string without changing
the generated bytes.

## Four compatibility-required spellings this package refuses to invent

§14 names five spellings a peer, a dependent or a pack author writes, and states that none is
transcribed in the specification. Nothing here guesses one. Four are read from build-time inputs,
each has a placeholder the tests pin the surrounding **shape** with, and each is an error rather than
a default when the build supplies nothing — because a package that has to agree with an original
cannot fall back to a placeholder and still claim to agree.

- **The eight argument type tags and their numbering (§5.1).** `transport::TagNumbering::declared`
  reads `RVL_146_COUNTERPART_ARGUMENT_TAGS` as `kind=number` pairs naming all eight kinds, and
  refuses a malformed entry, a name that is no kind, a kind numbered twice, one number carrying two
  kinds, and any kind left out. All eight are required, because a numbering that defaults an
  omission is one two peers can disagree about silently.
- **The saved compound key and its two entry keys (§6.1).** `persistence::Spellings::declared` reads
  `RVL_146_COUNTERPART_MEMORY_KEY`, `RVL_146_COUNTERPART_CURRENT_KEY` and
  `RVL_146_COUNTERPART_REMEMBERED_KEY`, and refuses two entry keys that are one word.
- **The two field spellings of the pair shape (§10).** `data::PairSpellings::declared` reads
  `RVL_146_COUNTERPART_PAIR_FIRST_KEY` and `RVL_146_COUNTERPART_PAIR_SECOND_KEY`.
- **The two field spellings of the ingredient shape (§7).** `quantity::PortionSpellings::declared`
  reads `RVL_146_COUNTERPART_INGREDIENT_MATCHER_KEY` and `RVL_146_COUNTERPART_INGREDIENT_COUNT_KEY`,
  and refuses an empty spelling and two spellings that are one word.
  `Portion::read_declared` reads a recipe file's portion under them: the matcher's own document under
  one, and an optional count under the other, defaulting to one and bounded exactly as the wire form
  is. What a matcher's document *means* is still the host's, so it is carried opaquely, in the same
  way the wire form carries the host's matcher bytes.

The fifth is the game method the access-widening declaration of §1.2 widens, and it is not carried
for a different reason: the reach itself is host-blocked. Ravel publishes no recipe-manager accessor,
so `recipes::Catalogue` is the seam a host or a dependent implements, and there is no place in this
package for the method's spelling to go.

## Behaviour changes, each declared rather than incidental

Every one of §13's twenty-one rows is here, plus this package's own. Each says why.

- **Single construction is per composition (§2).** The original's slot is guarded by a language
  assertion, which a normal launch disables. Ravel rejects duplicate mod identities when resolving
  a composition, so each composition admits one value per mod id. `Entry::new` builds independent
  state and claims no process-wide slot; another composition, test or diagnostic can construct
  the library again in the same process without inheriting a previous composition's lifetime.
  The obsolete optional singleton API and its global-only test are removed. The section row
  remains exercised: `checks/verify-notes-rows.sh` counts section-table verdicts, not Rust tests,
  and `checks/counterpart-verdicts.tsv` classifies both its former `answered` verdict and its
  current `fixed` verdict as exercised. No section row is added, removed or reclassified, so
  M6's 550 total, 432 exercised, 74 declarable-only and 44 excluded counts are unchanged;
  the immutable evidence is not rewritten.
- **A radius send with no running server answers (§2, §5).** The original reaches the server through
  the process-wide slot rather than through the accessor beside it, so an unset slot faults at the
  send while the accessor next to it would have answered absent. `Sends::resolve` answers
  `SendRefused::NoServer`.
- **A radius that is not a number is refused (§5).** `NaN` makes every distance comparison false, so
  the send reaches nobody and looks like a delivery that happened. A negative radius is kept as a
  send that reaches nobody, because that is a number and the platform's own answer.
- **A block's item form is optional (§4).** The original registers a default block item for every
  block that supplied no factory. A block entity with no obtainable form is ordinary and the original
  could not express it.
- **Capabilities need no wiring (§4).** The original's capability routine is exposed and subscribed
  to nothing, so a dependent that forgets it gets no capabilities and no diagnostic.
  `Collection::registered` attaches them while building its registered representation.
- **A block-entity type is keyed by type identity (§4).** Two classes with one name loaded by
  different loaders collide in the original and the second silently resolves to the first. `ClassId`
  is an identity; the name it carries is for a report. A declaration is looked up by path, and there
  is no lookup from a class back to its path, because §4 asks for none.
- **The argument count is a bounded variable-length quantity (§5.1).** The original's is one signed
  byte, so a vector past a hundred and twenty-seven wraps negative and the reader allocates an array
  of that length. The bound is applied at the writer as well as at the reader, and it is a hundred
  and twenty-seven: the most the original's own byte can carry, so no packet an original peer writes
  legally is refused here. §13 asks for a count that is bounded, not one that is narrower.
- **An action with no name is refused at the reader too (§5.1).** The writer refuses one and the
  reader used to decode the whole packet and then answer `Unknown`. Every other bound in the module
  is applied at both ends and this one now is.
- **The type tag is range-checked (§5.1).** The original indexes its tag table by the raw byte, so an
  out-of-range tag raises an index error and its own rejection branch is unreachable.
- **The decoded size is bounded, not just the element count (§5.1).** A hostile peer could otherwise
  force a hundred and twenty-seven stack or compound decodes per packet at no cost to itself.
- **An action declares its argument shape (§5.1).** The original's accessor casts each stored value
  blind, so a vector of the wrong arity or types reaches the handler and fails inside it, on the
  server thread, with the bytes already decoded. `Actions::dispatch` checks the surface, then the
  name, then the shape, and only then runs the handler.
- **A compound argument carries a compound (§5.1).** The original's compound arm stores whatever the
  buffer read yields, including nothing, so a handler expecting a tag can receive none.
- **A missing level is not a searched-and-empty recipe list (§6).** The original's no-match path sets
  the stuck flag as if every recipe had been examined, so a block whose level is momentarily absent
  parks. `Search::search` answers `Outcome::CouldNotSearch` and moves nothing.
- **The saved memory is read from the document it was written into (§6.1).** The original tests
  presence on the nested document and reads the value from the outer one, so both reads yield an
  empty string, which parses as a well-formed identifier whose lookup misses. Every saved recipe
  memory is silently lost across a save and a load. A dependent will notice this in the direction of
  the machine remembering its recipe across a restart, which is plainly what the save was for.
- **A level is not consulted on the way in (§6.1).** The original's load returns before reading
  anything when its level supplier yields nothing.
- **The failure is narrow (§6.1).** The original catches every throwable, so a genuine error inside
  the recipe lookup is swallowed as though it were a parse failure. `ReadFailure` has one arm, about
  the shape of the document. An identifier the registry no longer holds is still absent rather than
  an error, and a world holding a malformed entry opens with the failure recorded in
  `Entry::recovered` rather than only logged.
- **One count bound in both directions (§7).** The original's declared form refuses zero or less
  while its wire form accepts any thirty-two-bit value, including negatives and values far past any
  stack size.
- **The portion is immutable (§7).** The original reduces the candidate stack in place and its own
  outstanding count, so a shared instance is corrupted by one consume and its copy operation exists
  to work around that. Nothing here needs a defensive copy.
- **The two flavours are a sum type (§7).** The cast, the runtime flavour predicate and the failure
  mode go together.
- **The two three-channel colour constructors have distinct names (§8).** In the original they differ
  only in argument type, so a bare literal triple selects the integer overload and means one
  two-hundred-and-fifty-fifth of full intensity.
- **One highlight structure, keyed by everything its equality compares (§9).** The original's expiry
  dequeues the head and removes from the set whatever compares equal to it, and a face-agnostic entry
  equals a face-specific one at the same position, so expiry can evict a live entry and leave an
  expired one drawing forever.
- **Highlights expire on a call and are cleared on leaving a world (§9).** The original expires only
  while drawing and never clears, so entries survive into the next world until their stamp passes and
  the level test merely hides them.
- **A lifetime is a duration against a monotonic reading (§9).** The original compares against the
  system clock, so a clock adjustment lengthens or shortens every pending highlight.
- **A map has one encoding (§10).** The original iterates the map, so the output order is the default
  unordered map's iteration order and is not stable across runs. A repeated key still keeps its last
  occurrence on the way in, which is observable behaviour rather than a defect.
- **The list wire form is bounded (§10).** The original reads exactly the declared size with no upper
  bound, so on a client-to-server path a declared size of two billion loops two billion times.
- **A colour outside the unit interval packs at the edge (§8).** Nothing clamps the stored channel,
  which is what §8 says is representable, but a packing has one byte per channel and a channel of two
  does not fit one. Packing at the edge keeps a channel from running into the byte beside it. §8 does
  not say what the original does with that overflow and §14 does not record it, so this is a choice:
  a colour inside the interval packs identically and one outside it does not.
- **Three byte-level framings are this package's own (§5.1, §7, §10).** Each is named at its site as
  well as here, because §14 records neither and a peer that frames differently disagrees. An integer
  and a long travel as a zigzag variable-length quantity, seven bits per byte: §5.1 says only "a
  variable-length quantity". A list element carries its own four-byte length, because §10's element
  codec is the host's and opaque bytes cannot say where they end. And §7's wire form has no framing
  at all — the matcher's bytes then four bytes of count, exactly the shape §7 states — which is a
  framing this package removed rather than added, and the price is that only the host's own reader
  knows where a matcher form ends.
- **The registry-presence predicate is named for what it returns, and a malformed spelling is absent
  (§12).** The original's is named for invalidity, returns true when the identifier is present, and
  its string overload parses before testing, so the routine named for invalidity raises on a
  genuinely invalid identifier.
- **Events are counted rather than collected (§2).** This package's own: a list of every tick a
  session saw is a vector that only grows. `EventCounts` holds three saturating counters.
- **A run cannot refuse (§4).** Every refusal belongs to the declaring call — an illegal path, a
  duplicate, a ceiling, and a block item form that would carry a declared item's identifier — so
  running a collection that was built always answers. Without that, the surface a host asks what this
  mod declares had to turn a refusal into "this mod declares nothing", which is a swallowed error at
  the one place built to report the declaration.
- **Every collection and index is bounded.** Channel payloads, action tables, content declarations
  per kind, matcher members, highlight entries, saved memories and every wire length have a stated
  ceiling and refuse past it. Payloads and documents here are peers' and pack authors', so none of
  their sizes is this package's to trust. The specification does not require this and every ceiling
  is a constant with an upgrade path.
- **A ceiling never silently deletes a world's data (§6.1).** A world holding more saved memories
  than `MAX_MEMORIES` opens, because refusing would cost a player their world, and every memory that
  was not held is named in `Entry::recovered`. That world is then not saved: `save_saved_data`
  answers a refusal, which the host's contract says leaves the previous data on disk. Writing the
  held memories back would delete the rest permanently, which is the silent loss §6.1 exists to fix,
  moved one layer along. The upgrade path is a larger ceiling; there is no version of this where a
  ceiling is worth a player's saved machines.

## Declined outright, and why

§11's two, neither stubbed and neither reachable from this package:

- **The reflection kit.** It finds a method or field by trying several candidate names and makes it
  accessible, stripping the final modifier from a static final field on the way. The several-name
  search exists to survive naming differences between builds, and Ravel's generated-name mapping is a
  first-class supported surface that already meets that need.
- **Direct memory access.** The helper underneath it obtains the Java runtime's unsupported low-level
  memory interface through a private field, reads a privileged lookup at a computed offset, and reads
  and writes arbitrary fields through raw offsets. Ravel forbids unsafe code in every maintained
  crate with no exception. A dependent that needs another package's private state is a Ravel feature
  request.

Declining both removes nothing a dependent can observe through a declared API: §11 records that
neither has a caller inside the original's own tree.

§12's class-name hash strategy is a third decline. It exists only to key §4's class lookup by name, and
`registration::ClassId` keys by identity, so implementing the strategy would be implementing the collision.

## Declarable only, and why

Each needs a seam Ravel does not publish at the reviewed revision. None is stubbed, and in every case
the portable decision logic is implemented and tested.

- **Drawing in world space (§9).** `ravel_sdk::render_stages` publishes the stage name and the order
  among mods and says plainly that nothing draws at any stage. `highlights::declare_stage` produces a
  real resolved order and `Highlights::visible` decides what would be drawn.
- **Block-entity types, data components and capabilities (§4).** `ravel_sdk::content` publishes four
  kinds and none of these three. `Registered::declarable` names and orders what a host would register,
  and `Registered::attached` is which capability reaches which declaration.
- **Carrying a payload between a client and a server (§5).** `ravel_sdk::network` publishes the
  contract, the framing and the side rule and states that nothing routes a mod's packet yet.
  `Channel::contract` produces a real Ravel contract whose name is the identifier this channel
  derived, so what Ravel would route on and what a dependent declared are one string.
- **The trigger that clears a client's highlights (§9).** Ravel publishes no disconnect event, so
  `Entry::on_disconnect` is implemented and its caller is the host's.
- **The trigger that records a running server (§2, §5).** Ravel publishes no server-started event.
  What a radius send does with and without one is here.
- **The game's own stream forms (§5.1, §7).** An item stack, a compound tag and an ingredient travel
  in forms the host owns and Ravel publishes no reader for. They are carried as bounded opaque bytes:
  this package frames, counts and refuses them, and what they mean is the host's.
- **The recipe manager's listing of loaded recipes (§1.2, §6).** The one access-widening declaration
  the original ships. `recipes::Catalogue` is the seam; nothing here reaches a recipe list.

## Two shapes worth stating

- **A search state machine cannot be parked by something that did not happen.** `Search::search` returns
  `Outcome::CouldNotSearch` before it touches any of the four pieces of state, so the "could not
  search" case is the type's shape rather than a discipline a later edit could forget.
- **The client's highlights and clock are not built on a server.** They live in a `ClientOnly`, so a
  server side does not hold them rather than holding them and hiding them, and `Entry::advance_to`
  costs a server nothing.

## Evidence

Every rule with a number, an ordering, a refusal or a format has an inline test.

The wire format: every one of the eight kinds round-tripping, an absent argument vector and an empty
one staying different, a tag outside the numbering refused rather than indexed, a declared count of a
billion refused before anything is allocated, a vector past the bound refused at the writer too, a
body past the bound refused before a field is read, text and opaque fields bounded at both ends, a
truncated buffer, trailing bytes, a boolean that is neither zero nor one, an unnamed action, an
overlong length refused rather than wrapped, an unnamed action refused at the reader as well as at the
writer, a supplied numbering moving every tag and the two
encodings differing, six ways a supplied numbering is refused, the declared numbering absent rather
than defaulted, and reading an argument answering absent rather than casting.

The dispatch: a handler running with its effect observed, a wrong arity and a wrong kind never
reaching it, a bare payload reaching an action that declared no arguments, an unknown name ignored and
said so, a payload for the other surface refused before the name is looked up, an optional slot taking
its kind and nothing in its place, and a table refusing an unnamed action, a duplicate and a full
table.

The channel: a payload named in the channel's namespace in declaration order, the direction flag
deciding which way it travels, the Ravel contract carrying the derived identifier and the channel's
version, an illegal path, a duplicate and a full channel, a radius send with no server answering
rather than faulting, all four targets resolving once a server runs, a payload sent against its
direction, a radius that is not a number, and an unnamed level and player.

The search: a match remembering and clearing both flags, the fast path leaving the changed flag
exactly where it was, the first failed search after an input change not parking the block and the
second parking it, an input change unparking without searching, no level concluding nothing, a stale
remembered recipe falling through to a scan, a fresh block ticking, and a restored block holding its
recipes and neither flag.

The persistence: what was written being what is read back, an unset entry absent from the document, a
document with no nested part opening, an illegal spelling and a lost recipe both absent rather than
errors, an entry that is present and not text being the one reported failure, a value written into the
outer document not being read as the nested one, the envelope carrying the document and the version,
three ways supplied spellings are refused with the declared ones absent, and supplied spellings
changing the document.

The quantity's declared form: a portion written and read back under the supplied spellings, supplied
spellings changing the document and a document under one spelling not reading under another, three
ways the spellings are refused with the declared pair absent, an omitted count meaning one, a count of
none and a negative and an over-large one refused the way the wire form refuses them, and a document
that is not an object, one with no matcher, and one whose count is not a whole number.

The quantity: the min-of-available-and-outstanding rule at both ends, a shared portion not being an
accumulator, nothing moving with nothing outstanding, nothing moving for a rejected candidate, a fluid
and an item portion never confused, a declared count of none refused, a count past the bound refused
at the declaration and the writer, a matcher accepting nothing or too much, the wire round trip, a
negative count off the wire, a matcher form past the bound refused before anything is taken, a buffer
with no room for the count refused, and the matcher form being whatever precedes the count because
nothing frames it.

The colour: a byte-built colour round-tripping exactly, every packing truncating toward zero, a
channel outside the unit interval representable and packing at the edge, two colours within one part
in two hundred and fifty-five being one colour and one hash, a colour finding itself as a key, the two
three-channel constructors not being confusable, a channel that is not a number packing as none of it,
and the alpha channel sitting where each packing says.

The highlights: an entry shown in its own level until its time passes, a face-agnostic and a
face-specific entry at one position being two entries with expiry taking only the one whose time came,
an already-held entry keeping its own expiry, expiry running without a frame while drawing removes
nothing, leaving a world forgetting everything, a lifetime as a duration from a monotonic reading with
the reading saturating, a predicate hiding an entry without dropping it, an unnamed level, a lifetime
of nothing and a full set, the box and colour carried, and the stage resolving on the client alone.

Content registration: the identifier being the namespace and the path, a block declared with no item
form, this package's own run order and what is declarable rather than registered, the barrier claiming
what the collection ran, a capability attaching with no wiring call, an item capability reaching a
block's item form and not a block without one, a block-entity type keyed by identity with its name
carried only for a report, one class holding one block-entity type, an illegal path and four
duplicates, a block and an item sharing a path, a block item form and an item at one path refused in
both orders, a capability declared twice and a capability list past its ceiling, and a kind past its
ceiling.

The codec shapes: a pair written and read under its spellings, supplied spellings changing the
document, three refusals with the declared spellings absent, documents of the wrong shape, an
unordered map having one encoding whatever order it was built in and its pairs coming out sorted, a
repeated key keeping its last occurrence, a list round trip and an empty list,
a declared count and a declared element length past their bounds, a truncated and a trailing list, the
writer's own bounds, the optional form with an absent value carrying nothing after its flag, a
compound form telling an empty compound from no compound, and a component type pairing its two forms.

The utilities: a clamp at both bounds and for a value that is not a number, the presence test
returning what its name says, a malformed spelling not being held rather than failing, every
loaded-package question reaching the loader rather than a second list this package keeps, a quarter
turn about each axis, and a rotation leaving its
centre where it is.

And the library itself: the declared identity, a library that declared nothing claiming nothing, what
a dependent declared being what the barrier claims, client-only state absent on a server and costing
it nothing, a client tick advancing the clock and expiring what its time has passed, events counted
rather than collected and the count saturating, leaving a world forgetting the highlights, the whole
lifecycle, a memory surviving a save and a load, a malformed world entry opening with the failure
recorded, a fresh world holding nothing, memories named and bounded, the search a dependent drives
being the one this library saves, a running server recorded where a radius send reads it, single
construction claimed once and the second claim refused, the exported entry building a mod every time it
is called, a world holding more memories than this build can opening with every dropped memory named
and refusing to save over them, and no area being both declined and host-blocked.

The generated documents: `generated/transport.txt` is the wire table, one row per argument kind with the tag
the placeholder numbering gives it, the kind that numbering reads the tag back as, and the body a
one-argument action carrying it encodes to; `generated/slots.txt` is the whole admittance relation, every
slot this package can express against every argument kind. Both are rendered by `src/dumps.rs` from the
crate's own functions and nothing in either is typed by hand, so a change to the format or to what a slot
admits changes the document. One test, `dumps::tests::the_generated_documents_regenerate_byte_for_byte`,
rebuilds both and compares them against the bytes on disk, and writes them instead when
`RAVEL_REGENERATE_GENERATED` is set. Flipping one cell of `generated/slots.txt` fails it, which is how it
was proved rather than read.

`checks/verify-counterpart-build.sh` runs the counterpart tests, including numeric conversion edge cases.
