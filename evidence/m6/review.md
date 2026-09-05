# M6-W6 Independent Counterpart Review

Result: PASS

**Two rounds have run. The first returned FAIL and the second returned PASS on a
tree corrected inside the round.** Between them they raised five blocking
findings, and the per-round headings below are the only place that number is
written: four in round 1, one in round 2. Every one was closed by changing a
tracked artefact and re-checked by breaking that artefact and requiring the
gate to refuse it, not by reading the change.

Round 1 read the sealed twelve-package wave, the counterpart summary and the
manifest generator's output, and found the test total stale, the M6-W5 evidence
schema rejecting every row it described, a false sentence about what the
structure reviewer had seen, and a denominator whose per-row derivation was kept
nowhere with every ambiguous row resolved in the flattering direction. Round 2
re-examined each of those on the corrected trees, re-derived the denominator
correction independently row by row, found one further stale clause in the
manifest generator and one transcription the private gate did not bind, and
recorded its verdict after both were corrected and re-broken. Its closing section
names the commits and the summary digest the pass certifies, and the earlier
commit and digest it certified before a one-line provenance correction moved
them.

A pass certifies the tree its reviewer read, with nothing landed on top of it.
The manifest that binds this record was written after the pass, from the
generator whose output round 2 read; if the committed manifest is anything other
than that generator's output over the certified summary, this record has not
read it. One non-blocking finding is carried forward rather than made a condition
of the pass and is stated in round 2's closing section.

The reviewer was an agent with read access to both repositories and no upstream
source, given the round-1 standard in writing: a figure stated twice by hand, a
bound nobody enforces, ambiguity resolved in the flattering direction. Scratch
paths in the records below are named as the session scratch directory, and the
private repository as the private registry.

## The rounds

### Round 1 — FAIL, four blocking

Result: FAIL

**Round 1 of the M6-W6 review has run and returned FAIL with four blocking
findings.** None of them is an arithmetic error in the counterpart summary. Every
figure that record publishes re-derived exactly, from the NOTES tables upward,
and the cluster scenario turned out to prove more than the summary claims for it.
What failed is the layer above the figures: a test count in the manifest that the
sealed tree contradicts, a process artefact that cannot do the job it is named
for, a sentence about who read what that is false in the public manifest and in
twelve tracked private records, and a denominator whose per-row derivation exists
nowhere either repository will keep.

Three of the four are the defect class `evidence/m5/review.md` names — a producer
and its checker sharing a rule, so the rows that would contradict them are never
built. Here the shared rule is usually that nothing checks at all: the figure is
prose, the schema is unwired, the vocabulary is unenforced, the classification is
a file in a temporary directory. M5's campaign found the class inside machinery
built to catch its predecessor. M6's instances sit in the places where no
machinery was built.

#### What this file is (round 1)

The record of the independent review M6-W6 requires. This is round 1. The
reviewer is independent of the work under review: it wrote no specification, no
counterpart, no check and no summary, and it read the artefacts as a stranger to
them.

Nothing here is sanitized in the way `evidence/m5/review.md` had to be, because
this round quotes only from the two public files, from tracked private prose that
carries no upstream identifier, and from session artefacts. Where a private
counterpart is named it is named by its Ravel canonical id, which is Ravel's own.

#### How this round was run (round 1)

Read-only in both repositories throughout. No file was edited, no commit was
made, and no `cargo` was run in the public checkout, where a gate was in flight.

Four private checks were run, all of which only read files:

    verify-counterparts          exit 0   238 source files and 39 generated artefacts
                                          against 892 upstream identifiers and 87 published names
    verify-counterpart-packages  exit 0   12 packages, 244 digests recomputed, every file accounted
    verify-spec-identifiers      exit 0   43 files read against 892 upstream identifiers,
                                          no hit under rules U1 or N1
    verify-provenance            exit 0   12 wave entries across 1 waves carry a complete record

`checks/verify-counterparts.sh`'s closing counts are the two the summary
publishes as `filesScanned` and `artifactsScanned`, and both matched on this
round's own run rather than on the transcription: 238 and 39.

#### What re-derived, and what that is worth (round 1)

**Every published total.** The twelve rows sum to `declarableOnly` 65,
`exercised` 433 and `observableAreas` 498, and each row's two parts sum to its own
whole. The gate enforces all six of those identities in `M6_ARITHMETIC`, so this
was a confirmation rather than a discovery.

**Every per-package figure, from the classification below it.** Recomputing
exercised and declarable-only per package from the 547 classified rows reproduced
all twelve rows of the summary exactly, with no disagreement.

**The classification's coverage of the tables it claims to read.** Counting
`^| §` rows in each of the twelve tracked `NOTES.md` files gives 42, 60, 17, 71,
47, 55, 53, 60, 22, 46, 34 and 40, summing to 547 — the same 547 rows, per
package and in total, with none dropped and none invented. This is the strongest
single thing in the milestone: the denominator is bound to tracked prose row for
row, which is more than M5's inventory figures ever were.

**The wave.** The twelve `canonicalId` values and every dependency list in
`waves/m6-foundation.json` are identical to the summary's `wave.entries`, and to
the twelve `.counterparts[].entry` values, and to the twelve package directories
under `mods/`. The gate holds the last of those pairs equal in both directions.

**Both mutation injections, by their logs.** Each names the rule and the file:

> counterparts: rule U1: rvl-002-counterpart/src/lib.rs carries an upstream identifier: ae2things
>
> counterparts: rule U1: rvl-002-counterpart/generated/census.json carries an upstream identifier: ae2things

The clean run's log carries the check's success line. See finding N1 for which
tree they were taken on.

**Every digest this round could recompute.** The summary's own sha256 in the
manifest's `benchmarks.results` matches the file, and all four `inputs` digests
match their public files.

**Redaction.** Neither public file carries a home-directory path, a path into
the ignored local fixture tree, a path into the private evidence tree, or the
private workspace's name. Those are the patterns `verify_redaction` refuses, and
it runs over the summary on every gate.

