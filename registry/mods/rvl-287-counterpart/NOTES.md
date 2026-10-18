# RVL-287 counterpart

An original Rust implementation of `specifications/m6-foundation/RVL-287.md`, written against
`ravel-sdk` and nothing else. No upstream source was opened, decompiled or read to write it; the
specification is the only description of the behaviour that was used.

The entry is a client-side rendering library with no gameplay of its own. §1.1 says what survives as
genuine counterpart work — the quadrant selection rule, the connection condition language, the
neighbourhood probe, the per-kind geometry and the texture slot arithmetic — and this package is
that. Everything Ravel already owns, side isolation above all, is expressed through Ravel rather
than rebuilt. The bake-time and atlas-facing halves are host-blocked rather than declined, because
Ravel publishes no model-baking surface at the reviewed public revision.

## Root workspace lint migration

The crate inherits the root workspace's strict Clippy rules. `SheetFlags` groups its four joins
into `north_south` and `west_east` axis pairs; `SheetFlags::new` now accepts those two `[bool; 2]`
pairs rather than four separate booleans. Within each pair, negative precedes positive. Callers
use `get(face)` for a direction's join. State-property names and geometry remain unchanged;
derived Debug diagnostics now expose the actual `north_south` and `west_east` fields.

## What each section became

| Section | Here | Where |
| --- | --- | --- |
| §2 side, and doing nothing on a server | implemented | `src/lib.rs` |
| §2 lifecycle: kinds registered at construction | implemented | `src/lib.rs`, `src/kinds.rs` |
| §2 the reload listener | implemented, the pack walk host-blocked | `src/lib.rs` |
| §3 what the three dependents need | declarable only; not reachable: all three consume the rendered result | — |
| §4.1 path one, and its short circuit | implemented | `src/lookup.rs` |
| §4.1 path two | host-blocked | `src/lookup.rs` |
| §4.2 the reader over the game's own documents | host-blocked | `src/kinds.rs` |
| §4.3 offering an identifier to every kind | implemented, deterministic | `src/kinds.rs` |
| §4.3 substituting the model at bake time | host-blocked | — |
| §4.3 refusing the inventory variant | implemented | `src/kinds.rs` |
| §5 eight kind names over seven behaviours | implemented, duplicate pair kept | `src/kinds.rs` |
| §5 refusing a second registration | implemented | `src/kinds.rs` |
| §6 the quadrant rule, all eight rows | implemented | `src/quadrant.rs` |
| §7 the condition language and every degenerate row | implemented, defect 5 fixed | `src/conditions.rs` |
| §8 the eight-sample probe and the face basis | implemented, basis derived | `src/neighbourhood.rs` |
| §8 and §9.3 composed onto one face's neighbourhood | implemented | `src/kinds.rs` |
| §8 the appearance query and the occlusion test | implemented as a trait, live access host-blocked | `src/neighbourhood.rs` |
| §9.1 connected block | implemented | `src/geometry.rs` |
| §9.2 connected sheet | implemented, three declared choices | `src/geometry.rs` |
| §9.3 connected pane, geometry and connection rule | implemented, slot assignment declared | `src/geometry.rs` |
| §9.4 large tiled block | implemented, defects 1 and 2 fixed | `src/geometry.rs`, `src/slots.rs` |
| §9.5 rotating column | implemented, defects 3 and 4 fixed | `src/geometry.rs` |
| §9.6 upright column | implemented | `src/geometry.rs` |
| §9.7 pane column | implemented | `src/geometry.rs` |
| §10 the flat scheme and the pane edge defaults | implemented | `src/slots.rs` |
| §10 the per-face scheme and sequential indices | implemented | `src/slots.rs` |
| §10 the three declaration shapes, two parse attempts | implemented | `src/slots.rs` |
| §11 precomputation, baking, culling and the atlas | host-blocked; the stage is declared | `src/lib.rs` |
| §11 the render layer vocabulary | implemented; carried as data, validated | `src/kinds.rs` |
| §12 defects 1, 2, 3, 5 and 6 | fixed, each at a named assertion | `src/geometry.rs`, `src/slots.rs`, `src/conditions.rs`, `src/kinds.rs` |
| §12 defect 4 | fixed by delegation, with nowhere to write the defect | `src/geometry.rs` |
| §12 defect 7 | excluded; not inherited: no such container is written | — |
| §14 the five compatibility-required spellings | not carried; coined instead | throughout |

## Behaviour changes, each declared rather than incidental

- **A missing or unreadable group name never connects (§12 defect 5).** The original raises out of
  declaration parsing and fails the whole model. `conditions::parse_named_group` answers
  never-connects, like every sibling form, and counts the refusal in `conditions::Report`.
