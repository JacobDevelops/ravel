# Claims without a second reading

Status: accepted before M5-W8, for every repository and workspace inside this
project's boundary. It is not an architecture decision gate and carries no ADG
number: `SPEC.md` §15 owns those, and its unused numbers are reserved for subjects
this record is not about. Decided without a measurement, for the reason the
decision itself gives — the class of claim it governs is the class nothing
measures — so under its own rule that sentence belongs in the `Status` line rather
than in a footnote. The instances that forced it are the licence triage in M5's
private compatibility inventory and the read-observation mechanism built to
protect it.

## The problem this record exists for

Every other figure Ravel publishes is falsifiable by a second reading. A count of
class files is wrong if a recount disagrees. A graph's edge total is wrong if the
edges are re-summed. A manifest's coverage figure is wrong if the gate's own
output says something else. In each case the producer computes a number, a
different piece of code computes it again from a different starting point, and a
falsified producer is caught because the two disagree.

Independent review rounds over M5 each found the same defect wearing a new coat,
and the shape was always this: *a producer and its checker shared a rule, so they
agreed, and the rows that would have contradicted them were never built*. The
repair each time was to give the checker a second, independent reading — walk the
archive instead of reading a filename, re-sum the rows instead of trusting the
total, observe what was read instead of believing a vocabulary table. Each repair
closed its instance, and the class reappeared one step over, inside whatever had
just been built to catch its predecessor.

Two situations defeat that repair outright, and they are what this record decides.

**A judgement with nothing to measure.** Nothing about a licence is measurable
from the artifact it governs. A declared licence string is a name; what that name
permits is a reading a person took. There is no second reading to take, because
there is no measurement to repeat. A hand that changes the code deciding what a
licence family permits, and changes the table recording that decision, has moved
the answer for every artifact in the family and no gate anywhere can contradict
it — because there is no fact in the fixture the new answer disagrees with. Two
reviewers independently went looking for a second reading, through a schema enum,
through a restatement in another document, through a reason column; each candidate
either follows the mutated value or constrains only the vocabulary the value is
drawn from. The boundary is real and irreducible.

**A check that asserts its own completeness.** A mechanism was built to observe
rather than declare that no restricted content had been read. It wrapped the
channels by which archives are opened and classified every read, and it declared —
in the file that acts on the claim, and nowhere else — that those were the only
channels, and that a reader added anywhere in the checks tree would be seen by
construction, "because what is watched is the standard library rather than the
caller". A reviewer then added a reader that used only the standard library,
through a channel that was not wrapped. It decompressed the whole archive's entry
bodies and wrote the bytecode to disk. The gate exited zero while printing that no
entry bodies had been read, and the observation re-ran that same reader inside its
own sandbox and still did not see it. What was watched was four methods of one
standard-library class, not "the standard library".

The premise of a completeness claim is itself a claim, and this one was written
once, by the hand relying on it. That is the same defect one level up: not a
shared rule about the data, but a shared rule about what the mechanism can see.

Licences and that mechanism forced this record. They are not its scope: the shape
appears wherever a human judgement is recorded as data and then consumed as if it
were a measurement, and wherever a check decides for itself how much it sees.

## Decision

**A claim that no second independent measurement can falsify is governed by review
of a recorded decision, not by a gate. The decision is recorded as data, one row
per decision, each row carrying its own reason. The gate's job is to make the
recorded decisions and the behaviour exactly congruent — in both directions — so
that changing an answer requires editing a row a reviewer reads.**

Every claim this project acts on is therefore of one of two kinds, and **which
kind it is has to be legible from the artifact that carries the claim**, not from
the reader's knowledge of how the claim was produced. A claim is **measured** when
some path independent of its producer re-derives the same fact; whether that holds
is settled by mutation, by breaking the producer and requiring a nonzero exit. A
producer and a checker that share the rule are one reading written twice, and they
are not this. A claim is **judged** when no such path exists.

A surface carrying judged claims must satisfy all four of these:

1. **The judgement is data, not control flow.** Every distinct input a judgement
   is taken over gets a row in a tracked table, and the row names the verdict. A
   judgement expressed only as a branch, a regex, or a threshold inside a producer
   is not recorded: it is executed. The judgement that has to be recorded is the
   one that decides the consequence, not the one nearest the input.
