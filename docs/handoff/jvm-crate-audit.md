# Ravel JVM-boundary and artifact-library audit

Audit date: 2026-10-15. Repository: `ravel`; ripwire snapshot `3d68f96cf`. Read-only investigation; no source changes,
builds, tests, or dependency installation.

## Executive verdict

The JNI migration and Java bytecode-library migration have **already happened**: Ravel uses `jni 0.22.4` and
`java.lang.classfile`, respectively. Do not propose those as new work. The strongest remaining replacements are the Rust
classfile decoder and ZIP reader, followed by Java reflection descriptor helpers and the ProGuard grammar parser.
Estimated combined saving: **330–500 production lines**, not the thousands of lines of application-specific generation
and tests surrounding them. Estimates below are [INFERENCE], not measured patches. All recommended replacements can
preserve `#![forbid(unsafe_code)]` in every Ravel crate; that does not imply every dependency is internally unsafe-free.

## Inventory

Paths are repository-relative. LOC means physical lines, including comments/tests; production boundaries are called out
to avoid inflating savings. `B` = `crates/ravel-bootstrap/src/`; `G` = `crates/ravel-minecraft-generate/src/`; `J` =
`jvm/bootstrap/dev/ravel/jvm/`.

| Subsystem                                                  | Files and measured LOC                                                                                                                                                                           | Existing implementation / unsafe supplier                                                                                                                                                 |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native entry, references, exceptions, panic/error boundary | `B/guard.rs` 175; `B/dispatch.rs` 132; `B/runtime.rs` 348; `B/lib.rs` 152                                                                                                                        | `EnvUnowned::with_env`, borrowed local objects, generated mangled exports; `jni 0.22.4` → `jni-sys 0.4.1` supplies JNI unsafe internals. No separate raw-JNI boundary.                    |
| Other JNI entry points                                     | `B/content.rs` 261; `mods.rs` 426; `persistence.rs` 722; `profiling.rs` 305; `settings.rs` 399; `report.rs` 41; `outward.rs` 141; `packs.rs` 93; `game_directory.rs` 81; `side.rs` 79            | Same `jni` safe boundary, not independent attach/exception implementations. Counts include substantial non-JNI domain logic.                                                              |
| Public handles / generated initialization                  | `crates/ravel-minecraft/src/lib.rs` 608; `crates/ravel-build/src/generate.rs` 154 (112–148); `crates/ravel-composed-host/src/lib.rs` 456 (59–68); `crates/ravel-minecraft-adapter/src/lib.rs` 97 | Borrowed `JObject<'local>` and `#[jni::jni_mangle]`; adapter is version tables, not another JNI runtime.                                                                                  |
| Native registration / Java-to-Rust wire                    | `B/wire.rs` 93; generated natives in `G/render.rs`; `J/Content.java` 242; `Outward.java` 143; `Persistence.java` 415                                                                             | JNI name-based linking; application UTF-16-length-prefixed wire strings. `jni` handles string access. No `RegisterNatives` or hand-rolled raw function-pointer registry.                  |
| Java loading / agent lifecycle                             | `J/Agent.java` 206; `Boot.java` 98; `Screens.java` 276; `Datapack.java` 138; `HookTarget.java` 28                                                                                                | Java Instrumentation, JarFile, classloaders and reflection; JVM/JDK owns native implementation. Boot appends its JAR to bootstrap search.                                                 |
| Reflection descriptors/member lookup                       | `J/Members.java` 195                                                                                                                                                                             | Hand-built descriptors at 118–158; lookup, loader choice and linked-name policy are Ravel-specific.                                                                                       |
| Java bytecode rewriting                                    | `J/Transformer.java` 487                                                                                                                                                                         | **Standard `java.lang.classfile`**, not ASM. Custom hook-selection/injection policy; no handwritten classfile serializer or stack-map generator.                                          |
| Rust classfile inspection                                  | `G/classfile.rs` 506; production 1–300                                                                                                                                                           | Handwritten constant pool, byte reader, attributes and member/hierarchy extraction; no unsafe dependency currently.                                                                       |
| Rust JAR access                                            | `G/jar.rs` 350; production 1–185                                                                                                                                                                 | Handwritten EOCD/central/local ZIP headers; `miniz_oxide 0.8.9` only supplies DEFLATE. Test ZIP writer starts at 187; not a production packager.                                          |
| Mappings                                                   | `G/mapping.rs` 1,334; production 1–693                                                                                                                                                           | Handwritten ProGuard grammar, descriptors, bidirectional mapping and strict reflection validation. No dedicated mappings dependency.                                                      |
| Source/native-hook generation                              | `G/render.rs` 2,192; production 1–1,371; `resolve.rs` 559; `roles.rs` 401; `inputs.rs` 1,198; `lib.rs` 2,310                                                                                     | Rust source/string generation plus game/version/schema policy. Native ABI generation already uses jni macros. No unsafe generation dependency.                                            |
| Stub and game UI generation                                | `G/stubs.rs` 1,108; `stub_render.rs` 238; `screen.rs` 2,385                                                                                                                                      | Stub surface resolution, Java declarations and version-specific subclasses/templates. Java compiler handles classfile generation.                                                         |
| Harness classfile remapping                                | `jvm/harness/remap/Remap.java` 389                                                                                                                                                               | Handwritten constant-pool rewriting and attribute walk; Java DataInputStream handles modified UTF-8. Invoked by `jvm/harness/run-harness.sh:63–75`.                                       |
| Java compilation / archive writing                         | `jvm/build/build-agent.sh` 47 (34–46); `build-screen.sh` 89 (46–87); `jvm/harness/run-harness.sh` 1,819                                                                                          | Standard `javac` and `jar`; no production handwritten Java JAR writer. Screen build refuses packaged stubs/unlisted generated classes; harness orchestration is not removable parser LOC. |

