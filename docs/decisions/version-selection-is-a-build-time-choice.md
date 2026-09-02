# Version selection is a build-time choice

Status: accepted while scoping the second version directory, ahead of the work
package that authors `minecraft/versions/1.21.1/`. One build generates against
one Minecraft version and emits that version's artefacts; several version
directories may be tracked, locked, and reviewed side by side, and the choice of
which one a build reads is made before the compiler runs rather than by the
running game. Runtime selection is deferred, and the condition that reopens it is
named below.

This record takes no ADG number, for the reason [`README.md`](README.md) gives
about the records that already do not: `SPEC.md` §15 owns those numbers and
reserves its unused ones for other subjects. It opens no gate. It decides which
of two existing mechanisms carries a capability
[ADG-3](adg-3-bindings.md) already authorised the data for.

It is decided without a new measurement, and that sentence belongs here rather
than in a footnote because
[Claims without a second reading](claims-without-a-second-reading.md) says so.
What a measurement could compare is two mechanisms, and only one of them exists:
the rejected one would have to be built before it could be timed, and the reason
it is rejected is not a number. What is measurable is that the selected mechanism
behaves as this record says, and that is proved by mutation in the generator's
own tests rather than asserted here — see "Evidence".

## Why this record exists

`ROADMAP.md` wants a `1.21.1` version directory beside the pinned `26.1.2` one,
and `CLAUDE.md` already describes Ravel as supporting more than one Minecraft
version, with a package declaring its versions in `compatibility.minecraft` and
per-version data under `minecraft/versions/<version>/`. The package half of that
is built: `Compatibility::check_minecraft` evaluates an exact version or a semver
range against one running version.

The adapter half refuses. `generate::inputs::pinned` reads the version
directories, and where it finds more than one it stops, saying so in its own
words: two versions would need two hook classes and two exported symbol sets, and
nothing has decided how those are told apart. That refusal is correct and is not
a bug to be deleted. It names an undecided question, and this record is the
decision it was waiting for.

The question is undecided because the collision it names is real. `HOOK_CLASS` is
one constant, `dev.ravel.jvm.Hook`. A generated export's Java method name is
derived from the hook id alone, with no version component, and the symbol is
mangled against that class. Two versions review the same capabilities, so both
inventories carry `server.tick`, both render `serverTick`, and both mangle to
`Java_dev_ravel_jvm_Hook_serverTick`. The generator already refuses two hooks that
render the same Java name within one version; across versions there is no such
guard, because until now the case could not arise. The same shape repeats in four
more places: `Hook.java` is one file at one fixed path, the stub tree is keyed by
each game class's own fully qualified name, the generated Rust arrives as one
`include!` behind `pub mod pinned` with one `MINECRAFT_VERSION` const, and
`links` metadata forwards exactly one `exports` path that `ravel-bootstrap` reads
as `DEP_RAVEL_MINECRAFT_ADAPTER_EXPORTS`.

Only the two locks, `mappings.lock` and `stubs.lock`, are already per version.

## Decision

**The multi-version unit is the binary, not the process it runs in. One build
selects one version directory and emits that version's artefacts. The
distribution carries a `.so` and an agent jar per supported version, and the
launcher points at the one matching the game it is about to start.**

Selection is explicit and fails closed:

- Exactly one tracked version directory is the default; nothing has to be set.
- More than one requires `RAVEL_MINECRAFT_VERSION` to name one of them.
- A name that matches no directory is refused, and the refusal reports the names
  the build actually found rather than the one it wanted.
- The build script declares the variable with `cargo:rerun-if-env-changed`, so
  changing the selection regenerates rather than serving a stale tree.

Everything downstream of the selection stays singular and is untouched. One
`Hook` class, one symbol namespace, one stub tree, one `pinned` module, one
`links` entry.

### The four questions this settles

**Is the multi-version unit the binary or the runtime?** The binary. A process
runs exactly one Minecraft version, so a binary carrying two carries one it will
never call. Nothing is gained by paying for the second at link time.

**How are two hook classes and two exported symbol sets told apart?** They are
never told apart, because they never coexist. The collision that made the
generator refuse — two inventories both reviewing `server.tick`, both rendering
`serverTick`, both mangling to `Java_dev_ravel_jvm_Hook_serverTick` in one
library — cannot arise when one build emits one version's exports. The same
answer disposes of the fixed `Hook.java` path, the shared stub tree, and the
single `links` entry: each is a collision between two emissions, and there is
only ever one emission.

