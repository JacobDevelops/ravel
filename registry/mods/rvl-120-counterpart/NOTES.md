# RVL-120 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-120.md`, written against
`ravel-sdk`, with checked `num-traits::ToPrimitive` conversions after explicit truncation
and saturation. No upstream source was opened, decompiled or read to write it; the
specification is the only description of the behaviour that was used.

The entry is a foundation seed with no gameplay of its own and five admitted dependents waiting on
it. §1.1 says what survives as counterpart work and this package is that: the decision rules, the
wire contracts and the portable arithmetic of §5 to §12. Everything Ravel already owns — content
declaration and the registration barrier, configuration declaration and resolution, saved-data
ownership, side isolation, packet contracts and their framing — is expressed through Ravel rather
than rebuilt. The screen toolkit, the container machinery, the data-generation helpers and the
disabled multi-part block are the four areas §17 excludes and none of them is here.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side, lifecycle and the observed signals | implemented | `src/lib.rs` |
| §2 the client-only observers | declarable only, as Ravel's own side isolation | `src/lib.rs` |
| §3 the three server settings and their bounds | implemented | `src/settings.rs` |
| §3 the seventy colours as a document shape | implemented | `src/settings.rs` |
| §3 the hexadecimal storage, the forced alpha, the unguarded parse | implemented, one declared change | `src/settings/colour.rs` |
| §3 the re-resolve scoped to the client document | implemented, driven by the side | `src/lib.rs`, `src/settings/colour.rs` |
| §4 the one recipe kind | implemented, claimed at the barrier | `src/census.rs`, `src/lib.rs` |
| §4 the two attachments and seven component kinds | declarable only, carried as data | `src/census.rs` |
| §4 the unreachable block-state document | not shipped, deliberately | `src/census.rs` |
| §4 the two real language keys and the one texture | implemented as data | `src/census.rs` |
| §5 the four fields and their defaults | implemented | `src/preferences.rs` |
| §5 the dirty flag set at construction | implemented | `src/preferences.rs` |
| §5 reset anchoring where construction does not | implemented | `src/preferences.rs` |
| §5 the two setters and their different refusals | implemented | `src/preferences.rs` |
| §5 not surviving a death or a dimension change | declarable only, no attachment registry exists | — |
| §6 the four fields and the unowned default | implemented | `src/data.rs` |
| §6 the saved display word | implemented, spellings read from a build-time input | `src/data.rs` |
| §6 the ordinal wire layout | implemented | `src/data.rs` |
| §6 the optional identity beside the non-optional name | implemented | `src/data.rs` |
| §6 ownership gated on the server setting | implemented | `src/data.rs`, `src/settings.rs` |
| §7 the three command shapes, and running one | implemented, handler and answer both | `src/commands.rs` |
| §7 the five exempt element types and the codec rule | implemented, as a declaration-time refusal | `src/commands.rs` |
| §7 the two key shapes and the return-false contract | implemented, one declared change | `src/commands.rs` |
| §7 the typed argument bag | implemented, tagged on the wire | `src/commands.rs`, `src/network.rs` |
| §7 discovery by inspecting a type | host-blocked | — |
| §8 the ordered map and the coarse dirty rule | implemented | `src/values.rs` |
| §8 the positional wire form and its ordering requirement | implemented, and carried by a payload | `src/values.rs`, `src/network.rs` |
| §8 the enumeration adapter's bounds | implemented, with the declared change | `src/values.rs` |
| §9 the channel, its version and its optionality | implemented as data | `src/network.rs` |
| §9 twelve of the thirteen payload kinds | implemented, as thirteen contracts | `src/network.rs` |
| §9 the framing, the contract identity and the version | implemented through the SDK's `Packet` | `src/network.rs` |
| §9 the login-finalised payload | not shipped, deliberately | `src/network.rs` |
| §9 the opaque payload's fixed prefix and silent drop | implemented, bounded | `src/network.rs` |
| §9 command names and argument bags as untrusted input | implemented | `src/commands.rs`, `src/network.rs` |
| §10 the overworld pinning, the factory and the client refusal | implemented | `src/save.rs` |
| §11 the counter, the persistence and the six-neighbour merge | implemented | `src/grouping.rs` |
| §11 the argument-order asymmetry | implemented as one fixed order, declared | `src/grouping.rs` |
| §11 the three-or-more re-stamp | implemented over the merged ids only, declared | `src/grouping.rs` |
| §11 removal, the two passes and the one reused id | implemented | `src/grouping.rs` |
| §11 the unbounded recursion and the chunk-loading limit | implemented as bounded iteration, declared | `src/grouping.rs` |
| §12.1 the signed accumulator and the arithmetic shift | implemented, pinned by vectors | `src/arithmetic.rs` |
| §12.2 the table, the two integer entry points, the count first | implemented, polynomial from a build-time input | `src/arithmetic.rs` |
| §12.3 the clamp and the ignored capacity | implemented | `src/arithmetic.rs` |
| §12.3 the disabled fuller scheme | not carried, by the specification's own exclusion | — |
| §12.4 the coder, the cap and the framing | implemented, with the declared change | `src/primitives.rs` |
| §12.5 the interner, the rounding helpers, the orientations | implemented | `src/primitives.rs` |
| §13 the assembly rule, its grid order and its overwrite | implemented | `src/recipes.rs` |
| §13 the recipe document and its matching | declarable only | — |
| §14 the one binding and its category | implemented as data; default F1 recorded below | `src/lib.rs` |
| §14 the poll with no edge detection | implemented as a press, with the declared change | `src/lib.rs` |
| §14 the modal input interception | not carried, by the specification's own exclusion | — |
| §15 the four optional integrations | host-blocked, recorded as pending | `src/lib.rs` |
| §15 the manual latch, and absence not being an error | implemented, re-readable | `src/lib.rs` |
| §16 what dependents need, as crate-root public functions | implemented | `src/lib.rs` |
| §17 the four excluded areas | not carried, by the specification's own exclusion | — |

