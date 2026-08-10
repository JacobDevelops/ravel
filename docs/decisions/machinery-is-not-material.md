# Machinery is not material

Status: accepted before M6-W2, for every repository and workspace inside this
project's boundary; amended before M6-W3 to decide what a material record may
hold, after M6-W2 shipped five of the six capabilities M5 filed as gaps and found
the frozen record could not say so. The amendment is recorded under "Amendment: a
material record carries a finding, not a lifecycle" below, and nothing about
which files are material changed. It is not an architecture decision gate and
carries no ADG
number: `SPEC.md` §15 owns those, and its unused numbers — ADG-8 for
official-server interoperability and ADG-9 for the official Minecraft host — are
reserved for subjects this record is not about. It is the second record in the
non-gate class opened by
[Claims without a second reading](claims-without-a-second-reading.md), and it is
decided by an argument rather than a measurement for the reason the argument
gives: what a bound digest means is a question about the record, and no run of
anything answers it. The instance that forced it is M5's closed evidence, which
froze the private workspace's `checks/` directory hard enough that M6 could not
add the enforcing check `ROADMAP.md` requires of it.

## The problem this record exists for

M5 published its inventory as aggregates over material a public tree may not
hold. To keep those aggregates honest, `evidence/m5/inventory-summary.json` binds
fifteen private files by sha256, and the private gate recomputes every one on
every run: a bound file that moves without its published digest moving fails
there. A count of the private gate's own files, `.primitiveScan.filesScanned`, is
held the same way.

That was the right instinct and it was applied to two different things at once.

Seven of the fifteen are **material**: the inventory rows, the provenance record,
the public-safe proposals. M5's published figures are counts *of those files*.
Move one and the figures describing it become false, and there is no story under
which that is acceptable. Eight are **machinery**: the private gate's own scripts
and the tables it reads. Nothing M5 published is a count of `verify.sh`'s bytes.
What their digests record is which gate produced the figures — a fact about the
tree M5 certified, at the moment it certified it.

Treating the second set as a live invariant has a consequence nobody chose. The
private gate fails whenever its own directory changes, so **adding a check to a
closed milestone's checking machinery breaks the closed milestone's record**.
M6-W6 requires a rewrite-integrity check that is "enforcing rather than
advisory", meaning a nonzero exit that fails the gate. It could not be added.
M6-W1's wave check was parked in a directory the gate does not call, and it said
so in its own header: the check exists, it is correct, and it runs only when
somebody remembers to run it. A milestone that cannot extend its own checking
machinery without falsifying an earlier record will accumulate parked checks, and
a parked check is an advisory check, and an advisory check is what `ROADMAP.md`
already refuses.

The defect is not the binding. It is that one mechanism was made to carry two
meanings, and the stricter of the two was applied to both because the record did
not say which was which.

## Decision

**A closed milestone's evidence may freeze material, but not machinery.**

Every artifact a milestone's evidence binds by digest declares, in the record and
per entry, which of two classes it is in. An entry with no class, or with a class
this record does not name, fails the gate. There is no default.

- **Material.** A file whose contents the published figures are figures *about*.
  Its digest is a permanent bind. Drift falsifies the published figures, so drift
  fails, and no later milestone may amend it. A material figure that has to change
  is a milestone that has to be reopened.
- **Machinery.** A file that is part of how the figures were produced or checked —
  the gate's scripts, the tables they read, and counts over them. Its digest, and
  any published figure that is a count of the machinery rather than of the
  material, records the tree at the earlier milestone's closure. It remains true
  of that tree. It is **not** an invariant of every later tree.

A later milestone that changes the machinery **amends the earlier record's
machinery figures, rebinds them, and records why in the earlier record**. It
never amends the material figures. The amendment is a tracked edit carrying the
amending milestone and its reason, and the gate requires that trail to exist and
to name only machinery entries — so amending a material digest under cover of an
amendment is a gate failure rather than a diff nobody read.

There is precedent for the shape. `evidence/m4/` was amended where later work
falsified what it said, in the open, with each amended entry left stating what it
closed on and naming the repair beside it. This record generalises that from a
one-off correction to a standing rule with a class attached.

### The class is data; the checker enforces consequences

This project's recurring defect is that *a producer and its checker share a rule,
so they agree, and the rows that would contradict them are never built*. A class
attached to a bound artifact is exactly the kind of thing that defect eats: if the
checker derived the class the same way the record did, the two would agree
whatever either said, and the label would be decoration.

So the class is written into the record, by hand, as data — and the checker does
not derive it. The checker reads the declared class, applies the consequence, and
**holds the declaration against a second and independent reading of the tree**:
whether the bound path is one the private gate's own scan enumerates as its
machinery. A material entry that turns out to be machinery fails. A machinery
entry that turns out to be material fails — which is the mutation that matters,
because that direction is the escape hatch, and an escape hatch that swallows a
material bind is the whole risk this record creates.

