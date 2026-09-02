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

## Paused mid-queue

These were authorised. The first two landed in `mlkuzwxu`; the rest are untouched and ordered as they
should resume.

Done — 1.21.1's Java compiles clean under `-Xlint:all -Werror`, which settled three things that had
been reasoned but never met a compiler: `eql$a` as a top-level class whose name carries `$`, the
`@Override` on a linked descriptor matching the generated stub's erased signature, and raw
`Supplier`/`BiFunction` lambdas that `-Werror` could have rejected. Every screen also loads by the name
its version's `ScreenNames` records — bare for 1.21.1, package-qualified for 26.1.2.

1. **Reflective member verification against the jar.** `verify_reflection` compares classes only, and
   against Mojang's mapping rather than the jar. Every member name and arity the agent jar reflects on
   is checked by nothing, under either version.
2. **Two `Transformer.java` defects.** `slot(4)` computes local 5 against 1.21.1's four-parameter
   `doWorldLoad` and emits `iload 5` where no int lives — a `VerifyError` at class load, so the launch
   aborts. And the title-menu bridge guards on an INT return that is void at 1.21.1, so the mods button
   silently never appears.
3. **Transformer-shape check.** The scaffold verifies a hook the transformer names is declared, not
   that the transformer's assumptions about its shape still hold. Parameter positions and return kinds
   are the two that bit.
4. **Persistence seam.** `Persistence.java` resolves storage by 26.1.2's member names and arities.
   1.21.1 uses `computeIfAbsent(SavedData$Factory, String)` and has no `tryWrite`. It compiles because
   it is reflective, so only a launch would notice. Agreed shape: extract `read()`/`write()` into a
   shared class with a per-version override, about thirty lines, rather than copying all 379.

## Launch-only remainders

Two things no gate can reach, to watch for on screen. Both are also recorded in
`evidence/m4/linux-process-matrix.json`, beside the check that does not cover them.

- The stub tree models no nestmate access, so a flat `eql$a` standing in for nested `SavedData$Factory`
  can compile and fail at link.
- The classloader check proves an ordinary classpath, while the launch appends the screen jar through
  `Instrumentation.appendToSystemClassLoaderSearch` with the game already loaded. Same name, different
  loader.

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