2. **Each row carries its reason, and its basis, in the row.** A verdict with no
   reason beside it is not reviewable, because a reviewer cannot tell a decision
   from a typo. The row also declares that it rests on judgement and points at the
   record deciding it, because the reader who has to tell the two kinds apart is
   usually reading one row and not the document beside it.
3. **The tuples are congruent in both directions.** The set of (input, verdict)
   pairs the code produces must equal the set the table holds, checked as set
   equality rather than containment. Containment one way lets an unlisted verdict
   pass; containment the other way lets a stale row rot.
4. **The consequences of a verdict are restated independently of the producer.**
   Where a verdict has downstream effects that *are* measurable — a reserved asset
   must come out prohibited, a surface nothing ships must come out
   not-applicable — those are checked against measured counts the producer does not
   supply, so at least the mechanical part of the decision has a second reader.

And one prohibition, which is the same rule applied to a mechanism rather than to
a value:

**A mechanism may not assert its own completeness.** A check that acts on "these
are all the channels", "nothing else can reach this", or "anything added would be
seen by construction" has made a claim about what it can see, and that claim is
judged until something establishes otherwise. Either the premise gets a second
reading — something outside the mechanism derives the same set, and a disagreement
fails the gate — or the mechanism states its actual bound in the file that acts on
the claim *and in what it prints on a passing run*, naming what it watches rather
than the category that thing belongs to. *This catches an accident, not a
determined hand* is an admissible sentence. *Seen by construction* is not.

The first branch is open and is the better one where it can be built. A second
reading of a completeness premise looks like this: something outside the
mechanism derives the reachable set independently — a static closure over what
the check transitively reaches, swept for the primitives in question — and a
member of that set which is neither routed through a watched channel nor
recorded as an exception fails the gate. That is a genuine second reading and
this record licenses it. It does not discharge the rest: the list of primitives
swept for, and every recorded exception, are judged claims that inherit the four
requirements above, and the closure's own reach — what its resolution cannot
follow — is a completeness premise in turn, so it is disclosed under this same
prohibition rather than assumed total.

What this buys, stated exactly: a recorded decision cannot be changed by a regex
nobody diffs. It arrives as a tracked edit to a short table, with a reason beside
it, where a wrong row reads as self-evidently wrong to a human who knows the
subject. That is a change in **who can audit the claim and how cheaply**. It is
not a change in whether the claim is verified.

## What this does not buy, stated plainly

It does not make the claim checkable, and this record refuses to imply otherwise.

A table records a decision; it does not tell a right decision from a wrong one. A
hand that edits the producer and the table together has changed a recorded
decision in the open, and every gate will pass. The four requirements move the
falsification from a machine to a reviewer; they do not perform it. Anyone reading
a figure that rests on such a claim is reading a reviewed judgement, and the
confidence available in it is the confidence available in the review — a different
and smaller thing than the confidence available in a recomputed digest.

**An unbound claim that looks bound is worse than one that looks unbound.** The
earlier version of the completeness claim was an unbound assertion everybody knew
was unbound, and it was read accordingly. Its replacement read as proof, so
nothing downstream of it was read sceptically, and a gate that exited zero while
missing an entire archive is what that cost. The disclosure obligation is not
decoration on a working mechanism; it is the part that keeps a reader calibrated
when the mechanism is wrong.

This is the honest boundary and it is not engineered around. Two consequences
follow and are accepted:

- **A milestone resting on such claims cannot close by declaring the class
  closed.** It closes by disclosing the class, naming its instances, and stating
  the termination condition it actually used.
- **The condition for stopping is "no published figure is falsified", not "no
  reviewer finds anything".** Each repair to the shared-rule class adds checking
  machinery, and the following round finds an instance of the class living inside
  the new machinery. That loop does not terminate on its own, so a milestone that
  waits for a clean round waits forever.

## What a machine can hold, and what it cannot

