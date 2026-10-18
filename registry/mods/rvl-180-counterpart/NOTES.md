# RVL-180 counterpart

Malformed readout packets preserve `InvalidPacket` classification. The loader rejects
that delivery without changing the receiving mod's `Running` state.

An original Rust implementation of `specifications/m6-foundation/RVL-180.md`, written against
`ravel-sdk` and nothing else. No upstream source was opened, decompiled or read to write it, and no
compiled entry of the pinned artifact was opened either. The specification is the only description of
the behaviour that was used, and the specification is itself a black-box respecification: its author
read the artifact's non-code documents and the project's public landing page, and nothing else.

That method decides what this package can be. §11 of the specification orders what a counterpart
must provide by how well the document supports it, and says plainly that a counterpart built from
items 1 to 5 alone satisfies the tag, language, asset and side tests and will not load a single one
of the seven dependents. This package carries four of those five: items 1, 2, 4 and 5. Item 3, an
equivalent sprite set, is **not provided**, so this package does not satisfy the asset test either.
That is undone work rather than blocked work and the section below says so under its own heading.
Nothing here softens §11's statement, and the declarable-only list is where the rest of it went.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §1 no world content of its own | implemented | `src/lib.rs` |
| §2 a version identity that satisfies a range | implemented | `package.json`, `Cargo.toml` |
| §2 what the seven dependents call | declarable only | — |
| §3 the loader declaration | implemented | `package.json` |
| §3 the seventy-seven access widenings | declarable only; not reachable | — |
| §3 the five client-side bytecode transformations | declarable only; not reachable | — |
| §4 the thirty-four groups | implemented | `src/grouping.rs` |
| §4 the aggregate resolving through its leaves | implemented | `src/grouping.rs` |
| §5 the three commands | implemented | `src/commands.rs`, `src/population.rs` |
| §5 the permission level of each | declarable only; intended operator level recorded below | — |
| §5 the eighteen translated lines | implemented | `src/presentation.rs` |
| §5 the two read-only displays | implemented | `src/presentation.rs` |
| §6 loading on both sides, the server half not stubbed | implemented | `src/lib.rs` |
| §7 configuration | deliberately none | — |
| §8 the atlas document | implemented, and stitches nothing | `src/sprites.rs` |
| §8 the twenty-one interface images (§11 item 3) | not provided | `src/sprites.rs` |
| §9 the bundled utility library | provides nothing | — |
| §10 a facility a package name suggested | coined here, not recovered | `src/transport.rs` |
| §10 the original's wire contract, framing and side rule | excluded; not recovered, nothing to implement | — |
| §10 wire compatibility with an existing dependent | declarable only; not reachable | — |
| §3 the version-check endpoint | provides nothing | — |
| §1 mathematics, transformations, model rendering | excluded; not specified | — |

## Behaviour changes, each declared rather than incidental

- **The package's version is a version identity rather than a release count (§2).** §11 item 4 asks
  for a declaration satisfying a range whose floor is at or below the pinned artifact's version and
  whose ceiling is the next major. Five of the seven dependents require at least the minor version
  the artifact carries and two require one patch below it, and all seven cap at the next major, so
  this package declares `4.6.1`: at or above both floors, below the ceiling. That is a number chosen
  to satisfy a range rather than this package's own first release, and it is the reason the version
  is not the `0.1.0` its siblings carry.

- **The reclamation reports over what this package holds (§5).** The original's second command
  reports a reading before and after a collection it performs, and §5 calls it a diagnostic that acts
  on the process. There is no process heap a mod may collect from here: Ravel publishes no
  process-memory surface and a Rust mod has no collector to run. So `Console::reclaim` releases what
  this package itself retains — the notes the removal command accumulates — and the two readings are
  counts of that. The command still performs a release and still reports either side of it, which is
  the shape §5 recovers; what it is a reading *of* is different and is stated here rather than left
  to be discovered. The reclamation is the one command whose own report is not retained: every other
  command's report leaves a note behind, and retaining the reclamation's would refill what the two
  readings it just reported were readings around.
- **All three commands are server-side (§5).** §5 records the side of each as unknown and names
  server-side with an operator requirement as the safe decision for the second and third. This
  package extends that to the first, because counting what is loaded reads world state as much as
  removing it does. `Command::runs_on` refuses a client for all three and the refusal is enforced
  before anything is read or changed.
- **A wide selection holding a player refuses whole (§5).** §5 requires a distinct refusal for an
  attempt that would remove players, and says the command declines the player case rather than
  executing it. This package reads that as declining the whole selection: `entity_sweep all` with a
  player present removes nothing at all rather than removing every non-player and reporting a
  refusal. Removing most of what was asked for and calling it a refusal is the reading that would
  surprise an operator.
