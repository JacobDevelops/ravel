# M5-W8 Independent Inventory Review

Result: PASS

**Eight independent rounds have run. Seven returned FAIL and the last returned
PASS.** Between them they raised thirteen blocking findings — the per-round
headings below are the only place that number is written, and it is their sum.
Eleven instances of a single defect class have been found. Not every blocking
finding is an instance of it, and two of the instances were found by a repair
auditing its own work rather than by a round.

That headline overstated the rounds' tally by one until round 7, folding a
repair-found instance into the reviewer-found total — in the flattering
direction, in the document this manifest names as the one place that counts.
`verify_review_blocking_counts` in `tools/ravel-xtask/src/lib.rs` now sums the
per-round headings and requires this summary to agree with them, so the figure
has one source and a heading that changes drags the sentence with it.

It also understated how many rounds had run, until this closing round, and that
matters more than the correction. Rounds 7 and 8 had both run and neither had a
heading here, so the stated count and the count derived from the headings agreed
at six — the class itself, in the record that exists to describe the class. The
rows that would have contradicted them were the two missing round sections.
Nothing in this repository knows independently how many rounds were run: the
check compares this document to itself, and a round that is never written in is
invisible to it exactly as it was here. That limit is disclosed below rather
than closed, under the rule `docs/decisions/claims-without-a-second-reading.md`
decides.

A pass certifies the tree its reviewer read, with nothing landed on top of it.
Round 8 read the repaired tree — the repair, this record, and the public gate
beside it — and nothing has landed on top of it since, which is why the line
above is `PASS` and `evidence/m5/manifest.json` records the same under
`review.result`. `verify_recorded_verdict` in `tools/ravel-xtask/src/lib.rs`
requires the two to agree, so they move together. `evidence/m4/review.md` states
the same rule and stayed `FAIL` between its own rounds for the same reason.

## What this file is

The record the independent M5-W8 review's rounds are written into, and **the one
place that says how many rounds have run**. No other document in this repository
states that count, and `verify_review_round_counts` in
`tools/ravel-xtask/src/lib.rs` sweeps `ROADMAP.md` and every non-exempt
milestone's README and manifest to keep it that way. That sweep used to name its
subjects by hand and went stale the first time a milestone was added to it; it
now derives them from the manifest list, so M5 was swept the day this file
landed.

Its digest is recorded in `evidence/m5/manifest.json` under `review.record`, and
`verify_recorded_review` recomputes it on every `ravel:verify`. Writing a round
into this file means rebinding that digest in the same change.

### This record is sanitized, and M3's and M4's are not

`evidence/m3/review.md` and `evidence/m4/review.md` reproduce every verdict
verbatim. **This one deliberately does not, and the reason is not convenience.**
Those reviews are of this public tree; a verbatim reproduction discloses
nothing that is not already here. M5's reviews are of restricted material. The
verdicts as written contain the private workspace's absolute path, the filenames
of third-party artifacts, upstream identifiers, and in the most recent round a
sample of decompressed class-file bytes. Reproducing them would put all of that
into a public repository, which is the one thing this milestone exists not to
do.

So every round below is **written from the verdict as returned, and sanitized**:
aggregates, verdicts, defect descriptions and references into this public
repository are kept; private paths, artifact names, upstream identifiers and
byte samples are not. Nothing here is a summary standing in for a transcript
that was never written — the transcripts exist, they are complete, and they are
held where restricted review material is held rather than in this repository.
This paragraph is the disclosure that a reader is not getting the primary text
and why, which is the same obligation the manifest's deviations carry about
every other figure here.

## The defect class

Every blocking finding in this campaign is one shape, and naming it is worth
more than listing the instances:

> **A producer and its checker share a rule, so they agree, and the rows that
> would contradict them are never built.**

A checker is not a second reading when it asks the same question the producer
answered. It re-derives the same value from the same rule and confirms it, which
is one reading written twice. Whatever that shared rule is wrong about is
invisible by construction: no row exists that disagrees, because the rule that
would have built such a row is the rule under test.

**Eleven instances across eight rounds**: a hand-rolled declaration parser shared by
both readers; a provenance record bound to nothing it asserted; a shared rule
discarding nested declarations; a shared discovery rule for which nested
libraries exist at all; a shared depth rule stopping the behaviour catalogue at
the outer archive; a shared licence-family rule; the family-to-status rule one
function below it; a derivation list keyed on filenames rather than on code; a
completeness premise about which channels can open an archive; an interpreter
invoked with a script name computed at run time, which a scan of static text
cannot resolve; and finally a scan that counted matching lines while the table
beside it called them uses — where that table had been generated from the same
miscount, so the two agreed and the published figure was short by two.