**Enforceable, and gated.** That a judged claim carries its reason and its
declared basis; that the basis points at a record which exists and is accepted;
that the recorded tuples and the produced tuples are equal as sets in both
directions; that a verdict's measurable consequences match counts the producer did
not supply. These are presence, resolution and set-equality checks of the kind
`ravel-verify-scaffold` already performs, and they fail closed.

**Not enforceable, and stated here rather than gated.**

- **Whether a judged value is right.** No check derives a licence, and none will.
  A reviewer reading a small diff is the whole mechanism, which is why the diff
  being small is a property worth paying for.
- **Whether a declared basis is honest.** A check that a claim declares itself
  judged is a vocabulary constraint, and it agrees with whatever it is told — the
  same limit that defeated the schema enum, and it defeats this too. What the
  label buys is that a wrong one is a visible lie in a reviewable place rather
  than an absent thought.
- **Whether a disclosed bound is the real bound.** A gate can require a mechanism
  to state what it watches. It cannot compare that statement against what the
  mechanism actually watches without becoming a second implementation, at which
  point the second implementation carries the same premise. A reviewer who adds a
  reader outside the watched set is the only thing that has ever found this.

Writing any of the second list as though it were the first is the defect this
record exists to name, so it is named here rather than left to a gate that would
not hold.

## What this decides against

- **A schema enum, a restatement, or a reason column as the second reading.** An
  enum constrains the vocabulary a value is drawn from and admits every member of
  it, so a mutation that stays inside the vocabulary passes. A restatement in
  another document, an asset, or an adjacent column is one reading written twice:
  it follows the mutated value, because whoever falsifies the producer falsifies
  it in the same change. Both were tried on the licence instance by independent
  reviewers, and both are what proved the boundary real. The reason column is
  still required by requirement 2 — as review material, not as verification.
- **Deriving the judgement from something measurable anyway.** Every candidate
  proxy — string similarity between licence texts, a canonical identifier
  registry, an upstream metadata field — moves the judgement rather than removing
  it: something still has to decide the proxy is the right proxy, and that
  decision is a claim of exactly this class one level up. A proxy that looks
  measurable is worse than an admitted judgement, because it reads as a
  measurement.
- **Widening the verdict to "unresolved" whenever a reading was required.**
  Refusing to decide is itself a decision with consequences, and a licence that
  plainly withholds a use is not unknown. Collapsing readings into "we do not
  know" would make the record less true, not more cautious.
- **Letting the producer be its own record.** A comment in the producer naming the
  judgement satisfies none of the four requirements: it is not a set, nothing
  compares it to behaviour, and it moves silently when the code beside it moves.
  This is the completeness instance exactly — the premise lived in the file that
  relied on it and nowhere else.
- **A second implementation of the same judgement.** Two functions written from
  the same reading agree by construction, which is the defect class this whole
  record is downstream of. Duplication produces agreement, not independence.
- **Widening a watched set *by itself*, instead of disclosing the bound.**
  Watching more channels makes a mechanism catch more and does not change the kind
  of claim its completeness premise is: the wider set is still asserted by the hand
  that relies on it. This rejects widening offered *as* the discharge, not widening
  as such — a wider set derived independently and checked against the mechanism is
  the first branch above and is the outcome this record wants. What a given
  mechanism should watch is not this record's to decide.
- **A gate over the truth of a judgement.** Unbuildable, and writing an
  unenforceable rule as though it were enforced is the failure recorded above. The
  honest form is the split in the preceding section.

## The deviation-anchor rule this record makes mandatory

A milestone's evidence carries deviations, and a deviation is a claim about the
tree that no measurement takes — the same class. From **M5 onward**, one narrow
part of that class is pulled back into the machine:

> A deviation or risk naming a Rust symbol must carry a machine-checkable anchor:
> a claim of the form *symbol S has no caller outside path set P*, checkable by
> grep over the tracked tree.

The anchor is not the deviation and does not verify it. It is the one narrow claim
inside the deviation that a machine can hold, so that a deviation which has
silently gone stale — because the symbol it describes acquired a caller somewhere
the deviation never contemplated — fails a gate instead of ageing quietly into a
false statement.