The two readings can disagree, which is the property that makes the check worth
running. Neither is generated from the other.

### What this does not buy

It does not make an amendment right. A hand that changes the machinery and
rewrites the machinery figures in the same edit passes every gate, and is meant
to — that is what an amendment *is*. What the gate holds is that the amendment
happened in the open, that it names a milestone and a reason, and that it did not
reach a material entry on the way past. Whether the stated reason is the real
reason is a judged claim under
[Claims without a second reading](claims-without-a-second-reading.md), and it
inherits that record's four requirements rather than escaping them.

Nor does the class label prove itself honest. The independent reading above
catches a label that contradicts the tree; it cannot catch a label that is wrong
about *why* a file matters. A tracked table with one row per bound artifact,
each carrying its class, is the reviewable surface, and a reviewer reading it is
the mechanism — the same admission the earlier record makes and for the same
reason.

## What this decides against

- **Freezing everything, and reopening a milestone to change a check.** This is
  what M5 did by omission and it is what parked M6-W1's check. A milestone's
  review certified a verdict about material; it did not certify that the gate
  which read that material may never gain a second reader. Reopening a closed
  milestone to add a check to a later one inverts the dependency order the roadmap
  is built on.
- **Freezing nothing, and dropping the machinery digests.** The machinery digests
  are not useless: they are how a reader of M5 learns which gate produced M5's
  figures. Deleting them would trade a rule that is too strict for a record that
  says less.
- **Deciding the class from the path in the checker alone, with nothing in the
  record.** Then the class is a rule the checker keeps privately, no reviewer sees
  it, and the record cannot be wrong — which is not a strength. It is the
  producer-is-its-own-record form the earlier decision already rejects.
- **Generating the class from the path and writing it into the record.** The
  reverse failure and the worse one: the record would look like a reviewed
  declaration and would in fact be a transcript of what the checker was about to
  conclude. Agreement by construction, which is the class of defect this project
  has spent eight review rounds on.
- **A per-entry `frozen: true|false` flag instead of a class.** It names the
  consequence rather than the reason, so a reviewer reading a row cannot tell a
  decision from a convenience, and nothing independent can contradict it. The
  class names what the file *is*, which is a fact about the tree and therefore
  falsifiable.
- **Letting a milestone amend its own material figures once closed.** A material
  figure that is wrong is a failed close, not an amendment. `evidence/m4/`'s
  amendment stands as precedent for correcting prose that later work falsified,
  and each amended entry there was left stating what it closed on for exactly this
  reason.
- **Treating the judged licence tables as material because they are tables.**
  `licence-families.tsv` and `licence-status.tsv` hold reviewed verdicts, and it is
  tempting to call a reviewed verdict material. They are machinery: they are read
  *by* the private gate to decide rights positions, no M5 figure is a count of
  their bytes, and their contents are governed by
  [Claims without a second reading](claims-without-a-second-reading.md), which
  already requires a reviewed edit to change a row. Classing them material would
  add nothing that record does not already require and would freeze the gate's own
  input.

## Evidence

There is none to re-run, and that is the decision's content rather than a gap in
it. What is bound and what that binding means are properties of the record; no
measurement is addressed to them, in the same way `ADG-6` records a property no
measurement was addressed to and `claims-without-a-second-reading.md` decides an
argument about what is measurable.

What can be pointed at is the failure the rule is drawn from, and it is recorded
rather than hypothetical: `waves/verify.sh` in the private workspace carried, in
its own header, the sentence that adopting it into the gate would rewrite a record
M5 froze — so the check that M6-W1 built ran outside the gate that would have
enforced it, by the author's own statement, for the whole of M6-W1.

The enforceable half is proved by mutation rather than asserted. A material entry
whose digest no longer matches must fail; a machinery entry whose digest no longer
matches must fail until the record is amended and pass after; an entry with no
class or an unknown class must fail; and an inventory file declared as machinery
must fail. Each of those is a nonzero exit from the check that reads the class,
recorded where the mutation was run.

## Consequences

- Milestone evidence that binds artifacts by digest declares a class per entry
  from M6 forward. `ravel-xtask` requires the field, requires it to be a known
  class, and requires the amendment trail to name only machinery entries.
- The rule is not retrofitted below M5. M1 through M4 bind no private artifact by
  digest, and rewriting a closed milestone's record to satisfy a rule written after
  it is what this record refuses in the other direction.
- M5's machinery figures become amendable, and M5's material figures do not. M5's
  review verdict is untouched: it certified a tree, and the amended record still
  says which tree that was.
