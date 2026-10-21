# Minecraft 1.12.2 / SevTech: Ages 3.2.3 integration plan

## Decision and evidence boundary

- **Recommended destination:** three independently pinned versions—1.12.2, 1.21.1 and 26.1.2—with SevTech: Ages 3.2.3
  replacing ATM10 as the forward proof workload, not deleting ATM10's historical evidence. Version-specific internals
  remain behind adapters; native counterparts remain Rust packages, not hosted Forge Java mods. This preserves Ravel's
  existing architecture rather than turning it into Cleanroom. [SPEC.md:206–225;
  docs/architecture/overview.md:23–49,78–87]
- **Latest GA observed during research is JDK 27**, GA 15 September 2026; OpenJDK identifies G1 everywhere and compact
  object headers by default as release features. The repository already pins Temurin 27.0.0+35; there is no reason to
  retreat to Java 8, 21 or 25 for the new target. Re-resolve the latest GA when implementation starts and record the
  exact vendor/build, rather than treating “latest” as a reproducible pin. [https://openjdk.org/projects/jdk/27/;
  mise.toml:17–23]
- **Intrusion recommendation:** replace the legacy launch/library assumptions, LWJGL2 path, renderer and memory-heavy
  model/state structures where measurements justify it. Preserve the official game runtime, premain/JNI contract, world
  semantics and entitlement boundary; do not preserve obsolete implementation details merely for binary compatibility.
  [Design recommendation; docs/architecture/overview.md:3–9,32–57]
- **Not an existing performance result:** the repository itself records that 1.21.1 has generated/harness coverage but
  has not been attached to a real process; its persistence path is not yet live-proven. A new version directory or
  passing stub harness is therefore not proof of a playable pack. [ROADMAP.md:1042]
- Primary-source project READMEs establish feasibility, not Ravel benchmarks. No measured ranking of SevTech's
  memory/startup/tick hogs was obtained; the candidates below are explicitly profiling hypotheses. Do not publish
  “fastest/lowest-resource” until the acceptance comparisons exist. [Evidence boundary;
  docs/architecture/overview.md:72–82]

## 1. Repository integration map

### Version data and generation

| Existing seam                                          | What adding 1.12.2 actually requires                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `minecraft/versions/1.21.1/adapter.toml`               | Model for a hash-pinned version input set, source Java release, mappings and reviewed local files. Add a sibling directory; distinguish original game bytecode level from the JDK used to build/run Ravel. Do not change the two existing manifests. [adapter.toml:1–103; crates/ravel-minecraft-generate/src/inputs.rs:29–75,140–160]                 |
| `hooks.toml`                                           | Semantic capability inventory, exact JVM descriptors, side and refusal constraints—not a list of names to search/replace. 1.21.1 already excludes incompatible capabilities; 1.12.2 needs fresh review, including deliberate replacements for essential missing seams. [minecraft/versions/1.21.1/hooks.toml:1–48,93–142]                              |
| `mappings.lock`                                        | Exact class/member resolutions against pinned bytes; regenerate from authenticated inputs, never manually transplant modern resolutions. [minecraft/versions/1.21.1/mappings.lock:1–15; crates/ravel-minecraft-generate/src/resolve.rs:72–117,134–230]                                                                                                 |
| `reflection.toml`                                      | Generated `GameNames`/`GameMembers` roles consumed by shared bootstrap reflection; port shapes as well as spellings. [minecraft/versions/1.21.1/reflection.toml:1–48; jvm/bootstrap/dev/ravel/jvm/Members.java:9–13,55–59]                                                                                                                             |
| `stubs.toml`, `stubs.lock`, local stub/template inputs | Compile-only game API stand-ins, resolved and checked against pinned game classes. Legacy stand-ins must match 1.12.2, not invent modern registry/saved-data APIs. [minecraft/versions/1.21.1/stubs.toml:1–38; adapter.toml:72–103; crates/ravel-minecraft-generate/src/stubs.rs:114–157,348–412]                                                      |
| Version harness and local overrides                    | 1.21.1 documents eight game harnesses and a sibling override; shared/local templates and reach checks support API-shape changes. Add genuine 1.12.2 harness cases plus live tests; distinguish a compile-time stand-in from game execution. [minecraft/versions/1.21.1/README.md:27–60; crates/ravel-minecraft-generate/src/screen.rs:297–309,377–401] |

- `crates/ravel-minecraft-generate` owns input/schema/hash validation, mapping interpretation, role agreement, hook
  resolution, stub checks and Java/Rust provenance emission. Its current lock regeneration requires **both a client jar
  and a DFU jar**, and generation reads ProGuard mappings on the mapped path; these assumptions must become explicit
  capabilities before 1.12.2 can be represented honestly. [src/inputs/schema.rs:249–318; src/lib.rs:232–260,291–379;
  src/mapping.rs:24–56,194–230; src/roles.rs:48–89,124–143; src/render.rs:343–406,781–820]
- `crates/ravel-minecraft-adapter/build.rs` selects `RAVEL_MINECRAFT_VERSION`, invokes generation and exports metadata;
  the shared adapter consumes generated targets. Keep one selected version per artifact and reject cross-version
  lock/artifact reuse. [build.rs:17–44; src/lib.rs:7–18,59–63; docs/architecture/overview.md:97–101]
- The `jvm/` agent resolves generated entry names, detects side at distribution entry, and shares reflection through
  `Members`; `Content`, `Datapack`, `Outward` and `Persistence` contain actual game API assumptions. A successful
  mappings port alone cannot make those shared adapters work on 1.12.2.
  [jvm/bootstrap/dev/ravel/jvm/Agent.java:14–24,66–70; Members.java:9–13,55–59; Content.java:190–202;
  Datapack.java:55–67; Outward.java:100–118; Persistence.java:12–18]
