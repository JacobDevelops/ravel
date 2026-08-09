# M4-W8 Independent SDK Review

Result: PASS

**Round 1 ran and returned FAIL**, on three blocking findings and eight
non-blocking ones. **Round 2 read the tree Round 1's repairs produced and
returned PASS**, on six non-blocking findings and four mutations that did not
fail. **Round 3 read the tree Round 2's repairs produced and returned PASS**, on
one new non-blocking finding and with an explicit statement that M4 is closeable
on the tree it read. Every verdict is reproduced below and the line above is a
reviewer's conclusion rather than a placeholder standing in for one. It is also
the closure state `evidence/m4/manifest.json` records under `review.result`, and
`schemas/evidence.schema.json` admits exactly three values for it — `pass`, `fail`
and `not-required`. `verify_recorded_verdict` in `tools/ravel-xtask/src/lib.rs`
requires this document's first `Result:` line to agree with that field, so the two
move together.

A pass certifies the tree its reviewer read and nothing landed on top of it,
which is why this line stayed `FAIL` after Round 2: the repairs answering that
round's six findings were changes it never saw. Round 3 read those repairs and
returned a pass, and its one new finding is recorded as a deviation in
`evidence/m4/manifest.json` rather than repaired — deliberately, because
repairing it would have been a change no round had read and would have owed a
further round. So the line is `PASS` over the tree Round 3 read, and it moved in
the same change as `review.result`, as the rebound `review.record` digest, and as
the verdict Round 3 wrote, below.

## What this file is

The record the independent M4-W8 review's rounds are written into. It is one file
rather than one file per round because `review.record` in
`schemas/evidence.schema.json` binds one hashed artifact per milestone, and the
rounds only mean anything read in order — each verifies the repairs answering the
one before it. `evidence/m3/review.md` is the same shape and is worth reading
first for how a round is reproduced here.

Its digest is recorded in `evidence/m4/manifest.json` under `review.record`, and
`verify_recorded_review` recomputes it on every `ravel:verify`. So writing a round
into this file means rebinding that digest in the same change.

## The convention this file is held to

A round's verdict is written into this file **before, or in the same change as,
the repairs that answer it**. M3 learned this the hard way: two rounds had their
repairs landed with their verdicts left out, a later round blocked the milestone
over exactly that, and both texts were recovered from outside the repository only
because they still happened to exist. That was luck rather than a control, and
this file inherits the convention rather than the luck.

Each verdict is reproduced as written, without softening and with nothing added.
`file:line` anchors are as of the tree each reviewer read; later repairs move
some of them.

This file's completeness can only ever be as of its last write. The repository
cannot know a round has run until someone writes it down, so between a reviewer
finishing and its verdict landing here the file is missing a round and nothing in
the gate can detect it. The convention above is the only bound on that window and
it is a convention on the writer, not a check on the tree.

This file is the one place that says how many rounds have run. No other document
may restate that count — `verify_review_round_counts` in
`tools/ravel-xtask/src/lib.rs` enforces that for M2 and M3, and the same reason
applies here: a count restated somewhere else is a count that goes stale
somewhere else.

## What a reviewer is asked to read

- `evidence/m4/manifest.json` — the inventory, the recorded gate run, and every
  deviation and risk M4 records, including the ones that name what M4 does not do.
- `evidence/m4/linux-process-matrix.json` — every gate process, its exit status,
  and what each one does not prove. `gameLaunch.occurred` is `false` and that is
  the record's central claim about its own scope.
- `evidence/m4/sdk-surface.json` — what each work package shipped, what proves it,
  and what it leaves unproven.
- `evidence/m4/README.md` — how to recompute the above.
- `docs/guide/` — the author-facing surface M4-W7 created, `limits.md` in
  particular.
- `ROADMAP.md`'s M4 table.

## Round 1 — VERDICT: FAIL

I read `CLAUDE.md`, `SPEC.md`, `ROADMAP.md`'s M4 table and its ADG obligation
rows, `docs/decisions/`, all five files under `evidence/m4/`,
`evidence/m3/review.md` for the shape a round takes here, all eight documents
under `docs/guide/`, `nix/common.nix`, the M4 checks in
`tools/ravel-xtask/src/lib.rs`, `jvm/bootstrap/dev/ravel/jvm/Persistence.java`,
and the M4 conformance suites named in `sdk-surface.json`. I wrote none of M4.
The tree is change `mtvwzxwk`, parent `tmzvkynp` (`69ef93a8`), working copy
clean before and after every mutation below. I reproduced the baseline:
`ravel:verify` exit 0 with 1267/1267 tests, `verify-milestone:m0..m3` exit 0,
`verify-milestone:m4` exit 1 naming "milestone review must be independent and
pass". The disclosure quality in this evidence set is unusually high — I checked
deviations 1, 2, 3 and 5 against the code and all four are accurate, and every
anchor deviation 1 cites says what it is said to say. The milestone fails on
three findings, one of which is a check that does not reach the document it
exists for.

### Mutations that did NOT fail

**Mutation A — an invented gate process survives every check.** I appended to
`evidence/m4/linux-process-matrix.json` a process row `{"surface":"invented
process","command":"devenv tasks run
ravel:this-task-does-not-exist","exitStatus":0,...}` and rebound the record's
`sha256` in `manifest.json` so only a list check could object. `devenv tasks run
ravel:verify-scaffold` → **exit 0**. `devenv tasks run
ravel:verify-milestone:m4` → exit 1 naming only `- milestone review must be
independent and pass`, i.e. the pre-existing open-review reason and nothing
about the fabricated row.

**Mutation B — a recorded gate failure survives.** I set the `dependency audit`
row's `exitStatus` from 0 to 1 while the record's top-level `result` stayed
`"pass"`, and rebound the digest. `ravel:verify-scaffold` → **exit 0**.

**Mutation K — a review-round count in `ROADMAP.md` survives.** I changed
`ROADMAP.md:965` from "eleven M3 review rounds" to "forty M3 review rounds".
`ravel:verify-scaffold` → **exit 0**.

### Mutations that failed correctly

| Mutation | Judged by | Exit | Failure text |
| --- | --- | --- | --- |
| C: appended `// leaked: 26.1.2` to `crates/ravel-core/src/lib.rs` | `ravel:verify-scaffold` | 1 | "hardcoded Minecraft version outside version data … `crates/ravel-core/src/lib.rs`" |
| D: added `net.minecraft.world.entity.Entity` to `Persistence.java` | `ravel:verify-scaffold` | 1 | "names the game class net.minecraft.world.entity.Entity, which is neither in the agent jar's declared reflective surface nor a hook target … pins" |
| E: appended an uncompiled ` ```rust ` block to `docs/guide/limits.md` | `cargo nextest -p ravel-conformance-tests --test guide_snippets` | 100 | "`docs/guide/limits.md` line 81 shows Rust that `examples/guide-snippets/src` does not contain, so nothing compiles it" |
| G: appended a ` ```rust,ignore ` block (fence-attribute escape) | same | 100 | `every_fence_in_the_guide_names_a_language_this_file_knows` FAILED — the escape is closed |
| F: appended a line to `examples/minimal-mod/src/lib.rs` | `ravel:verify-example-package` | 1 | "`src/lib.rs` hashes 495a0e02… but the manifest records ce80c4d0…" |
| H: set `tests.lineCoveragePercent` and `gate.coverage.linesPercent` **both** to 45.0, digest rebound | `ravel:verify-scaffold` | 1 | "records line coverage below the floor the gate enforces"; "must be at least 80%" |
| I: added the fixture path (containing the placeholder word) to a manifest deviation | `ravel:verify-milestone:m4` | 1 | "native milestone evidence must not contain synthetic placeholders" |
| J (control for A/B): removed `limits.md` from `sdk-surface.json`'s `inventory.guideDocuments`, digest rebound | `ravel:verify-scaffold` | 1 | "evidence/m4/sdk-surface.json omits limits.md from inventory.guideDocuments, which the repository holds" |