The pattern matters more than any instance. Each repair was genuine — every
round confirmed its predecessor's repair held and re-derived the figures itself
— and each repair closed its instance by giving the checker a path that did not
share the producer's rule. **And each time, the next round found the class
inside the machinery that had just been built to catch its predecessor.** The
rule moved up a level; it did not go away. By the last two rounds the class had
climbed out of the data entirely and into claims about what the checking
machinery could see.

## Why this milestone will not claim the class is closed

The loop is unbounded by construction. Every repair adds checking machinery, and
that machinery is written by the same hand, from the same understanding, as the
thing it checks — so it carries a premise, and a premise written once is the
defect in its next form. Waiting for a round that finds nothing is waiting for a
round that did not look hard enough.

So the condition this campaign stops on is not "a reviewer finds nothing". It is
that **no published figure is falsified, and every limit that remains is
disclosed as a limit rather than presented as closed.**

The second half of that is not decoration, and the last round is why it is
written down. The two most recent instances were not wrong numbers. Every
committed figure re-derived exactly in the last round, from readers its reviewer
wrote from scratch. What was wrong was a mechanism that read as proof while
proving less than it claimed — and that is worse than an obviously unbound
claim, because nothing downstream of it gets read sceptically. The earlier form
of the same claim was an assertion everyone knew was an assertion, and it was
treated accordingly.

`docs/decisions/claims-without-a-second-reading.md` is where that generalisation
is decided rather than restated here.

## The rounds

Each round re-derived the committed figures from readers its reviewer wrote
independently and confirmed both working copies unchanged at the end. Rounds 2
onward mutated a throwaway copy; round 1 mutated in place and restored from `cp`
backups, which is the same discipline with a worse failure mode and is written
here rather than smoothed into the sentence that describes the others.

### Round 1 — FAIL, two blocking

**B1.** The declaration reader was a hand-rolled line scanner that recognised
one TOML spelling of a dependency block. Artifacts declaring the same structure
in other, valid TOML had every declaration dropped — no node, no edge, no
diagnostic. The reviewer re-derived the graph with a real TOML parser and an
explicit traversal sharing no expression with the workspace, and found six
missing edges. The consequence was not the count but the partition: a required
intra-pack edge was missing, so an artifact sat as a singleton instead of inside
the cluster it belongs to. The committed edge, cluster and largest-cluster
figures were all wrong.

**B2.** The tracked provenance record asserted facts that the later work
packages had falsified, and nothing bound it to them.

Five non-blocking findings, including a boundary check that skipped its entire
upstream-identifier half when an input was absent and reported success, and an
asset-reservation rule that fired in only one licence bucket.

### Round 2 — FAIL, one blocking

Round 1's two blocking findings were confirmed repaired. The new one was the
same class, in the place the repair's centrepiece was built to make impossible.

Both readers parsed nested-library declarations in full and both then discarded
every row except the identity. **58 declared dependencies were read by both and
dropped by both**, with no node, no edge and no diagnostic, across 31 of the
nested libraries. The second reader was not a second opinion: it was the same
rule written twice, once in Python and once in awk. The cluster partition was
wrong by one, and the workspace's own prose claimed one node per declared mod
and one edge per declared dependency.

Five non-blocking findings, including three unbound statements in the provenance
record, one of which could be made to contradict the milestone's central privacy
claim with the whole gate green.

### Round 3 — FAIL, one blocking

Round 2's repair held; all six headline figures reproduced exactly against a
reader written from scratch. The new finding was the fourth instance, one level
up from round 2's third: not what a nested declaration says, but **which nested
libraries exist at all**.

Both readers enumerated nested libraries from exactly one manifest and returned
nothing when it was absent. A library an artifact ships but does not list there
is read by neither reader, accounted for by neither ledger, and invisible to
every check. The fixture contains one today, so this was live rather than
hypothetical, and the reviewer proved no check could see it by mutation rather
than by argument.

Four non-blocking findings, including a ledger whose count came from the
reader's own report of what it chose to return, so a drop inside the parser
balanced it; a verification script that printed numbers it never checked; and
nested walking limited to one level in both readers, undisclosed.

### Round 4 — FAIL, one blocking

Round 3's repair held: its figures reproduced, its three newly found archives
were the only three in the fixture, and its new accounting caught every
discovery narrowing the reviewer could invent.