- `xtask generate java` calls Java generation; `xtask generate locks` resolves/regenerates locks. Add mapping-kind and
  optional-DFU support through these existing verbs, not a separate legacy generator. Preserve explicit regeneration
  before Java builds because cached Cargo build scripts are not sufficient.
  [tools/ravel-xtask/src/main.rs:69–90,108–119; Makefile.toml:328–332,415–421]
- `Makefile.toml` sweeps version directories but mapped-version eligibility currently depends on `client.txt`;
  generalize eligibility to the manifest's mapping input, not a fake ProGuard file for Tiny mappings. Include 1.12.2 in
  Java, lock, harness and adapter matrices. `mise.toml` selects default 26.1.2 and Java 27.0.0+35; retain the default
  unless Jacob chooses otherwise. [Makefile.toml:94–126,280–330,403–421; mise.toml:17–23]
- Documentation contracts to amend: SPEC's default/proof statement, exact-version compatibility policy and ATM10
  definition; ROADMAP's version tree, version-boundary checks and unproven live integration; architecture's currently
  ProGuard-only binding description. Preserve historical milestone claims rather than rewriting their meaning.
  [SPEC.md:4–5,206–225,484–496; ROADMAP.md:269–275,398–410,1042; docs/architecture/overview.md:46–49,78–82]

### ATM10 wiring that the cutover must cover

| Surface                     | Existing coupling and required disposition                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Descriptors                 | `tests/fixtures/descriptors/atm10-tts-2.0.2.json:3–25` pins workload identity, hashes, MC/NeoForge and 324 references. Add a SevTech descriptor; preserve ATM10. Update fixture provenance instructions and add the 1.12.2 runtime-input descriptor beside existing versions. [tests/fixtures/descriptors/README.md:7–31; runtime-inputs-1.21.1.json:1–12; runtime-inputs-26.1.2.json:1–12]                                                                                    |
| xtask default and tests     | `tools/ravel-xtask/src/commands.rs:4–6,11–27` supplies default descriptor/archive behavior; integration fixture tests hard-code ATM10 expectations and helpers. Make the new default explicit and retain named ATM10 regression cases. [tools/ravel-xtask/tests/integration/fixture.rs:12–16,164–175,288–290]                                                                                                                                                                  |
| Schema task / launch recipe | The Make schema task explicitly names ATM10 and runtime 1.21.1; Prism's runbook names ATM10/1.21.1 and a Java 21.0.7 launch. Add all supported descriptor checks and a JDK27/1.12.2 recipe, without relabeling the old launch evidence. [Makefile.toml:148–151; docs/runbooks/prism-launch.md:9–13]                                                                                                                                                                            |
| Evidence arithmetic         | M6 closes a particular historical inventory; M7 defaults to **324** unless corrected. New evidence must bind workload identity and derive its denominator from the new manifest, with separate references/jars/runtime-container counts. Do not merely replace 324 globally. [tools/ravel-xtask/src/evidence/m6.rs:1–40; m7.rs:1–20,40–70]                                                                                                                                     |
| Registry defaults           | `RAVEL_FIXTURE_ID` defaults to ATM10 in the README and `checks/public-repo.sh`, `rebuild-proposals.sh`, `verify-counterparts.sh`, `verify-spec-identifiers.sh`, `verify-wave.sh`. Update the default consistently; explicit fixture selection must continue to replay ATM10. [registry/README.md:52–56; respective scripts:22–24,23–25,53–57,15–17,32–36]                                                                                                                      |
| Registry consumers          | `verify-reads.sh`, `verify-rights.sh`, `verify-surfaces.sh` consume the selected fixture. Exercise both identities and avoid accidental reads from the old inventory. [registry/checks/verify-reads.sh:70–72; verify-rights.sh:18–20; verify-surfaces.sh:28–30]                                                                                                                                                                                                                |
| Historical inventory        | Keep `registry/inventory/atm10-tts-2.0.2/{graph,references,plans,rights,surfaces}.json` and `registry/provenance/atm10-tts-2.0.2.json` intact; build parallel SevTech records and new waves. The old provenance includes prohibited/uncleared rights and cannot authorize new counterparts. [graph.json:11012–11013; references.json:14–30; plans.json:28–30; rights.json:15936–15939; surfaces.json:22075–22078; registry/provenance/atm10-tts-2.0.2.json:4–32,56–57,107–120] |
| Counterpart selection       | Reuse the rights-aware foundation/gameplay selector, not ATM10's numerical candidate set or modern-version package identity assumptions. Existing M6 foundation selection is workload-specific. [registry/checks/select.py:26–29,54–107; registry/waves/m6-foundation.json:4–8,25–36]                                                                                                                                                                                          |

## 2. Primary-source findings and design consequences

### Latest-Java runtime and LWJGL replacement