- **A non-square tiled grid addresses the cell it declares (§12 defects 1 and 2).**
  `slots::Grid::index` multiplies the row by the grid's width, and `geometry::tiled_coordinate`
  mirrors a coordinate by the same extent it reduces it modulo. A test walks every world position in
  a forty-block cube on all six faces and requires a declared cell every time.
- **A column reads its own axis (§12 defect 3).** `geometry::rotating_column` takes the axis as an
  argument, so there is no shape in which an occluding neighbour's appearance could supply one.
- **A column continues by block identity (§12 defect 4).** `geometry::Continuation` is a pair of
  booleans the caller answers with the block test §9.6 already uses, so two states of one column
  block continue into each other. This one is delegated rather than asserted: the crate compares no
  two states for a column, so there is no assertion here that could hold it and no shape in which
  the original's reference-identity test could be written. It is fixed by there being nowhere to
  put it.
- **The kind table iterates in registration order (§12 defect 6).** `kinds::Registry::names` hands
  that order out and `kinds::Registry::claim` takes the first kind in it that holds a declaration.
  `kinds::BUILT_IN` is the recorded order. Under one shared reload directory a document names one
  kind, so at most one kind can claim an identifier from path one alone; the order is fixed anyway
  so that the host-blocked second lookup path cannot make the answer unspecified when it arrives.
- **Property values are compared by value, not by reference.** §7 says the original compares the
  game's interned property values by reference identity. `BlockState` compares by value, which the
  specification itself calls the same answer by a safer route.
- **Block and texture identities stay qualified.** `BlockState::new` takes the SDK's validated
  `ContentId`; condition, vocabulary, group and texture keys keep that same type rather than
  dropping it back to text. A neighbourhood returns `Option<BlockState>`, with `None` meaning an
  empty position; neither a magic block name nor an invalid default state represents absence.
  An absent centre cannot occlude or connect; pane context construction returns its empty/default
  context because there is no own appearance. A direction-specific `appearance_from` of `None`
  likewise never connects, even to an occupied neighbour. This deliberately replaces the old
  sentinel's accidental same-block equality between two absent positions.
- **The condition language is bounded.** `conditions::MAX_DEPTH` and `conditions::MAX_ELEMENTS` cap
  nesting and list length, at parse and again at evaluation. A resource pack is untrusted input and
  a condition is the one part of a declaration a pack author can nest without limit. Both are
  numbers to raise, not rules to remove; both refuse rather than truncate, and every refusal is
  counted in `conditions::Report` so a refused pack is diagnosable rather than silently inert.
- **A quad whose geometry is not a number does not exist.** `geometry::Quad::new` refuses a
  non-finite inset or edge. Cullability is "cullable exactly when the inset is zero", and a NaN
  inset compares false against zero, so a quad holding one would answer that question by accident.
- **A texture set, a grid, a kind table and the supply queue all have enforced caps.**
  `slots::MAX_SLOTS`, `slots::MAX_CELLS`, `kinds::MAX_KINDS`, `lookup::MAX_DOCUMENTS` and
  `MAX_PENDING`. Each refuses and counts; a grid past the cap is refused where it is read rather
  than allocated. Nothing else accumulates: the signals `Mod::observe` sees are a count and the last
  one rather than a list, because a client tick arrives every tick.
- **A refused declaration is answered as refused, not as absent.** `Entry::model` answers a
  three-state `Resolved`: nobody shipped a declaration under that identifier, a kind claimed it and
  read it, or a kind claimed it and §10 rejected it. The third carries the claiming kind and the
  `ModelError`, so a pack whose declaration names an unknown layer, an unreadable texture or a slot
  the kind does not have is diagnosable rather than silently untextured.
  Public kind, model and texture-slot errors implement `Display` and `Error`; lifecycle refusals
  retain those readable diagnostics rather than exposing Rust debug representations.
- **The neighbourhood is read through the model that draws from it.** `Model::context` is where §8's
  probe and §9.3's flag rule part. A pane kind reads no condition, so its neighbours can only come
  from `geometry::pane_sample` — same block *and* both perpendicular flags set — and a host is never
  left composing the rule itself. The world position, the axis property and the column continuation
  stay the caller's, because they are not facts about a neighbourhood.
- **A packaging fault is raised rather than defaulted.** A refusal from `kinds::Registry::built_in`
  would leave a client with no kind and no lookup that ever answers. `Entry::new` keeps it and
  `Mod::enter` returns it at the first lifecycle step. `BUILT_IN` is a constant, so this cannot fire
  today, which is the argument for raising it rather than silently building an empty table.
- **A reload replaces rather than adds.** `lookup::Catalogue::rebuild` takes the whole set, and
  `Entry::reload` drains the supply queue with it. A pack that stops shipping a document stops
  supplying it, which is what makes disabling a pack take effect, and the queue cannot grow across
  reloads.