- **The leaf membership is inferred (§4).** §4 records that the thirty-two leaf documents were listed
  and not read, and says a counterpart should read its own sixteen against the artifact's rather than
  assume the obvious membership. This package could not read them, so it ships the obvious membership
  — the base game's dyed wool of that colour, in the registry the group groups — and
  `grouping::conventional_member` says so where it is written. If the artifact's leaves hold anything
  else, this is where the difference is.
- **The nesting lives in this package and the generated document is its flattening (§4).** Ravel's
  tag model has no tag that names a tag: a content declaration puts registry keys in a group and
  nothing else. `grouping::Aggregation` is the nested model §4 requires, and what reaches Ravel is
  what `Aggregation::resolve` computed. The ceiling that leaves: a member contributed to a leaf after
  the declaration was built reaches the aggregate by rebuilding the declaration
  (`Entry::rebuild_declaration`), not by the generated document picking it up on its own. Raising
  that ceiling needs a nested tag model in Ravel, not a change here. Resolution is breadth-first, so
  a group is entered at the shallowest depth that reaches it and `MAX_DEPTH` prunes only what is
  genuinely deep; and a resolution that did hit the bound refuses the declaration rather than
  shipping a group short of what it holds.
- **The whole transport is this package's own (§10).** The facility exists here because a class-path
  count filed eighteen of the original's compiled entries under a networking package name and its
  one-sentence description names packets; §1 and §10 both call that a hypothesis rather than a
  finding, and §11 files it under what is named but not specified. So nothing of the original's
  networking was recovered and nothing of it is reproduced: `transport::CONTRACT`, the framing, the
  refusals and the side rule are a contract this package coined against a facility a file layout
  suggested. **This package is not wire-compatible with any unmodified dependent of the original, and
  cannot be made so from anything the specification contains.** Read `src/transport.rs` as this
  package's own design, never as evidence of what the original does.
- **A contained fluid's name is truncated rather than refused (§5).** The name arrives from a
  dependent or off the wire and is bounded at `presentation::MAX_FLUID_NAME` **bytes**, cut back to
  a character boundary. Bytes rather than characters because bytes are what the readout costs on the
  wire. A display with a long name is still a display, and refusing one would leave a dependent with
  nothing to show.
- **The outbox drops the newest and the client view drops the oldest (§10, §5).** Both are bounded
  because nothing in Ravel drains a mod's queue for it at the reviewed revision. A queue that has
  stopped being drained has a stale backlog, so `transport::Outbox` keeps the oldest and counts what
  it dropped; a display shows only the newest reading, so `ClientState` keeps a short window and drops
  the oldest. `commands::Console` follows the outbox's rule for the notes it retains: a note past the
  bound is dropped and the drop is counted in `Console::dropped`, because a bound that discards
  without a count is a bound nobody can see was reached.
- **A population saturates and is capped (§5).** `population::Population` takes counts from outside this
  package, so a kind beyond `MAX_KINDS` is refused, a count saturates at `MAX_PER_KIND` rather than
  wrapping, and a kind name outside a declared charset and length is refused with nothing recorded.
  A kind the removal command takes is dropped from the population rather than left counting zero, so
  a long-lived server that has swept its world clean still has room under `MAX_KINDS` for a kind it
  has not seen before.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision, or a fact the
specification does not contain. None is stubbed: where a shape is declarable the package declares it,
and nothing carries a pretend implementation.

- **What the seven dependents call (§2).** The dependency graph records that an edge exists, not what
  crosses it, and no dependent was read. This is the specification's own integration risk and no code
  here can close it.
- **The seventy-seven access widenings (§3).** They reshape the host so that members not normally
  reachable become reachable, and a dependent may be relying on that rather than on the entry's own
  code. Ravel has no access-widening mechanism and every maintained crate forbids unsafe code, so
  there is nothing here to widen and nothing to declare.
- **The five client-side bytecode transformations (§3).** Bytecode manipulation is not a thing a
  Ravel mod does. The declaration is evidence about sides, which §6 and this package both use, and
  nothing else about it is carried.
- **Each command's permission level (§5).** The intended operator level is two, recorded here rather
  than exposed by an unused constant and accessor. Nothing enforces it, and nothing here can:
  a `CommandInvocation` carries the claimed key, side and words, but no invoker or permission level.
  The side *is* enforced.
- **Where the population comes from (§5).** Ravel publishes no entity surface, so the two
  world-acting commands read and change `population::Population` rather than a world. Every rule the commands
  decide — the totals, the per-kind lines, the empty case and the player refusal — is decided over
  that value and is tested. What is host-blocked is the filling of it.
