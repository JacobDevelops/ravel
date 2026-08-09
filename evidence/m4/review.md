# M4-W8 Independent SDK Review

Result: FAIL

**Round 1 ran and returned FAIL**, on three blocking findings and eight
non-blocking ones. Its verdict is reproduced below and the line above is that
reviewer's conclusion rather than a placeholder standing in for one. It is also
the closure state `evidence/m4/manifest.json` records under `review.result`, and
`schemas/evidence.schema.json` admits exactly three values for it — `pass`, `fail`
and `not-required`. `verify_recorded_verdict` in `tools/ravel-xtask/src/lib.rs`
requires this document's first `Result:` line to agree with that field, so the two
move together.

The line changes to `PASS` only when an independent round returns a pass on the
state it read, and it changes at the same time as `review.result` and as the
verdict that round wrote, below.

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