- **Grid cell-count refusals distinguish shortage from surplus.** A short cell list names its first
  missing index; a surplus list reports both its declared texture count and addressable area.
  Parsing validates dimensions and count before decoding textures, including invalid out-of-area cells.
  A present but unreadable cell reports `UnreadableTexture` with its cell index, not `MissingCell`.
- **A model layer must be a string.** Non-string layer values report `MalformedLayer` with the
  declared value instead of being misreported as an unknown empty layer name.

## Derived rather than reproduced, and why

§15 puts the exact coordinate tables in the pinned artifact and requires a counterpart to derive
them there. Nothing upstream was opened to write this package, so they are derived from geometry
instead and are **not** a claim of parity with the original's numbers. Each is written out once:

- **The six-by-four in-plane offset table of §8.** `Face::right` and `Face::up` give each face the
  right-handed basis whose cross product is that face's outward normal; the four orthogonals are
  those two vectors and their reverses and the four diagonals are their sums. One consequence
  matches the specification independently: exactly three of the six faces have exactly one basis
  vector along a negative world axis, which is §9.4's "one of them mirrored for three of the six
  faces", and a test pins the count at three.
- **The edge positions, insets and rotation table of §9 and §11.** Every number is a multiple of
  `geometry::PART`. The pane sheet sits at the block centre, the thin sheet of §9.2 sits one
  sixteenth above the block floor, and a column's rotation is a quarter turn exactly when its axis
  projects onto the face's right vector rather than its up vector.
- **Two slot choices §9.2 leaves open.** The specification says the all-eight case draws "from a
  different slot for the top face than for the bottom" and does not say which two. The top takes the
  fully-surrounded slot and the bottom the isolated one. It also does not say which slot the thin
  rim on a horizontal-normal face draws; that takes the isolated slot too.
- **Which of §9.2's two insets is the sixteenth.** §9.2 says the inset "differs between the top face
  and the bottom face, placing the sheet one-sixteenth of a block from the respective surface",
  which fits the top face's fifteen sixteenths or the bottom face's one sixteenth but not both at
  once. The rim settles it: the side quad spans `v` from zero to one sixteenth, so the slab rests on
  the block's floor. The top quad is inset by fifteen sixteenths and the bottom quad by nothing,
  which makes the slab that thickness rather than a plane and leaves the flush bottom quad cullable
  against whatever it rests on, per §9's own default.
- **The slot each pane quad draws.** §9.3 describes the shapes and not their slots. The centre quad
  of a fully connected face takes the fully-surrounded slot, a side quad takes the corresponding
  edge slot, the narrow middle strip takes the isolated slot, and the cap's centre and arms take the
  two pane edge slots §10 adds.

## Declarable only, and why

Each of these needs a seam Ravel does not publish at the reviewed revision. None is stubbed: where a
shape is declarable the package declares it and carries no pretend implementation.

- **The whole of §11.** Ravel publishes no model-baking surface: no element baker, no atlas, no
  chunk mesh, no vertex format. The precomputation, the per-face box transform, the null-face
  pooling of non-cullable quads and the catch-log-rethrow around quad collection are all absent.
  What is declarable is where the drawing would go, and `Entry::stage_order` declares a
  contribution to Ravel's published `after_terrain` stage and resolves it. Nothing draws.
- **The two injected hooks of §4.2 and §4.3.** Borrowing the model bakery's already-parsed documents
  and wrapping its per-model bake call both need the same absent surface. `lookup::Lookup` names the
  point the fall-through to path two would happen, so the short circuit is testable today and the
  second path plugs in at one place.
  `kinds::Registry::catalogue_mut` is the table that path would fill; it is reachable rather than
  sealed for exactly that reason. The short circuit itself is answered by
  `kinds::Registry::falls_through` rather than by any one kind's table, because §4.1 short-circuits
  on the *directory*: a document the directory held ends the lookup even where its kind key names
  nothing registered, and no per-kind table holds such a document to say so.
  `lookup::Lookup::falls_through` is the same question asked of one kind's table, which is the
  narrower answer.
- **The live appearance query and the occlusion test of §8.** Both need block state access from the
  render thread. `neighbourhood::Neighbourhood` is the trait a host implements; the probe, the
  emptiness guard and the occlusion rule are all pure functions of what it answers, and the tests
  answer it from a written-out neighbourhood.
- **The walk over loaded resource packs.** Ravel's `GameEvent::ResourcesReloaded` says a reload
  happened and carries nothing from it. `Entry::supply` is where a host hands the collected
  documents over and the reload drains them, so the table and the short circuit are exercised
  without the walk.