## Two compatibility-required values this package refuses to invent

Both are serialised spellings or constants the specification marks obtainable only from the pinned
artifact. Neither is guessed. Each is read from a build-time input, each has a placeholder the tests
pin the surrounding **format** with, and each is an error rather than a default when the build
supplies nothing — because a package that has to agree with the original cannot fall back to a
placeholder and still claim to agree.

- **The three saved redstone words (§6).** `data::Spellings::declared` reads
  `RVL_120_COUNTERPART_REDSTONE_SPELLINGS` as three comma-separated words in ordinal order and
  refuses anything that is not three distinct non-empty ones. `data::Spellings::placeholder` carries
  this package's own three words, and every saved round trip is tested against those.
- **The 64-bit check's polynomial (§12.2).** The specification states the width, the byte order, the
  zero seed and the absent inversion, and does not state the polynomial.
  `arithmetic::Table64::declared` reads `RVL_120_COUNTERPART_CHECK64_POLYNOMIAL` as hexadecimal;
  `arithmetic::Table64::placeholder` derives the table from a published ordinary polynomial that
  matches the stated conventions and is not a guess at the artifact's. The table is derived from the
  polynomial rather than transcribed, and both its entries and the byte-order rules are pinned.

The 32-bit check of §12.1 is stated completely and is implemented outright, with vectors derived from
the stated arithmetic. It is signed, the shift is arithmetic, and a test holds it apart from the
textbook rule so that a later "simplification" to an unsigned accumulator fails rather than passes.

## Behaviour changes, each declared rather than incidental

- **The re-resolve of the seventy colours is scoped to the client document on both paths (§3).** The
  original tests which document arrived on load and does not test it on reload, so reloading the
  server document on a dedicated server re-reads seventy client values with no loaded backing.
  `Colours::resolve` takes a `Document` and answers false for the server one, and `Entry::configure`
  passes `Document::of(side)`, so the scoping runs in production and not only under test: an instance
  built for either server side reads nothing where the original reads seventy.
- **The three server settings are declared `common` rather than `dedicated-server` (§3).** Ravel has
  no "any server" side, and `SettingSide::DedicatedServer` applies to that side alone. This package
  declares `integrated-server` as a supported side, so declaring the three the way §3's server
  document reads would put them on no screen in single-player: a player could never turn the
  ownership record off or move the infusion ceiling. They are `common`, which reaches every side a
  server runs on and one it does not.
- **The compatibility test is called in one fixed order (§11).** The original calls it with the
  neighbour first in the one-neighbour case and with this block's group first in the many-neighbour
  case, and nothing requires the dependent's test to be symmetric. `Grouping::place` calls it
  `(own, neighbour)` in both, so an asymmetric test answers the same question the same way however
  many neighbours there happen to be.