##### The cluster scenario is better evidence than the summary claims (round 1)

The summary's reload step records `record=advanced, twice, byte-identical`, and
"byte-identical" is not a string the harness prints. It would have been fair to
suspect a decoration. It is not one. `jvm/harness/run-harness.sh` runs the reload
a third time from the same stored record and compares the two aggregates with
`cmp -s`, failing with "two reloads of one record wrote different aggregates, so
what a reload reads is not the record it was handed". `record=advanced` is
likewise a real inequality: `ClusterMain.java` fails the step with `record=unread`
when the second visit writes the first visit's bytes back, on the stated ground
that a visit which ignored the record would produce exactly those bytes. The
mutation case flips one byte at the midpoint of the stored record and requires
both a nonzero exit and a refusal line naming the step.

`$S/cluster-rerun.log` shows this ran at 16:38–16:42 on 2026-09-05, after the
structure-review fixes landed at 16:37, and the host fingerprint `ab2b7c27…`, the
host digest `901608a4…` and the harness log digest `8de07ec1…` in that log are the
three the summary publishes. The composed host was built from the sealed tree.

#### Findings (round 1)

##### B1 — the manifest publishes a test count the sealed tree contradicts (round 1)

`environment.settings.gateStatus["M6-W3"].evidence` says:

> the per-package totals are recorded in the summary and sum to 1961 tests with
> no failure

The summary's own twelve `suites.total` values are 138, 211, 86, 241, 151, 192,
275, 147, 70, 182, 103, 154. They sum to **1950**. So does the private build
check's closing line, recorded after the structure-review fixes:

> verify-counterpart-build   0   12 packages compiled, formatted and linted, 1950 tests passed

1961 is the figure from before those fixes: `$S/m6-cluster2.md` transcribes the
build check at "1961 tests passed", and the first of the two tables in
`$S/m6-rows.md` carries the pre-fix per-package counts and the line "sum tests
1961 (matches 1961)". Eight packages then lost and gained tests as the structure
findings were closed, the second table records the new counts, the summary was
written from the second table, and this one sentence in the manifest was not.

The sentence is the only place either public file states a test total, and
nothing derives it. `M6_ARITHMETIC` has no rule over `suites.total` beyond
`> 0 and .failed == 0`, and `M6_CARRIED_FIGURES` does not carry it, so the gate
cannot see the disagreement. The figure is stated by hand in prose where it
should have been summed from the rows directly beside it.

**Blocking.** Fix: correct the sentence to 1950, and add a carried figure or an
arithmetic rule so that a per-package suite count and any total stated over it
cannot drift apart again. Recording the sum as a field under
`environment.settings.counterparts` and carrying it from
`[.counterparts[].suites.total] | add` costs one line in each table and removes
the hand entirely.

##### B2 — the M6-W5 evidence schema rejects every row of the record it describes (round 1)

`checks/counterpart-evidence.schema.json` is one of the three process artefacts
M6-W5 exists to produce, named in the summary's `processValidation.evidenceSchema`
and in the manifest's M6-W5 evidence sentence. Its own title is "Counterpart
evidence row" and its description says it is "One row of
evidence/m6/counterpart-summary.json's counterparts[]".

Validating the first such row against it fails three ways:

    $.id: 'rvl_002_counterpart' does not match '^RVL-[0-9]{3}$'
    $.entry: 'RVL-002' does not match '^mods/rvl-[0-9]{3}-counterpart$'
    $.specification.route: 'source' is not one of
        ['source-informed-reimplementation', 'black-box-respecification']

The schema has `id` and `entry` the other way round from the record, and its
`route` enum holds the wave manifest's two `method` strings rather than the two
the record uses. The public gate's own record predicate requires
`.specification.route == "source" or .specification.route == "observed"`, so the
gate and this schema disagree about the same field. All twelve rows fail
identically; there is no row this schema accepts.

The manifest already discloses that "the evidence-row schema is not yet wired into
the private gate", and that disclosure is what let the defect live: an artefact
nothing runs is an artefact nothing corrects. But the deviation says the schema is
unwired, not that it is wrong, and the M6-W5 gateStatus row asserts the three
artefacts "exist at the locators the summary names" as its evidence that the work
package is closed. A schema that cannot validate a single row of its subject has
not validated the authoring process; it has only been written.

**Blocking.** Fix: correct the two patterns and the enum so the schema accepts the
twelve rows, and wire it into `checks/verify.sh` over
`evidence/m6/counterpart-summary.json`'s `counterparts[]`. `check-jsonschema` is
already on `PATH` in the shell the checklist tells an author to use, and the
checklist already names three checks to run; this is a fourth line. Leaving it
unwired after correcting it would mean the same defect is available again the next
time the record's shape moves.

##### B3 — "had seen neither the source nor the counterpart" is false, in the manifest and in twelve tracked records (round 1)

The manifest says it twice. In `gateStatus["M6-W5"]`:

> Every specification was reviewed for structure by a reader who had seen neither
> the source nor the counterpart

and in the fifth deviation:

> Every specification was read by a reviewer that had seen neither the source nor
> the counterpart

The same sentence is written into all twelve tracked provenance records under
`publication.reviewedBy`, in the form "structure-reviewed 2026-09-05 by an
independent Claude reader that had seen neither the source nor the counterpart".

Every one of the twelve structure-review reports cites the counterpart's Rust by
file and line. From the RVL-002 report:

> **Reproduced.** `src/tools.rs:147` declares `pub struct RepairMaterial` with
> `new()`, `get(&self, lookup: &dyn Fn() -> Option<String>)` and `resolved()`, and
> the module header at `src/tools.rs:5-6` repeats "looked up through a memoised
> supplier".

and

> **Partly reproduced.** `src/census.rs:277-283` carries the mechanism into the
> counterpart's own doc prose verbatim in substance … and `census.rs:474` asserts
> `the_block_items_follow_the_block_registrys_own_insertion_order`.

Twelve of twelve reports quote counterpart source this way, the four whose verdict
is a plain "accepted" included. The reviewer read the counterparts. Nothing here
suggests it read upstream source, so the first half of the sentence stands; the
second half does not.

