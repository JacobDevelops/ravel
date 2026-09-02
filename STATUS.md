# Status

Where the project actually is, as of 2026-09-02. `ROADMAP.md` is the dependency-ordered plan and
`evidence/` records what each closed milestone measured; this file is the current state of play and the
honest distance to the goal. It goes stale — trust `devenv tasks run ravel:verify-milestone:<id>` over
anything written here.

## The goal

Play All the Mods 10: To the Sky 2.0.2 with a Rust mod loader instead of Java. TTS runs on Minecraft
1.21.1, which is why 1.21.1 is the target.

## Where we are

Ravel targets Minecraft 1.21.1. `minecraft/versions/1.21.1/` exists, carries all seven required files,
and both its locks are resolved against the real obfuscated client jar. The suite runs once per tracked
version and passes at both 1.21.1 and 26.1.2 — 1640 tests each.

Nothing has been launched. No game process has ever started with the 1.21.1 data attached.

## What is not true yet

**TTS's mods do not exist in Rust.** This is the whole remaining project and no decision shortens it.
The private registry holds 321 crates that compile and pass their tests, but every one has a no-op
`enter()`, 320 carry `HOST_BLOCKERS` enumerating real mod behaviour as out of scope, and the registry's
own `mods/ADMISSIBILITY-ANALYSIS.md` records them as inadmissible as counterparts. That is M6 and M7.
M8–M10 sit behind them.

What is reachable much sooner is Ravel attached to a real 1.21.1 client running *its own* Rust mods
inside a TTS instance. That is not the same thing as playing TTS, and the difference is roughly the
whole of M6/M7.

## Landed this session

Newest last. All local, nothing pushed.

| Change | Subject |
| --- | --- |
| `omqxmqwv` | amend ADG-3 to admit Mojang's published mapping file as a second source |
| `lxouzzut` | make `mappingSource` a generator decision; add the proguard mapping layer |
| `pxsoorlm` | decide version selection as a build-time choice |
| `wqtmwuvy` | select the version a build generates against instead of refusing two |
| `uvquypor` | sweep every version directory instead of naming 26.1.2 |
| `yntowsum` | generate the agent jar's game names from the reviewed reflective surface |
| `sstskusv` | resolve game classes through generated names instead of literals |
| `oqqtuolr` | translate a stub seed's members out of the linked name space |
| `xsrrmzpo` | generate the screen sources in the name space each version links |
| `qnxpmmvk` | declare the Minecraft version selection every build entry point reads |
| `qtuntyxt` | bind screen templates to roles each version's stub seeds resolve |
| `ssslwxmn` | target 1.21.1, with per-version screen sources where the API differs |
| `mlkuzwxu` | compile every tracked version's Java and load its screens by name |
| `sqnnpusy` | derive the transformer's slots and return kinds from each version's hook descriptors |
| `nnsklvqw` | verify the reflective surface's members against each version's jar and generate their linked names |
| `woukurwn` | resolve persistence members by generated name and descriptor, with a per-version storage shape |
| `nyookuop` | resolve the content and outward adapters' game members by generated name and descriptor |

## The queue is drained

The four items that were paused here all landed, each proven by mutation against the 1.21.1 jar:

- **Reflective members are verified against the jar.** Every member the surface declares, by descriptor
  or by arity, must exist on the linked class in the version's client jar with a consistent descriptor.
  The check runs on `compose` and on `regenerate_locks`, under both versions. Both surfaces now carry
  members (39 at 1.21.1, 45 at 26.1.2), and the adapter generates `GameMembers` beside `GameNames` so
  Java reaches a member as a linked name plus linked descriptor rather than a literal.
- **The transformer reads its shape from the descriptor.** The world-open boolean is the last declared
  parameter, not slot 5, and the title-menu bridge injects on int or void per the hook's return. A
  throwaway class-load proof showed the old transformer's `VerifyError` and the new one loading. The
  scaffold now holds three transformer seams against every tracked version's `hooks.toml` and writes
  `target/ravel-jvm/transformer-shape.txt` from the versions that held.
- **Persistence, content and outward resolve members by name and descriptor.** Name-and-arity could
  never have worked at 1.21.1: `DimensionDataStorage` links `computeIfAbsent` and `set` both as a
  two-argument `a`. The per-version storage shape lives in `RavelWorldData`'s template override, three
  entry points, and 1.21.1's storage having no `close` is an `AutoCloseable` cast.

## Next, before a launch means much