Reference-management scope: inspected native crossings use JVM-owned native-call frames and local borrows; no retained
global-reference pool, `JavaVM` attachment service, or explicit detach manager was found. Do not add these merely to
wrap a library feature already unnecessary here. `guard.rs:67–71,91–99` delegates entry/panic/error resolution to `jni`;
its own work is collecting arguments, dispatching hooks and distinguishing veto from failure. Safety contract:
`docs/architecture/safety-boundary.md:1–60`; pinned dependencies: `Cargo.toml:19–25`, `Cargo.lock:121–168`. Current MSRV
is **1.95**.

## Ranked replacements

### 1. REPLACE: Rust classfile binary decoder → `cafebabe 0.9.0`

- Delete the bespoke byte reader, pool-tag decoding, fixed-width/header parsing and attribute skipping in
  `G/classfile.rs:201–300`; adapt cafebabe's borrowed class view into existing `Member`/hierarchy structures
  (`:100–155`).
- Estimated net saving **150–220 production LOC**; retain selection, access checks and parser regression tests.
  Investigate descriptor-module reuse in `render.rs:231–305` separately; do not count all type-rendering policy as
  deleted.
- Correctness benefit: current `CONSTANT_Utf8` decoding uses `String::from_utf8_lossy` (`classfile.rs:212–214`), but
  classfiles use **modified UTF-8**. Cafebabe uses Java CESU-8 decoding and preserves otherwise unrepresentable UTF-8
  constants rather than silently treating them as ordinary UTF-8.
- Risk **medium**: preserve unknown attributes, malformed-input refusal, supported constant-pool versions, member
  ordering and extraction semantics. Confirm parser support against both admitted game JARs before cutover.
- Unsafe: Ravel stays forbidden; cafebabe itself declares `#![forbid(unsafe_code)]`. Still review its transitive
  dependencies.