The timing is wrong in the same way and the private stack shows it directly.
`docs(m6): structure-review the twelve specifications and record each verdict`
(`knvyqymywlul`) sits above every one of the twelve
`feat(counterparts): add the RVL-NNN counterpart` changes, and above
`refactor(counterparts): remove the structure that crossed from eight
specifications` (`krmpqzootqwk`), which changed both specifications and
counterparts in response. The reviews came after implementation, not before it.

That matters because the field they set has a tracked definition that says
otherwise. `counterpart-evidence.schema.json` defines `structureReviewed` as
"Whether the module list, public types and section table were reviewed **before
implementation**, per step 3 of records/counterpart-checklist.md", and step 3 of
the checklist is headed "Structure review before implementation" and says the
review "fails it before a line of Rust is written, which is the only cheap moment
to catch it". `ROADMAP.md` is where the design comes from and is more explicit
still: the review "needs no source access, no comparison and no judgement about a
finished counterpart — it is one reading of one document asking one question".
The review that ran is a comparison and a judgement about a finished counterpart.
`.specification.structureReviewed == true` is recorded for all twelve, and the
gate's own rationale for that rule restates the before-an-implementer-sees-it
design.

None of this makes the reviews worthless — they found real crossings, eight
packages changed because of them, and reading the counterpart is plausibly a
*stronger* instrument than reading the specification alone. That is the reason to
correct the record rather than to redo the work. But three documents and twelve
provenance records describe an instrument that was not the one used, in the
direction that makes the process look more disciplined than it was, and the
deviations disclose the process artefacts' lateness without disclosing this.

**Blocking.** Fix: in all three places say what happened — the reviewer read the
specification and the finished counterpart together, after implementation, and had
no upstream access. Correct `structureReviewed`'s description in the schema, or
rename the field, so it no longer asserts a timing nothing met. Add a deviation
recording the departure from the roadmap's before-implementation design and what
was gained and lost by it. Rewriting the twelve `reviewedBy` sentences is
mechanical and the change that wrote them shows the form.

##### B4 — the denominator's per-row derivation is kept nowhere, and every ambiguous row moved the same way (round 1)

The second deviation says:

> the 547 rows were classified into exercised, declarable-only and excluded by one
> reader under a written rule … The rule and every row's class are kept beside the
> record

The rule is kept: `records/counterpart-checklist.md` step 2 states it as a
four-verdict table. **Every row's class is not.** Searching both repositories for
the classification finds only the schema, the summary and the gate's source; the
547 classified rows exist only as `m6-rows.tsv` and `m6-rows.json` in this
session's temporary scratchpad. This round could re-derive all twelve per-package
figures only because that directory still exists. After it is gone, no reader can
reproduce the denominator, contest a row, or tell which rows the deviation's
"three rows reclassified on review" were. The one figure `ROADMAP.md` singles out
— "the honest denominator this milestone's green sits over, and … what later carry
work is ordered by" — is derived from an artefact neither repository holds.

The written rule also governs much less than the deviation implies. The
checklist's table admits four verdict strings and says "That table is the
denominator the evidence schema counts". The sealed packages use 240 rows carrying
one of those four and **307 rows carrying something else**: `host-blocked`,
`gating only`, `declared as names only`, `implemented as data`, `not carried, by
§17's own exclusion`, `provides nothing`, `not reachable`, and roughly two hundred
further one-off phrasings. Nothing checks the vocabulary, so for 307 of 547 rows
the classification was a reader interpreting free prose rather than reading a
verdict. This is the checklist's own step 5 defect — "a bound nobody enforces" —
inside the checklist.

And the interpretation is not neutral. Four rows carry mixed verdicts and all four
were classified `exercised`:

    rvl_042  §9 published events                     one implemented, three declarable only
    rvl_210  §15 the seven subcommands …             implemented; two of the seven act, five are declarable only
    rvl_025  §8.1 the three filling routes …         two implemented, one declarable only
    rvl_321  §13 the two signals that drive them     the reload signal implemented; the rest declarable only

Five further rows say the thing was not reached rather than not shipped, and were
put outside the denominator entirely rather than into declarable-only:
`rvl_042 §6.2 … not reached`, `rvl_180 §3 … not reachable` (twice),
`rvl_180 §10 … not reachable`, and
`rvl_287 §3 … not reachable: all three consume the rendered result`. "Not reached"
is the checklist's own word for outside the denominator, so those follow the
letter of the rule; they also describe something present and unexercised, which is
what declarable-only means.

Every ambiguity resolved in the direction that lowers declarable-only or shrinks
the denominator. The deviation discloses that "a different reader could move a
handful of rows and the totals would move with them" — it does not disclose that
this reader moved every ambiguous row the same way, and it is the flattering
direction, in the figure this milestone exists to publish honestly. M5's record is
emphatic that a flattering error in the document that exists to count is the
serious kind.

**Blocking.** Fix, in order of what it buys:

1. Track the 547 classified rows in the private workspace beside the packages, so
   the deviation's sentence becomes true and the denominator is reproducible.
2. Split the four mixed rows into their exercised and declarable-only parts, or
   state in the deviation that a mixed row counts wholly as exercised and give the
   four so a reader can adjust.
3. Say in the deviation which way the residual judgement leans, naming the five
   not-reached rows, rather than only that rows could move.
4. Make the private gate derive each package's three counts from the tracked rows
   and require the summary to agree, and require every NOTES verdict to be one the
   checklist publishes. Both are cheap and both close the shared-rule gap: today
   the producer of the counts and the record of the counts are the same hand with
   nothing between them.

#### Non-blocking findings (round 1)