- **The block and group registries the condition language resolves names against.**
  `conditions::Vocabulary` and `conditions::Groups` are values handed in. Parsing and evaluation are
  pure functions of whatever they answer, and `ClientState::vocabulary_mut` is where a host fills them.
- **Pack compatibility with the original.** §14 lists five spellings a pack author writes by hand
  and the rights record leaves the names surface unresolved, so none is transcribed. Every name here
  is coined: the reload directory (`lookup::DIRECTORY`), the kind key (`lookup::KIND_KEY`), the
  eight kind identifiers (`kinds::BUILT_IN`), the condition discriminator with its eight form words
  and two key names (the parser and key constants in `conditions`), and the texture slot
  names with the four render-layer names (`quadrant::Quadrant::declared_name`,
  `slots::ColumnSlot::declared_name`, `slots::PaneEdge::declared_name`, `kinds::Layer`). **A pack
  written for the original does not load against this package**, which is the direct consequence of
  the unresolved names surface and not an oversight.

## Two smaller shapes worth stating

- **This package declares no content and no configuration.** §2 says the entry registers nothing, so
  `Mod::content_declaration`, `Mod::configuration_declaration` and `Mod::saved_data_declaration` all
  answer none, and a test holds them to it.
- **Defect 7 is not inherited.** The original's nullable-keyed face map has key-set and entry-set
  views that index off the ends of their arrays. Nothing here ports that container: quads are
  returned per face as a list and the pooling of non-cullable quads under a null key belongs to §11,
  which is host-blocked.

## Evidence

One hundred and three inline tests. Every rule with a number, an ordering, a refusal or a bound is
held to an assertion: the eight rows of the quadrant table written out by hand rather than
generated, every degenerate row of §7's table including the non-object element that turns a
two-element list into a single-element one, the six-face basis proved by cross product, the three
mirrored faces counted,
every world position in a forty-block cube addressing a declared cell, the four column slots, the
pane's four flag branches and its sixteenth-of-a-block inner-edge shift, the three connected-block
declaration shapes, the sequential per-face indices with sprite deduplication, the shared object
whose one bad entry the second parse attempt cannot see, §9.3's rule refusing a same-block
neighbour that runs into the face on all four horizontal faces, the carpet drawing a face the
connected block kind drops, the directory ending the lookup for a document naming no kind of ours,
the deterministic claim order, the refused second registration, the inventory variant, and every
cap. `checks/verify-counterpart-build.sh` runs them.

Ten were proved by mutation rather than by reading. Each mutation was applied, the suite run, and
the file restored:

| Mutation | What went red |
| --- | --- |
| Row multiplied by the grid's height (defect 1 restored) | `slots::tests::a_grid_multiplies_the_row_by_the_width_rather_than_the_height` |
| The condition parser's depth check removed | `conditions::tests::a_condition_nested_past_the_depth_cap_is_refused_and_the_parse_returns_at_once` |
| The reload cloning the queue instead of draining it | two tests in `src/lib.rs` |
| Path one answering absent instead of refused | two tests in `src/lookup.rs` |
| A non-finite inset no longer refused | `geometry::tests::a_quad_whose_geometry_is_not_a_number_is_refused_rather_than_carried` |
| The duplicate-registration check removed | `kinds::tests::registering_an_identifier_twice_is_refused_rather_than_overwriting` |
| §9.3's perpendicular-flag test dropped from the pane sample | `geometry::tests::a_pane_neighbour_that_runs_into_the_face_does_not_connect` |
| A pane model's neighbours taken from §7's condition instead | `kinds::tests::a_pane_model_reads_its_neighbourhood_by_the_flag_rule_and_not_the_condition` |
| A refused declaration answered as unclaimed | `tests::a_refused_declaration_is_answered_as_refused_rather_than_as_absent` |
| The directory holding only documents routed to a kind | `kinds::tests::the_directory_ends_the_lookup_even_where_the_document_names_no_kind_of_ours` |

Three of those rules are also written out as documents under `generated/`, rendered by
`src/dumps.rs` from the same functions the library runs and from nothing else: `quadrant.txt` is
`quadrant::select` answering all eight input combinations with the slot name and number it selects,
`neighbourhood.json` is §8's per-face basis with the eight offsets each face places, and
`slots.json` is §10's slot vocabulary as each of §5's eight kinds admits it, in registration order.
Nothing there is transcribed: `dumps::tests::the_generated_documents_regenerate_byte_for_byte`
rebuilds every document and compares it against the bytes on disk, so a changed rule that leaves the
tree alone is a red test. It was proved by mutation like the ten above: renaming one slot in
`generated/quadrant.txt` turned it red, and restoring the file turned it green.

What no test here can reach is anything about a running game: no quad this package produces has ever
been baked, drawn or seen, because §11 is host-blocked and the surface it needs does not exist.