- **Content registration is wrong-shaped at 1.21.1.** `Content.withId` spells `setId`, which 26.1.2's
  properties builders require and 1.21.1 does not declare. A member one version lacks cannot be declared
  for it, so under 1.21.1 that lookup throws, `one` catches it, and every block and item is reported as
  unregistered. 1.21.1 needs no `ResourceKey` on the properties at all, so the fix is a per-version
  registration shape, the way `RavelWorldData` answered the same problem for persistence. No work
  package owns it yet. A launch still reaches the title screen and the mods button without it.
- `evidence/m3/manifest.json` describes the reflective surface as reaching no members, which stopped
  being true this session for all three adapters. Amending closed evidence is its own ceremony.

## Launch-only remainders

Two things no gate can reach, to watch for on screen. Both are also recorded in
`evidence/m4/linux-process-matrix.json`, beside the check that does not cover them.

- The stub tree models no nestmate access, so a flat `eql$a` standing in for nested `SavedData$Factory`
  can compile and fail at link. The agent jar no longer touches the factory directly; the sibling's
  compiled reference is the one place left. The JVM checks nest membership only for private access
  and the factory is a public record, so the reading is that it links, but that is reasoning, not
  evidence.
- The classloader check now builds the loader the agent builds, requires each screen to be defined by it,
  and reproduces the signed-jar certificate failure both ways over probe jars it signs itself. What it
  still cannot reach is that an access a different runtime package forbids is raised when the member is
  resolved, not when the class is defined, so a screen that loads there can still fail on the first click.

## Known open, beyond the above

- Seven drifted non-persistence hooks retargeted but not written.
- M4's three disclosed limits: no package discovery in a launch, no packet delivery, no pack read back.
- M7's `provision` resolves libraries, but no launch builds a `Provision` from the packages it composed.
- `profilingCommandCounter` — needs a command-claiming mod in the composed host and a namespaced root
  typed in a live client. Small.
- `profilingTickCounter` — needs a tick boundary wired on the game's tick path.
- ADG-3's re-pin of the agent jar's reflective surface is owned by no work package.

## Standing decisions

- Screen sources are generated, not hand-written.
- Version selection is a build-time choice, declared per entry point, no default.
- Per-version sources only where the API genuinely differs; role indirection for renames.
- No new ADRs. Records serve the work, not the reverse.
- Ravel has never been released, so there are no compat contracts. Breaking changes are fine where they
  are the cleaner design.
- 26.1.2 stays tracked for now — it is the only thing proving the multi-version machinery against two
  real versions. Dropping it is cheap once 1.21.1 plays.

## The recurring defect class

Worth knowing before writing any check here. Nearly every defect found this session was a check that
existed on one path and not the other, or that fired only when something downstream happened to reach
it. The largest was the gate reporting exit 0 for weeks while pinned to 26.1.2, so nothing ever built
the version the work targeted. Identity mapping at an unobfuscated version hides this whole class.

Prefer the check that runs regardless of which path, which machine, and whether anything downstream
reaches it. And prove a check by mutation: break its input and require a nonzero exit. A check nobody
has seen fail is not a check.

Note also that `devenv tasks run` shows neither stdout nor stderr for a task that succeeds, so anything
that announces itself by printing is invisible where it matters. Write the fact to a file and fail
outright when nothing was verifiable.

Write that record when the work *succeeds*, never when it starts. `ravel-test` recorded
`tested: <version>` before running the suite, so a run that died on its first version would have left a
file claiming that version was tested — a record naming a version nothing proved. The same shape, one
layer down.

## Traps that cost real time

- Setting `RAVEL_MINECRAFT_VERSION` as a prefix to `devenv shell` does nothing. devenv exports its own
  `env` over what it inherits, silently, so a "successful" 1.21.1 build quietly builds 26.1.2. It has
  to go inside: `devenv shell -- bash -c 'RAVEL_MINECRAFT_VERSION=1.21.1 cargo build'`.
- Generating 1.21.1 needs its published mapping, not just its jars — `generate::run` reads `client.txt`
  on every build under a translating source, lock path included. That file is untracked local material,
  so 1.21.1 is skipped on every machine but this one. Both new checks record the skip rather than
  passing quietly.
- Adding a sub-task to `ravel:verify` requires a matching row in
  `evidence/m4/linux-process-matrix.json` with its digest re-pinned in `evidence/m4/manifest.json`.
  `verify_m4_process_matrix` reconciles both directions and the scaffold refuses otherwise. That is the
  amendment path a closed milestone was designed for, not a violation of it.
- Squash into the nearest change that owns a file, never past a descendant that also touches it. A
  file-granular squash into an ancestor replays the diff onto that ancestor's copy, and the context may
  not exist there. jj marks the change conflicted rather than losing work; `jj op restore` recovers.