**N1 — the mutation proof was taken on a tree the milestone did not seal.** The
clean-run log closes with "…892 upstream identifiers and **85** published names";
this round's run of the same check on the sealed tree reports **87**. The lexicon
gained `declaration`, `registration` and `helpers` and lost `facade` when the
structure findings renamed three modules, which is +2. The mutation logs are
timestamped 16:04, the structure fixes 16:37, the cluster rerun 16:42. So the
three exit statuses the summary publishes under `mutationProof` were measured
before the tree they are recorded against existed. The substance survives — rule
U1 tests the 892 upstream identifiers, which did not change, and this round
confirmed the clean run still exits 0 on the sealed tree — but nothing binds the
recorded proof to the sealed tree, and a reviewer without shell access to the
private workspace could not have noticed. Re-run both injections on the sealed
tree, and record the check's closing line alongside each exit status so a
mismatched count is visible.

**N2 — one block, two checks, one name.** `rewriteIntegrity.check` says
"checks/verify-counterparts.sh, run by checks/verify.sh", and the block carries
`specificationViolations`, which that script does not produce;
`verify-spec-identifiers.sh` does, over 43 files. Name both.

**N3 — nothing binds a per-package suite count to what ran.** The build check
reports one sum. The summary carries twelve per-package numbers, and the only
place the twelve appear beside the sum is a scratch file. Have the build check
print its per-package counts and record them.

**N4 — two rows are classified on a naming rationale rather than an exercise
one.** `rvl_210 §4 … carried as this package's own names, never the original's`
and `rvl_210 §6 … four kinds, named here rather than transcribed` are
declarable-only rows whose stated reason is rewrite integrity, not the absence of
a seam. The direction is conservative, so this is a rule-application note rather
than a figure correction.

**N5 — the second-counterpart claim rests on facts no artefact records.** The
M6-W5 sentence "The second cluster leaf, rvl_086_counterpart, was authored by a
hand other than the first leaf's" is supported by the two counterparts being
separate changes and by this session's own history, and by nothing in either
repository: both changes carry the same committer. The claim is almost certainly
true and is not checkable, which is the same shape as M5's licence judgements.
Record it as a judgement rather than as a measurement.

#### What was not reviewed (round 1)

An honest account of where this verdict does not extend.

1. **No upstream source was read, and no counterpart's Rust was read for
   transliteration.** The review was instructed not to open the fixture tree or
   `mods/rvl-180-counterpart/src`, and it read no counterpart source at all. So
   this round says nothing about whether any of the twelve is an independent
   design. It confirms that the check which would catch a *name* runs, refuses
   both injections and exits 0 clean, and it reports in B3 that the instrument
   meant to catch *structure* is described wrongly. Whether the structure reviews
   were competent is not a question this round asked.
2. **The public gate was not run.** A gate run was in flight in the public
   checkout and this round was barred from `cargo` there. So the M6 rules in
   `M6_RECORDS`, `M6_ARITHMETIC`, `M6_CARRIED_FIGURES` and `verify_m6_manifest`
   were read rather than executed, and the manifest's `tests` block — 1776 passed,
   0 failed — was not verified against anything. B1 was found by reading the gate
   for a rule that is absent, which is easier to get wrong than running it.
3. **Neither mutation was re-run, and no check was proved by mutation by this
   round.** Read-only access forbids injection. Every mutation result here is
   transcribed from a log, which N1 shows is not the same as measured on the
   sealed tree.
4. **The twelve specifications were not read.** So the specification-side claims —
   that each is accepted, that each carries no upstream identifier — rest on
   `verify-spec-identifiers.sh` exiting 0 over 43 files and on nothing this round
   read for itself. The check reports what it does not catch, and its disclosure
   is the right place to start a later round: "a rewrite that renamed everything
   is invisible, and M5 recorded no Java member or field name".
5. **The manifest's `revision`, `commands.logSha256` and `review` fields are
   `pending` by design** and were not assessed. A later round should confirm the
   review digest binds this file after it is written, the way M5's does.
6. **The private working copy was clean and sealed at `krmpqzootqwk`**, and the
   public one at `trqnlwuworsr`, both with no uncommitted changes. This verdict
   certifies those two trees and nothing landed on top of them.

#### What is weakest here (round 1)

The single largest fact about M6 is the one its first deviation states at length
and this round can only confirm from the inside: no public gate can falsify a
private figure. This round ran the private checks and re-derived the private
figures because it had shell access to the private workspace. A reader of the
public repository has the summary, the manifest, and the arithmetic the gate
enforces between them — which proves the record agrees with itself, and B1 is an
example of a figure escaping even that, because it was written in prose where no
rule reaches.

Second, three of the four blocking findings are about sentences rather than about
numbers, and sentences are what nothing in either repository checks. B1 is a
number inside a sentence. B3 is a sentence repeated in fourteen places. B2 is a
document that nothing runs. B4 is a rule that governs 240 of 547 rows. M5 ended by
observing that a mechanism reading as proof while proving less than it claims is
worse than an unbound assertion, because nothing downstream of it gets read
sceptically. M6's process artefacts — a checklist, a template, a schema — are
exactly that shape: they read as a validated authoring process, and one of them
cannot validate anything.

Third, this round is one reading by one reviewer, and its own arithmetic is one
hand's. Its per-package recomputation shares an input with the producer's: both
read `m6-rows.tsv`. What it does not share is the step below that, the count of
`NOTES.md` table rows per package, which was taken independently and agreed
exactly. If that agreement is wrong, it is wrong the way M5's round 2 was wrong —
the same rule written twice — and this round would not have seen it.

#### Result (round 1)

Four blocking findings: B1, B2, B3, B4. Under the rule this repository's review
records use, a round with any blocking finding fails.

Result: fail

### Round 2 — PASS, one blocking

Result: pass

**Round 2 returned fail on one blocking finding, that finding was corrected, and
this record's verdict is on the corrected tree.** The history below is left as it
happened rather than smoothed: the round found one blocking defect and one
actionable non-blocking one, both were repaired inside the round, and each repair
was re-checked here by mutating the tracked artefact rather than by reading the
change that made it. The trees this pass certifies are named in the closing
section, and nothing has landed on top of them.

#### What the round found (round 2)

