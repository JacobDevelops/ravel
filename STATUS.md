# Status

`ROADMAP.md` is the dependency-ordered plan and `evidence/` records what each milestone measured.
This file summarizes the current state and the distance to the goal. Trust
`cargo make ravel:verify-milestone <id>` over a written closure claim.

## The goal

Play All the Mods 10: To the Sky 2.0.2 with a Rust mod loader instead of Java.
The default build target is 26.1.2; the ATM10 To the Sky proof workload is pinned to 1.21.1.

## Where we are

Both versions have pinned version data.
Version-specific bindings, hooks and screen sources are generated from each version's pinned inputs.
The JVM harness runs all thirteen sections at every tracked version; `ravel:verify-gate-versions`
holds the recorded runs to that set.

**Ravel has run inside a real Minecraft 1.21.1 client.** Private launch captures under
`evidence/private/launch-1.21.1/` prove source-distributed examples compiled into the instance's host,
loaded through `-Dravel.host`, and listed on the mods screen. A Rust-declared block renders, places,
drops loot and crafts in a new world's first session, and survives save and reopen. The mods screen
shows pack notices and per-mod settings entries in a scrolling list; settings and profiling render.

The host writes generated assets to `<game>/resourcepacks/ravel/` and data to
`<world>/datapacks/ravel/`. New-world creation stages and selects the datapack through the game's
own reload path. The absent-pack and already-selected Re-Create branches have client proof; the
replaced-tree forced reload and refused-pack-id report have harness and mutation proof only.
A fresh instance's resource-pack selection is seeded on its second launch, after the game has
created `options.txt` with its own version line. A player who disables the pack wins.

Host builds use `cargo make ravel:host-build <instance> <cache> <minecraft>`;
fixture setup uses `cargo make ravel:fixture-reconstruct` and
`cargo make ravel:fixture-verify`. The standalone CLI and performance/resource
tooling are retired. Closed measurements and their manifest-bound inputs remain
historical evidence, not current capture or budget gates.

## What is not true yet

**The full TTS pack is not playable through Ravel yet.** M6 records twelve accepted foundation
counterparts in `evidence/m6/counterpart-summary.json`; that is not full-pack coverage. Completing
the remaining counterparts and proving the pack still stand between the current loader and the
goal. Running Ravel's own Rust mods inside a TTS instance is not the same thing as playing TTS.

Installed examples are digest-bound source packages. Refresh an installed copy when changing the
checkout: an old package can verify and compile correctly while lacking a new declaration.
Registered items have no creative tab yet; `/give` is how they are obtained. The stateful example
has no in-game call that changes its journal; the saved-data test mod supplies the visit-count proof.

## Open decisions

- **How does a player's machine obtain the pinned version inputs?** Both tracked versions pin the
  client jar under `tests/fixtures/local/`; `1.21.1` also needs Mojang's `client.txt`, while `26.1.2`
  reads unobfuscated class files. The adapter generates bindings from these untracked local inputs,
  so a checkout alone cannot build the host on another machine. Options are shipping generated
  bindings, deriving them from the launcher's jar and the hash-pinned mapping URL where needed,
  or explicitly staying checkout-only.
- **Is a Cargo compile inside the launch sequence acceptable?** `evidence/m3/pack-compile.json`
  records cold and warm pack costs. The stock Prism integration does not own a pre-launch compile
  step, and a build needs the toolchain and registry dependencies.
- **Ravel needs Java 24+** because the transformer uses `java.lang.classfile`. Port it or declare
  the requirement to players; the 1.21.1 launch uses the pinned Java 25 runtime with
  `IgnoreJavaCompatibility=true`.
- **Should registered content have one shared creative tab?** The pending default is one tab keyed
  `ravel.creative_tab`, with fallback label "Ravel". It needs eight more stub seeds per version and
  the reworked `26.1.2` creative-tab API.

## Remaining proof and implementation gaps

- The headless real-client route stalled at Prism's account requirement. The direct JVM route and
  recovery notes live in the evidence skill; that attempt produced no game log.
- Returning from settings and profiling to the opening mods screen is harness-proven but unseen in
  a client. The settings cap and its explanatory line also have unit proof only.
- Signed-jar child-loader checks use probe jars. Runtime-package access can still fail when a member
  is first resolved rather than when a screen class is defined. The stub tree does not model
  nestmate access; launch proof, not compilation alone, closes those questions.
- Seven drifted non-persistence hooks are retargeted but not written.
- Packet delivery remains unimplemented. M4's recorded launch-discovery and pack-read limits describe
  its measured tree, not the player path now proven above.
- M7's `provision` resolves libraries, but no launch builds a `Provision` from composed packages.
- `profilingCommandCounter` needs a command-claiming mod and a namespaced root typed in a client;
  `profilingTickCounter` needs a boundary wired on the game's tick path.
- Closed M3 evidence describes the reflective surface before the later member verification work.
  Re-pinning that evidence is a separate amendment, not an edit to a historical measurement.
- A failed save reaches the player only as a log line; player-visible failure reporting remains
  deferred.
- Windows validation of the JNI world path used for recursive deletion, including `C:/x`, remains
  deferred until Windows is supported.

## Standing decisions and development traps

- Screen sources are generated. Version selection is a build-time choice declared per entry point.
  Per-version sources handle genuine API differences; role indirection handles renames.
- Ravel has never been released, so there is no installed compatibility contract to preserve.
- Mise defaults `RAVEL_MINECRAFT_VERSION` to `26.1.2`; an outer environment value overrides it.
  Select another version explicitly, for example
  `RAVEL_MINECRAFT_VERSION=1.21.1 cargo make ravel:typecheck`.
- Translating bindings need the published mapping as well as the jars. Missing untracked local inputs
  must be recorded as a skip, not reported as a version that passed.
- Check every path, including obfuscated versions, and prove a check by mutation. Write a durable
  success record after the work succeeds, never before it starts.
- Adding a sub-task to `ravel:verify` requires reconciling
  `evidence/m4/linux-process-matrix.json` and its manifest digest through the evidence amendment path.
- Squash into the nearest change that owns a file, not past a descendant that also touches it; see
  [the stacked-change workflow](docs/jj-stacked-changes.md).
