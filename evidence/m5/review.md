# M5-W8 Independent Inventory Review

Result: FAIL

**Round 1 read the inventory as first built. Round 2, Round 3 and Round 4 each
returned FAIL on a blocking finding that falsified a published figure. Round 5
returned a verdict whose findings were restructurings rather than falsifications
— it changed no published number except one off-by-one. Round 6 returned FAIL on
the most serious finding of the campaign, and the repairs answering it are a
disclosure rather than a fix.** No round has read the tree those repairs left, so
the line above is `FAIL` and `evidence/m5/manifest.json` records the same under
`review.result`. `verify_recorded_verdict` in `tools/ravel-xtask/src/lib.rs`
requires the two to agree, so they move together.

A pass certifies the tree its reviewer read, with nothing landed on top of it.
That is why this line is not `PASS`: Round 6's finding was answered after Round 6
finished, and answering it is exactly the kind of change a verdict cannot be
carried forward across. M4's record states the same rule and stayed `FAIL` for
the same reason between its Round 2 and its Round 3.

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

### How this file differs from M3's and M4's, stated up front

`evidence/m3/review.md` and `evidence/m4/review.md` reproduce every verdict
verbatim. **This one does not, because there is nothing to reproduce.** M5's
rounds ran against a separate private workspace and no transcript of any of them
was written into either repository — not here, not under `evidence/private/`, not
in the private workspace itself. What exists is the record the repairs left: the
private workspace's own prose says what each round found, in the sections the
finding forced to be rewritten, and the commits answering them are in its
history.

So this file is a **history assembled at closure from that record**, not a
transcript, and it says so rather than presenting a summary in a transcript's
clothes. That is itself a finding about the campaign and it is the first entry in
"What is weakest here". M3 lost two verdicts the same way and recovered them by
luck; M5 did not lose them, because they were never written down at all.

## The defect class

Every blocking finding in this campaign is one shape, and naming it is more
useful than listing the instances:

> **A producer and its checker share a rule, so they agree, and the rows that
> would contradict them are never built.**

The checker is not a second reading when it asks the same question the producer
answered. It re-derives the same value from the same rule and confirms it, which
is one reading written twice. Whatever the shared rule is wrong about is
invisible: no row exists that disagrees, because the rule that would have built
such a row is the rule under test.

**Eight instances**, across the rounds recorded below: a hand-rolled parser
shared between producer and checker; a shared row filter; a shared discovery
rule; a shared depth rule; a shared licence-family rule; a shared
family-to-status rule; shared asset-reservation rules; shared port-lane
thresholds.

The pattern across the campaign matters more than any single instance. Each
repair fixed its instance by giving the checker a genuinely independent path —
walk the archive instead of reading a filename, re-sum the rows instead of
trusting the total, watch the standard library instead of believing a table of
phrases. **And each time, the next round found the class living inside the
machinery that had just been built to catch its predecessor.** The shared rule
moved up a level; it did not go away.

## Why this milestone does not claim the class is closed

The loop is unbounded by construction. Every repair adds checking machinery, and
the machinery is written by the same hand, from the same understanding, as the
thing it checks — so it carries a premise, and a premise written once is the
defect in its next form. Waiting for a round that finds nothing means waiting for
a round that fails to look hard enough.

So the condition this campaign used to stop is not "a reviewer finds nothing". It
is:

> **No published figure is falsified.**

That is a weaker claim than it sounds and it is meant to be read as such. It says
the numbers this repository publishes survived six rounds of people trying to
break them, and the last three attempts to break a *figure* failed. It does not
say the class is gone, and Round 6 is the proof that it is not: the class
reappeared one level above the figures, in the premise of the mechanism that
licensed taking them at all.

`docs/decisions/claims-without-a-second-reading.md` is where that generalisation
is decided rather than restated here.

## The rounds

### Round 1

Read the inventory as first built, over the pinned fixture's references, graph,
surfaces, rights and plans.

### Round 2 — FAIL, blocking

Falsified a published figure. The producer and the checker shared a hand-rolled
parser for declared metadata, so both read the same subset of declarations the
same wrong way, and the artifacts whose declarations were written in a spelling
neither handled were counted as declaring nothing. The checker agreed because it
was the same parser.

Repair: every spelling of a dependency declaration is read, and read twice by
paths that do not share the reader.

### Round 3 — FAIL, blocking

Falsified a published figure. A shared row filter and a shared discovery rule.
The rule that found a shipped library inside an artifact was written once and
consulted by both sides, so a library the rule could not find was absent from
both the count and the check of the count. The nested-library figure was wrong
and nothing could disagree with it.

Repair: the shipped library is found from the archive's table of contents rather
than from its manifest, and counted against the artifact that ships it; the
recount walks independently.

### Round 4 — FAIL, blocking

Falsified a published figure. A shared depth rule, and the first of the licence
findings: the rule deciding a licence family lived in a regex consulted by the
producer and by the checker, so a reclassification was invisible and a
misclassification was unanimous.

Repair: the licence family became a tracked table of one row per distinct
declared string, and the depth rule was given a second walk that checks where the
producer stopped rather than what it counted.

### Round 5 — no figure falsified

Two restructurings and one off-by-one. It changed no published number except that
one, and it is recorded here rather than folded away because its two findings are
the most instructive in the campaign:

- **A family is not a verdict.** Round 4 had written down the family decision, and
  Round 5 found it was the least valuable of the three available recordings. The
  function turning a family into a rights verdict sat one level below the family
  test and was still written once. Changing one line in it moves 65 artifacts
  from source-prohibited to source-cleared, and 65 counterpart plans from
  respecification to reading the source, **without moving a single family** — so
  the family table stays silent and the gate exits 0. The judgement that has to be
  recorded is the one that decides the consequence, not the one nearest the input.
- **The derivation list was a chain of filenames.** Each record declared what its
  producer read, derived from a vocabulary keyed by check name. Nothing read a
  check's body, so a reader of 1,113 class-file bodies injected into a producer
  changed no vocabulary entry, no record and no exit code: every record still said
  `decompiled: false`, `restrictedContentRead: false`, `implementationMethod:
  metadata-only`. A phrase is a claim about code, and only the code answers it.

The off-by-one: an icon whose suffix was written in upper case was counted by
neither reader, because both suffix tests were case-sensitive — the same question
asked twice by one hand.

Repair: the read observation was built. Rather than declaring what a producer
reads, it wraps the channels by which archives are opened and classifies every
read that comes back, so a record's claim is compared against what was seen.

### Round 6 — FAIL, blocking

**Broke the premise underneath the repair Round 5 forced.** The read observation
wraps four methods of Python's `zipfile` and shims the `unzip` binary. The file
installing those wrappers asserted that they were the only two ways to open an
archive in that workspace, and that a reader added anywhere would therefore be
seen by construction.

The reviewer added a reader that used only the standard library and none of the
wrapped channels: it opened all 235 artifacts with the builtin, walked the local
file headers with `struct`, and inflated **50,357 class files** with `zlib`. The
gate exited 0 while printing that zero entry bodies had been read, and the
observation re-ran that same reader inside its own sandbox and still did not see
it.

This is the most serious finding in the campaign, because the sentence it
falsified — that the method is metadata-only — is the sentence that makes reading
235 restricted-licence artifacts a lawful thing to have done. It is also the
clearest statement of the class: not a shared rule about the data, but **a shared
rule about what the mechanism can see**, written once, by the hand relying on it.

Repairs so far, and they are a disclosure rather than a fix:

- The private workspace's prose now states the bound instead of the completeness
  claim, and the count of entry bodies is computed rather than a literal zero.
- `docs/decisions/claims-without-a-second-reading.md` decides the general rule: a
  mechanism may not assert its own completeness, and either the premise gets a
  second reading or the mechanism states its actual bound in the file that acts
  on the claim and in what it prints.
- `evidence/m5/inventory-summary.json` carries that bound beside the figure it
  qualifies, so a reader of the public record meets the limit where they meet the
  number.

**What is not repaired:** the file installing the wrappers still asserts its own
completeness, in the two forms the decision record names as inadmissible — that
there are exactly two ways to open an archive there, and that a reader added
anywhere is seen by construction. An accepted decision record forbids that
sentence and the file still carries it. No round has read the tree these repairs
left.

## What is weakest here

An honest account of where a reader should not extend their confidence. This
section is the most useful thing in this file.

1. **No verdict in this campaign was written down as it was returned.** Every
   round above is reconstructed from the repairs it forced and the prose it made
   somebody rewrite. That is a weaker record than M3's or M4's by a wide margin,
   and it means a round whose finding was answered without leaving a mark on the
   prose is a round this file cannot see. The count of rounds is as reliable as
   the workspace's own README; the content of each is an inference from its
   repair.
2. **The rights figures rest on judgements no measurement can falsify.** Three of
   them — the licence family, the source-use verdict a family carries, and
   whether an author reserved their assets apart from their code. A hand that
   changed the deciding code and the recording table together would pass every
   check in both repositories. The tables moved the decision into a small diff a
   reviewer reads; they did not make it checkable, and the deviation in
   `evidence/m5/manifest.json` says so in those words.
3. **The metadata-only claim is bounded, not proved.** See Round 6. What the
   figure `0 entry bodies read` means is: through the two watched channels, over
   the six producers that were run. It does not mean no entry body was read.
4. **The public gate cannot re-take any measurement in this manifest.** It checks
   that the summary record's parts sum to its totals, that the manifest's figures
   follow from those parts, and that the private files hash to what is recorded.
   A figure that is wrong in the private inventory, and consistently wrong,
   survives all three. There is no public second reading and there cannot be one
   while the material stays private.
5. **The graph's 7 blocked nodes are unknowns, not zeros.** Those artifacts
   declared no dependency metadata. Their edges are absent from the 703 because
   nobody knows them, and any downstream reasoning about cluster boundaries
   inherits that.
6. **235 of 235 planned is coverage of artifacts, not of behaviour.** Nothing in
   M5 ran the fixture. The surface counts are counts of entries in archives.
7. **One repository reviewed itself.** Every round was an independent reviewer of
   the work, but every round also worked inside the same two repositories with the
   same tools, and the class this campaign is about is precisely the class that
   shared context produces. A reviewer who had never seen either tree has not
   looked at this.

## Open limits carried forward

- **The class is disclosed, not closed.** Recorded as a deviation in
  `evidence/m5/manifest.json`. A later milestone must not read M5's closure as
  the class being solved, and should expect the next instance to be inside
  whatever M5 built last.
- **The completeness assertion in the private workspace's wrapper file.** It
  contradicts an accepted decision record. It is not something this repository
  can repair and it is named here so that it is not lost.
- **A round that reads the post-Round-6 tree is owed.** Until one runs, the
  verdict above is `FAIL` and `ravel:verify-milestone:m5` exits 1, which is a true
  statement about an open milestone rather than a broken repository.