- **The re-stamp covers only the groups that merged (§11).** The original adds every adjacent id to
  the set it floods, whether or not the test accepted it, so a refused neighbour is absorbed anyway
  as soon as a third group is touching. That contradicts its own two-block case. `Grouping::place`
  floods the union of the absorbed ids and its own, and `Placement::Merged` names the refused ones.
- **Both walks are iterative and bounded (§11).** The original's flood fill and group discovery
  recurse over neighbours with no depth bound, so a large group exhausts the stack, and the clearing
  walk carries no visited set so it revisits what it has already stamped. Both here are queue-driven
  with a visited set and refuse past `grouping::MAX_REGION` rather than failing in the runtime.
- **The enumeration adapter validates instead of indexing past its array (§8).** The original indexes
  its value array by the received number with no bounds check and raises on a mismatch.
  `values::read` refuses with the index and the count, and `values::clamped_index` is there for a
  caller that would rather carry on than refuse.
- **A duplicate key is diagnosable in both places a declaration is filed (§7).** The original
  replaces silently under either key shape. `Commands::declare` answers a `Replaced` naming what the side
  table held *and* what the shared element table held, so two namespaces declaring one list-command
  name no longer disagree about the element codec in silence. The return-false contract for an
  *unknown* key is not changed, because dependents branch on it: `Commands::dispatch` answers `None`
  where the original answers false.
- **A handler that answers something its shape did not promise is refused (§7).** The original's
  three shapes are a property of the method a marking is put on and nothing checks the pairing.
  `Commands::dispatch` runs the handler filed with the declaration and refuses an answer that does
  not match the declared shape, so `Shape` is a promise about what comes back rather than a label.
- **The argument bag is typed on the wire (§7, §9).** All three of §7's argument types cross the
  network as themselves: `network::TypedCommand` writes a tag beside each argument, and
  `TypedCommand::bag` is where a tag this package does not publish is refused, alongside the count
  and text-length bounds the same untrusted bag is held to.
- **A ceiling is not a framing error (§9).** `ravel_sdk::network::WireError` is a closed set whose
  `Truncated` arm means the buffer ended inside a field, and it has no arm for a field that is longer
  than this end admits. Reporting an over-long payload through it would print "the packet ends early"
  about a packet that ended late, so `network::admit` decodes through the SDK and then applies this
  package's own `PayloadRefused::TooLarge`.
- **The value map has a payload to travel on (§8, §9).** §9's census counts a server-to-client screen
  data kind apart from the opaque payload that travels both ways. `network::ScreenValues` is it: it
  carries what `values::write` produced and fills a map of the same shape at the other end.
- **A write is bounded wherever the matching read is (§6, §8, §9).** `values::write`,
  `BlockData::to_saved` and the argument count all refuse at the sender what the reader would refuse,
  so a mistake surfaces where it was made rather than at the far end.
- **A partly applied grouping change is not reachable (§11).** `Grouping::place`, `clear_around`,
  `reassign` and `remove` compute every walk and issue every id before writing a stamp. The original
  stamps as it goes, so a walk that gives up part way leaves a block stamped with an id nothing
  merged, or leaves whole regions reading as belonging to no group with no path back.
- **A float never makes a subscription dirty forever (§8).** `Subscription::refresh_if_changed` compares floats by
  their bit pattern. Comparing them by equality would make a map holding a value that is not a number
  report dirty on every check, re-sending the whole value set every tick for the life of the screen.
- **Neither the lifecycle nor the observed signals are kept as a log (§2).** The loader broadcasts an
  event per tick. `Entry` keeps a count and the last one for each, so a running server does not grow
  for the length of a session.
- **A malformed declaration is reported rather than swallowed (§2, §3, §4).** Both declarations are
  built once in `Entry::new` and their refusals are carried into `enter`. Reading a malformed
  configuration declaration as "this package declares none" would drop all seventy-three settings to
  their shipped defaults with nothing said anywhere.
- **A list command with no codec is refused at declaration (§7).** The original discovers this by
  instantiating the codec interface its marking defaults to, which fails, and the surrounding catch
  raises when the type is first inspected. `Declaration::check` refuses at the declaration point,
  which is the same startup-time fault said plainly.
- **The key binding is a press (§14).** The original polls the binding with a direct device query and
  no edge detection, so holding the key re-opens the manual on every client tick.
  `Entry::manual_key` edge-detects, so holding it opens the manual once.