**One blocking finding, and it was a single stale clause in the closing record.**
Every substantive finding round 1 raised is closed, and this round confirmed each
one by mutation or by reading the tracked file rather than the sentence
describing it. The denominator is now derived from tracked rows by a check that
refuses eleven different ways of lying to it; the evidence schema validates the
twelve rows it describes and fails the gate when reverted; the false
"seen neither the source nor the counterpart" sentence survives nowhere in either
repository; the stale test total is now computed by the generator rather than
typed. The correction did not move a single row in the flattering direction, and
this round proved that mechanically rather than taking it on trust.

One finding this round raised was also closed inside it. The public summary's
three figures per counterpart were a transcription no standing check compared,
and a mutation of mine passed both gates; the private check now holds those
figures to the rows in both directions, and the same mutation exits 1 naming the
package. That was re-checked here by running the mutation again rather than by
reading the change.

What failed the round was that the manifest still told its reader the evidence-row
schema was not wired into the private gate. It was wired, and reverting it exits
with status 1. That clause sat in the same generator whose neighbouring sentence had been
fixed to derive its figure, so one hand-written statement about the machinery was
regenerated and the one two entries below it was not — round 1's B1 in a sentence
instead of a number. It has since been corrected; the finding below carries the
replacement and the re-check.

#### What this file is (round 2)

Round 2 of the independent review M6-W6 requires, and the last round. Round 1's
record is `review.md` beside this file and is the round this one is measured
against. Same reviewer, same read-only discipline, no writes of any kind in
either repository, no `cargo` in the public checkout.

#### How this round was run (round 2)

Both working copies were clean throughout: the private tree sealed at
`mwmkxrktslny`, the public tree at `slkxuxtvxlwm`.

Eight private checks were run, all read-only, all exit 0:

    verify-notes-rows            0  550 section rows over 12 packages, 22 published verdicts;
                                    432 exercised, 74 declarable-only, 44 excluded, denominator 506;
                                    records/m6-notes-row-counts.json agrees row for row;
                                    12 rows of the public summary validated against the schema
    verify-counterparts          0  238 source files and 39 generated artefacts, 892 identifiers, 87 names
    verify-counterpart-packages  0  12 packages, 244 digests recomputed, every file accounted
    verify-spec-identifiers      0  43 files read against 892 upstream identifiers, no U1 or N1 hit
    verify-provenance            0  12 wave entries across 1 waves carry a complete record
    verify-primitives            0  40 files under checks/ scanned, 27 primitives, 95 accounted uses
    verify-reads                 0  5 producers watched, 2382081 archive reads classified, 0 entry bodies read
    verify-boundary              0  15 files bound by digest, each recomputed here; 489 tracked files

Twenty-three mutations were run in two passes against copies of the private tree
in this session's scratch directory, never against the tree itself. The second
pass re-ran every mutation of the first over the amended check, because a check
that has changed has to be broken again.

The public gate's result is reported rather than measured here: exit 0, log
digest `eb598b77…`, 1776 tests, 89.40% line coverage. This round did not run it.

#### Round 1's findings, each re-examined (round 2)

##### B1 — the stale 1961 test total: **closed** (round 2)

`gen-manifest.jq` no longer carries the figure. It builds the sentence from the
record:

    "... and sum to " + ($sum.counterparts | map(.suites.total) | add | tostring) + " tests ..."

Running the generator over the sealed summary produces "sum to 1950 tests with no
failure". The twelve `suites.total` values are unchanged from round 1 and still
add to 1950, which is also what the private build check reports. No literal test
count survives anywhere in the generator. Round 1 asked for the figure to derive
from the rows beside it; it does.

`verify-counterpart-build.sh` now prints a line per package as round 1's N3
asked, so a per-package number has a source outside the summary for the first
time.

##### B2 — the evidence schema that rejected every row: **closed, and now enforcing** (round 2)

`checks/counterpart-evidence.schema.json` has `id` and `entry` the right way
round, `id` matching `^rvl_[0-9]{3}_counterpart$` and `entry` matching
`^RVL-[0-9]{3}$`, and its `route` enum is `source` and `observed`, which is what
the record uses and what the public gate's own predicate admits. Its `route`
description now says why the wave manifest's longer method strings are a
different vocabulary rather than this one.

It is no longer a document nothing runs. `verify-notes-rows.sh` writes each row
of the public summary to a file and runs `check-jsonschema` over all twelve, and
`verify.sh` runs `verify-notes-rows.sh`. Proved by mutation rather than by
reading: reverting `properties.id.pattern` to the old `^RVL-[0-9]{3}$` in a copy
of the tree makes the check exit 1 and name all twelve rows —

> checks/counterpart-evidence.schema.json rejects a row of the record it
> describes: … row-1.json::$.id: 'rvl_002_counterpart' does not match
> '^RVL-[0-9]{3}$' …

— and restoring it returns exit 0. See B1 of this round for the one place the
record still says otherwise.

##### B3 — "had seen neither the source nor the counterpart": **closed** (round 2)

The phrase survives in neither repository; a search of every tracked JSON and
Markdown file in the private workspace and of both public files returns nothing.
All twelve provenance records now read:

> structure-reviewed 2026-09-05 by an independent Claude reader that had seen no
> upstream source; it read the specification first and then the counterpart's
> module list and public types to see whether a structural noun had crossed,
> after the counterpart was implemented

The per-entry finding counts in those sentences (5, 3, 0, 5, 0, 1, 4, 4, 1, 4, 0,
0) are the ones the fix record carries, and the four "accepted" entries are the
four whose structure reports carry a plain verdict.

Step 3 of the checklist is retitled "Structure review", states that the review
runs before implementation for a counterpart written from here on, and then says
plainly what happened to these twelve, including the part that does not flatter
the process:

> Reading the finished counterpart is plausibly a stronger instrument than
> reading the specification alone, and it is a more expensive one, because what
> it finds is a rewrite rather than an edit to a document.

The schema's `structureReviewed` description no longer asserts a timing it cannot
hold, and says the checklist records which of the two timings each entry had. The
manifest's M6-W5 evidence and its fifth deviation both say the review ran after
implementation, and the deviation adds that the roadmap places it before, so a
reader is told about the departure rather than left to find it. Round 1 asked for
exactly this and got more than it asked for.