The fifth instance was one file over. Round 3's repair had built a depth-aware
listing, produced twice and compared byte for byte — and **no consumer that
counts a behaviour surface read it**. The catalogue still stopped at the outer
archive, producer and independent recount alike, the recount reading the same
listing the model used while its own comment said it recounted from the
archives. **12,571 class files inside nested libraries were counted by nothing**,
and ten counterpart plans carried the wrong size as a result.

Six non-blocking findings, including a derivation list bound by nothing that
could contradict the recorded implementation method without the gate noticing,
and an upstream identifier appearing verbatim in this public repository that no
check would catch.

### Round 5 — FAIL, two blocking

The round-4 repair's arithmetic was sound. Every committed figure re-derived
from readers the reviewer wrote, the depth defect was genuinely fixed, and the
recount genuinely walked the archives — the reviewer tried to make the model and
the recount agree while both were wrong about depth, and could not.

**B1 — a family is not a verdict.** Round 4 had recorded the licence-family
decision in a tracked table. The function immediately below it turns a family
into a rights verdict, is run by the producer and again by the consumer, and
nothing recorded what that mapping was supposed to be. **One line changed there
moves 65 artifacts from source-prohibited to source-cleared and 65 counterpart
plans from black-box respecification to reading the source** — no family moves,
so the table stays silent, and the gate exits 0. The judgement that has to be
recorded is the one that decides the consequence, not the one nearest the input.

**B2 — a table of filenames is not a claim about code.** The mechanism built to
close round 4's finding bound the recorded reading claims to a hand-kept table
keyed on filenames. It did not bind them to what the code does. The reviewer
made a producer read and write out the bodies of **1,113 class files** and the
gate exited 0, still asserting metadata-only, nothing decompiled, no restricted
content read, and an unchanged reading list.

Five non-blocking findings, including a figure correction of one texture counted
by neither reader because both classified the file extension case-sensitively —
the same question asked twice by one hand. This called itself the campaign's only
figure correction until round 7, which the section three above it contradicts: the
round-4 finding moved 12,571 class files and the size band of ten plans. Corrected
headline figures are the ordinary outcome of a round here, not the exception, and
a sentence calling one of them unique was the campaign reading better than it was.

### Round 6 — FAIL, one blocking

Every committed number re-derived. The reviewer wrote two readers and reproduced
the graph, all thirteen surface families including round 5's correction, the
licence table, the memory bands and the plan counts exactly — not one
disagreement in the whole inventory, where round 5 had one. The arithmetic, the
licence tables and the asset restatement were all sound and could not be broken.

The ninth instance is in the newest machinery. Round 5's B2 was closed by
building an observation that wraps the channels by which archives are opened and
classifies every read, so a claim about reading is compared against what was
seen. The file installing those wrappers declares that there are **exactly two
ways to open an archive in that workspace** and that a reader added anywhere is
therefore **seen by construction**. Nothing asks whether that premise is true.

It is not. The reviewer injected a reader into a producer that opens each of the
235 artifacts with the builtin and inflates their class entries with the
standard compression module — standard library throughout, no wrapped channel —
and it **decompressed 50,357 class files and wrote their bytecode to disk while
the whole gate exited 0**, printing that six producers were watched, 1,448,401
archive reads were classified and zero entry bodies were read. The observation
re-ran that same producer inside its own sandbox and did not see it. A second
probe showed that starting the interpreter without site initialisation also
defeats the wrapper.

That is round 5's B2 reproduced one layer past the mechanism built to prevent
it, and it is worse: B2 needed a table to stay silent, this needs nothing at
all — one edit to one producer, no table, no record, no vocabulary entry. What
is watched is not "the standard library"; it is four methods of one
standard-library class.

The finding names this milestone's evidence as the thing that fails: an evidence
manifest here would record a green gate as proof of a sentence the gate does not
hold. That is why this record and the manifest say `FAIL`.

**The repair, and what it does not claim.** The premise now gets a second
reading instead of an assertion. A check computes what the gate actually
reaches, sweeps that closure for read primitives, and requires every occurrence
to be routed through a watched channel or accounted for by name with a reason.
The vocabulary behind it is congruent three ways: each declared pattern must
match the probe recorded beside it, an accounted primitive that nothing uses is
stale and fails, and the occurrence table and the vocabulary must name the same
primitives in both directions. The rule earned itself immediately by failing its
own author's tree over a reason too short to be a reason, which was rewritten
rather than having the threshold lowered.