- **The run-length reader has an explicit end (§12.4).** The original answers zero forever past the
  end, so a caller has to already know how many values it expects. `Runs::next_value` answers
  nothing, and `Runs::next_or_zero` keeps the original rule for a dependent that relies on it.
- **Every decoded count and index is bounded (§6, §8, §9, §10, §11, §12.4).** Length prefixes,
  argument counts, text fields, record names, walk sizes and coded-buffer totals are all checked
  against a stated ceiling before anything allocates or indexes. The specification adds this
  requirement for the network surface and the same reasoning covers saved data.
- **The login-finalised payload is not shipped (§9).** It is registered, sent and inert in the
  original. `network::PAYLOADS` names the thirteen contracts carrying the other twelve kinds, and a
  test written from §9's own census holds it to naming no login payload.
- **The unreachable block-state document is not shipped (§4).** It names a model for a block nothing
  registers at the reviewed revision.
- **Configuration access failures are refused (§3).** `Settings::read` returns missing-key and
  wrong-kind errors rather than replacing them with defaults. `Colours::resolve` preserves the same
  typed errors through `ColourRefused::Value`; a refused client document changes no colours.
  An infusion ceiling outside one to the signed 32-bit maximum still keeps the shipped default
  rather than being clamped.
- **Wire refusals identify the failed boundary (§5, §9, §12.4).** Negative or incomplete framed
  lengths are truncation, not decoded-value limit breaches. A complete preference style containing
  invalid UTF-8 is `NotText`, not truncation. A complete preference record followed by surplus
  bytes is refused as `Trailing`. The framed-block reader reads only its first block and leaves
  following bytes uninterpreted, preserving its existing one-block contract.
- **Specification-only metadata stays in prose.** The manual binding defaults to F1, the original
  channel version is `1.0`, and the nested recipe field and book category are `recipe` and `misc`.
  None currently drives a host mechanism, so unused constants do not pretend to implement one.
- **The manual latch may be re-read (§15).** The original latches presence once at common setup and
  never re-checks. Nothing depends on the latch being stable, and absence is not an error either way.

## Declarable only, and why

Each needs a seam Ravel does not publish at the reviewed revision. None is stubbed.

- **Discovering a dependent's statically declared commands (§7).** Ravel publishes no way for one
  package to inspect another's types, so the original's annotation scan has no equivalent. What it
  does publish is an ordered registration window, so a dependent declares into
  `Entry::commands_mut` at its own declaration point and the guarantee that a command exists before
  it is called comes from declaration order. Everything a dependent observes about the transport —
  the three shapes, the exempt element set, the startup refusal, the return-false rule and the
  bounded argument bag — is implemented.
- **The two attachments and the seven item data component kinds (§4).** Ravel's registry has four
  kinds and none of them is a per-owner attachment or an item data component, so the nine role names
  are carried as `census::ATTACHMENTS` and `census::COMPONENTS` rather than claimed. What a
  registry-parity run measures is the census and the roles, and both are here; the one recipe kind is
  a real claim through `content::claim_all`.
- **Carrying a packet between a client and a server (§9).** `ravel_sdk::network` publishes the
  contract, the framing and the side rule and states plainly that nothing carries a mod's packet yet.
  Every payload here implements `Packet`, so `encode` and `decode` own the framing and each body
  travels behind its own contract id and version; `network::sent` and `network::admit` are the two
  ends and nothing writes an envelope by hand. Each round trip and each refusal is proved in the
  package's own tests. The leg after that is Ravel's and is not written.
- **The preference record not surviving a death or a dimension change (§5).** Ravel publishes no
  attachment registry, so there is nothing this package could attach the record to and no lifetime it
  could declare for one. `census::ATTACHMENTS` carries the two role names as data and
  `src/preferences.rs` implements the record itself: the four fields, the dirty rule and the two
  setters. Nothing here expresses the non-persistence rule and nothing pretends to.
- **The recipe document and its matching (§13).** Ravel claims a recipe kind as a registry entry and
  decides nothing about what one does, so no document is serialised. The portable half is the
  assembly rule, and `recipes::assemble` is it, including the grid order and the overwrite the
  original neither prevents nor documents.
- **The four optional integrations (§15).** Ravel publishes no facility for optional cross-mod
  integration. `INTEGRATIONS` names the four roles and nothing stubs one. The rule that survives is
  that absence is never an error, which `Entry::latch_manual` and `Entry::manual_present` hold.