##### B4 — the untracked, one-directionally biased denominator: **closed, and the correction verified row by row** (round 2)

The rows are the record now. Every `| §…` row of every section table begins with
a verdict published in `checks/counterpart-verdicts.tsv`, that file gives each of
its 22 verdicts one of three classes, and `verify-notes-rows.sh` adds them up per
package and holds `records/m6-notes-row-counts.json` to the sum. The record is
generated by the check's own `--write` form and never written by hand.

**The check was proved by mutation, nine ways.** Each ran against a scratch copy;
each was reverted and the baseline reconfirmed at exit 0:

| mutation | exit |
| --- | --- |
| a row's verdict replaced by an unpublished word | 1 |
| a row's verdict flipped from declarable-only to implemented | 1 |
| a row deleted | 1 |
| a row reduced to two cells | 1 |
| one count in `records/m6-notes-row-counts.json` falsified | 1 |
| the schema's `id` pattern reverted | 1 |
| an unused verdict appended to the vocabulary | 1 |
| the section heading duplicated in one package | 1 |
| a summary row deleted | 1 |
| a summary id naming no package | 1 |
| a row flipped **and** the private record rewritten to agree | 1 |
| a row flipped **and** the private record **and** the public summary rewritten to agree | 0 |

Every one of these was run twice: once against the check as this round found it,
and again after the amendment described below, because a check that has changed
has to be broken again. Both passes gave the same verdicts except for the flipped
row, which the first pass caught only through the private record and the second
catches through the public summary as well.

The last line is the check's own disclosed ceiling, printed on every run: "a hand
that classifies a row wrongly and publishes a verdict that agrees with it passes;
what it stops is a row with no verdict at all and a record that disagrees with
its rows." It is now one file wider than it was — a hand has to move the table,
the private record and the public summary together — and it still leaves a tell,
because the printed counts move. The check says so about itself rather than
implying it is caught. Refusing a padded vocabulary is the same rule the lexicon's
L3 applies and it is a real one: that mutation was one appended line.

**The correction moved nothing in the flattering direction, and that was checked
rather than believed.** This round parsed all twelve section tables with its own
reader, classified every row from the vocabulary independently of the check, and
joined the result to round 1's own 547-row classification. Exactly nine rows
differ and every one is a round-1 finding being closed:

- Five rows move from excluded to declarable-only, all five the "not
  reached"/"not reachable" rows round 1 named: `rvl_042 §6.2`, `rvl_180 §3`
  twice, `rvl_180 §10`, `rvl_287 §3`.
- Four mixed-verdict rows are split into an exercised row and a declarable-only
  row, all four the ones round 1 listed: `rvl_025 §8.1`, `rvl_042 §9`,
  `rvl_210 §15`, `rvl_321 §13`.
- One row that had been counted was a marker in `rvl_287`'s mutation-evidence
  table rather than in its section table; the check now scopes to the table under
  "What each section became" and reports the count of markers outside it, which
  is 1, rather than dropping them silently.

No other row's class changed in either direction. The arithmetic of the
correction closes against round 1's own figures: 65 + 5 + 4 = 74 declarable-only,
433 − 1 = 432 exercised, 49 − 5 = 44 excluded, 547 + 4 − 1 = 550 rows. The
denominator rose from 498 to 506, which is the direction an honest correction
moves it.

Two departures from the vocabulary list were made deliberately and both are
defensible. "not shipped" counts as excluded, which is the same shape as
"declined" and is right: a thing the package deliberately does not ship leaves
nothing for a later revision to exercise. "not reached" was removed from the
vocabulary entirely and those rows carry "declarable only;" explicitly, which is
the stronger choice, because it forces the row to say which class it means
instead of leaving a word that reads both ways.