Mutation J is the control that makes A and B findings rather than observations:
the inventory lists in one M4 record are reconciled against the tree in both
directions, and the process list in the record beside it is reconciled by
nothing.

---

### Findings

**BLOCKING — B1. The process matrix lists a process the global Linux gate does
not run, and nothing reconciles the list.**
`evidence/m4/linux-process-matrix.json:7` states the record holds "Every process
the global Linux gate runs, with the exit status it returned", `:21-22` sets
`gate.command` to `devenv tasks run ravel:verify` with `exitStatus: 0`, and
`evidence/m4/README.md:41` says "The individual rows of the matrix are the
sub-tasks it runs". `evidence/m4/linux-process-matrix.json:125-130` is a row for
`devenv tasks run ravel:verify-environment`. `nix/common.nix:588-603` is
`ravel:verify`'s complete `after` list and it has thirteen entries; the matrix
has fourteen rows; the set difference is exactly `ravel:verify-environment`. I
confirmed it empirically: the recorded gate run I reproduced contains zero `nix
flake check` evaluation. `ROADMAP.md:114-116` is correct and says so plainly —
"`ravel:verify` is the fast code-policy graph. `ravel:verify-environment` runs
`devenv test` and `nix flake check`" — so the repository's own plan
distinguishes the two gates and this record merges them.

Three consequences, in increasing severity.

The record's central claim about its own scope is false: one row of the fourteen
was never run by the command whose exit status heads the table.

Deviation 10 (`manifest.json:158`) rests on it. It reports the `flake.nix`
`check-jsonschema` defect and argues "It failed closed rather than falsely green
— `devenv tasks run ravel:verify-environment` returned a nonzero status and
named the reason — so what was lost was the check's coverage rather than its
honesty." That reading requires the failing task to be part of the gate. It is
not. Throughout the period when the flake's scaffold check could not validate
any evidence schema, `devenv tasks run ravel:verify` returned 0 and could not
have returned anything else. The defect was found by a human running a task by
hand, and the deviation presents it as the gate having held.

Nothing recomputes the list, so it is free to drift in either direction —
mutation A added a process that does not exist and mutation B recorded a failing
process under a `"result": "pass"` record, and `ravel:verify-scaffold` returned
0 for both. Deviation 11 (`manifest.json:162`) is the deviation written
specifically about this hazard: it names `verify_m4_gate_figures` for the
`tests` block and `verify_m4_surface` for `sdk-surface.json`'s lists, and it
does not name the matrix's own `processes` array, which is the one list in M4's
evidence set that no check reconciles. As a supporting detail: every command in
this session printed "devenv 2.1.2 is newer than devenv input (2.1.0) in
devenv.lock", and `verify-environment` is precisely the task that would have
judged that against the pinned flake — so the one row claiming environment
reproducibility is both outside the gate and currently reporting on an
environment whose binary and pinned input disagree.

**BLOCKING — B2. `docs/guide/running.md:113-118` makes a false exhaustive claim
about a public enum.** The passage lists five ways a payload is refused —
truncated inside a field, trailing bytes, another contract version, naming one
contract and carrying another, arriving on a side the direction does not permit
— and then asserts "Those six refusals are the whole of `WireError`". Three
things are wrong. The count does not match the list (five items, "six"). The
fifth item is not a `WireError` at all: a side refusal is
`DeliveryRefused::WrongSide` (`crates/ravel-network/src/lib.rs:475-481`), a
different enum. And the real six variants are `Truncated`, `TrailingBytes`,
`NotUtf8`, `NotABool`, `Identity`, `Version`
(`crates/ravel-network/src/lib.rs:393-435`) — so the guide omits `NotUtf8` and
`NotABool`, which are exactly the two an author writing an exhaustive `match`
will be surprised by, while telling them the set is complete. The same sentence
points at the `network` module's rustdoc, which gets it right.

This is M4-W7's central deliverable stating a false absolute count about a
public API surface, in the one document class the milestone exists to produce.
`tests/conformance/tests/guide_snippets.rs` cannot reach it — it compiles fenced
Rust and this is prose, which is the boundary the suite's own module doc states.
Mutations E and G show that suite is sound; this finding is about what it
deliberately does not cover, and nothing else covers it either.

**BLOCKING — B3. The check that exists for restated review-round counts does not
reach `ROADMAP.md`, and there is a wrong count there today.** `ROADMAP.md:965`
reads "the same construction that hid three blocking defects across eleven M3
review rounds". Twelve rounds ran. `evidence/m3/review.md:24-28` says so itself:
"One round is not here. The earliest round of all reviewed the prototype rather
than [the milestone] … **The numbering below counts only the rounds this file
holds, so 'Round 1' is the second round that ran.**" The file holds eleven
numbered rounds; the ROADMAP counts rounds, not headings.

Separately from being off by one, the sentence restates a count that the
repository has ruled may be stated in exactly one place.
`evidence/m3/README.md:61` — "`review.md` is the one place that says how many
rounds there have been: neither this document nor `manifest.json` restates a
count, because a count written outside the record is falsified by the next
round" — and `evidence/m4/review.md:52-56` repeats the rule and names
`verify_review_round_counts` as its enforcement. Mutation K shows the
enforcement does not sweep `ROADMAP.md`: "forty M3 review rounds" passes
`ravel:verify-scaffold` with exit 0. M3's own Round M-1 blocked over five false
round counts; the class was gated for the documents someone thought of, and the
document M4-W8 itself edited was not one of them. Either the count comes out of
`ROADMAP.md` or `verify_review_round_counts` gains that file — the second is the
repair that also catches the next one.

---

**non-blocking — N1. Deviation 6 and `ROADMAP.md:960` overstate the agent jar's
drift gap.** `manifest.json:142` says the adapter "resolves game classes and
members by name … and no member of that set appears in
`minecraft/versions/26.1.2/hooks.toml`, `stubs.toml`, `stubs.lock` or
`mappings.lock`", and `ROADMAP.md:960` says "none of them is in `stubs.toml` or
`mappings.lock`". Both are false as written:
`net.minecraft.server.MinecraftServer` and
`net.minecraft.world.level.storage.SavedDataStorage` are both in `hooks.toml`
and `mappings.lock` (`hooks.toml:280-302`, `mappings.lock:194-206`), so a rename
of either class does land in a diff. What is genuinely unpinned is
`LevelResource`, `DataFixers`, every member name and arity, the two
`LevelResource` field constants and the three-argument `SavedDataStorage`
constructor. The error is conservative — it understates coverage — and
`Persistence.java`'s own comment at the lookup states the gap precisely and
correctly. Two evidence documents state it imprecisely; make them say what the
code comment says.

**non-blocking — N2. A matrix row title says the opposite of the record's own
deviation.** `evidence/m4/linux-process-matrix.json:83` titles the surface "pack
example, written and **read back off disk**". Its own `doesNotProve` two lines
later says "That Ravel reads a written pack", and deviation 3 spends a paragraph
on the round trip being unprovable by construction. A reader scanning surface
titles gets the one impression the rest of the record works hardest to prevent.
Rename it.