- **The client-only observers of §2.** Ravel's `sides::ClientOnly` is where client-only work goes and
  the loader decides what runs where, so the original's three extra client registration points are
  Ravel's side isolation rather than a runtime side test in this package.

## Two shapes worth stating

- **The value map is insertion-ordered, and that is a wire format.** §8's encoding carries no names
  and no count, so the two sides agree only because they walk the same map in the same order.
  `ValueMap` is a vector of pairs and never iterates a hash, redeclaring a name keeps the position it
  already held, and `ValueMap::shape` makes "the declared set changed" a checkable fact rather than a
  silent disagreement a screen discovers by never updating.
- **Nothing this package declares travels to a joining client.** Both configuration documents are
  declared and neither is synchronised, which preserves §2's arrangement: the seventy colours are
  declared on the client side, the three server settings are declared `common` for the reason above,
  and a client and a server that disagree about §3 disagree silently.

## A gap found in the published surface

- **`ravel_sdk::save::SavedDataVersion::new` answers an `Option` and `SavedDataError` has no arm for
  a zero version.** A package that builds its version from a constant has to invent an error to carry
  the impossible case, and `save::version` names `NoVersionsDeclared` for it, which is the nearest
  true statement rather than the right one. A `SavedDataError::ZeroVersion`, or a checked constructor
  that returns the error type the rest of the module uses, would close it.

## Evidence