The manifest's second deviation now discloses the residual judgement and its
direction by name, including the four splits, the five moved rows, the reason
("the first classification had resolved every ambiguity in the flattering
direction") and round 1's N4 about the two naming-reason rows. That is more than
round 1 asked for.

##### N1 — the mutation proof taken on an unsealed tree: **closed, thinly recorded** (round 2)

The summary now carries `mutationProof.measuredOn`: "the sealed private tree at
change mwmkxrkt, 2026-09-05T07:15:58Z". The full private gate ran green on that
tree at 17:15:32 local, which is the same moment. See N4 of this round for what
the recorded evidence does not carry.

##### N2, N3, N5 — **closed** (round 2)

`rewriteIntegrity.check` now names both scripts: "checks/verify-counterparts.sh
over the packages and checks/verify-spec-identifiers.sh over the specifications,
both run by checks/verify.sh". The build check prints per-package lines. The
manifest's M6-W5 sentence now says the second-counterpart claim "is a judgement
from this session's own history, not a fact either repository records", which is
the honest form of a claim nothing can check.

##### The cluster, re-run on the sealed tree (round 2)

The rerun at 17:21 produced the fingerprint `ab2b7c27…` unchanged, host digest
`f6af2666…` and harness log digest `bdad5afe…`, and those are the three the
summary publishes. All six harness lines are present: fresh-play, save, exit, two
reloads and the refusal naming the corrupted field. The private gate ran green
immediately before it, which round 1 did not have.

#### Raised and closed inside this round (round 2)

**The published denominator was a transcription no standing check compared.**
`records/m6-notes-row-counts.json` was derived from the rows and held to them;
`evidence/m6/counterpart-summary.json` carried the same twelve triples and
nothing on either side compared the two. This round proved it by mutation: a
summary publishing `rvl_042_counterpart` as 18 exercised and 0 declarable-only,
with the two totals recomputed to match, satisfied all eight arithmetic rules the
public gate enforces — every one evaluated true — while the private check exited
0 and went on printing 74 declarable-only, because it validated the rows' shape
and never their values. That was round 1's B4 one level up, inside the machinery
built to close it, which is the shape `evidence/m5/review.md` says to expect.

It was closed by amending `zlqxmvuvxulu` in place. `verify-notes-rows.sh` now
holds each of the summary's three figures per counterpart to the rows it derives,
refuses a summary id no package carries rows for, refuses a package the summary
carries no row for, and the limit that said it validated shape "and not their
values" is gone, replaced by a narrower one that says what it still leaves to the
sibling checks.

Re-checked here by running the same mutation rather than by reading the change:

> evidence/m6/counterpart-summary.json publishes rvl_042_counterpart as exercised
> 18, declarable-only 0, observableAreas 18, and its rows carry exercised 10,
> declarable-only 8, observableAreas 18

Exit 1; the clean tree exits 0; the closing line now reads "records/m6-notes-row-counts.json
and the public summary both agree row for row". The seven other private checks
still pass on the amended tree with their counts unmoved, including
`verify-primitives` at 40 files and 95 accounted uses, so the M5 machinery
amendment survives it.

**Counted as fixed in this round, not as an open finding.** It is recorded here
rather than dropped because a later reader should know the binding is one day old
and was written in response to a mutation, not designed in.

#### Findings (round 2)

##### B1 — the manifest said the evidence schema was not wired into the gate, and it was — **corrected** (round 2)

The third deviation, as the generator produces it, ends:

> The second counterpart therefore followed the process rather than the document,
> and the evidence-row schema is not yet wired into the private gate.

It is wired. `checks/verify-notes-rows.sh` runs `check-jsonschema` over the
schema against every row of the public summary, `checks/verify.sh` runs
`verify-notes-rows.sh` on the line above `verify-counterparts.sh`, and this round
proved the wiring bites by reverting the schema's `id` pattern in a scratch copy
and watching the check exit 1 naming all twelve rows. The check's own closing
line reports it on every run: "12 rows of the public summary validated against
checks/counterpart-evidence.schema.json".

This is the clause round 1's B2 asked to be made false, and the work that made it
false is the work this round confirmed. The sentence was simply not regenerated
with it. It is a hand-written literal in `gen-manifest.jq`, in the same
`deviations` array whose neighbouring M6-W3 sentence was changed to compute its
figure from the record — so the generator was opened, one stale statement about
the machinery was fixed, and the one two entries below it was not.

It matters more than its size. The manifest is where a later milestone reads what
M6 built, and this deviation is the record of whether M6-W5's process artefacts
work. A reader is told the schema still does not run, so a reader has no reason
to trust it, and the next hand to move the record's shape has no reason to expect
a check to complain. Round 1 failed this milestone for a stale figure in this
same document; a stale statement about whether a check runs is the same defect
with words instead of digits, and the direction it errs in does not change what it
is.

**Blocking when raised.** The fix asked for was to delete the clause or replace it
with what the tree does, keeping the deviation's first half, which is true and
worth keeping.

**Corrected, and re-checked here.** The generator now ends that deviation:

> The second counterpart therefore followed the process rather than the document.
> The evidence-row schema runs inside the private gate through
> checks/verify-notes-rows.sh, which the first review round caused to exist; it
> was not run by anything before that round.

Both halves are true and the second is the more useful sentence, because it
records when the wiring appeared as well as that it exists. Regenerating the
manifest over the sealed summary reproduces it, and the deviation's first half is
intact. One further occurrence of the words "not yet" survives in the generator
and is unrelated and correct: the fourth risk says a build output the packages do
not yet produce is outside the artefact scan, which is true and is the reason M7
widens it.

#### Non-blocking findings (round 2)

**N1 — the sealed-tree mutation proof named a change whose tree had since moved
— corrected after this record's first pass.** `mutationProof.measuredOn` says "the sealed private tree at change
mwmkxrkt, 2026-09-05T07:15:58Z". `mwmkxrktslny` was `d9f8610b` when this round
began and is `b1f30739` now, because amending `zlqxmvuvxulu` to close the finding
below rebased it. A change id is stable across a rewrite by design, which is why
this repository refers to work by change id — and it is therefore not a name for
a tree. The recorded evidence is four lines reading `change=mwmkxrkt`,
`clean=0`, `source=1`, `artifact=1`, with no check output, no rule name and
nothing derived from the tree, so a later reader has an assertion where round 1's
N1 asked for the check's closing line beside each exit status. The fix asked for
was to re-run the two injections on the final tree and record the closing line,
or the commit id, or both.

**Corrected, and re-checked here.** Both injections were re-run on a copy of the
final tree and the field now reads:

> a copy of the sealed private tree at change mwmkxrkt at commit b1f307395b17,
> 2026-09-05T07:45:44Z; the clean run closed with: 238 source files and 39
> generated artefacts read against 892 upstream identifiers and 87 published
> names

That is all three things at once, and the third is the one that matters: a commit
id pins the tree a change id cannot, and a closing line is a figure a later
reader can reproduce instead of an exit status they must believe. Reproduced here
by running `checks/verify-counterparts.sh` on the sealed tree, which prints that
line verbatim, and `b1f307395b17` is the `mwmkxrktslny` commit this record
certifies.

**N2 — the public gate type-checks none of the four new fields.** `M6_RECORDS`
names every other field of the record and requires its type, "for the reason
`M5_RECORDS` gives". It says nothing about `denominator`, `derivedBy`,
`excludedRows`, `excludedMeans` or `mutationProof.measuredOn`. Deleting the whole
`denominator` block leaves the gate green, so the block that answers round 1's
central finding is the one part of the record the gate does not require to exist.
Add the five fields to the predicate.

**N3 — the manifest stated a superseded row count and never the current one —
corrected after this record's first pass.** The second deviation said "the 547
rows were classified …", which is true as history, and the manifest never stated
550, so a reader comparing it to the check's output found two row counts and no
derivation of either.

**Corrected, and re-checked here.** Regenerating the manifest over the current
summary produces: "the first classification read 547 rows by one reader under a
written rule, one of which was never a section row and four of which were later
split, so the tables carry 550 rows and the private gate counts 550." Both counts
are now stated, the path between them is arithmetic a reader can check — 547
minus one plus four is 550 — and it agrees with the nine changed rows this round
derived independently.

**N4 — the per-package build lines were not checked against the twelve
packages.** `verify-counterpart-build.sh` keys its new lines on the crate name
parsed out of cargo's output, and this round could not run it. Whether twelve
crate names map one-to-one onto twelve package directories is unverified here.

#### What was not reviewed (round 2)

1. **The public gate was not run by this round.** `cargo` was barred in the
   public checkout. The eight arithmetic rules in this round's mutation proof
   were evaluated with `jq` against the rule text in
   `tools/ravel-xtask/src/lib.rs`, not by running the gate. That file is
   unchanged by every public change in this round, which is itself a finding
   under N2. The gate's green result — exit 0, log digest `eb598b77…`, 1776
   tests, 89.40% coverage — is reported to this round by the maintainer and is
   not something it measured.
2. **No upstream source and no counterpart Rust were read**, and
   `mods/rvl-180-counterpart/src` was not opened. So this round says nothing
   about whether any counterpart is an independent design. It confirms that the
   name check runs and refuses, and that the structure review is now described
   accurately; the quality of those twelve reviews is not a question either round
   asked.
3. **`verify-counterpart-build.sh` was not run**, so the twelve suite totals and
   the 1950 sum are transcribed from a log rather than measured here, exactly as
   in round 1.
4. **The twelve specifications were not read.**
5. **The manifest is uncommitted.** Its `revision`, `commands.logSha256` and
   `review` fields are still `pending`, and the prose reviewed above is the
   generator's output over the sealed summary rather than a tracked file. A
   maintainer who edits the manifest by hand after this review invalidates that
   reading, which is the reason to keep generating it.

#### What is weakest here (round 2)

The milestone's figures are now better founded than round 1 could have hoped: the
denominator is derived from tracked rows by a check that this round broke eleven
ways and that failed every time, and the correction it wrapped was verified row
by row to move only in the direction round 1 asked for. The transcription gap that let a
falsified public figure through both gates was found and closed inside this
round, so the private check now reads the public record and refuses a
disagreement in either direction.

What has not changed is the boundary itself. That new binding runs in the private
workspace, so it protects a reader who can run the private gate and not a reader
of the public repository, for whom the figures remain something to trust or to
reproduce the way this round did. The direction of the dependency makes that
unavoidable and both records now say so; it is worth being clear that a green
public gate still does not check a private count.

The second weakness is that both rounds found their last defect in prose. Round 1
found four blocking findings, three of which were sentences. Round 2 found one,
and it is a sentence. The checks in this repository are now good enough that the
figures survive being attacked; the sentences around them are written once, by
hand, and are corrected only when someone reads them. `gen-manifest.jq` is the
right answer to that and it is still only half-applied: one deviation computes
its figure from the record and the rest are literals that happen to be true
today. Both of this round's blocking clauses were literals in that file, and the
next stale sentence will be one too. A later milestone should expect its last
defect in the prose rather than in the numbers, and should look first at the
sentences that describe what a check does.

#### The verdict, and what it certifies (round 2)

Both corrections were re-checked on the tree as it now stands, by running the
mutation rather than reading the change:

- The evidence-row schema clause reads as the tree behaves, and the manifest the
  generator produces over the sealed summary carries the corrected sentence, the
  derived test sum of 1950, and the corrected denominator of 74, 432 and 506.
- The summary binding is in the tracked check, its false limit about validating
  shape "and not their values" is gone, and both directions of the binding are
  there. The mutation that passed both gates earlier in this round now exits 1:

  > evidence/m6/counterpart-summary.json publishes rvl_042_counterpart as
  > exercised 18, declarable-only 0, observableAreas 18, and its rows carry
  > exercised 10, declarable-only 8, observableAreas 18

  The clean tree exits 0 either side of it, and the closing line now reads
  "records/m6-notes-row-counts.json and the public summary both agree row for
  row".

Two further non-blocking findings were corrected after the paragraph above was
first written, and one of them moved the digest this record certifies. Both were
re-checked before the certification was restated:

- `mutationProof.measuredOn` now names the change, the commit, the time and the
  clean run's closing line. That line reproduces verbatim on a live run of the
  counterpart check against the sealed tree, so it is the first figure in this
  field a later reader can measure rather than believe.
- The manifest's second deviation now states both row counts and the arithmetic
  between them, 547 less one that was never a section row plus four that were
  split, giving the 550 the gate counts.

The first is a one-line edit to the summary, and this record confirmed it is only
that. Recovering the version certified below at its old commit and diffing it
against the current file returns exactly one changed line, the `measuredOn`
string; stripping that field from both and comparing the canonicalised remainder
byte for byte returns identical. Every figure, digest, row and cluster line this
record checked is therefore the same text it checked, and the digest moved
because one sentence about provenance got longer.

**A pass certifies the tree its reviewer read, with nothing landed on top of
it.** This one certifies the private workspace at `zlqxmvuvxulu` (`1eab65b1`) and
`mwmkxrktslny` (`b1f30739`), unchanged throughout, and the public checkout at
`slkxuxtvxlwm` (`45a68237`) whose `evidence/m6/counterpart-summary.json` hashes
to `e9235518a5318d7458d0ce6c8c7ecea2fe23a612ef9b93815bc132bb8fa93ffc`. The
version certified before the correction was `050c9665` and
`52317804d16ab4e4de62693884c9769ee7f258efc9ad63448b448e82fa45542c`, and it is
named here so the one-line move between them is on the record rather than
silently absorbed. Both working copies were clean.

It does not certify a manifest, because there is not one yet. The prose reviewed
above is what `gen-manifest.jq` produces over that summary, and the file it will
write is untracked at the time of this reading. If the committed
`evidence/m6/manifest.json` is anything other than that generator's output over
that summary, this record has not read it. One non-blocking finding remains open
and is carried forward rather than made a condition of this pass: the public gate
type-checks none of the record's `denominator` fields, so the block that answers
round 1's central finding is the part of the record the gate does not require to
exist.

Result: pass