- The private gate gains checks over time without a milestone being reopened, so
  "parked outside the gate" stops being the only way to add one. A check that
  belongs in the gate goes in the gate.
- A reviewer reading a bound-artifact row now has to check one more thing, and it
  is the cheap thing: does this file's class match what the file is. That is the
  cost this record charges.

## Limits of this record

- **The independent reading is a reading of location, not of purpose.** It asks
  whether a bound path is inside the set the private gate scans as its own
  machinery. A machinery file placed among the material, or a material file placed
  among the machinery, would be classed by where it sits rather than by what it is.
  That is a real hole and it is stated rather than gated: what closes it is that
  moving a bound file changes its bound name, which is a tracked edit a reviewer
  reads.
- **An amendment's reason is judged.** The gate holds that a reason is present and
  that a milestone is named. It cannot hold that the reason is true, and a
  plausible false reason passes. This inherits the earlier record's disclosure
  obligation rather than discharging it.
- **Two classes, and the argument that two is enough is a judgement.** Nothing
  here establishes that no third kind of bound artifact exists. The evidence that
  one might is that this project's defect class has reappeared one level up after
  every repair, and this record is a repair.
- **It says nothing about how much machinery a milestone should bind.** Binding
  the gate's scripts at all is a choice M5 made, and this record makes that choice
  survivable rather than revisiting it.

## Amendment: a material record carries a finding, not a lifecycle

Amended before M6-W3. **A record classed material may hold only what its
milestone found. A field that tracks what happened to the finding afterwards does
not belong in it, and the fix when one is already there is to separate the
finding from the disposition — never to unfreeze the material.** The two classes
above are unchanged, and so is every file's class.

### What forced the amendment

M5-W7 filed six SDK capability gaps in the private workspace's
`proposals/sdk-gaps.json`, each naming the module a shipped answer would occupy
and each carrying `publicSurface.shipped: false`. That record is classed
material, so its digest is a permanent bind and, under the rule above, no later
milestone may amend it.

M6-W2 shipped five of the six. The private gate then failed five times over,
saying that a shipped capability is not a gap — and there was no way to make it
pass honestly. Leaving the frozen `false` was a lie about the SDK. Setting it
`true` was a different failure and an unreachable one besides: the field is
frozen twice, once by the digest and once by the recomputation the check ends
with, which writes it `false` from the vocabulary. A correct, expected, planned
outcome — a gap being closed by the work package whose job was to close it — made
the gate red, and every state the record could reach was red too.

That is not a defect in the freeze and not a defect in the check. Both did
exactly what this record decides they should. The defect is in the record: one
file was carrying two different kinds of statement, and only one of them was
permanent.

### The distinction

A **finding** is what a milestone measured, about the tree it measured. "These
six capabilities were absent from Ravel's SDK when M5 counted the inventory" is
permanently true and will never stop being true, whatever ships later. That is
what material means, and freezing it costs nothing because nothing about it can
change.

A **lifecycle** is what happened to the finding afterwards — resolved, deferred,
withdrawn, superseded. It is not a measurement of the earlier tree at all. It is
a later milestone's fact about a later tree, and putting it inside the earlier
milestone's record freezes the status along with the finding. The record then
asserts, permanently, a present tense it has no authority over.

`shipped: false` reads like a measurement and behaves like a status. That is
exactly the ambiguity that made it wrong to hold: as a measurement it is true
forever and belongs where it is; as a status it went stale the moment M6-W2
landed and belongs somewhere M6 owns.

### What the fix is, and what it is not

The disposition moves to a record the later milestone owns. In the instance that
forced this, M6 writes `resolutions/sdk-gaps.json` in the private workspace: one
row per gap a work package closed, naming the proposal, the work package that
closed it, and the public SDK module the answer now occupies. M5's record is not
touched, not reclassified, and not amended. The frozen field is superseded rather
than deleted — deleting it would move the material — and the check that meets it
says so where a reader meets it too.

The check's rule becomes a question asked of two sides that can disagree: the
public SDK tree, which is where "does this ship" is read, and the ledger, which
is where "did anyone resolve it" is read. Neither is generated from the other. A
module that ships with no ledger row is a gap closed by nobody; a ledger row
whose module does not ship is a resolution nobody delivered; a row naming a
proposal that does not exist, or naming no work package, fails on its own terms.
Absence from the ledger is the ordinary state of an open gap and is not an
omission — the sixth capability has no row, because nothing has shipped it.

What this is not is an exception to the rule above. Nothing here unfreezes a
material digest, admits a third class, or lets a milestone amend its own material
figures once closed. It narrows what may be *written into* a material record in
the first place, which is a constraint on the producer rather than a relaxation
for the checker.

### Consequences of this amendment