**Where does the running version come from?** From the build. Nothing reads it
out of the live JVM, and the compile-time constant continues to reach
`ravel_loader::admitted` as an argument, exactly as it does today. This is the
part of the deferral worth stating plainly: runtime discovery is a chicken and
egg. Reading the running version from inside the JVM means hooking a class whose
own name is version-dependent, which is the problem the version data exists to
solve. A mechanism that has to know the version before it can find out the
version is not a cheaper alternative to knowing it at build time; it is the same
knowledge, obtained later and less reliably.

**Does the SDK see version selection?**
[`dependency-rules.md`](../architecture/dependency-rules.md) forbids the SDK
exposing bootstrap, JVM, or version internals. It does not have to be defended
here, because build-time selection leaves a mod nothing to select: by the time a
mod's code exists, the version is already the only one the binary has. A mod
declares the versions it supports in `compatibility.minecraft` and the loader
checks that declaration against the one running version, which is a package-layer
predicate over a scalar and stays exactly that.

### Unselected version directories are not read

The generator reads the selected directory and no other. It does not parse,
hash-verify, or resolve a directory it was not asked to generate against, and a
malformed unselected directory does not fail a build that did not select it.

Two reasons. A build's success would otherwise depend on data no artefact of that
build derives from, which makes the refusal say something the build cannot act
on. And the repository-wide guarantee that every tracked version directory is
well formed is a different guarantee with a different owner: the scaffold
contract in `ravel-xtask` sweeps tracked files on every `ravel:verify` regardless
of what any build selected, and it is the right place for a check that has to
hold for all versions at once. Making the generator do it too would be one rule
written twice, in two places that can disagree.

What follows from that is a real obligation and not a loophole: the scaffold
contract has to stop naming `26.1.2` by path before a second directory is
tracked, or a malformed second version is caught by nothing. That is the work
package named under "What is not decided here".

## Rejected

**One binary carrying N versions, with everything version-qualified.** Generated
Java would go to `dev.ravel.jvm.v<sanitised>.Hook` so symbols mangle per class
and never collide, `pinned` would become `pinned::v1_21_1` beside
`pinned::v26_1_2`, the target table would become a registry keyed by version, and
the agent would pick a `Hook` class after reading the running version. It is the
option that genuinely delivers a multi-version artefact, and it is rejected on
its costs, which are not speculative:

- It changes [ADG-1](adg-1-jvm-entry.md)'s entry mechanism. Every amendment that
  record carries went out of its way to leave the mechanism alone; admitting a
  second `Hook` class into one library, or putting a version inside a mangled
  symbol name, is precisely the change they avoided.
- Roughly a dozen hand-written exports in `ravel-bootstrap` spell
  `dev.ravel.jvm.Hook` directly. Under a version-qualified generated class they
  either stay on a shared unversioned class — two Java classes with different
  lifetimes, and a new rule about which export belongs on which — or they are
  duplicated per version, touching every one of those files.
- The stub tree is keyed by the game's own fully qualified name, so it needs one
  directory per version and the screen jar needs one compile per version.
- `links` metadata forwards one path per key and cannot express N without N
  crates or a combining file that reintroduces the same question one layer down.
- Binary size grows with every version supported, for code a given process never
  calls.
- It does not answer where the running version comes from. That question is left
  open above, and this option needs it closed before it can dispatch.

**Reversal condition.** A modpack that must span Minecraft versions within a
single install. If one installed instance has to load, in one process, content
pinned to two game versions, then the binary is no longer the multi-version unit
and this rejection is wrong. Nothing else reopens it: shipping several versions,
a launcher choosing between them, or a package declaring a wide range are all
served by the decision above. Runtime selection would need its own record
answering where the running version comes from before this one could be revised.

## Evidence

No measurement decides this record, for the reason its `Status` line gives. What
is measured is that the mechanism it selects behaves as described, and that is
proved by mutation in `ravel-minecraft-adapter`'s own tests, against a synthetic
second version directory carried as a public test fixture rather than as tracked
version data:

- Two well-formed version directories, each named in turn as the generator's
  selection, generate the named version's bindings and not the other's. The
  tests pass that selection as an argument to the generator rather than through
  the environment, because the generator takes it as one: `build.rs` is the
  single place that reads `RAVEL_MINECRAFT_VERSION`, and a generator that read
  its own environment would answer the same call differently in two processes.