**non-blocking — N3. The placeholder rule is defeated by relocation, and M4
demonstrated it.** Mutation I confirms `verify_admitted_manifest`
(`tools/ravel-xtask/src/lib.rs:3158-3163`) is live and would have refused this
manifest. Deviation 9 (`manifest.json:154`) discloses the workaround honestly.
But the rule's stated purpose is that "a native milestone must not close on
placeholder evidence", and it is scoped to the manifest string set only, so
`sdk-surface.json:136` carries the word freely while being digest-bound into the
same evidence set. The rule as written checks a document, not the evidence. I
agree with M4's refusal to narrow it into a judgement call; the honest options
are to widen it to the whole record set and accept M4's disclosed exception
explicitly, or to retire it.

**non-blocking — N4. M4's Verification cells were rewritten by M4's own stack,
with no amendment note.** Change `kvnynxqkqwkx` ("docs: state in the roadmap
what M4 shipped and what it does not") replaced all eight M4 rows. W6's original
cell read "shared adapter and general crates compile/test without
26.1.2-specific edits or a new crate"; it now reads as the two narrower
properties plus "Not proved, and not claimed". I checked the practice and it is
the repository's norm — `ROADMAP.md` has been amended in twenty changes and W5's
row was narrowed inside M4-W5 itself — so this is not novel and I do not block
on it. On the substance I find the narrowing honest: the amended row's literal
reading is met, the omission is stated in three places, and the original cell's
literal wording is arguably met too, since the general crates do compile and
test and contain no version-specific edits. What is missing is the amendment
being visible. `ROADMAP.md:960` shows the repository knows how to do this — it
records its own retraction in the row it retracts. The M4 section should carry
one sentence saying its Verification column was restated in M4-W8 and why;
without it, "does this row meet its gate" is a question a reviewer cannot answer
from the tree.

**non-blocking — N5. `docs/guide/package-manifest.md:154-157` shows a refusal
message the loader does not emit.** The passage is about the loader's validation
("Before a package loads, every declared source file is hashed…", `:130`) and
quotes the message as `` `src/lib.rs` hashes <actual> but the manifest records
<declared> ``. `crates/ravel-package/src/integrity.rs:210` formats `{file}`, and
`file` comes from `RelativePath::file_name()` at `:88`, which returns the last
non-empty segment — `lib.rs`. `crates/ravel-core/src/paths.rs:70-71` states the
intent outright: "Every integrity failure therefore names a file, never a path."
So the guide contradicts `docs/guide/conforming.md:46-47` ("no published message
names a filesystem path"), which is the reason the truncation exists. The
confusion is understandable — `tools/ravel-xtask/src/package_digests.rs:76`
emits the same sentence with the full relative path, which is what mutation F
printed — but the paragraph the quote sits in is about the loader, not the
subcommand.

**non-blocking — N6. `docs/guide/running.md:24` understates the event audience
filter.** "The loader broadcasts every event to every running mod in resolved
load order." `LifecycleMachine::audience`
(`crates/ravel-loader/src/events.rs:218-229`) filters twice — by whether the mod
loads on the running side, and by whether the event happens on that side at all
— and returns empty for the second case. The second filter is nowhere in the
guide, and it is the one that produces a surprising empty audience.

**non-blocking — N7. `docs/decisions/README.md:23` "Each gate has one task" does
not hold.** Seven ADRs, five `ravel:spike-adg-*` tasks
(`nix/common.nix:572-576`), plus one script. The prose at `:38-40` excepts ADG-7
and explains why. ADG-6 has neither a task nor a stated exception. Pre-existing
rather than M4's, but it is in the ADR set I was asked to read and it is the
same ungated-figure class.

**non-blocking — N8. A further inventory of roughly fifteen ungated absolute
counts exists across `ROADMAP.md`, `SPEC.md`, the ADRs and `docs/guide/`.** I
confirmed the two wrong ones personally (B3 and N7) and did not verify the
remainder one by one, so I report their existence rather than their contents.
The highest-value single addition I did confirm is worth naming:
`docs/decisions/adg-1-jvm-entry.md:231` states the adapter "resolves five
classes by name" against `PERSISTENCE_REFLECTED_CLASSES`
(`tools/ravel-xtask/src/lib.rs:1262`), a list `verify_persistence_reflection`
already holds in both directions and which is designed to grow — so the prose
anchor is cheap and the check is already there.

---

### On the eleven deviations' classification

I pressed each of the six the review brief singled out and I do not think any of
them is a blocking gap carried as a disclosure. Deviation 1 (no packet send) is
accurate at all three anchors it cites and `ROADMAP.md` places wire adapters
after the abstractions, so W3's row closes on what its Output column asks for.
Deviation 2 is accurate: I confirmed `ravel_loader::admitted` has no caller
outside `crates/ravel-testkit/src/pipeline.rs`, so no launch path reaches it.
Deviation 3 is owned by `ROADMAP.md:965`'s obligation row and M8. Deviation 4's
narrowing is honest on the merits (see N4). Deviation 5 is accurate and is
stated to authors in `limits.md:35-43` and in the matrix's own `doesNotProve`.
Deviation 6 claims *no* drift coverage where a little exists (N1) —
conservative, not overclaimed. The risk entry at `manifest.json:172` — an author
can build a conforming mod and cannot run it — states the composite plainly and
is the right place for it. The three findings above are the reasons for the
FAIL; the deviation set is not.

### Hard rules

Clean. Every `crates/*/src/lib.rs` carries `#![forbid(unsafe_code)]`. There is
no `#[allow(…)]` and no `#[expect(…)]` anywhere in `crates/` or `tools/` except
as string literals inside the scaffold check that detects them
(`tools/ravel-xtask/src/lib.rs:2921,2923`), and no `#[ignore]`. `ravel-core`
depends on no Ravel crate; `ravel-testkit` appears in no production crate's
`Cargo.toml`; `ravel-sdk`'s dependency set contains no bootstrap, JVM or version
crate. The pinned artifacts under `minecraft/versions/26.1.2/` are untouched by
M4's change stack. A redaction sweep over the tracked file set found no
credential, account, world name, launcher path, instance path, username or
absolute user path; the `/home/player/...` strings in
`crates/ravel-bootstrap/src/mods.rs:417` and
`crates/ravel-core/src/diagnostics.rs:158` are the redactor's own test fixtures,
and the Prism strings under `.agents/skills/` are capture tooling, not captured
material.

### What I did not examine

I did not run `ravel:verify-environment`, so I did not verify the `flake.nix`
repair itself — only that the task it lives in is outside the gate. I did not
read `tests/conformance/tests/` beyond `guide_snippets.rs`, `sdk_boundary.rs`'s
premise and the suites' names, so I did not independently assess whether
`sides_and_packets.rs`, `content_declarations.rs`,
`config_saves_and_services.rs` or `pack_closure.rs` prove the properties their
rows claim — I took the gate's green as evidence they pass, not as evidence they
are the right tests. I did not review `test-mods/waystone-ledger` as a mod, only
that it exists and is inventoried. I did not audit
`tools/ravel-xtask/src/lib.rs` as a whole; I read the M4 checks,
`verify_admitted_manifest`, `verify_m4_surface` and `verify_m4_gate_figures`,
and I located `verify_review_round_counts` only by its effect. I did not verify
the roughly fifteen ungated figures in N8 individually. I did not attempt a
mutation against `verify_recorded_run`'s provenance or `revision.changeId`
binding, which the coordinating reviewer had already covered. And I read
`evidence/m3/review.md` only for shape and for the round-count question, not as
a check on M3.

## Round 2 — VERDICT: PASS

I read `CLAUDE.md`, Round 1's verdict in `evidence/m4/review.md` in place,
`SPEC.md`, `ROADMAP.md`'s M4 table and its ADG obligation rows,
`docs/decisions/` including `README.md` and ADG-1 and ADG-6, all five files
under `evidence/m4/`, all eight documents under `docs/guide/`,
`nix/common.nix`'s task graph, and in `tools/ravel-xtask/src/lib.rs` the
functions `verify_m4_process_matrix`, `verify_guide_enumerations`,
`verify_review_round_counts`, `verify_resolved_class_figure`,
`verify_m4_gate_figures`, `numeral`, `numbers_before`, `numbers_after` and
their tests. I read each of the six changes in the repair stack with
`jj show`. I wrote none of M4 and none of the repairs.

Tree: working copy `rylkmosq` (empty, no description), parent `wmpnxukq`
`aab2e4c9` "docs: re-record M4's gate run after the round-1 repairs changed
it". The working copy was clean before every mutation below and is clean
after all of them.

Baseline I reproduced myself: `devenv tasks run ravel:verify` → **exit 0**,
1275 tests run, 1275 passed, 0 skipped. `ravel:verify-milestone:m0..m3` →
exit 0. `ravel:verify-milestone:m4` → exit 1 naming only `- milestone review
must be independent and pass`.

### Round 1's eleven findings, disposed

**B1 — repaired.** `verify_m4_process_matrix`
(`tools/ravel-xtask/src/lib.rs:3912`) reconciles the record's `processes`
against `ravel:verify`'s `after` list in `nix/common.nix:589-602` in both
directions and refuses a nonzero `exitStatus` under a record whose own
`result` is `pass`. I confirmed the two lists agree today: thirteen `after`
entries, thirteen `processes` rows. Round 1's mutations A and B now fail by
name (below), and so does the relocation escape (mutation L). The rewritten
deviation 10 (`evidence/m4/manifest.json:158`) now says the opposite of what
Round 1 refuted and says it correctly — it states that `ravel:verify` does
not run `ravel:verify-environment`, that the gate returned 0 and could not
have returned otherwise throughout, and that a person found the defect by
hand. I verified the underlying fact independently: `ravel:verify-environment`
is absent from the `after` list.

**B2 — repaired.** `docs/guide/running.md:120-135` now names all six
variants and no others, and separates `DeliveryRefused::WrongSide` into its
own paragraph. I checked the list against
`crates/ravel-network/src/lib.rs`: `WireError` declares exactly `Truncated`,
`TrailingBytes`, `NotUtf8`, `NotABool`, `Identity`, `Version`. The false
absolute count is gone — the passage no longer states a number.
`Delivery::decode` does return `WrongSide` before decoding
(`crates/ravel-network/src/lib.rs:596` versus the `decode` call at `:615`),
so the new sentence is true. `verify_guide_enumerations` holds the list in
both directions; mutations N and O prove it.

**B3 — repaired.** `verify_review_round_counts` now sweeps `ROADMAP.md`
(`tools/ravel-xtask/src/lib.rs:1061-1072`) and the wrong count is out of
`ROADMAP.md:976`. Mutation K1 confirms the sweep is live and reports the
right number. The limit is real and is measured under "the CARDINALS hole"
below.

**N1 — repaired, and more accurately than Round 1 stated it.** I checked all
five reflected class names against the pinned files. Three are present in
both `hooks.toml` and `mappings.lock` — `net.minecraft.client.Minecraft`
(5 hits each), `net.minecraft.server.MinecraftServer` (6 each) and
`net.minecraft.world.level.storage.SavedDataStorage` (3 each). Round 1 named
only the latter two. `LevelResource` and `DataFixers` appear in none of
`hooks.toml`, `stubs.toml` or `mappings.lock`. `manifest.json:142` and
`ROADMAP.md:971` now say exactly this, and `ROADMAP.md:971` marks its own
retraction in place.

**N2 — repaired.** `evidence/m4/linux-process-matrix.json:83` now reads
"pack example, written to disk and compared against two independent lists".

**N3 — deliberately left alone by decision. Not a defect**, and I did not
re-open it.

**N4 — repaired.** `ROADMAP.md:786-792` carries the amendment note. I
spot-checked all eight M4 rows: each is narrowed and each names what it
excludes.

**N5 — repaired.** `docs/guide/package-manifest.md:154-162` now quotes
`` `lib.rs` `` and explains the file-not-path rule and the two-files-same-
basename consequence. I confirmed the code: `IntegrityError::Mismatch`
formats `{file}` (`crates/ravel-package/src/integrity.rs:204-211`) and
`file` comes from `path.file_name()` at `:88`.

**N6 — repaired.** `docs/guide/running.md:29-33` states the second filter and
gives the `ClientTick`-on-a-dedicated-server case. It matches
`LifecycleMachine::audience` (`crates/ravel-loader/src/events.rs:229-236`),
which returns empty when `!event.happens_on(self.side())`.

**N7 — repaired but incompletely, and the replacement is itself false.** See
new finding R3.

**N8's named item — repaired and proven.** `verify_resolved_class_figure`
(`tools/ravel-xtask/src/lib.rs:1428`) anchors ADG-1's prose figure to
`PERSISTENCE_REFLECTED_CLASSES.len()`. Mutation Q proves it. The remaining
roughly fifteen ungated figures are untouched, which is what N8 asked for.

### Round 2 — Mutations that did NOT fail

**Mutation K2 — a spelled round count above twenty survives.** I replaced
`ROADMAP.md`'s pack-row clause with "hid three blocking defects across forty
M3 review rounds." `devenv tasks run ravel:verify-scaffold` → **exit 0**.

**Mutation K4 — a hyphenated compound survives.** Same site, "twenty-one M3
review rounds". `ravel:verify-scaffold` → **exit 0**. `numeral`
(`tools/ravel-xtask/src/lib.rs:909`) trims only leading and trailing
non-alphanumerics, so `twenty-one` never matches a `CARDINALS` entry either.

**Mutation M — a false review-round count in M4's own evidence survives.** I
appended "Four independent review rounds have run against M4." to
`evidence/m4/README.md`. `ravel:verify-scaffold` → **exit 0**.

**Mutation P — the guide's completeness claim can be satisfied from another
page.** I deleted `` `WireError::NotUtf8` `` from the enumeration in
`docs/guide/running.md:124` and added the string to the end of
`docs/guide/limits.md`. `ravel:verify-scaffold` → **exit 0**, because
`verify_guide_enumerations` concatenates every file under `docs/guide` before
comparing.

All four are new findings R4, R5 and R6 below. None of them is a defect Round
1 blocked on.

### Round 2 — Mutations that failed correctly

| Mutation | Judged by | Exit | Failure text |
| --- | --- | --- | --- |
| A (re-run): appended a `processes` row `devenv tasks run ravel:this-task-does-not-exist`, digest rebound | `ravel:verify-scaffold` | 1 | "records ravel:this-task-does-not-exist under processes, which nix/common.nix does not list among the sub-tasks ravel:verify runs; a row the gate never ran took its exit status from somewhere the record does not name" |
| B (re-run): set the `dependency audit` row's `exitStatus` to 1 under `"result": "pass"`, digest rebound | `ravel:verify-scaffold` | 1 | "records a nonzero exit status for `devenv tasks run ravel:audit` under .processes while its own result is pass; a record that reports a failed process and calls itself green states two things that cannot both be true" |
| K1 (re-run): "hid three blocking defects across eleven M3 review rounds." into `ROADMAP.md` | `ravel:verify-scaffold` | 1 | "ROADMAP.md restates that 11 review rounds exist; each milestone's review.md is the one record that counts its own rounds, and a count written in the plan is falsified by the next round" |
| K3: same site, "40 M3 review rounds" in digits | `ravel:verify-scaffold` | 1 | same shape, naming 40 |
| L: moved the whole `ravel:audit` row from `processes` into `checksOutsideTheGate`, digest rebound | `ravel:verify-scaffold` | 1 | two violations — "omits ravel:audit from processes …" and "records ravel:audit under checksOutsideTheGate while nix/common.nix lists it among the sub-tasks ravel:verify runs; that section is for checks the gate does not run and is not a way out of the reconciliation" |
| N: removed `` `WireError::NotUtf8` `` from `docs/guide/running.md` | `ravel:verify-scaffold` | 1 | "docs/guide enumerates WireError and omits WireError::NotUtf8, which crates/ravel-network/src/lib.rs declares; a variant left out of a list an author reads as complete is the one their match arm will not have" |
| O: renamed `WireError::NotABool` to `WireError::NotAByte` in the guide | `ravel:verify-scaffold` | 1 | "docs/guide names WireError::NotAByte and crates/ravel-network/src/lib.rs declares no such variant …" plus the omission of `NotABool` |
| Q: changed ADG-1's "resolves five classes by name" to "six" | `ravel:verify-scaffold` | 1 | "docs/decisions/adg-1-jvm-entry.md says the persistence adapter resolves 6 classes by name while the declared reflective surface holds 5" |
| R: edited `linux-process-matrix.json` without rebinding its digest | `ravel:verify-scaffold` | 1 | "evidence/m4/manifest.json records linux-process-matrix.json as 0ed2cc2d… but it hashes 18a9b543…" |
| S: set the matrix's `gate.tests.run` to 1274, digest rebound, manifest left at 1275 | `ravel:verify-scaffold` | 1 | "records .tests.total 1275 while evidence/m4/linux-process-matrix.json records .gate.tests.run 1274; they describe one gate run" |

Mutation L is the control that answers the brief's third attack: the
`checksOutsideTheGate` section is not a way out. A gate task moved there
fails twice, once for each direction of the reconciliation, and any third
invented section fails the same way because the `processes` side is checked
for omissions independently.

### Round 2 — The `CARDINALS` hole, sized

The writer's account of Round 1's mutation K is **correct, and I reproduced
it exactly.** Replacing only the phrase — leaving the following clause ",
each one found only after the construction that hid it was described" — and
writing "forty M3 review rounds" gives `ravel:verify-scaffold` exit 1 with a
single violation reading "ROADMAP.md restates that **1** review rounds
exist". The catch comes from `one` in `each one`, four words after `rounds`,
inside `numbers_after`'s window. Nothing read `forty`. Removing that clause
(mutation K2) gives exit 0.

Four checks are built on `CARDINALS` through `numeral`:

- `verify_campaign_inventory` — `numbers_before(document, &["surface",
  "launches"])` at `lib.rs:857`
- `verify_review_round_counts` — `round_count_restatements` at `lib.rs:1082`
- `verify_harness_lifecycles` — `numbers_before(&document, &["world",
  "lifecycles"])` at `lib.rs:1255`
- `verify_resolved_class_figure` — `numbers_before(&record, &["classes",
  "by", "name"])` at `lib.rs:1437`

Three of the four **fail closed** on the hole: each requires a readable count
and pushes a violation when `stated.is_empty()`, so a figure spelled above
twenty makes them fail, not pass. Only `verify_review_round_counts` fails
open, because it is a prohibition rather than a match — an unreadable numeral
produces no violation. So the hole reaches exactly one check in the dangerous
direction, which is narrower than the disclosure implies and worth recording.

Is there a real prose count above twenty escaping today? I swept every `.md`,
`.json`, `.toml` and `.rs` file outside `target/` and `.jj/` for spelled
numerals above twenty. Every hit is either a magnitude rather than a count
("a thousand devirtualized increments", "a few hundred mods", "thirty-thousand-
entry archive"), a description of a past mutation ("forty zeroes", "thirty-two
`z`s"), a line offset (`evidence/m3/review.md:2603` "twenty-four lines above"),
or an experiment's own measured figure (`experiments/m3-pack-compile`
"already forty of them", which is not swept and not a claim about the gate).
**No swept document states a round count above twenty today**, and no
fail-closed check's figure is above twenty either — the largest is
`sdk-surface.json`'s inventory lists, which are compared as sets and not as
spelled counts. The hole is real and currently unexercised.

### Round 2 — New findings

**non-blocking — R1. `evidence/m4/README.md:80` and `:97` carry a count the
repair falsified.** Line 80 reads "Two of its checks exist because a record
of in-gate work is the easiest kind to let drift:" and is followed by three
bullets — `verify_m4_gate_figures`, `verify_m4_surface` and, added by
`tpxmonru`, `verify_m4_process_matrix` at `:91-95`. Line 97 then reads "Both
are proven by mutation rather than by reading". Both sentences were true
before the repair and are false now. This is the ungated-prose-count class
Round 1 catalogued in N8, in the document its own reading list names, in the
change whose purpose was to close a drift hole.

**non-blocking — R2. `evidence/m4/manifest.json:150` still asserts the
sentence B1 refuted.** Deviation 9 reads "Every row in
`evidence/m4/linux-process-matrix.json` is a gate process". The record now
holds a row under `checksOutsideTheGate` that is by construction not a gate
process, and the same change rewrote `resultCeiling`
(`linux-process-matrix.json:18`) to say so — "The one row that is not a gate
process is under `checksOutsideTheGate`" — and rewrote deviation 10 eight
lines below this one. The sweep reached the record and missed the manifest.
It is non-blocking rather than blocking because the deviation's conclusion
survives its false premise: the row it forgets is itself recomputable and
reaches nothing about a running game, so no claim about what M4 proves
changes, and the row a reader would misread carries a field literally named
`whyItIsNotAGateProcess`.

**non-blocking — R3. `docs/decisions/README.md:23` replaced a false sentence
with another one.** It now reads "Each gate that took a measurement has one
task, and two of the gates did not take one." ADG-7 took a measurement and
has no task — the same file says so at `:37-39`, "a script rather than a task
because no task runs it". So the first clause is false. The second clause is
false on its natural reading too, since "one" parallels "a measurement" and
only ADG-6 took none — which `:41-42` states in as many words, "the one gate
here decided without a new measurement". I confirmed the facts:
`nix/common.nix:572-576` declares five `ravel:spike-adg-*` tasks for ADG-1
to ADG-5, ADG-7 has `jvm/harness/run-datafix-byte-preservation.sh`, and
ADG-6's `Status` line records that no new measurement was taken. The
paragraph N7 asked for exists at `:41-46` and is correct; the summary
sentence above it contradicts it.

**non-blocking — R4. `verify_review_round_counts` does not sweep M4's own
evidence set, while `evidence/m4/review.md:50-54` states the rule for M4.**
The swept pairs are M3's record against `evidence/m3/README.md` and
`evidence/m3/manifest.json`, M2's against its two, plus `ROADMAP.md`
(`lib.rs:1030-1050`). Nothing under `evidence/m4/` is read. Mutation M
measures it: a false count in `evidence/m4/README.md` passes with exit 0.
`review.md:52` is honest — it says the check "enforces that for M2 and M3"
— so this is an unclosed rule rather than a false claim, and there is no
wrong M4 count in the tree today. It is the same class as B3 and the repair
that closed B3 for `ROADMAP.md` did not extend the pairs.

**non-blocking — R5. The `CARDINALS` ceiling as measured** — mutations K2 and
K4, sized above. The doc comment at `lib.rs:878-883` discloses it accurately
and argues extending the list is speculation. I record two things it does not
say: the escape is one-directional and reaches only
`verify_review_round_counts`, and it also swallows hyphenated compounds like
`twenty-one`, which "stops at twenty" reads as excluding by one rather than
by the whole tail.

**non-blocking — R6. `verify_guide_enumerations` proves a weaker property
than the passage it exists for.** The check concatenates every file under
`docs/guide` (`lib.rs:1424-1436`) before comparing, so the completeness it
enforces is over the directory and not over the enumeration in
`docs/guide/running.md:120-130`, which is the passage that told an author the
set was whole. Mutation P: with `NotUtf8` deleted from that list and the
string added anywhere in `limits.md`, the check returns exit 0 while the
enumeration an author reads is again incomplete. The function's doc comment
states two other limits explicitly and does not state this one.

### Round 2 — Hard rules

Clean. Every `crates/*/src/lib.rs` carries `#![forbid(unsafe_code)]` — a
sweep for a root missing it returned nothing. No `#[allow(…)]`,
`#[expect(…)]` or `#[ignore]` exists anywhere under `crates/`, `tools/` or
`tests/` except as string literals inside the scaffold check that detects
them. No `unsafe` block exists outside the experiment fence. The pinned
artifacts under `minecraft/versions/26.1.2/` are untouched by the repair
stack; I read them and edited none of them. A redaction sweep over the nine
files the six changes touch found no credential, account, world name,
launcher path, instance path, username or absolute user path — the `Prism`
and `instance` strings in `ROADMAP.md` and `docs/decisions/README.md` are
subject names, not captured material.

### Round 2 — The re-recorded gate run

Confirmed against a run I made myself on this tree. `devenv tasks run
ravel:verify` → exit 0, "1275 tests run: 1275 passed, 0 skipped", which
matches `manifest.json:101-104` (`total` 1275, `passed` 1275, `failed` 0,
`skipped` 0) and `linux-process-matrix.json:24-27`. Coverage recomputed with
the record's own `howToRecompute` command gives `TOTAL … 89.24% … 85.66% …
88.41%`, matching `regionsPercent` 89.24, `functionsPercent` 85.66 and
`linesPercent` 88.41 exactly; 88.41 is above the recorded floor of 80.
`revision.changeId` `poylotnsnumzxnxklmnruyuomnlrnqqw` resolves to commit
`f0519617e778baa0e1c540243f106b597ed6ab8b`, which is `revision.id`, on a real
ancestor change described "docs: record M4's evidence and the limits it ships
with". Both `benchmarks.results` digests recompute — `0ed2cc2d…` for the
matrix, `2b971e2d…` for `sdk-surface.json` — and `review.record.sha256`
`ff68c73d…` matches `evidence/m4/review.md`. Mutation R proves that binding
is live and mutation S proves the two documents' figures are reconciled.
`commands[0].logSha256` and `durationMs` bind a private log and remain
unverifiable, which is the ceiling `verify_recorded_run` already records.

### Round 2 — What I did not examine

I did not run `ravel:verify-environment`, so the `flake.nix` repair itself is
still unverified by me; I verified only that the task is outside the gate. I
did not read `tests/conformance/tests/` beyond confirming
`guide_snippets.rs` still compiles the guide under the baseline run, so I
took the gate's green as evidence the M4 suites pass rather than as evidence
they are the right tests. I did not re-run Round 1's mutations C through J,
which the repairs did not touch. I did not audit
`tools/ravel-xtask/src/lib.rs` as a whole — I read the four `CARDINALS`
consumers, the two new M4 checks, `verify_m4_gate_figures` and their tests.
I did not verify the roughly fifteen ungated figures of N8 individually
beyond the sweep for spelled numerals above twenty described above. I did not
review `test-mods/waystone-ledger` as a mod. I read `evidence/m3/review.md`
only for the round-count question and the twenty-four-lines hit, not as a
check on M3. I did not attempt to defeat `gate_sub_tasks`' textual parse of
`nix/common.nix` by restructuring that file, having judged that it fails
closed — an unparseable or empty `after` list pushes a violation.

## Round 3 — VERDICT: PASS

I read `CLAUDE.md`, Round 1's and Round 2's verdicts in `evidence/m4/review.md`
in place along with the file's own convention sections, `SPEC.md`, `ROADMAP.md`'s
M4 table and its ADG obligation rows, `docs/decisions/README.md` and the ADG-1,
ADG-6 and ADG-7 records it indexes, all five files under `evidence/m4/`, all
eight documents under `docs/guide/`, `nix/common.nix`'s `ravel:verify` `after`
list, and in `tools/ravel-xtask/src/lib.rs` the functions
`verify_guide_enumerations`, `enumerating_passage`, `numeral`, `tens_value`,
`verify_review_round_counts` and their tests. I read each of the five changes in
the round-2 repair stack with `jj show`. I wrote none of M4 and none of the
repairs.

Tree: working copy `zznvyqom` (empty, no description), parent `xksmuvml`
`144be29e` "docs: re-record M4's gate run after the round-2 repairs added
tests". The working copy was clean before every mutation below and is clean
after all of them.

Baseline I reproduced myself: `devenv tasks run ravel:verify` → **exit 0**,
"1278 tests run: 1278 passed, 0 skipped". `ravel:verify-milestone:m0` through
`:m3` → exit 0. `ravel:verify-milestone:m4` → exit 1 naming only
`- milestone review must be independent and pass`.

### Round 3 — Round 2's six findings, disposed

**R1 — repaired.** `evidence/m4/README.md:80` now reads "The checks below exist
because a record of in-gate work is the easiest kind to let drift:" and `:97`
reads "Each is proven by mutation rather than by reading". Neither states a
count, so neither can go stale against the three bullets at `:83-95`. The
ungated-prose-count class that produced it is unchanged and still disclosed.

**R2 — repaired.** `evidence/m4/manifest.json:150` (deviation 8) now reads
"Every row under processes in evidence/m4/linux-process-matrix.json is a gate
process, and the row that is not sits under checksOutsideTheGate carrying a
field named whyItIsNotAGateProcess that says so; verify_m4_process_matrix holds
that separation in both directions." I checked the record: the
`checksOutsideTheGate` row does carry that field, and the deviation now agrees
with `linux-process-matrix.json:18` and with deviation 10 rather than
contradicting them.

**R3 — repaired.** `docs/decisions/README.md:23-25` now reads "Every gate that
took a measurement can be re-run from here; the two paragraphs after the block
say why one of them is a script rather than a task and why one gate has
neither." Both clauses are true on the tree: `nix/common.nix:572-576` declares
the five `ravel:spike-adg-*` tasks, the block's sixth line is ADG-7's
`jvm/harness/run-datafix-byte-preservation.sh`, `:40-42` explains the script,
and `:43-48` explains ADG-6. The false summary sentence is gone and the
paragraph N7 asked for is untouched.

**R4 — repaired.** `verify_review_round_counts` now sweeps a third pair,
`evidence/m4/review.md` against `evidence/m4/README.md` and
`evidence/m4/manifest.json` (`tools/ravel-xtask/src/lib.rs:1084-1087`).
Round 2's mutation M re-run fails by name, and so does the same false count
placed in the manifest instead. The comment at `:1078-1083` records why M4 was
added and M0/M1 were not.

**R5 — repaired to a stated ceiling, and the new ceiling is real.** `CARDINALS`
keeps twenty-one entries, `TENS` is new, and `numeral`
(`tools/ravel-xtask/src/lib.rs:937-959`) reads bare tens and hyphenated
compounds up to ninety-nine, refusing `twenty-eleven` rather than inventing a
figure. `forty` and `twenty-one` now both fail by the right number. The doc
comment at `:922-936` replaces the old argument, states what still escapes, and
states which of the four consumers fails open. I confirmed the fail-closed
direction is unchanged: the other three consumers still require a readable count
and report its absence, so the hole reaches only
`verify_review_round_counts`. I measured the disclosed escapes rather than
taking them: `twenty one` and `one hundred and five` do **not** escape — they
are caught, reporting 1 and 5 respectively, because a prohibition catches on any
readable numeral in the window. What actually escapes is a phrase with no
readable numeral at all, and `a hundred M3 review rounds` is one; it returns
exit 0. That is inside what `:931-933` discloses.

**R6 — repaired but incompletely.** The rewrite is a real narrowing and it holds
against three separate attacks: Round 2's mutation P now fails, splitting the
claim from the list fails, and deleting the claim sentence fails, each by name.
It does not hold against a duplicate claim. See new finding R7.

### Round 3 — Mutations that did NOT fail

**Mutation T1 — a second paragraph carrying the same claim displaces the held
one.** In `docs/guide/running.md` I deleted `` `WireError::NotUtf8` `` from the
enumeration at `:120-130` and inserted, near the top of the same file, a
paragraph reading "Historically, `WireError` is the whole of those refusals:"
followed by all six variant names. `devenv tasks run ravel:verify-scaffold` →
**exit 0**. This is new finding R7.

**Mutation W — a count with no readable numeral still escapes.** I replaced
`ROADMAP.md:976`'s pack-row clause with "hid three blocking defects across a
hundred M3 review rounds." `devenv tasks run ravel:verify-scaffold` → **exit
0**. This is the ceiling `tools/ravel-xtask/src/lib.rs:931-933` states in as
many words, measured rather than assumed. Not a new finding — it is R5's
disclosed residue and I record it so the next round does not have to rediscover
where the line now sits.

### Round 3 — Mutations that failed correctly

| Mutation | Judged by | Exit | Failure text |
| --- | --- | --- | --- |
| P (re-run): deleted `` `WireError::NotUtf8` `` from `running.md`'s enumeration, pasted the string into `limits.md` | `ravel:verify-scaffold` | 1 | "docs/guide/running.md's enumeration of WireError omits WireError::NotUtf8, which crates/ravel-network/src/lib.rs declares; a variant left out of a list an author reads as complete is the one their match arm will not have, and naming it on another page does not complete this one" |
| M (re-run): appended "Four independent review rounds have run against M4." to `evidence/m4/README.md` | `ravel:verify-scaffold` | 1 | "evidence/m4/README.md restates that 4 review rounds exist; evidence/m4/review.md is the one record that counts them" |
| M′: put "Nine independent review rounds have run." into a `evidence/m4/manifest.json` deviation summary | `ravel:verify-scaffold` | 1 | "evidence/m4/manifest.json restates that 9 review rounds exist; evidence/m4/review.md is the one record that counts them" |
| K2 (re-run): "forty M3 review rounds" at `ROADMAP.md:976`, surrounding clause removed | `ravel:verify-scaffold` | 1 | "ROADMAP.md restates that 40 review rounds exist; each milestone's review.md is the one record that counts its own rounds, and a count written in the plan is falsified by the next round" |
| K4 (re-run): "twenty-one M3 review rounds", same site | `ravel:verify-scaffold` | 1 | same shape, naming 21 |
| K5: "thirty-one M3 review rounds", same site | `ravel:verify-scaffold` | 1 | same shape, naming 31 |
| K6: "twenty one M3 review rounds" (unhyphenated), same site | `ravel:verify-scaffold` | 1 | same shape, naming 1 — caught on the wrong figure but caught |
| K7: "one hundred and five M3 review rounds", same site | `ravel:verify-scaffold` | 1 | same shape, naming 5 |
| K8: "a hundred and five M3 review rounds", same site | `ravel:verify-scaffold` | 1 | same shape, naming 5 |
| T2: split the completeness claim into its own paragraph, leaving the list in the next | `ravel:verify-scaffold` | 1 | one violation per declared variant — "docs/guide/running.md's enumeration of WireError omits WireError::Truncated …" and the same for `TrailingBytes`, `NotUtf8`, `NotABool`, `Identity`, `Version` |
| T3: deleted the claim sentence, left the list intact | `ravel:verify-scaffold` | 1 | "docs/guide/running.md no longer holds the passage saying WireError \"is the whole of those refusals\", so no list an author reads as complete is held to crates/ravel-network/src/lib.rs; the claim is what this check exists for and dropping it while keeping the list is not a way out of it" |
| U: edited `linux-process-matrix.json`'s `floorPercent` without rebinding its digest | `ravel:verify-scaffold` | 1 | "evidence/m4/manifest.json records linux-process-matrix.json as 2b1234c7… but it hashes 5bfb948e…" |
| V: set the matrix's `gate.tests.run` to 1277, digest rebound, manifest left at 1278 | `ravel:verify-scaffold` | 1 | "evidence/m4/manifest.json records .tests.total 1278 while evidence/m4/linux-process-matrix.json records .gate.tests.run 1277; they describe one gate run" |

T2 and T3 are the two evasions the repair was most likely to have missed and it
catches both, in the fail-closed direction: a passage that stops enumerating
fails as six omissions, and a guide that stops claiming fails as a lost claim.

### Round 3 — The verbatim reproduction of Round 2

I diffed the landed text at `evidence/m4/review.md:378-679` against Round 2's
original. The diff is exactly six lines: the six `###` headings gained a
`Round 2 —` prefix, plus a trailing newline the original file lacked. No
finding text, no mutation-table row, no exit code, no failure string, no
disposition and no "what I did not examine" item was altered, softened, reordered
or dropped. The reproduction is faithful and the deviation is the one
`markdownlint` MD024 forces.

### Round 3 — The re-recorded gate run

Confirmed against runs I made myself on this tree. `devenv tasks run
ravel:verify` → exit 0, "1278 tests run: 1278 passed, 0 skipped", matching
`manifest.json:101-105` (`total` 1278, `passed` 1278, `failed` 0, `skipped` 0,
`lineCoveragePercent` 88.43) and `linux-process-matrix.json:24-27`. Coverage
recomputed with the record's own `howToRecompute` command gives `TOTAL … 89.26%
… 85.69% … 88.43%`, matching `regionsPercent` 89.26, `functionsPercent` 85.69
and `linesPercent` 88.43 exactly; 88.43 is above the recorded floor of 80. Both
`benchmarks.results` digests recompute — `2b1234c7…` for the matrix, `2b971e2d…`
for `sdk-surface.json` — and `review.record.sha256` `7d23be65…` matches
`evidence/m4/review.md` as it stands after Round 2's verdict landed in it.
`revision.changeId` `poylotnsnumzxnxklmnruyuomnlrnqqw` resolves to commit
`f0519617e778baa0e1c540243f106b597ed6ab8b`, which is `revision.id`, on a real
ancestor of `@` described "docs: record M4's evidence and the limits it ships
with". Mutations U and V prove the two bindings are live.
`commands[0].logSha256` and `durationMs` bind a private log and remain
unverifiable, which is the ceiling `verify_recorded_run` already records.

### Round 3 — New findings

**non-blocking — R7. `enumerating_passage` takes the first paragraph carrying the
claim, and nothing requires there to be only one.**
`tools/ravel-xtask/src/lib.rs:1644-1648` is
`document.split("\n\n").find(|paragraph| paragraph.contains(COMPLETENESS_CLAIM))`.
`docs/guide/running.md` holds that string once today, at `:121`, and the
enumeration under it is complete and correct. But a second paragraph anywhere
earlier in the same file that repeats the claim verbatim becomes the held
passage, and the passage an author actually reads is held by nothing. Mutation
T1 measures it: with a decoy paragraph naming all six variants inserted near the
top and `NotUtf8` deleted from the real enumeration, `ravel:verify-scaffold`
returns exit 0. The function's doc comment at `:1511-1513` discloses the
adjacent case — "a second passage elsewhere in the guide claiming the same set in
different words is held by nothing until its own claim is named here" — but that
describes a second claim going unheld, whereas this one displaces the held claim.
It is R6 repaired but incompletely: the escape moved from anywhere under
`docs/guide` to one specific file, which is a real narrowing, and the remaining
form needs a deliberate verbatim duplicate rather than the paste that worked
before. It is non-blocking for the same reason Round 2 gave R6 — the enumeration
in the tree today is complete and correct, no author is being misled, and the
class is disclosed. The closing move is one line: require exactly one paragraph
carrying the claim and treat a second as a violation, which is the same shape as
the absent-claim branch already there.

### Round 3 — Closure

**M4 is closeable on this tree.** Nothing here blocks it. The one new finding is
non-blocking, and I state plainly that I do not require it repaired for closure:
recorded as a deviation or as this round's text, it is honest disclosure of a
narrowed hole in a check that was strengthened, not a gap being carried as one.
If it is repaired instead, the repair is a change no round has read and a fourth
round is owed — that trade is the writer's to make, and choosing not to repair is
the choice that ends the loop.

Walking `ROADMAP.md`'s M4 table row by row against what is in the tree:

- **M4-W1** — met. `ravel-sdk` and `ravel-sdk-macros` exist,
  `tests/conformance/tests/sdk_boundary.rs` is the compile-pass/fail suite, and
  the row's own claim that the boundary is derived from the `Cargo.lock` closure
  rather than asserted is what that suite recomputes.
- **M4-W2** — met. `content_declarations.rs` and the `content-mod` example cover
  declarations, conflicts and deterministic output; the row already excludes a
  game process registering content.
- **M4-W3** — met as narrowed. `sides_and_packets.rs` and the `client-only` and
  `networking` test mods exist. The row states the two exclusions — no mod can
  send a packet, and the shipped host reads no manifest — and both are carried
  as deviations 1 and 2 and stated in `docs/guide/limits.md`.
- **M4-W4** — met as narrowed. `config_saves_and_services.rs` and
  `configuration.rs` cover migration chains, service dependencies and failure
  closure; the fixed-composition exclusion is stated in the row and in deviation
  2.
- **M4-W5** — met as narrowed. `pack_closure.rs` and `ravel-pack` exist,
  `ravel:verify-example-pack` is in the gate's `after` list, and the write-half
  restriction is deviation 3 and an ADG obligation row that assigns the reading
  half to M8.
- **M4-W6** — met as narrowed. `tests/fixtures/public/versions/99.0.0-synthetic`
  is the second-version fixture; the exclusion — no general crate compiled or
  run against the second version's target table — is the row's own sentence and
  deviation 4.
- **M4-W7** — met. `docs/guide/` exists with eight documents,
  `guide_snippets.rs` compiles every Rust block in it under the gate,
  `cargo xtask package digests` is implemented at
  `tools/ravel-xtask/src/main.rs:13-52`, and `test-mods/waystone-ledger` is the
  resulting original mod. The row states its own premise repair in place.
- **M4-W8** — met, conditional on the recording step below. The global Linux gate
  passes at exit 0, `linux-process-matrix.json` reconciles against
  `nix/common.nix:589-602` in both directions, `sdk-surface.json`'s inventory
  reconciles against the repository, and this is the third independent review
  round.

The eleven deviations: I read each summary against the tree. Each names a
property M4 does not prove, names why, and names who would close it; none of them
is a blocking gap wearing a disclosure. Deviations 1, 2, 3, 4 and 6 are the ones
that most look like they might be — each is a *narrowing of a claim* rather than
a defect in what is claimed, each has its ROADMAP row narrowed to match, and 1,
2 and 3 additionally appear in `docs/guide/limits.md` where an author reads them
rather than only where a reviewer does. Deviation 9's self-referential shape
(the manifest never spelling the fixture directory's adjective) is odd but true
and checkable. Deviation 8 was the one Round 2 refuted and it now states the
fact correctly.

What remains before `ravel:verify-milestone:m4` exits 0 is mechanical and is the
convention this file states at `:15-20`: this verdict lands in
`evidence/m4/review.md`, the `Result:` line at `:3` changes to `PASS`,
`review.result` in `evidence/m4/manifest.json` changes to `pass`, and
`review.record.sha256` is rebound in the same change. `verify_recorded_verdict`
holds the first two together and `verify_recorded_review` holds the third. If
anything else is repaired after that change, the pass certifies a tree that no
longer exists and a fourth round is owed.

### Round 3 — Hard rules

Clean. Every `crates/*/src/lib.rs` carries `#![forbid(unsafe_code)]` — a sweep
for a root missing it returned nothing. No `#[allow(…)]`, `#[expect(…)]` or
`#[ignore]` exists under `crates/`, `tools/` or `tests/` outside the scaffold
check's own string literals. The pinned files under `minecraft/versions/26.1.2/`
were not edited, transiently or otherwise; `jj st` was clean before and after
every mutation and I restored by `cp` from backups rather than by `jj restore`.
A redaction sweep over the files the five changes touched found no credential,
token, world name, username, launcher path, instance path or absolute user path
— every hit is a literal inside the redaction check itself or a quoted example
in a prior round's text.

### Round 3 — What I did not examine

I did not re-run Round 1's mutations C through J or Round 2's mutations A, B, K1,
K3, L, N, O, Q, R and S, which the round-2 repairs did not touch; I re-ran the
two bindings most likely to have broken when the digests were rebound (U and V)
and took the rest on the earlier rounds. I did not run
`ravel:verify-environment`, so the `flake.nix` repair is still unverified by me.
I did not audit `tools/ravel-xtask/src/lib.rs` as a whole — I read
`verify_guide_enumerations`, `enumerating_passage`, `numeral`, `tens_value`,
`verify_review_round_counts` and their tests, and nothing else in the file
beyond what a grep surfaced. I did not re-verify the roughly fifteen ungated
prose figures of N8, and I did not repeat Round 2's tree-wide sweep for spelled
numerals; the numeral reader now covers more of that space but the sweep itself
is a round-2 measurement I inherited rather than reproduced. I did not read
`tests/conformance/tests/` as tests — I confirmed the suites named in the
closure walk exist and took the gate's green as evidence they pass, not as
evidence they are the right tests. I did not review `test-mods/waystone-ledger`
as a mod. I did not check `docs/guide/` for prose claims outside the `WireError`
enumeration, having read the eight documents rather than audited them.
