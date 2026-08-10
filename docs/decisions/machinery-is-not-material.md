# Machinery is not material

Status: accepted before M6-W2, for every repository and workspace inside this
project's boundary. It is not an architecture decision gate and carries no ADG
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