- The same two directories with no selection are refused, and the refusal names
  both directories found.
- A selection naming a directory that does not exist is refused, and the refusal
  reports what was found.
- A malformed directory the build did not select does not fail the build, and
  the same corruption does fail a build that selects it. This is the section
  above proved rather than asserted: under a generator mutated to parse every
  tracked directory before choosing, that test exits 101, and as written it
  exits 0.
- A selection that is not valid UTF-8 is refused rather than read as no
  selection, so an operator who steered a build is never silently ignored.
- `26.1.2` selected reproduces the generated tree byte for byte.

The last of those is the one that matters most, because it is what makes this
decision cost nothing to adopt: under a single tracked version, selection is the
behaviour that already existed.

## Consequences

- The distribution ships one `.so` and one agent jar per supported Minecraft
  version, and the launcher points the instance at the pair matching its game.
  [ADG-5](adg-5-prism-integration.md)'s two stock instance arguments are
  unchanged in form; what they point at becomes per version.
- `compatibility.minecraft` stays a package-layer predicate over the one running
  version, and stays correct. A package declaring `>=1.21, <1.22` is admitted by
  whichever build serves a version in that range and refused by the others, which
  is the answer a player needs from the instance they actually launched.
- Adding a Minecraft version is data plus a re-pin, which is what `SPEC.md` means
  when it says a second version is an adapter task rather than a rewrite. Nothing
  in `ravel-bootstrap`, `jvm/`, or the generator's rendering changes to add one.
- The generator's refusal becomes a selection. The sentence in
  `generate::inputs::pinned` that named this undecided question is replaced by
  the decision, not deleted.
- A build that selects nothing when several versions are tracked is an error, not
  a default. Alphabetical order, most recent version, and "the one 26.1.2 used to
  be" are all silent answers to a question the operator should have answered.

## What is not decided here, and who owns it

- **The scaffold contract's per-version sweep.** `ravel-xtask` lists `26.1.2`'s
  files literally and treats `minecraft/versions/26.1.2/hooks.toml` as *the* hook
  inventory for its conformance, hook-count, and agent-reflection checks. Those
  have to become per-directory sweeps before a second directory is tracked. The
  `AGENT_REFLECTED_CLASSES` list is the sharp edge: it pins seventeen `26.1.2`
  class names and at least one of them does not exist at 1.21.1, so that constant
  becomes per-version data in that work package rather than in the one that
  authors the version.
- **`minecraft/versions/1.21.1/` itself.** The version data, its locks, and its
  review are the work package this record unblocks, not the one it is part of.
- **Runtime selection.** Deferred, with the reversal condition named above. It
  would need its own record.
- **Exporting a selection from every build entry point.** Nothing in the
  repository sets `RAVEL_MINECRAFT_VERSION` today, and nothing has to while one
  directory is tracked. From the moment a second one is, a plain `cargo build`,
  `cargo test` and `devenv tasks run ravel:verify` each reach the refusal this
  record decided, so each entry point has to export a selection first. That is a
  precondition of the scaffold-contract work package named above and belongs in
  it, not in the one that authors the version data. It is deliberately not
  solved with a default: a default is the silent answer this record refuses.

## Limits of this record

- Nothing here was measured on a launched game. The claim is about what a build
  emits.
- Only one version directory is tracked today, so the multi-version behaviour is
  proved against a synthetic fixture rather than against two real Minecraft
  versions. A second real version can still surprise this decision in ways a
  fixture cannot — a mapping source that needs a different input set, or an
  inventory whose shape the current schema cannot express — and neither of those
  is what this record decides.
- The distribution consequence is stated, not built. No packaging in this
  repository ships N libraries yet, and nothing checks that a launcher points at
  the right one.
- Two lines of `build.rs` are proved by reading rather than by a test. A build
  script runs only as a build, so nothing can call it and assert on what it
  emitted: the `cargo::rerun-if-env-changed` directive that makes a build which
  changes selection generate again instead of serving the previous version's
  cached tree, and the `var_os` read that fetches the value. The parse of that
  value is testable and is tested, because it is a function the generator
  exposes rather than an environment read. Deleting the rerun directive would
  leave every test green and yield a stale wrong-version tree.