**A tenth instance was found and closed by the repair itself, unprompted.** The
scan reads static text, so it could account for an interpreter invocation while
being unable to resolve the variable naming the script it was pointed at — the
same shape one level down. The interpreter shim now records the script it was
given and requires it to be a scanned file in the checked tree. That is closed
at run time rather than disclosed.

Twelve mutations across seven channels now exit nonzero, including both of the
round's own probes. **One exits zero and is disclosed rather than hidden**: a
fully coordinated hand that deletes a primitive's refusal row, injects the
reader, and writes its occurrence rows in the same edit passes, because the
accounted half is a table and a table records what it is told. It leaves a tell
rather than a failure — the gate's own line visibly reports one fewer refused
primitive than it should — and the repair explicitly does not claim it as
caught. The gate prints what it cannot reach on every run, pass or fail, ending
with the honest sentence: this catches an accident and a careless hand, and does
not catch a determined one. Emptying that disclosure is itself a failure.

Whether that is enough is the next round's question, not this record's.

Five non-blocking findings, including two prose counts that had gone stale and a
gate line hardcoding the number it reports.

Round 6 also **seconded the licence disclosure**: it went looking independently
for a second reading of the licence judgements — through a schema enum, through
a restatement in another document, through an adjacent column, through a reason
column — and found that each either follows the mutated value or constrains only
the vocabulary the value is drawn from. It reports finding no second reading
that the workspace had missed. That is the strongest available support for
treating the licence decisions as judged rather than measured, and it is why
`docs/decisions/claims-without-a-second-reading.md` exists.

### Round 7 — FAIL, five blocking

The first round to read the public evidence record itself rather than the
inventory behind it, and every one of its findings is about this repository's
own prose and machinery.

**B1.** The read-primitive scan counted with a line-counting flag while the
table beside it called its rows uses. Two lines under the private workspace's
checks carry two occurrences each, so the published `accountedUses` was 74 where
the tree held 76 — and the occurrence table had been generated from that same
miscount, so table and scan agreed and no row disagreed with either. The
eleventh instance of the class, inside the machinery the round-6 repair built to
catch the ninth.

**B2.** The manifest's digest deviation said "the twelve digests under
`boundArtifacts`". There are fifteen.

**B3.** The same deviation presented a closable limit as a necessary one. It
said the digests bound for a reader with the private workspace and for nobody
else, and that recomputing them would mean this repository reaching into a
sibling tree it may not name. The dependency runs the other way and always did:
the private workspace already reads this checkout in eight places and hashes
files, so the reader the sentence said existed for nobody was its own gate.

**B4.** M5-W1 closed on a completeness claim the tree falsifies — the tracked
content sweep called its two rules "the two shapes a leak could arrive as",
which the record's own history contradicted.

**B5.** This record published two false figures about itself: nine blocking
findings where its headings summed to eight, and a claim that one correction was
the campaign's only figure correction, contradicted three sections above it.

All five were repaired. The repair rebuilt the occurrence count from matches
rather than lines, closed B3 from the private side rather than rewording it, and
wrote the provenance of the wrong figure into the table that carried it.

### Round 8 — PASS, zero blocking

The closing certification: a reader over the repaired tree, deliberately scoped
to confirming round 7's five findings closed rather than opening a fresh hunt,
because seven rounds of hunting had reached the point where each new round
found the class in the machinery the last repair built and the campaign's cost
had stopped being proportionate to what it was still buying.

All five confirmed closed, each by reading or running the mechanism rather than
the sentence describing it. The digest bind of B3 was confirmed to execute and
to cover every entry, and afterwards proved by mutation: falsifying one
published digest makes the private boundary check exit 1 naming the file whose
digest no longer matches, and restoring it returns exit 0 with the tree
byte-identical. The round looked specifically for a twelfth instance in the
repair's own counting machinery and found the shape present but disclosed, which
this record's stopping condition makes non-blocking.

Two non-blocking observations, both recorded below: a too-narrow pattern for a
refused primitive is indistinguishable from a clean workspace, and the private
boundary check prints its bound count rather than comparing it.

The round did not run the full private gate, spending two narrow checks instead,
and did not re-derive any inventory figure. Both gaps are its own disclosure,
not a discovery of this record.

## What is weakest here

An honest account of where a reader should not extend their confidence. This is
the most useful section in this file.

1. **Nothing here knows independently how many rounds ran, or what they found.**
   This document is the only record of the campaign in this repository, and the
   check that holds its tally honest compares the summary to the headings in the
   same file. That caught a wrong total and cannot catch a missing round: rounds
   7 and 8 went unwritten here while both had run, and the stated count and the
   derived count agreed at six the whole time. A heading is now required to
   carry a blocking count, so a round written in cannot slip out of the sum —
   but a round never written in at all remains invisible, and the verdicts
   themselves are held where restricted review material is held. A reader is
   trusting that the sections below are all of them.