- **Delivery of a packet (§10).** Ravel's network module states that nothing carries a mod's packet
  to the other end at this revision. The contract, the framing, the refusals and the side rule are
  this package's own and are tested as written here rather than against the original; the leg after
  them is not written anywhere in Ravel.
- **The bundled utility library (§9).** A general-purpose Java utility library is not part of this
  entry's behaviour, and a Rust counterpart needs none of it. §9's open question — whether anything
  else in the pack expects that library on the class path because this entry put it there — is
  unknown and this package provides nothing for it.
- **The version-check endpoint (§3).** A live network call keyed to the game version. §3 says a
  counterpart must not make it, so this one does not.
- **Three-dimensional mathematics, transformations, model rendering, colours and configuration
  (§1).** Named in one sentence of public description and nowhere else. §12 says nine words of
  documentation is not a specification of anything, and this package does not invent one. The
  sixteen dyes of `colour` are the exception, and they are there because §4 observes sixteen colour
  sub-tags rather than because the sentence names colours.

## Not provided, and not blocked

**The twenty-one interface images (§8, §11 item 3).** This is the one thing §11 lists under
"implement as specified" that this package does not carry, and it is listed here rather than above
because nothing blocks it. `policies/source-access.json` permits shipping art rewritten with the
originals as reference and gates only release on author permission, and Ravel publishes no seam that
a `.png` under the package's own assets would need. No image has been drawn, and that is the whole
reason: it is undone work.

Two consequences, both real. This package fails the asset test §11 says items 1 to 5 satisfy.
And `sprites::atlas_document` declares a directory source over a directory holding no image, so a
dependent addressing a stitched sprite by name today resolves to nothing. The atlas is kept because
the path, the single source and the prefix are what §8 observed and are what a drawn set would land
under; it is a document waiting for its images, and reading it as a working sprite atlas would be
reading it wrong.

Whoever draws the set should know one thing about the reference. The black-box method that produced
the specification never opened any of the twenty-one images, so nothing here has seen them. A set
drawn from this package alone is drawn from the twenty-one names and from §8's inferred nine-slice
reading, not from the originals.

## Configuration, deliberately none

§7 records that the entry declares a configuration facility in its own description, ships no
configuration document, and has no pack override; that whether the facility is offered to dependents
or used for the entry's own settings is unknown; and that a counterpart therefore has no schema to
reproduce and should offer a dependent-facing configuration facility only if a dependent turns out to
need one. No dependent has. This package declares no configuration document, which is also why it
implements no `configure`: a mod that declares no settings accepts silently, and one that declared
settings nothing read would be settings a player changes with no effect.

Nothing here writes any file outside what Ravel generates from the content declaration, which §7
notes diverges from nothing the specification can see.

## Evidence

Every rule with a number, an ordering, a refusal or a bound has an inline test. The thirty-four
groups and their distinctness, the aggregate holding nothing directly, a third-party member reaching
the aggregate through a leaf, the eighteen lines split nine and nine, every report printing through a
supplied line, the player refusal changing nothing, the empty case, the wrong-side refusal, the two
units, the twenty-one sprites across three groups, the eight button states, the atlas declaring one
source, the round trip, the short, long and appended payloads, the client that may not send and the
server that may not receive, and every bound named above are all held to an assertion.
`checks/verify-counterpart-build.sh` runs the package's inline tests and its formatter and clippy gates.

Three documents under `generated/` are written by `src/dumps.rs` out of the same functions the rest
of this package uses, and nothing in them is typed by hand. `generated/declarations.json` is the pack
`Entry::declaration` generates, path by path, with each generated file parsed back into the document
it is; `generated/grouping.txt` is one row per member each of the thirty-four groups resolves to, so
the sixteen members an aggregate reaches only through its leaves can be counted rather than taken on
trust; `generated/sprites.txt` is the twenty-one sprite names with their group, whether §8's inferred
reading scales them, and the path a drawing would land on — and no image sits behind any of those
paths, which is the same undone work the section above states. The seventieth test,
`the_generated_documents_regenerate_byte_for_byte`, rebuilds all three and fails if the written tree
differs by a byte; setting `RAVEL_REGENERATE_GENERATED` writes them instead of comparing, and that is
the only way they are produced.

Twelve were proved by mutation rather than by reading. Removing the player refusal, removing the
resolution depth bound, removing the visited set that stops a group naming itself, lifting the
retained-note cap, removing the zero-capacity guard that keeps a fill fraction a number, and removing
the population's kind cap each turned a passing test red. So did the six reverted after review:
leaving a removed kind counting zero instead of dropping it, retaining the reclamation's own report,
resolving depth-first instead of breadth-first, declaring a group whose resolution hit the depth
bound, dropping a retained note without counting it, and bounding a fluid name in characters rather
than bytes.