- A milestone writing a record it expects to be classed material writes findings
  only. A field whose value a later milestone would want to change is a field
  that belongs in that later milestone's own record.
- A material record that already carries one is repaired by adding the
  disposition elsewhere and superseding the frozen field in place, with the
  supersession stated where the field is read. The frozen value stays what it was
  and stops being consulted.
- The read that decides a superseded field's real answer comes from the tree
  itself, not from the record that supersedes it, so the disposition and the
  world can still contradict each other.

### Limits of this amendment

- **"Finding" and "lifecycle" is a judgement made when the record is written.**
  Nothing here can be computed. `shipped` was written as a measurement in good
  faith and turned out to be a status, and a reviewer reading the field name at
  the time would not obviously have caught it. The tell that generalises is
  tense: a field whose truth is stated in the present, about a thing outside the
  record, is a status wearing a measurement's clothes.
- **A superseded field is still there to mislead.** The repair leaves a false-
  looking `false` in a published record, and what stops it being read as current
  is prose at the point of use rather than anything structural. Deleting it would
  be cleaner and would move the material, so the ambiguity is paid rather than
  removed.
- **This says nothing about how many dispositions a finding may collect.** One
  ledger per finding-set is what the instance needed. Whether a gap that is
  resolved, then regressed, then resolved again wants a history rather than a row
  is undecided, and the row is what exists.

## Amendment: a closed record measures its own tree

Accepted before M7's first work package, while resolving the third instance of
the same obstruction. It does not retrofit anything and it does not rewrite a
closed record. It narrows what a gate is entitled to ask one.

### What forced this second amendment

`evidence/m4/sdk-surface.json` enumerates the SDK surface M4 closed on: its
crates, test mods, examples, guide documents and conformance suites.
`verify_m4_surface` reconciled that enumeration against the repository in both
directions on every gate run. So a later milestone that added any of those five
things put the gate into violation until somebody edited M4's closed evidence —
silently, with no trail, because no mechanism recorded that an amendment had
happened at all.

It happened when M6-W2 added five test mods. It happened again when the M4
evidence amendment was parked as its own change. It happened a third time when
M7's library-provision work added a `provision` test mod, and that time it was
the sole failure of an otherwise green gate. Three milestones, one obstruction,
and each repair had been to edit the closed record again.

The record above already refuses to retrofit the machinery class below M5, on the
grounds that rewriting a closed milestone's record to satisfy a rule written
afterwards is the thing it refuses in the other direction. That refusal stands and
is not what changed. What the third instance exposed is that the obstruction was
never really about classing M4's entries — it was about a *gate* holding a closed
record to a tree that did not exist when the record was written.

### The distinction between the two questions

A closed record states what its milestone measured. That statement is about one
tree, at one moment, and it is permanently true of that tree.

Two questions can be asked of it later, and they are not the same question:

- **Does everything this record named still exist?** A deletion falsifies what the
  record claims to have measured, so the record is genuinely wrong now, and the
  gate is right to say so.
- **Is everything that exists named in this record?** A later addition falsifies
  nothing. The record never claimed to enumerate future trees, and reading it as
  though it did converts ordinary growth into a defect in a closed milestone.

The second question is legitimate — surfaces should be accounted for — but it is
the *current* milestone's question, and its answer belongs to the milestone that
adds the surface, in that milestone's own evidence. Asking it of a closed record
is what forced the silent amendments.

### The decision

A gate may hold a closed record to the first question and not to the second.
`verify_m4_surface` checks in one direction: every name M4 recorded must still be
present. Growth is not a violation.

### Consequences of the closed-record rule

- M4's closed evidence stops needing edits, which is the point. Task-level
  amendment machinery for it becomes unnecessary rather than deferred.
- **A named loss of coverage.** For crates, test mods and examples nothing is
  given up: `verify_inventory` reconciles the maintained constants against their
  directories independently. For guide documents and conformance suites there is
  no such constant, so a file added to either after M4's closure is now accounted
  for by nothing in this check. That is deliberate and it is a real gap. It is not
  replaced with speculative machinery here, because the duty belongs to the
  milestone that adds such a file, and inventing a mechanism before something
  needs it is what this project's standing preference refuses.
- The general rule reaches further than M4. Any closed record enumerating a
  living directory is subject to the same reading, and the next one written should
  be built knowing a gate will only ever ask it the first question.

### Limits of the closed-record rule

It says nothing about records that bind by digest, which are already governed
above; a digest that no longer matches is the first question, not the second.

It assumes the second question is always answerable by the adding milestone. Where
a surface is added by no milestone in particular — a stray file, a document
written between work packages — nothing now catches it, and this record does not
pretend otherwise.

It is a rule about gates, not about evidence. Every closed record in this
repository keeps exactly the words it had.
