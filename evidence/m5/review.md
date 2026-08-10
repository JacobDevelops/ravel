# M5-W8 Independent Inventory Review

Result: FAIL

**Six independent rounds have run and every one of them returned FAIL.** Between
them they raised nine blocking findings, and all nine are instances of a single
defect class. The most recent round returned FAIL on the ninth, which is the
most serious of the campaign: it falsified the premise under the mechanism that
licenses the whole inventory. A repair is in progress and no round has read the
tree it leaves, so the line above is `FAIL` and `evidence/m5/manifest.json`
records the same under `review.result`. `verify_recorded_verdict` in
`tools/ravel-xtask/src/lib.rs` requires the two to agree, so they move together.

A pass certifies the tree its reviewer read, with nothing landed on top of it.
The next round is what can close this milestone, over the tree the current
repair leaves. `evidence/m4/review.md` states the same rule and stayed `FAIL`
between its own rounds for the same reason.

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

**Nine instances across six rounds**: a hand-rolled declaration parser shared by
both readers; a provenance record bound to nothing it asserted; a shared rule
discarding nested declarations; a shared discovery rule for which nested
libraries exist at all; a shared depth rule stopping the behaviour catalogue at
the outer archive; a shared licence-family rule; the family-to-status rule one
function below it; a derivation list keyed on filenames rather than on code; and
finally a completeness premise about which channels can open an archive.

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
independently, mutated the workspace on a throwaway copy rather than in place,
and confirmed both working copies unchanged at the end.

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
reader written from scratch. The new finding was the third instance, one level
up from round 2's: not what a nested declaration says, but **which nested
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

The fourth instance was one file over. Round 3's repair had built a depth-aware
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

Five non-blocking findings, including the campaign's only figure correction: one
texture counted by neither reader because both classified the file extension
case-sensitively — the same question asked twice by one hand.

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

## What is weakest here

An honest account of where a reader should not extend their confidence. This is
the most useful section in this file.

1. **The figures this milestone publishes are provisional while a repair is in
   progress.** The workspace they come from is being rewritten to answer round
   6, and the read-observation figures in particular will change. The manifest
   marks them as provisional rather than presenting them as settled.
2. **`metadata-only` is not currently an established property.** It is the
   sentence that makes opening source-prohibited artifacts lawful, and round 6
   showed the mechanism asserting it does not hold it. Until a round certifies
   the repair, nothing in this repository should be read as evidence for it, and
   the manifest says so in a deviation rather than in a footnote.
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
   measured rather than assumed. They bind for a reader holding the private
   workspace and for nobody else.
5. **This record is sanitized**, so a reader is taking its account of six rounds
   on trust rather than reading the verdicts. See the section above for why, and
   treat the round descriptions as a reviewer's findings restated by a different
   hand.
6. **The graph's blocked nodes are unknowns, not zeros.** Those artifacts
   declared no dependency metadata, so their edges are absent because nobody
   knows them, and any reasoning about cluster boundaries inherits that.
7. **235 of 235 planned is coverage of artifacts, not of behaviour.** Nothing in
   M5 ran the fixture. The surface counts are counts of entries in archives.
8. **Two limits stayed open across every round and are disclosed rather than
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
- **A round that reads the post-repair tree is owed.** Until one runs, the
  verdict above is `FAIL` and `ravel:verify-milestone:m5` exits 1, which is a
  true statement about an open milestone rather than a broken repository.
- **The completeness obligation now has a decision record.** A mechanism that
  claims to observe exhaustively must either verify its own premise by a second
  reading or state, in the file that acts on the claim and in what the mechanism
  prints, what it does not catch. Whether the repair satisfies that is the next
  round's question.