- Sources: [cafebabe docs/source](https://docs.rs/cafebabe/0.9.0/cafebabe/),
  [repository](https://github.com/staktrace/cafebabe), [release metadata](https://crates.io/api/v1/crates/cafebabe).
- Caveat: its last release is 2025-06-03 and repository push 2025-06-23: a small, slower-moving parser, **not** the most
  actively maintained option. `noak 0.7.0` is the actively released alternative if class-version coverage or
  adversarial-input review favors it; do not adopt both.

### 2. REPLACE: handwritten ZIP container reader → `zip 9.0.3`

- Delete central-directory indexing (`G/jar.rs:49–96`), local-header arithmetic and format helpers (`:118–185`); retain
  the small game-JAR abstraction and artifact-admission rules.
- Estimated net saving **100–140 production LOC**. `miniz_oxide` is a compression engine, not a ZIP parser; it cannot
  replace the missing container validation.
- Use read-only `ZipArchive`/entry reads with default features disabled and only the required Rust DEFLATE backend
  enabled. Avoid extracting files onto disk and avoid bringing in unrelated encryption/codecs.
- Risk **medium**: explicitly preserve deterministic enumeration, missing-entry errors, duplicate-entry semantics and
  decompressed-size limits. Current code caps decompression using the declared entry size (`jar.rs:145–155`); a library
  migration must not turn this into unbounded `read_to_end`. Mutable archive access also changes the current
  owner/access pattern.
- Unsafe: safe public ZIP APIs need no Ravel unsafe. ZIP/dependency internals remain part of the audited dependency
  boundary. MSRV 1.88 fits Ravel 1.95.
- Sources: [zip API](https://docs.rs/zip/9.0.3/zip/), [repository](https://github.com/zip-rs/zip2),
  [metadata](https://crates.io/api/v1/crates/zip). Current narrow-parser rationale is `jvm/README.md:231–236`.

### 3. REPLACE: reflected Java descriptor construction → JDK APIs

- Replace `Members.java:124–158` with `MethodType.methodType(returned, parameters).toMethodDescriptorString()` and
  `Class.descriptorString()`; keep the Method wrapper and actual member lookup rules.
- Estimated net saving **30–40 LOC**; **low** migration risk. Arrays, primitives, references and void have
  platform-defined descriptors, so Ravel need not duplicate the switch.
- Dependency/version: existing modern JDK; `MethodType` descriptor support since Java 7, `Class.descriptorString` since
  Java 12. No extra crate and no Rust unsafe.
- Sources:
  [MethodType API](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/lang/invoke/MethodType.html),
  [Class API](<https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/lang/Class.html#descriptorString()>).

### 4. REPLACE: ProGuard record grammar only → `proguard 5.10.4`

- Replace line tokenization, member/line-range grammar and low-level declared-record parsing
  (`G/mapping.rs:376–479,590–637`) with record-level `ProguardRecord::try_parse`.
- Estimated net saving **50–100 production LOC**, not 1,334. Keep the namespace model, descriptor translation
  (`:650–692`), strict duplicates/inverse-collision checks and reflection verification (`:192–337`).
- Risk **medium**: **do not** replace Ravel's mapping with the library's retrace-oriented `ProguardMapper`, or silently
  accept failed records. Preserve rejection of members before classes, empty maps, malformed types and non-bijective
  maps. Feed every relevant record through existing validation.
- Unsafe: public parsing APIs require no Ravel unsafe; MSRV 1.83 fits. This is lowest payoff because validation is the
  bulk of the subsystem, but shared R8/ProGuard grammar is worthwhile if its accepted syntax matches the pinned inputs.
- Sources: [API and record parser](https://docs.rs/proguard/5.10.4/proguard/),
  [repository](https://github.com/getsentry/rust-proguard), [metadata](https://crates.io/api/v1/crates/proguard).

## KEEP verdicts and rejected substitutions

**JNI calls, local/global ownership, exceptions and attachment — KEEP `jni 0.22.4`.** The 0.22 redesign is already in
use: `EnvUnowned`, scoped `Env`, `with_env`, resolution policies and generated mangled exports. Do not rewrap it in a
bespoke VM/attach abstraction. Direct `jni-sys` would move unsafe operations into forbidden Ravel code; it is an
implementation dependency, not a preferable application API. `jvmti`/`jvmti-rs` add low-level tooling interfaces, not a
drop-in safe replacement for the current Java Instrumentation hook path. No approved safe JVMTI boundary is present.
Sources: [jni 0.22.4 docs](https://docs.rs/jni/0.22.4/jni/),
[changelog](https://github.com/jni-rs/jni-rs/blob/master/CHANGELOG.md),
[jni-sys](https://docs.rs/jni-sys/0.4.1/jni_sys/), safety-boundary document above.

**Native registration — KEEP name-based linking.** Generated `#[jni::jni_mangle]` already replaces handwritten symbol
spelling (`ravel-build/src/generate.rs:112–148`). `RegisterNatives` requires raw native-function-pointer registration
and an unsafe boundary; no demonstrated benefit offsets that here. Keep Java declarations and generated Rust exports
synchronized by existing generation.

**Panic guard — KEEP the small policy wrapper, not a new guard crate.** `guard.rs:54–99` relies on jni entry/result
handling; it is not a separate panic-catching runtime. Keep Ravel's hook invocation accounting, absent-observer fast
path and veto mapping. Removing them in favor of a library would remove behavior, not duplication.

**Wire format — KEEP.** `wire.rs:1–36` and corresponding Java readers encode application records using UTF-16-unit
lengths; this is not a second JNI implementation. JSON/Serde would change both sides and add
escaping/allocation/protocol churn, without an existing library owning this domain protocol. Preserve exact Java/Rust
length semantics.

**Generated bindings, Java sources, hooks and stubs — KEEP the domain generators.** `robusta_jni` generates convenient
JNI wrappers; `duchess` offers a richer typed Rust↔Java binding system. Neither replaces Ravel's game-version mappings,
hook phases, obfuscated member resolution, stub selection or generated UI subclasses. `robusta_jni 0.2.2`'s published
API depends on the older jni generation; repository activity is recent, so “abandoned” would be inaccurate, but it is
not a clean replacement for current 0.22 exports. `duchess 0.3.3` is active and worth a bounded future experiment only
if typed invocation becomes a major handwritten surface. Macro-generated unsafe compatibility with Ravel's lint has
**not been demonstrated**; do not promise an unsafe-free cutover based solely on a safe-looking user API. Sources:
[robusta docs](https://docs.rs/robusta_jni/0.2.2/robusta_jni/), [repo](https://github.com/giovanniberti/robusta),
[duchess docs](https://docs.rs/duchess/0.3.3/duchess/), [repo](https://github.com/duchess-rs/duchess).

**Agent bytecode rewriting — KEEP `java.lang.classfile`; do not migrate to ASM now.** `Transformer.java:3–11,197–263`
already delegates parsing, transformation and derived classfile data to the JDK. `:329–487` implements Ravel hook
insertion, cancellation, slot use and special game behavior; ASM would still require this policy. The Class-File API was
finalized in JDK 24 ([JEP 484](https://openjdk.org/jeps/484)); [current JDK 27 GA](https://jdk.java.net/27/) includes
it. It follows the running JDK's format instead of requiring a separately updated bytecode dependency.
[ASM 9.11](https://asm.ow2.io/versions.html), released 2026-10-04, is the credible alternative for an older **runtime**
requirement; its current release supports Java 28 classfiles. Supporting Minecraft 1.12.2's old classfiles does **not**
by itself require Java 8 runtime support. If running the loader on Java 8 becomes a requirement, reconsider ASM for that
target explicitly; the standard API cannot run there. [Mixin](https://github.com/SpongePowered/Mixin) 0.8.7 is a
framework with loader services, transformation/remapping conventions and an annotation processor, not a drop-in
replacement for this small hook policy. Its upstream push date observed was 2024-08-05. Adopting it would expand the
loader contract, not merely delete a serializer.

**Classloader/JAR lifecycle and runtime reflection — KEEP.** `Boot.java:75–77` uses JDK `JarFile` and bootstrap search;
`Screens.java:128` constructs a parented `URLClassLoader`. Isolation, class visibility, signed packages and lifetime
policy are Ravel responsibilities. Replace descriptor spelling only, not these decisions. Production packaging is
already delegated to JDK `javac`/`jar` (`build-agent.sh:34–46`, `build-screen.sh:46–87`).

**Harness `Remap.java` — KEEP under its present byte-preservation contract.** It rewrites constant-pool class
tokens/descriptors/signatures, rejects shared text/type ambiguities and unsupported attributes, then copies bytes after
the pool unchanged (`Remap.java:1–155,183–337`; `jvm/README.md:114–117`). It is harness machinery, not the agent
transformer. ASM `ClassRemapper` would reduce parsing code **if** Ravel deliberately dropped the unchanged-tail
requirement; normal model reserialization is not proof of that property. Therefore no immediate 389-line deletion is
claimed. Keep its regression coverage; avoid creating another production remapper.

## Legacy mappings: Tiny, SRG and Feather

Current admitted versions/formats are narrowly defined; the mapping-input schema has identity/ProGuard, not an already
implemented general-format ecosystem (`G/inputs/schema.rs:8–26`). **KEEP current scope; for upcoming 1.12.2, prefer
`mapping-io 0.9.1` at build time over inventing Tiny/SRG parsers.** This is a conditional new-capability choice, not
current LOC savings. [Fabric mapping-io](https://github.com/FabricMC/mapping-io) provides visitor/tree APIs,
readers/writers for Tiny v1/v2, SRG/TSRG, ProGuard and others
([format enum](https://github.com/FabricMC/mapping-io/blob/dev/src/main/java/net/fabricmc/mappingio/format/MappingFormat.java));
[Maven metadata](https://maven.fabricmc.net/net/fabricmc/mapping-io/maven-metadata.xml) reports release 0.9.1, updated
2026-07-30. Run it as a pinned build-time normalizer and retain Ravel's namespace/bijection/admission validation.
Conversions can lose format-specific information; choose the output representation deliberately rather than assuming
equivalent semantics. [Forge SrgUtils](https://github.com/MinecraftForge/SrgUtils) is another established Java option if
scope stays SRG/TSRG-centric. Mapping-io is the better single multi-format candidate; a comparably comprehensive
maintained Rust suite was not established in this audit. **Feather is a mappings project, not a fourth independent wire
format.** [Ornithe Feather](https://github.com/OrnitheMC/feather) covers legacy releases including 1.12.2 and publishes
Tiny artifacts; consume its published namespace-bearing mappings, not its source `.tinydiff` graph as though that were
the distribution format. Build-time Java normalization introduces no unsafe Rust. Validate intermediary/named/official
namespaces against the actual target artifacts; Tiny and SRG are not automatically interchangeable with Mojang ProGuard
mappings.

## Candidate maintenance and release evidence

Dates below are latest stable **crate releases**, from each linked crates.io API; repository activity was checked
separately. Activity is a maintenance signal, not a soundness guarantee.

| Candidate                                                                  | Stable release / date | Assessment                                                                                                                              |
| -------------------------------------------------------------------------- | --------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| [jni](https://crates.io/api/v1/crates/jni)                                 | 0.22.4 / 2026-03-16   | Already pinned; repo pushed 2026-07-30; MSRV 1.85.                                                                                      |
| [jni-sys](https://crates.io/api/v1/crates/jni-sys)                         | 0.4.1 / 2026-01-09    | Already transitive; low-level unsafe ABI, not an app replacement.                                                                       |
| [jvmti](https://crates.io/api/v1/crates/jvmti)                             | 0.5.0 / 2022-10-12    | Old release; [repo](https://github.com/robberphex/jvmti-rust) last push 2022-10-12.                                                     |
| [jvmti-rs](https://crates.io/api/v1/crates/jvmti-rs)                       | 0.17.0 / 2024-08-03   | Registry repository metadata absent; low-level option, no verified forbid-safe bridge.                                                  |
| [robusta_jni](https://crates.io/api/v1/crates/robusta_jni)                 | 0.2.2 / 2023-10-30    | Old published integration API; repo nevertheless pushed 2026-09-29.                                                                     |
| [duchess](https://crates.io/api/v1/crates/duchess)                         | 0.3.3 / 2026-05-29    | Active repo, pushed 2026-10-15; broader binding/runtime adoption.                                                                       |
| [cafebabe](https://crates.io/api/v1/crates/cafebabe)                       | 0.9.0 / 2025-06-03    | Small borrowed parser, own crate forbids unsafe; slower maintenance cadence.                                                            |
| [classfile-parser](https://crates.io/api/v1/crates/classfile-parser)       | 0.3.8 / 2024-10-10    | [Repo](https://github.com/Palmr/classfile-parser) pushed 2026-04-24; documented Java SE 10 emphasis makes coverage review necessary.    |
| [ristretto_classfile](https://crates.io/api/v1/crates/ristretto_classfile) | 0.34.0 / 2026-09-27   | Active [repo](https://github.com/theseus-rs/ristretto); MSRV **1.97.1 exceeds Ravel 1.95**; not an immediate compatible pin.            |
| [noak](https://crates.io/api/v1/crates/noak)                               | 0.7.0 / 2026-07-10    | Active alternative, MSRV 1.85; [source](https://gitlab.com/frozo/noak) contains internal unsafe UTF handling, behind safe parsing APIs. |
| [zip](https://crates.io/api/v1/crates/zip)                                 | 9.0.3 / 2026-10-13    | Active repo pushed 2026-10-15; MSRV 1.88; keep feature surface narrow.                                                                  |
| [miniz_oxide](https://crates.io/api/v1/crates/miniz_oxide)                 | 0.9.1 / 2026-03-13    | Current repo uses 0.8.9; inflater only, no central-directory abstraction.                                                               |
| [proguard](https://crates.io/api/v1/crates/proguard)                       | 5.10.4 / 2026-07-28   | Maintained Sentry parser/retracer; MSRV 1.83; record API is appropriate seam.                                                           |

## RustSec / soundness scope

The [RustSec advisory tree](https://api.github.com/repos/RustSec/advisory-db/git/trees/main?recursive=1) was checked for
the named Rust candidates. The matching advisory found was
[RUSTSEC-2025-0168](https://rustsec.org/advisories/RUSTSEC-2025-0168.html): ZIP extraction path/symlink traversal,
affecting `zip >=1.3.0,<2.3.0`, fixed in 2.3.0. Proposed 9.0.3 is outside that range; read-only archive-entry access
also avoids extraction policy entirely. No matching advisory was found for the other named candidate crate directories
at audit time. **Absence is not a soundness certification**, and this was not a lockfile-wide/transitive `cargo audit`
or an audit of macro expansion. Java ASM/Mixin/JDK vulnerabilities are not covered by RustSec.

## Method / limits

Used
`ripwire . --for="JNI references exceptions attachment native registration panic guard wire protocol" --token-budget=1200`
and the equivalent classfile/JAR/mappings/generation/transformer query, plus literal/regex repository searches and
targeted reads. Ripwire returned low-confidence compact bundles, so file-level claims were checked against source rather
than treating rankings as proof. LOC counts were taken with `wc -l`; generated/test-heavy files are explicitly not
treated as fully removable. Version/release metadata, upstream source/docs and RustSec were queried directly; no
candidate was compiled against Ravel. Report recommendations are migration designs, not claims of exercised
compatibility.