- Cleanroom is a concrete 1.12.2 precedent: Java 25+, LWJGL3/SDL3, an LWJGL2 API shim named LWJGLY, Foundation replacing
  LaunchWrapper, and modernized dependencies. Its compatibility work includes Fugue; this is evidence that a broad
  runtime replacement is feasible, **not evidence that unmodified Forge/SevTech runs on JDK27**.
  [https://github.com/CleanroomMC/Cleanroom; https://github.com/CleanroomMC/Fugue]
- Recommended Ravel route: launch official 1.12.2 through the existing premain path without adopting Forge LaunchWrapper
  as the production loader; replace window/input/audio/context calls with pinned LWJGL3 and replace legacy rendering
  directly. Use a narrow legacy graphics facade only where retained vanilla code still requires it, then remove it at
  renderer cutover; do not grow a general Forge compatibility host. Cleanroom/LWJGLY is a reference implementation, with
  license review before reuse. [Design recommendation; docs/architecture/overview.md:27–57;
  https://github.com/CleanroomMC/Cleanroom]
- Expect strong module encapsulation (`InaccessibleObjectException`), obsolete `--illegal-access`, non-URL application
  class loaders, absent `rt.jar`/`tools.jar` and extension-directory assumptions, changed default charset and removed
  Java8-era components. Inventory actual referenced APIs/dependencies and replace them rather than guessing an unlimited
  list of module-opening flags.
  [https://docs.oracle.com/en/java/javase/25/migrate/migrating-jdk-8-later-jdk-releases.html]
- Audit reflection/Unsafe/final-field tricks, old bytecode tooling and native library loading. Prefer generated
  accessors/owned code changes; keep any indispensable `redefineModule` opens narrowly documented. Explicitly authorize
  native access for the actual Hook/LWJGL modules; classpath-based loading may require
  `--enable-native-access=ALL-UNNAMED`, which is broad and must be disclosed. Premain is not dynamic attach. [Design
  recommendation; docs/architecture/overview.md:32–57; https://openjdk.org/jeps/472; https://openjdk.org/jeps/403]
- Replacing LWJGL is larger than replacing jar coordinates: context ownership, render-thread affinity,
  resize/fullscreen/HiDPI, keyboard/mouse semantics, clipboard, sound, native extraction and platform libraries need
  integration evidence. This scope follows Cleanroom's distinct LWJGLY and runtime components; these are proposed
  acceptance areas, not claims of existing Ravel support. [Design inference; https://github.com/CleanroomMC/Cleanroom]

### Mapping choice before official Mojang mappings

| Candidate          | Primary evidence                                                                                                                                                                                                                                                                                                                                                   | Recommendation                                                                                                                                                                                                                                                              |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ornithe Feather    | CC0-1.0; legacy mapping project with 1.12.2 coverage and Tiny output, using `official` (obfuscated), Calamus `intermediary`, and human-readable `named`. [https://github.com/OrnitheMC/feather]                                                                                                                                                                    | **Preferred canonical names**. Pin a real published 1.12.2 artifact/commit, namespace chain and digest; prove class/member coverage against the official jar before committing. `official` here does **not** mean Mojang's readable official names. [Design recommendation] |
| MCP / SRG          | MCPConfig publishes SRG data under a modified zlib license and has a 1.12.2 configuration. That does not make all historical MCP human-name CSV artifacts identically licensed; MCPConfig explicitly distinguishes itself from MCP. [https://github.com/MinecraftForge/MCPConfig; https://github.com/MinecraftForge/MCPConfig/tree/master/versions/release/1.12.2] | Keep an audited SRG crosswalk for Forge-reference research and comparison fixtures if needed. Do not choose historical MCP human-readable names without identifying and reviewing the exact artifact/license. [Design recommendation]                                       |
| Legacy Fabric Yarn | CC0 mappings covering legacy versions, including 1.12.2 work; project contribution policy excludes copying MCP/Spigot/Mojang names. [https://github.com/Legacy-Fabric/yarn]                                                                                                                                                                                        | Credible alternate if Feather coverage fails; select one canonical namespace, not a blended dictionary with unclear provenance. [Design recommendation]                                                                                                                     |

- Mojang announced publication of mappings with future releases in snapshot 19w36a (September 2019); do not fabricate
  `client_mappings` for the older 1.12.2 manifest. Add a mapping-format abstraction that normalizes
  identity/unobfuscated, ProGuard and pinned Tiny into the existing resolver, including descriptors and inherited-member
  rules. [https://www.minecraft.net/en-us/article/minecraft-snapshot-19w36a;
  crates/ravel-minecraft-generate/src/lib.rs:344–379; src/resolve.rs:134–230]
- Preserve runtime obfuscated names at the agent boundary. Names aid authoring; pinned class bytes, descriptors, access
  flags and unique resolution remain the authority. Ship mapping/license metadata, not redistributed game code. [Design
  recommendation; docs/architecture/overview.md:46–49;
  https://www.minecraft.net/en-us/article/minecraft-snapshot-19w36a]

### Hook ports: semantic targets, not modern-name substitutions

The following legacy spellings are **research locators**, not approved Feather names or final obfuscated descriptors.
Resolve every chosen member from the pinned jars and record it in the new lock; verify both client/integrated and
dedicated-server paths. [Design requirement; crates/ravel-minecraft-generate/src/resolve.rs:72–117,134–230]

| Capability                   | 1.12.2 port and incompatibility                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Server/client tick           | Forge's server patch identifies `MinecraftServer.func_71217_p()V` and world-update `func_71190_q()V`; there is no modern `BooleanSupplier` parameter to carry. Locate client run-tick separately and define exactly once-per-tick semantics, including integrated-server override/delegation. Do not manufacture a has-time-left predicate. [https://github.com/MinecraftForge/MinecraftForge/blob/1.12.x/patches/minecraft/net/minecraft/server/MinecraftServer.java.patch; minecraft/versions/1.21.1/hooks.toml:136–148]          |
| Registration and freeze      | Legacy registries use resource keys **and numeric IDs**, with block/item registration ordering; there is no equivalent modern `BuiltInRegistries.freeze` contract. Introduce an explicit Ravel-controlled registration barrier and stable ID assignment/serialization policy, hooking actual legacy bootstrap/registration sites. [https://docs.minecraftforge.net/en/1.12.x/concepts/registries/; hooks.toml:93–134; docs/architecture/overview.md:18–25]                                                                          |
| Block states / item metadata | Preserve blockstate property identity, metadata conversion (legacy block metadata is 0–15), actual/extended states and save interpretation. Compact representations must not break reference identity expectations or silently flatten distinct metadata items. Investigate the pack's JustEnoughIDs implications before choosing numeric storage widths. [https://docs.minecraftforge.net/en/1.12.x/blocks/states/; publisher archive `modlist.html` below]                                                                        |
| Network payloads             | The Forge reference uses `SimpleNetworkWrapper`, discriminators and `ByteBuf`, with handlers arriving off the game thread and explicit scheduled tasks. Implement bounded Ravel packets at the corresponding vanilla channel/dispatch seams; preserve main-thread world mutation and do not pretend modern payload codecs exist. Forge-wire compatibility is a separate decision. [https://docs.minecraftforge.net/en/1.12.x/networking/simpleimpl/; docs/architecture/overview.md:27–28,69–71]                                     |
| Saved data                   | Port the aggregate to legacy `WorldSavedData` / `MapStorage`, `readFromNBT`, `writeToNBT`, `markDirty`, `getOrLoadData` and `setData`; distinguish global/per-dimension data and client/server instances. Preserve the existing 1MiB bound, staged/live identity, migration and failed-replacement behavior; modern codec/factory saved-data APIs are not targets. [https://docs.minecraftforge.net/en/1.12.x/datastorage/worldsaveddata/; docs/architecture/overview.md:63–68; jvm/bootstrap/dev/ravel/jvm/Persistence.java:12–18] |
| Rendering/screens            | Replace chunk compilation/drawing, model upload and state management as a subsystem, while keeping GUI observation separate from drawing/refusal. Legacy 1.12 render APIs do not provide modern screen-extraction semantics; absence requires a new reviewed capability rather than relabeling a draw hook. Nothirium demonstrates async chunk compilation and multiple GL backends in this generation. [minecraft/versions/1.21.1/hooks.toml:37–44; https://github.com/Meldexun/Nothirium]                                         |
| Resources/recipes            | Replace the modern datapack/reload assumptions with a version-local legacy resource manager, model/atlas reload and recipe-registration adapter. Define one resource-generation barrier for native counterparts; do not emit a server datapack-reload event for a seam that does not exist. [jvm/bootstrap/dev/ravel/jvm/Datapack.java:55–67; minecraft/versions/1.21.1/hooks.toml:20–35; https://github.com/embeddedt/VintageFix]                                                                                                  |
| Data fixing                  | 1.12.2 has the older `DataFixer(1343)` / Forge compound datafixer path, **not** modern Mojang DataFixerUpper. Make the generator's DFU jar optional by capability; preserve legacy migrations and do not bolt on modern DFU just to satisfy the current schema. [https://github.com/MinecraftForge/MinecraftForge/blob/1.12.x/patches/minecraft/net/minecraft/util/datafix/DataFixesManager.java.patch; crates/ravel-minecraft-generate/src/lib.rs:232–260]                                                                         |

### Exact proof inventory and performance hypotheses

- Source of truth: publisher release **file 3569915**, not the modpack's current source branch. The downloaded archive's
  `manifest.json` pins Minecraft **1.12.2**, **Forge 14.23.5.2860**, SevTech: Ages **3.2.3**. Its observed SHA-256 is
  `f872775f7ac0996fdd1bde177a1517aecc43e9209da97406dcbbe3ac842483b8`.
  [https://www.curseforge.com/minecraft/modpacks/sevtech-ages/files/3569915;
  https://mediafilez.forgecdn.net/files/3569/915/SevTech_Ages_3.2.3.zip — `manifest.json`]
- **Observed inventory correction:** the manifest contains **254 file references**, plus **five bundled override jars**:
  GalacticraftCore/Planets and MicdoodleCore 4.0.2.280, Nutrition 4.3.0, SevTweaksNPC 0.0.4. That is **259 jar inputs**,
  not a verified loaded-mod count; multi-container jars and library/coremod distinctions require separate accounting.
  The “about 180 mods” premise is not the correct denominator for this exact release. [Same publisher archive —
  `manifest.json`, `overrides/mods/`; complete published list: archive `modlist.html`]
- Capture every project/file ID, fetched jar hash, side, dependencies, embedded jars, configs/scripts, override assets
  and rights. The pack README's progression description is useful context, but repository HEAD is not an immutable 3.2.3
  configuration snapshot; use the release overrides. [Publisher archive above;
  https://github.com/DarkPacks/SevTech-Ages]

| Candidate pressure       | Exact-release components to profile, not a measured culprit ranking                                                                                                                                                                                                                                                                                                      |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Heap/model bake/startup  | Chisel, Chisels & Bits, CTM, Tinkers' Construct, Construct's Armory, JEI; plus many registry/property/model combinations. Inspect retained states/quads/textures, duplicate strings and recipe indexes. [Publisher archive `modlist.html`; mechanisms: https://github.com/asiekierka/FoamFix; https://github.com/embeddedt/VintageFix]                                   |
| Client CPU/GPU           | Better Foliage, Fancy Block Particles, Mo' Bends, Smooth Font, connected textures; profile visible-section compilation, animated texture work, particles and UI separately. [Publisher archive `modlist.html`; renderer precedent: https://github.com/Meldexun/Nothirium]                                                                                                |
| Startup/world generation | CraftTweaker/ContentTweaker/ModTweaker scripts; Geolosys, Streams/Farseek, Recurrent Complex, Traverse, Betweenlands, Twilight Forest, AbyssalCraft, Beneath, Galacticraft/ExtraPlanets. Separate script load/recipe expansion from first-use generation and exploration. [Publisher archive `modlist.html` and `overrides/`; https://github.com/DarkPacks/SevTech-Ages] |
| Tick/network             | Immersive Engineering/Petroleum, BuildCraft modules, AE2, Refined Storage, Mekanism, Integrated Dynamics, XNet, Industrial Foregoing, Better With Mods. Benchmark representative machines/networks/entities, not just an empty spawn. [Publisher archive `modlist.html`; test design recommendation]                                                                     |
| Progression              | Game/Recipe/Item/Ore/Mob/Dimension/Waila/Tinker/Multiblock/JourneyMap Stages, Triumph, Together Forever, Prestige, ZenStages/ZenLoot and Sev-specific patches. Stage-dependent visibility must be correct before a smaller retained data set can count as an optimization. [Publisher archive `modlist.html`; https://github.com/DarkPacks/SevTech-Ages]                 |

### Concrete optimization portfolio

| Technique and evidence                                                                   | Proposed ownership and guardrail                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FoamFix-style compact/deduplicated states/models; VintageFix dynamic model loading/cache | Keep game-facing Java identities where required; Rust can own packed geometry, immutable indexes, content-hash dedup and bounded workers. Measure reload and worst-case first-use stutter, not only menu heap. Do not retain an unbounded cache or both full Java and Rust mirrors. [https://github.com/asiekierka/FoamFix; https://github.com/embeddedt/VintageFix; docs/architecture/overview.md:50–53]                                                                                                                              |
| VanillaFix-style dynamic textures/resource fixes and reduced allocation                  | Port only measured behavior-preserving techniques; pack VanillaFix is **not** assumed installed. Its source is MIT; inspect exact code before reuse. Do not adopt exception swallowing or continue-after-corruption behavior as a speed feature. [https://github.com/DimensionalDevelopment/VanillaFix; publisher archive `modlist.html`]                                                                                                                                                                                              |
| Sodium-style architecture, with Nothirium's 1.12 precedent                               | Recommend a native section mesh pipeline: compact immutable snapshots, asynchronous meshing, batched upload, visibility/culling and explicit translucent sorting. Keep context-owning Java/LWJGL3 rendering on its thread; one bulk JNI exchange per batch, not per vertex/block. Exact backend/API choice requires hardware evidence. [Design recommendation; https://github.com/Meldexun/Nothirium; docs/architecture/overview.md:50–53]                                                                                             |
| String/resource-key canonicalization and recipe indexes                                  | Prefer load-scoped intern tables and compact numeric handles, released on reload, over blindly interning arbitrary/user/network strings forever. Rust owns native tables; Java owns object identities exposed to vanilla. Profile hash/index time and retained size. [Design recommendation motivated by https://github.com/asiekierka/FoamFix; https://docs.minecraftforge.net/en/1.12.x/blocks/states/]                                                                                                                              |
| Lazy migration/data initialization                                                       | Defer expensive optional resource/model work with bounded first-use costs; preserve required world migration before admission. “Lazy DFU” is not a direct 1.12.2 optimization because this target uses the older fixer. [Design recommendation; DataFixesManager patch above; docs/architecture/overview.md:63–68]                                                                                                                                                                                                                     |
| Modern GC / compact headers                                                              | Start with JDK27 defaults; compare G1 and available low-pause alternatives at equal correctness and total RSS. Record heap, metaspace, direct/native/GPU allocations and pauses; a smaller `-Xmx` alone is not lower resource use. [https://openjdk.org/projects/jdk/27/; measurement recommendation]                                                                                                                                                                                                                                  |
| CDS / AOT cache                                                                          | Treat as an experiment, not guaranteed startup savings. JEP483 limits AOT to built-in class loaders and rejects configurations involving certain module flags, class-file hooks or bootstrap/system search additions; Ravel currently adds its agent to bootstrap search and transforms classes. Verify exact JDK27 eligibility/cache-use logs; a silently ignored cache is failure. Do not dismantle premain for a speculative win. [https://openjdk.org/jeps/483; https://openjdk.org/jeps/514; docs/architecture/overview.md:32–39] |

The release already includes FoamFix, BetterFps, FastWorkbench, FastFurnace, Clumps, RoadRunner and RandomPatches.
Benchmark against the **actual optimized pack**, not a stripped baseline; do not count replacement of an
optimization-only mod as gameplay parity, or stack duplicate optimizers blindly. [Publisher archive `modlist.html`;
design recommendation]

## 3. Dependency-ordered work packages

All paths below are proposed edit/add ownership, not claims that new files already exist. Every package retains both
existing version targets; acceptance must be performed by the implementation owner, not inferred from this research.
Existing adapter/version-boundary contracts remain the starting point. [SPEC.md:206–225; ROADMAP.md:398–410]

### WP0 — Freeze scope, inputs, rights and measurement contract

- **Depends:** none. **Files/crates:** new SevTech descriptor/provenance inventory under existing
  `tests/fixtures/descriptors`, `registry/inventory`, `registry/provenance`; amend SPEC/ROADMAP/architecture. Use the
  exact archive and separate 254 references/5 bundled jars/loaded containers. [Existing conventions: descriptor
  README:7–31; registry/provenance/atm10-tts-2.0.2.json:4–32; publisher archive]
- **Work:** define native behavioral parity, progression milestones, supported OS/GPU set,
  networking/world-compatibility boundaries and benchmark scenes. Record each mod's license/allowed
  evidence/reimplementation basis before counterpart work; pack license does not grant every mod's rights. [Design
  recommendation; registry/checks/select.py:26–29,54–67]
- **Acceptance evidence:** signed-off immutable input ledger, resolved jar count/container taxonomy, per-input rights
  verdicts and reproducible baseline launch configuration. Freeze baseline worlds, seeds/configs and scripts, not just a
  mod list. [Design acceptance; docs/architecture/overview.md:72–82]
- **Risks:** redistributability of bundled jars/assets, unavailable historical downloads, incomplete parity definition.
  No unsupported claim of “full SevTech” before the complete inventory is dispositioned. [Rights boundary:
  registry/provenance/atm10-tts-2.0.2.json:56–57,107–120]

### WP1 — Generalize mappings and optional legacy capabilities

- **Depends:** WP0 input selection. **Files/crates:**
  `crates/ravel-minecraft-generate/src/{inputs/schema.rs,inputs.rs,mapping.rs,resolve.rs,lib.rs,roles.rs,stubs.rs,render.rs}`
  and generator tests; `tools/ravel-xtask/src/main.rs`. [Current responsibilities: repository map above]
- **Work:** add tagged mapping formats/namespaces with common resolver output; Feather/Tiny support and audited SRG
  crosswalk; remove mandatory DFU only for manifests declaring the legacy/no-DFU capability. Preserve strict
  unknown-field/hash/descriptor/uniqueness checks. [Design recommendation; inputs/schema.rs:249–318;
  lib.rs:232–260,344–379]
- **Acceptance evidence:** real 1.12.2 class-byte resolution, descriptor/overload/inheritance/namespace tests, negative
  wrong-hash and ambiguous-member cases, stable lock regeneration; byte-for-byte unchanged semantics for existing
  ProGuard/unobfuscated targets. [Design acceptance; resolve.rs:72–117,134–230]
- **Risks:** incomplete mapping coverage, intermediary/name drift, mistakenly treating SRG or Tiny `official` as
  readable Mojang names; license contamination through untracked merged dictionaries. [Feather/MCPConfig/Legacy Yarn
  sources above]

### WP2 — Add complete 1.12.2 version data and matrix participation

- **Depends:** WP1. **Files/crates:**
  `minecraft/versions/1.12.2/{adapter.toml,hooks.toml,mappings.lock,reflection.toml,stubs.toml,stubs.lock}` plus
  version-local templates/harness; `ravel-minecraft-adapter`; `Makefile.toml`, `mise.toml`, `jvm/README.md`. [Existing
  analogues: section1]
- **Work:** hash official client/server artifacts, mapping artifacts and local inputs; record source bytecode level
  separately from JDK27 execution. Inventory every essential hook and every unsupported optional capability explicitly.
  Generalize matrix eligibility and runtime-input fixture; keep default 26.1.2 unless decided otherwise. [Design
  recommendation; inputs.rs:140–160; Makefile.toml:94–126,280–330]
- **Acceptance evidence:** `generate locks`, `generate java`, adapter build and version-local harness for all three
  selected versions; deliberate cross-version/hash drift rejection; tracked-dataset boundary checks. This package proves
  generation, not live play. [Design acceptance; ROADMAP.md:398–410,1042]
- **Risks:** version leakage into shared loader/SDK; stubs that compile but misdescribe real bytecode; incomplete
  dedicated-server artifact coverage. [SPEC.md:206–225; docs/architecture/overview.md:23–25]

### WP3 — Latest-GA vanilla boot with LWJGL3 platform replacement

- **Depends:** WP2; may proceed alongside WP4 after hook shapes settle. **Files/crates:** version-local runtime/library
  inputs; `jvm/bootstrap`, agent build/harness entrypoints; Prism launch runbook; native graphics/platform boundary in
  the existing adapter ownership, with a new focused renderer crate only if WP5 establishes a real seam.
  [jvm/README.md:10–28,93–112; docs/architecture/overview.md:32–57]
- **Work:** remove production reliance on Java8 LaunchWrapper/classloader internals; replace legacy dependency/API
  assumptions; install pinned LWJGL3 natives and replace window/input/audio paths. Audit module opens/native-access
  permissions and preserve the premain-generated Hook/JNI surface. Do not add a generic Forge host. [Design
  recommendation; Cleanroom and Oracle migration/JEP472 sources above]
- **Acceptance evidence:** actual official 1.12.2 client reaches menu, creates/reopens a world, resizes/changes
  fullscreen, accepts input/audio and exits cleanly on exact JDK27; dedicated server starts/saves/stops without loading
  client natives; native-access denial audit passes with documented explicit grants. [Proposed acceptance based on
  docs/architecture/overview.md:32–57; JEP472]
- **Risks:** OS/context/input differences, unsupported legacy libraries, native crashes, ordering of class
  transformation/native registration. JDK25 Cleanroom success is insufficient acceptance.
  [https://github.com/CleanroomMC/Cleanroom; https://openjdk.org/jeps/472]

### WP4 — Port registration, tick, network, resources and persistence end to end

- **Depends:** WP2, with live closure on WP3. **Files/crates:** 1.12.2 hook/reflection/template/harness inputs;
  `jvm/bootstrap/dev/ravel/jvm/{Agent,Members,Content,Datapack,Outward,Persistence}.java`; `ravel-minecraft-adapter` and
  existing bootstrap callback consumers. [Existing seams: section1; architecture:32–71]
- **Work:** implement the semantic ports in the hook table, including explicit registration freeze, stable
  pre-flattening IDs/metadata, legacy resource/recipe lifecycle and WorldSavedData aggregate. Maintain one authoritative
  game state and batched outward actions; replace essential absent modern APIs rather than marking the entire feature
  unsupported. [Design recommendation; hook table; docs/architecture/overview.md:18–25,50–71]
- **Acceptance evidence:** live native content appears with correct states/metadata; exactly-once ticks; bounded
  malformed-packet rejection/main-thread handling; resource reload without stale handles; global/dimension
  save/reopen/migration and failed-replacement tests; dedicated/integrated parity. Test removed capabilities as explicit
  rejections, not silent no-ops. [Design acceptance; Forge registries/states/network/saved-data sources above]
- **Risks:** irreversible world-ID corruption, duplicate tick callbacks, thread misuse, client/server state collision,
  modern event names concealing different semantics. [Forge sources above; hooks.toml:20–44]

### WP5 — Replace the renderer and model/state memory pipeline

- **Depends:** WP3 and WP4 registration/resource barriers. **Files/crates:** version-local render hooks/templates; Java
  LWJGL3 context/upload layer; `ravel-minecraft-adapter` integration; proposed dedicated Rust renderer module/crate if
  it owns snapshots/meshes/jobs without exposing game internals to SDK. [Architectural boundary:
  overview.md:23–25,50–53; renderer precedents above]
- **Work:** build compact state tables and deduplicated/lazy models; immutable bounded section snapshots; Rust
  mesh/visibility workers; batched Java-owned GL submission; explicit translucent/animated/connected-model support;
  reload-generation cancellation. Remove obsolete LWJGL2/draw/model paths when replaced, not dual-render indefinitely.
  [Design recommendation; VintageFix/Nothirium/FoamFix sources above]
- **Acceptance evidence:** golden scenes and frame captures for opaque/translucent/animated/staged content, chunk
  updates and reload; frame-time percentiles, compile/upload time, total RSS/native/VRAM and first-use stalls;
  correctness under rapid world unload/reload. Compare against real SevTech optimized baseline on agreed GPUs. [Proposed
  evidence; exact inventory above]
- **Risks:** incorrect transparency/culling, snapshot races, native use-after-free, memory moved off heap rather than
  saved, modern-GL-only design excluding Jacob's machines. [Design risk analysis; Nothirium multiple-backend precedent
  above]
- **Status:** slices 1–4 completed: [vanilla baseline](vanilla-baseline.md),
  [pinned renderer lifecycle contract](../../../docs/runbooks/legacy-launch.md#renderer-lifecycle-observations),
  [bounded observational section snapshots](section-snapshots.md) and
  [bounded Rust mesh/visibility workers](mesh-workers.md).
  Official live reload/unload/reopen advances generations, rejects stale work and proves actual retired-pool
  drain. Captured game full-opaque-cube properties select geometry; unsupported state keys are explicit.
  Two Rust workers share eight work slots and a fixed native byte reservation ceiling. Physical-GPU and
  Xvfb moving stress both reach the ceiling without stale publication or leaked shutdown reservations.
  Slice 5 completed [bounded Java-owned batched upload](batched-upload.md), retained generation/key
  transfer and exact acknowledgement. Explicit off-by-default geometry-proof selection submits only
  eligible complete solid geometry offscreen; vanilla remains authoritative and owns world drawing.
  Default and selected silent Xvfb live acceptance pass; the selected actual same-section vanilla solid
  projection has nonempty foreground and zero RGBA/coverage/depth mismatches. Six locked RTX 5090
  physical trials (three off/three on) and separate strict reductions completed with disclosed
  frame/copy/upload/residency/RSS/heap/device-VRAM results and frozen-versus-live artifact differences.
  Stress p95/p99 trial-median changes were +3.19%/+0.32%, not a like-for-like vanilla speedup.
  Slices 6–7 retain model/texture/AO/layer/reload fidelity and default cutover.
  This is not full visual parity or demonstrated resource savings.

### WP6 — Profile-guided startup, tick and JVM optimization

- **Depends:** WP3–WP5 usable baselines; iterate on representative WP8 counterparts. **Files/crates:** generator/Java
  bootstrap only for measured load costs; native content/index/renderer owners; workload fixtures and existing evidence
  tooling. [Design ownership; overview.md:50–53,72–82]
- **Work:** classify cold/warm startup, class/model/script/recipe phases; canonicalize bounded immutable data, index
  recipes, batch crossings and native jobs; remove unnecessary modern-DFU assumptions; compare GC/header defaults and
  eligible CDS/AOT configurations. Do not move authoritative vanilla world state into a permanent Rust mirror.
  [Optimization portfolio; overview.md:50–53]
- **Acceptance evidence:** repeated cold/warm menu/world timings, fixed-path exploration, loaded-machine/stage scenes,
  p50/p95/p99 ticks/frames, GC pauses, CPU and total memory with cache eligibility logs; report distributions and
  ablations against each technique. Refuse “improvement” bought by missing content, reduced simulation or prewarmed-only
  comparisons. [Design acceptance; JEP483; overview.md:72–82]
- **Risks:** benchmarking cache/scene bias, hidden first-use work, resource shifting, native overhead exceeding saved
  Java allocations. No numeric win target is defensible until WP0 baselines are recorded. [Design risk analysis;
  evidence boundary]

### WP7 — Cut over descriptor/default/evidence machinery cleanly

- **Depends:** WP0 manifest/rights ledger and WP2 runtime descriptor. **Files/crates:** every ATM10 wiring row in
  section1; `tools/ravel-xtask` commands/tests/evidence; Make schema task; registry defaults/selectors; Prism
  documentation; SPEC/ROADMAP. [Cited repository inventory above]
- **Work:** new immutable `sevtech-ages-3.2.3` identity and archive/runtime provenance; default new work to it, preserve
  explicit ATM10 replay. Derive workload-aware denominators and surface separate jar/reference/container metrics; retain
  historical M6/M7 records under original identities. [Design recommendation; m7.rs:1–20,40–70; overview.md:78–82]
- **Acceptance evidence:** descriptor extraction reproduces archive hash and 254+5 accounting; tampered inputs fail; old
  ATM10 commands still replay explicitly; all schema/default/registry checks select the intended workload; new evidence
  cannot accidentally use denominator324 or old rights. [Design acceptance; section1 wiring]
- **Risks:** historical evidence rewritten by global replacements, pack references mistaken for gameplay parity, bundled
  jars omitted by manifest-only extraction. [Publisher archive; m7.rs:40–70]

### WP8 — SevTech-native counterpart starting set, then full closure

- **Depends:** WP0 rights and graph, WP4 hooks; use WP5/WP6 for performance-sensitive packages. **Files/crates:** new
  selected inventory/provenance/waves and rights-cleared native packages under `registry/mods`; shared reusable SDK
  behavior only where already version-independent. Do not rename ATM10 counterparts into proof of legacy parity.
  [registry/checks/select.py:54–107; overview.md:83–87]
- **Starting wave A (platform):** map dependency roles for stage predicates, recipe/content registration,
  config/scripts, registries/metadata, world storage, resource visibility and cross-mod IDs. Candidate references:
  CraftTweaker/ContentTweaker, Game/Recipe/Item/Ore Stages, Triumph/Together Forever and Sev-specific patches. Implement
  their required pack behavior natively, not a universal Java/ZenScript interpreter by default. [Publisher archive
  `modlist.html`/`overrides`; design recommendation]
- **Starting wave B (first playable progression):** follow the exact release's first-age scripts/advancements to select
  required items, blocks, interactions, worldgen/ores and shared libraries. Select by dependency closure and rights, not
  recognizable mod names; keep an executable mapping from each script/recipe/advancement requirement to native behavior.
  [Design recommendation; archive overrides; https://github.com/DarkPacks/SevTech-Ages]
- **Starting wave C (stress anchors):** select representative rights-cleared IE/BuildCraft/AE2-or-RS machine/network
  behavior, a high-cardinality model family, and one dimension/worldgen chain. These create early realistic
  memory/tick/render pressure, but are not a substitute for later progression closure. [Design recommendation;
  exact-release component inventory above]
- **Acceptance evidence:** each selected package has upstream-input provenance, rights disposition, native
  identity/aliases if justified, behavior scenarios and negative tests; a fresh world completes the selected progression
  slice without unobtainable recipes or stage leaks. Then expand waves until **every** release input is implemented,
  intentionally non-behavioral, replaced with equivalent proved behavior, or explicitly blocks full-pack acceptance.
  [Design acceptance; overview.md:18–21,72–87; registry/checks/select.py:26–29]
- **Risks:** scripts hide dependency edges, liberties change pack progression, incompatible world IDs, rights exclude
  crucial behavior sources. Starting waves are scheduling only; they must never be marketed as completed SevTech.
  [Design risk analysis; archive overrides; existing registry rights constraints above]

### WP9 — Live proof, regression matrix and claim gate

- **Depends:** WP3–WP8; full proof requires all inventory dispositions, not only starting waves. **Files/crates:**
  existing structured acceptance/transcript/evidence machinery, version harnesses, workload provenance and launch
  runbooks; update SPEC/ROADMAP only to match demonstrated status. [overview.md:72–82,92–101; ROADMAP.md:1042]
- **Acceptance evidence:** official 1.12.2 runtime on exact latest-GA pin with premain/JNI; full fresh-world progression
  and late-age machine/dimension scenarios; save/reopen, resource reload, integrated/dedicated play; independent
  correctness observations plus benchmark raw data/configs; all three version matrices pass. Preserve known absent
  optional capabilities explicitly. [Proposed acceptance; SPEC.md:206–225; overview.md:32–75]
- **Performance claim gate:** compare original SevTech/Forge with its shipped optimizers on a suitable original
  supported JVM, a current-Java Cleanroom comparison where compatible, and Ravel, holding
  content/scenes/settings/hardware constant and disclosing unavoidable runtime differences. Include empty loader cost
  separately from full-workload cost. Claim a win only for measured configurations; “fastest loader” without a defined
  competitor/task set is not established. [Design measurement recommendation; publisher manifest/modlist;
  https://github.com/CleanroomMC/Cleanroom]
- **Risks:** test worlds fail to cover gated late-game behavior; a renderer benchmark hides missing simulation; Java27
  incompatibility in the comparator is confused with Ravel performance. Negative results must remain in the record.
  [Design risk analysis; overview.md:72–82]

## 4. Decisions Jacob must make

| Decision                                                              | Recommendation and consequence                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Default selected MC version                                           | Keep 26.1.2 as build default; change the proof default to SevTech/1.12.2 explicitly. They are distinct concepts; selecting a workload must select matching version artifacts. [mise.toml:17–23; SPEC.md:4–5; design recommendation]                                                                                                                                                            |
| “SevTech support” means native parity or hosting original Forge jars? | **Native behavioral parity**, consistent with the existing contract. Original Forge/Cleanroom instances are comparison/evidence sources. Hosting 259 inputs as Java mods would be a different product scope requiring explicit authorization. [overview.md:27–49; publisher archive; design recommendation]                                                                                    |
| Canonical legacy mapping                                              | Feather/CC0 after pinned-artifact coverage proof; Legacy Yarn fallback. Separate SRG reference crosswalk only when audited; no unreviewed MCP CSV import. [Primary mapping sources above; design recommendation]                                                                                                                                                                               |
| Launch/runtime reuse                                                  | Own the minimal Ravel runtime cutover; study Cleanroom/LWJGLY/Foundation/Fugue rather than wholesale dependency adoption. Approve any code reuse only after exact license/maintenance review. [https://github.com/CleanroomMC/Cleanroom; https://github.com/CleanroomMC/Fugue; design recommendation]                                                                                          |
| Renderer API/hardware floor                                           | LWJGL3 with a capability-selected GL path initially; choose a higher floor or alternate backend only after Jacob names required GPUs/OS and representative evidence favors it. Do not preserve LWJGL2 for compatibility's sake. [Nothirium/Cleanroom precedents; design recommendation]                                                                                                        |
| Existing Forge saves / servers                                        | Preserve newly created Ravel 1.12.2 saves and metadata rigorously; treat import of existing Forge/JustEnoughIDs worlds and arbitrary Forge-wire interoperability as explicit separately tested requirements, not automatic promises. Recommend requiring pack-world import only if Jacob needs continuity. [Forge states/registries/network sources; overview.md:27–28; design recommendation] |
| Benchmark success bar                                                 | Require no behavioral regressions and lower total resource use/latency on disclosed representative scenes, then set numerical targets from WP0 measurements. Do not set invented heap/RSS/startup guarantees now. [overview.md:72–82; design recommendation]                                                                                                                                   |
| Rights-constrained counterpart scope                                  | Approve permitted/conditional evidence sources per package; do not waive existing registry rights gates to accelerate the new proof. A blocked critical input blocks a full parity claim until a lawful implementation basis is obtained. [registry/checks/select.py:26–29,54–67; design recommendation]                                                                                       |
| GC/AOT strategy                                                       | JDK27 defaults first, measured tuning second. Keep AOT optional and disclose incompatibilities with instrumentation/bootstrap search; prioritize deterministic game semantics and the existing premain architecture. [JDK27/JEP483 sources; overview.md:32–39; design recommendation]                                                                                                          |

**Completion definition:** three independently pinned/gated targets; official 1.12.2 genuinely running on the latest-GA
pin through Ravel; LWJGL3 and the replacement renderer working end to end; every SevTech3.2.3 input accounted for with
lawful native behavior or justified non-behavioral disposition; reproducible live progression/save/network/render
evidence and honest resource comparisons. None of the proposed work packages is an observed implementation result.
[Design acceptance grounded in SPEC.md:206–225; overview.md:18–25,32–82; exact publisher archive]