The rule is enforced rather than written down. `ravel-xtask` requires an anchor
for any finding whose summary names an identifier the tracked Rust sources
actually define, and checks each recorded anchor by sweeping the tracked tree. The
detection floor is stated where it is implemented: a symbol whose name is a single
lowercase word indistinguishable from ordinary English is not detected, so the
rule catches deviations naming real internal symbols and does not claim to catch
every possible phrasing. That sentence is itself the disclosure this record
requires of a mechanism that would otherwise imply completeness.

It is mandatory from M5 forward and is not retrofitted to M1 through M4, because
rewriting a closed milestone's findings to satisfy a rule written after them would
change evidence a review already certified.

## Evidence

There is none to re-run, and that is the decision's own content rather than a gap
in it. This record is decided on an argument about what is measurable, and the
argument's premise — that nothing about a licence is measurable from the artifact
it governs, and that a mechanism cannot establish its own reach — is what a reader
checks by reading, exactly as [ADG-6](adg-6-configuration-ui.md) records a
property no measurement was addressed to.

What can be pointed at is the failure the requirements are drawn from, and it is
recorded rather than hypothetical. In M5's private workspace the family verdict
was written down after one round, and the next round found that recording it was
the least valuable of the available recordings: the function one level below the
family test turned a family into a rights verdict, was written once, and flipping
its all-rights-reserved line moved a whole block of artifacts from
source-prohibited to source-cleared, and their counterpart plans from
respecification to reading the source — without moving a single family, so the
family table stayed silent and the gate exited zero. Requirement 1 is that finding
generalised. The completeness instance above is the same failure taken one level
further, against the mechanism built to watch the first one.

The public consequence of that triage is carried in M5's evidence manifest and
reconciled by `devenv tasks run ravel:verify-milestone:m5`, which binds the
private tables by digest without disclosing them.

## Consequences

- Ravel publishes two kinds of figure and says which is which. A recomputed figure
  is falsifiable by anyone who re-runs the gate; a reviewed judgement is
  falsifiable only by someone who knows the subject and reads the row.
- Any surface adding claims of this class inherits the four requirements, and a
  surface that cannot satisfy them cannot carry the claim.
- A judged claim goes stale when what it was judged about changes. It travels with
  the identity of its subject, a changed subject requires the judgement to be
  taken again rather than inherited, and the record is amended rather than the
  value quietly edited underneath it.
- An independent reviewer's job differs by kind. On a measured claim the reviewer
  falsifies the producer and requires a nonzero exit. On a judged claim there is
  nothing to falsify, so the reviewer reads the diff and says whether it reads
  wrong, reads the disclosure and says whether it is the disclosure a hostile
  reader would write, and — for a completeness claim — tries to reach the subject
  through a channel the mechanism does not name.
- Milestone evidence gains an enforced anchor rule for symbol-naming findings, and
  a deviation that outlives its subject fails a gate rather than misleading a
  reader.
- The rights half of the M5 inventory is a reviewed judgement. No later milestone
  may treat it as a measurement, and a change to it is a reviewed edit rather than
  a recomputation.

## Limits of this record

- **No measurement decided this, and none could.** The generalisation from two
  instances to a rule is itself a judgement, made by the same kind of reading this
  record asks reviewers to perform. It is subject to its own rule and its `Status`
  line is where that is declared.
- **This is the level the defect had reached, not the last level.** Every earlier
  repair closed its instance and the class reappeared one step over, inside the new
  mechanism. This record moves the rule to where the premise is written; it does
  not prove there is no level above that one. The evidence that there might be is
  that the same thing has already been said at every level below.
- **The enforceable half is thin on purpose.** Set equality, presence and
  resolution are what a machine holds here. If a later round finds a genuine second
  reading for either instance, that instance stops being judged and this record
  should be amended to say so rather than kept for symmetry.
- **It does not cover claims that are merely unmeasured.** A fact nobody has got
  round to measuring is not a judged claim, and labelling it as one would be the
  same lie in the other direction. The remedy there is to measure it.
- **Two instances, one milestone.** Both came from M5's inventory work. Nothing
  here establishes the rate at which this class occurs elsewhere in the project,
  only that it survived repeated independent review in the one place it was looked
  for hardest.
