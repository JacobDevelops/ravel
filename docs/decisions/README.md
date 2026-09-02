# Architecture decisions

One record per architecture decision gate, and seven records that are not gates. Each
one states what was decided, the Linux evidence it rests on, what was rejected and
why, and what the evidence does not cover. Results live in
[`evidence/m1/spikes/`](../../evidence/m1/spikes/) and are checked by
`devenv tasks run ravel:verify-spikes`.

A decision is not accepted because it is plausible. It is accepted because a
rejection is explicit, the limits are written down, and one of two things holds: a
measurement exists, or the record says plainly that none does and why none can.
The second form is the exception and it is disclosed rather than assumed —
[ADG-2](adg-2-mod-execution.md) and [ADG-6](adg-6-configuration-ui.md) each say so
in their own `Status` line, and
[Claims without a second reading](claims-without-a-second-reading.md) is what
decides the obligations that come with it.

| Gate | Decision | Status |
| --- | --- | --- |
| [ADG-1](adg-1-jvm-entry.md) | JVM entry and hook mechanism | accepted (M1-W2); amended before M3-W3 for the sibling Java artifact, at M3-W3 for the measured one-loader model, before M3-W4 for one receiver-aware adapter, and while scoping the second version directory to record that a second Minecraft version reaches none of the mechanism |
| [ADG-2](adg-2-mod-execution.md) | Rust mod execution and ABI | accepted (M3): interchangeable mods, distributed as source and compiled on the player's machine |
| [ADG-3](adg-3-bindings.md) | Minecraft bindings | accepted (M1-W4); amended while scoping the 1.21.1 mapping source, ahead of the version-directory work package, to admit Mojang's own published mapping file as a second source |
| [ADG-4](adg-4-state-ownership.md) | State ownership | accepted (M1-W5); amended (M3-W0) to admit opaque handles |
| [ADG-5](adg-5-prism-integration.md) | Prism integration | accepted: two stock instance arguments, launch observed |
| [ADG-6](adg-6-configuration-ui.md) | Configuration schema and Ravel-owned UI | accepted before M3-W3: declared in data, drawn in Java |
| [ADG-7](adg-7-world-persistence.md) | World persistence adapter | accepted before M3-W4: one bounded Rust aggregate through vanilla saved data |

Seven records are not gates and are not in the table, because they decide no gate.
[Claims without a second reading](claims-without-a-second-reading.md), accepted
before M5-W8, governs how a claim that no second measurement can falsify is
recorded and disclosed, wherever one appears.
[Machinery is not material](machinery-is-not-material.md), accepted before M6-W2,
decides what a closed milestone's bound digests freeze: the material its figures
describe, permanently, and the machinery that produced them, as a record of the
tree at closure that a later milestone amends in the open.
[Carrying an action outward](carrying-an-action-outward.md), accepted during
M6-W2, decides how an action a mod asks for reaches the running game: as an
instruction the Java adapter performs, never as a call Rust makes. It is the
measurement [ADG-6](adg-6-configuration-ui.md) deferred rather than a revision of
it.
[Counterpart identity and aliases](counterpart-identity-and-aliases.md), accepted
before M6-W3, decides that a counterpart's own identity is Ravel-native
everywhere and that identity mapping is a host concern, so the rewrite-integrity
check stands unmodified: where save compatibility genuinely needs an upstream id,
it is declared in one file per package whose only permitted content is alias
rows.
[The toolchain and the build cache are shared per machine](shared-toolchain-and-build-cache.md),
accepted before M7-W0, settles what ADG-2 left open about how many of each a
machine has: one, shared across every instance, because Cargo's cache key already
captures everything that could make two instances' artifacts legitimately differ
and a second pinned toolchain is waste with no compensating property.
[Driving acceptance through the loader's own crossings](driving-acceptance-through-loader-crossings.md),
accepted while scoping the agentic acceptance harness ahead of M8-W4 and M10,
decides that an acceptance run perceives the game through the loader's existing
events and acts through the instruction the adapter already performs, adding no
new crossing — it is [Carrying an action outward](carrying-an-action-outward.md)'s
mechanism put to a test's use, and the model it decides lives in
`crates/ravel-acceptance` with its live driver owned by M8-W0.
[Version selection is a build-time choice](version-selection-is-a-build-time-choice.md),
accepted while scoping the second version directory, decides that one build
generates against one Minecraft version and emits that version's artefacts, that
several version directories may be tracked and reviewed side by side, and that
the distribution rather than the binary is what carries more than one version. It
is the decision [ADG-3](adg-3-bindings.md)'s per-version data was waiting on and
it leaves [ADG-1](adg-1-jvm-entry.md)'s entry mechanism alone, which that record
is amended to say. None of these takes an ADG
number, because `SPEC.md` §15 owns those and reserves its unused ones for other
subjects.

## Reproducing the evidence

Every gate that took a measurement can be re-run from here; the two paragraphs
after the block say why one of them is a script rather than a task and why one
gate has neither. All of the tasks need the pinned local fixtures described in
[`tests/fixtures/local/README.md`](../../tests/fixtures/local/README.md), and none
of them authenticates or launches the game.

```console
devenv tasks run ravel:spike-adg-1
devenv tasks run ravel:spike-adg-2
devenv tasks run ravel:spike-adg-3
devenv tasks run ravel:spike-adg-4
devenv tasks run ravel:spike-adg-5
./jvm/harness/run-datafix-byte-preservation.sh # ADG-7 pinned fixer measurement
devenv tasks run ravel:measure-outward-crossing # not a gate; see below
```

The last task belongs to [Carrying an action outward](carrying-an-action-outward.md),
which is not a gate and so is not in the table above. It is listed here anyway
because it is the one measurement in this directory that ADG-6 asked a later
milestone to take, and a reader looking for it would look here.

The last one is a script rather than a task because no task runs it: it additionally
needs a real Minecraft library root for the fixer's own dependencies, so it stays a
measurement ADG-7 records rather than a gate. ADG-7 says what that costs.

[ADG-6](adg-6-configuration-ui.md) has neither a task nor a script, and that is
the one gate here decided without a new measurement. Its record says so in its
own `Status` line and its Evidence section says what decided it instead — a
property no measurement was addressed to — so there is nothing for a task to
re-run. Reading that section is the only way to check it, which is the cost of
deciding a gate that way and is stated there rather than here.

The shared measurement contract these tasks report through is
[`docs/architecture/m1-measurement-gate.md`](../architecture/m1-measurement-gate.md),
implemented by [`experiments/spike-harness`](../../experiments/spike-harness).