2. **`metadata-only` is held by a second reading with one disclosed way past
   it.** A coordinated hand that deletes a refusal row and writes the matching
   occurrence rows in the same edit passes, leaving a visible tell rather than a
   failure. The mechanism says so about itself, on every run, and this record
   repeats it rather than letting a green gate imply more. What changed is real
   — the premise is now checked instead of asserted, and a tenth instance was
   closed on the way — and it is still not a claim that a determined hand is
   caught.
3. **The rights figures rest on judgements no measurement can falsify** — the
   licence family, the source-use verdict a family carries, and whether an
   author reserved their assets apart from their code. Two independent reviewers
   searched for a second reading and neither found one. A hand changing the
   deciding code and the recording table together passes every check in both
   repositories. The tables moved the decision into a small diff a reviewer
   reads; they did not make it checkable.
4. **The public gate re-takes no measurement and recomputes none of the digests
   that bind it.** It checks that the summary record's parts sum to its totals
   and that the manifest's figures follow from those parts. The digests over
   private files are recorded and nothing here hashes them — falsifying one and
   rebinding the record's own digest leaves `ravel:verify` at exit 0, which was
   measured rather than assumed. That is a limit of this gate and it read as a
   limit of the system until round 7: the private gate holds both trees, and it
   now recomputes all fifteen digests against the published record on every run,
   so the two cannot drift apart. Neither side recomputes a figure the other
   produced, and a hand that moves a file and rewrites the record together still
   passes.
5. **This record is sanitized**, so a reader is taking its account of eight
   rounds on trust rather than reading the verdicts. See the section above for why, and
   treat the round descriptions as a reviewer's findings restated by a different
   hand.
6. **The graph's blocked nodes are unknowns, not zeros.** Those artifacts
   declared no dependency metadata, so their edges are absent because nobody
   knows them, and any reasoning about cluster boundaries inherits that.
7. **235 of 235 planned is coverage of artifacts, not of behaviour.** Nothing in
   M5 ran the fixture. The surface counts are counts of entries in archives.
8. **Rounds ran invalid probes and this record used not to say so.** Round 5
   read a gate that never ran as a green baseline and built a probe on top of
   it; round 7 stopped a licence probe one rebuild short and called its own
   result incomplete rather than a finding. Both were caught by the reviewer who
   made them, which is the reason to record them: a campaign in which reviewers
   repeatedly mistook a non-running gate for a green one is a fact about how far
   these verdicts should be trusted, and leaving it out made the section above
   read cleaner than the work was.
9. **Two limits stayed open across every round and are disclosed rather than
   closed**: whether the two directories both readers treat as the nested-library
   locations are the complete set a loader loads from — the second opinion there
   is a loader, and this workspace does not run one — and whether a declared
   licence string tells the truth, which nothing here opens a licence file to
   check.

## Open limits carried forward

- **The class is disclosed, not closed.** Recorded as a deviation in
  `evidence/m5/manifest.json`. A later milestone must not read M5's eventual
  closure as the class being solved, and should expect the next instance inside
  whatever M5 built last.
- **The round that reads the post-repair tree has run**, and is round 8 above.
  It is the round that closes this milestone, and it is also the narrowest round
  of the campaign: it certified the repair rather than re-deriving the
  inventory, and it says so itself. A later milestone reading this record should
  weigh round 8 as a certification and rounds 1 through 6 as the derivations.
- **The completeness obligation now has a decision record.** A mechanism that
  claims to observe exhaustively must either verify its own premise by a second
  reading or state, in the file that acts on the claim and in what the mechanism
  prints, what it does not catch. The repair does both. Whether it does them
  well enough is the next round's question.
- **Two operational hazards, reported by the repair against itself**, recorded
  here as fair warning to whoever runs this next. The tracked-file comparison is
  racy under concurrent version-control access: two gate runs at once both
  snapshot the working copy, and one reported a tracked file as missing that was
  not. Three sequential runs pass and an independent directory walk agrees
  exactly, so it is an operational hazard rather than a defect — but it means a
  single red run under concurrency is not evidence of anything. And a process
  wait loop matched its own command line, so the command clearing it killed the
  gate mid-run. Neither is a finding about the inventory; both will waste
  somebody's afternoon if they are not written down.