Every rule with a number, an ordering, a refusal or a format has an inline test: the 32-bit check's
zero-reading seed, its three pinned vectors, all-ones folding back to all-ones, the arithmetic shift
held apart from the textbook rule, the count-first path and two lengths that cannot collide; the
64-bit table derived rather than transcribed at three entries, the zero seed that is never inverted,
the byte vectors, the word entry point feeding least significant byte first, the single-argument one
keeping only the low byte and disagreeing with it, the count-first path, the two checks sharing no
seed and no bit order, and the polynomial refused three ways for what a build supplies and agreeing with whatever this build
supplied; the energy clamp
saturating above and truncating below, a negative amount passing through, a value under the signed
minimum wrapping, and the capacity ignored at four values; run coding with a cap continuing in a new
pair, every buffer reading back, an odd length refused, a hostile total refused before it allocates,
the explicit end beside the zero-filling reader, the frame's absent and present forms, and a frame
announcing more than it carries; interned ids from one upward never reissued; the floor helpers
rounding below zero where truncation would not, the single-precision ceiling, and saturation at both
ends and for a value that is not a number; twenty-four orientations with the face from the ordinal,
rotation cycling inside the group of four and returning after four, a fifth step being the first, and
twenty-four distinct names; three server settings and seventy colours declared, seventy roles named
once across seven components, the alpha dropped on the way out, the alpha forced on the way in so no
colour can be translucent, six ways a colour is refused, every shipped default round-tripping, the
three defaults, a resolved out-of-range ceiling keeping the default, missing setting keys and
wrong declared kinds refused, the shipped colours all opaque, a server document re-resolving nothing
and a client document re-resolving everything, the document each side resolves, the three server
settings reachable on both server sides, and refused colour resolution leaving all colours unchanged
even after a non-default earlier colour has been read; the unowned
default at all four fields, a name without an identity being representable, the three ordinals in
order, the placeholder words distinct and not the constant names, malformed supplied words refused
six ways, the supplied words refused for absence and agreeing with whatever this build supplied, both
round trips including the unowned case,
no placeholder identity written in either form, the saved word and the wire ordinal disagreeing by
design, an unknown saved word refused, six malformed documents refused, a wire record refused
truncated, trailing and out-of-range, a lying length prefix refused before it allocates, and an
over-long name truncated where a profile supplies it, refused where a record carries one, and refused
at the read; the fresh record's corner and offsets,
its dirty flag at construction, one send clearing it and a clean record sending nothing, every
mutation setting it, a refused change leaving it clean, reset anchoring top-left where construction
anchors bottom-right, the style setter taking all three published names and refusing four others, the
four corner ordinals, the record arriving clean over the wire, and malformed preference records
refused for truncation, trailing bytes, invalid UTF-8, corner and style; the five exempt element types
needing no codec and a sixth needing one, a missing codec
filing nothing at all, shape and element having to agree, one key kept apart by the side it runs on, a
duplicate key reporting what it replaced in the side table and in the shared element table, a list
command filing its element type, an unknown key answering nothing on both sides, a declared handler
running against the bag the caller sent, a command with a result carrying its typed values back, an
answer that does not match its declared shape refused, five illegal keys and two over-long halves
refused, and an argument bag bounded by count and by text length before it reaches a handler; the map
keeping declaration order, a redeclared name keeping
its position, the first difference dirtying the whole set and reporting once, a later change being
one verdict too, a value that is never equal to itself reporting dirty once rather than every check,
two floats differing only in their bits told apart, a text value refused at the write, every value
round-tripping positionally, a shorter shape refused as trailing, a
write ending inside a value refused with the map left alone, an enumeration index outside the set
refused at the count and one past it, clamping inside the set and answering nothing for an empty one,
a boolean that is neither zero nor one, a text field announcing more than it carries, and the shape
changing when the declared set does; the channel optional at its declared version, §9's census of
one kind both ways and six each way written out and held against the shipped table, the inert
thirteenth absent, thirteen legal content paths each named once, every shipped payload type
travelling under one of them, the opaque payload under two contracts, a payload refused for naming
another contract and for naming another version, its round trip read whole, both fields refused as
ceilings rather than as short packets, a buffer ending inside a field and one ending with bytes to
spare refused, four routing cases including the silent drop, the style choice round trip and its
refusal, the style being the only client-to-server path into the record, the preference push reading
back clean, the value push carrying a positional write to a map of the same shape and refused by one
of another shape, a typed command carrying an integer and a boolean as themselves, an invented key
refused, an argument tag this package does not publish refused at the bag, an announced count the
buffer does not carry refused before it is believed, an argument count refused as a ceiling, a
command result bounded, and the list request being a second contract told apart by its framing; the
client refusal on all three global accesses with nothing created, both server
sides reaching one store, the factory used only when absent, a name bounded at both ends, the record
ceiling with a held record still reachable at it, the overworld scope round trip, four malformed
payloads refused, and the declaration naming one version; ids only increasing, a block touching
nothing keeping its fresh group, one accepting and one refusing neighbour, the fixed test order in
both cases, several neighbours merging with the refused one left out, the re-stamp reaching the whole
of every merged group, a second placement refused, every touched block reading as no group between
the two passes, a removal dividing the group with the first new one reusing the id, a reused id
naming a different set afterwards, an end block leaving one group, the only block leaving nothing, a
removal of nothing refused, a walk past the bound refused, a merge past the bound leaving every stamp
as it was, a removal past
the bound leaving no block reading as no group, a reassignment with no id left to issue stamping
nothing, the counter having an end, a save carrying every group and the counter, a save taken between
the two passes refused at the write, one position filed under two ids refused, a counter behind its
own groups refused, and eight malformed saves refused; the recipe kind's contribution, absent data
skipped rather than cleared, undeclared data not
contributed, every matching ingredient contributing, two ingredients making the grid order-dependent
both ways round, and an empty grid; and one recipe kind
claimed at the barrier with nothing else, the counted documents held against what the declaration
actually claims, the census counts, the one role registered twice, the two language keys derived from
the declared identity and the declared binding, the one interface texture, the lifecycle counted with
its last step and the signals counted with the last one, ten thousand signals leaving nothing to
grow, a dedicated server re-resolving no colour where a client resolves seventy, an empty store
producing no envelope, a written record reaching one, the latch starting unset and re-readable, the
binding as a press and gated on an open screen, the four integrations, a dependent's declaration
being what a call runs, and the crate-root functions a dependent reaches for.

`checks/verify-counterpart-build.sh` runs the package's inline tests.

Three of the tables above are also rendered to disk under `generated/`, by `src/dumps.rs` and out of
the same functions the tests hold: `generated/census.json` is what `census::claimed` claims beside
the attachment, component, translation, texture and integration names and the two document counts;
`generated/network.txt` is the thirteen contracts `network::contracts` builds, each with its
direction and its version; and `generated/settings.txt` is the seventy-three settings
`settings::declaration` declares, each with its side and its default in the declaration's own JSON
form. Nothing there is transcribed and nothing is invented for the file: a table that changes
changes the document. `the_generated_documents_regenerate_byte_for_byte` is the one test that owns
them — it rebuilds all three and fails on any drift, and writes them instead when
`RAVEL_REGENERATE_GENERATED` is set, which is how the tree is produced.
