# Ravel's JVM side

Three artefacts enter the game process: the premain agent jar, the native
library, and the screen jar. This directory holds the Java sources for the two
jars. Ravel-owned screens are drawn in Java; see the
[screen mechanism](../docs/architecture/overview.md#mechanisms).

## Sources and builds

- `bootstrap/`: package `dev.ravel.jvm`, including the agent entry point,
  transformer, and generated hook target record.
- `build/build-agent.sh`: builds `target/ravel-jvm/ravel-agent.jar` from the
  tracked sources and generated `Hook` class.
- `screen/template/`: one template per screen-jar class. Game classes and members
  appear as `{{…}}` placeholders because their names must link directly in the
  constant pool. The adapter generator renders them into
  `target/ravel-jvm/generated-screen/` alongside the stubs they compile against.
- `build/build-screen.sh`: builds the screen jar from those generated sources.
- `harness/`: JVM drivers and stand-ins, including transformer targets, generated
  game templates, and the class-file remapper.

Build the generated sources before either jar:

```console
cargo run -p ravel-xtask -- generate java
./jvm/build/build-agent.sh
./jvm/build/build-screen.sh
```

Neither jar build needs the client jar. The generator can read the tracked
`mappings.lock`, and the transformer compiles without game types on its class
path. Fixed entry timestamps and sorted entries make jar builds deterministic.

## Launching with it

Set two stock Prism instance arguments:

```text
-javaagent:<path to ravel-agent.jar>
-Dravel.host=<absolute path to libravel_bootstrap.so>
```

`cargo make ravel:host-build <instance> <cache> <minecraft>` produces the host at
`<instance>/ravel/host/libravel_bootstrap.so`. Removing the launch arguments
switches Ravel off without uninstalling it.

`ravel-screen.jar` must sit beside the agent jar on clients and dedicated
servers. The agent takes no arguments and resolves that sibling by name. If it
is absent, launch prints `ravel-screen.jar is missing or not a regular file
beside the agent jar` and continues without settings screens, the mod list,
world persistence, or registered content. Servers need the jar's content
registration classes even though they draw no screen; without it, they print
`content registration has no component to register through`. Nothing checks
that the two jars were copied together.

The generated `Hook` initializer reads `ravel.host` and calls `System.load` on
that file. A missing property or invalid path fails before the transformer is
installed. At shutdown the agent reports rewritten classes, rewritten methods,
and hook invocations reaching native code.

### Why the second argument is a property and not a library path

Prism places instance arguments before its own
`-Djava.library.path=<instance>/natives`, so the launcher's last value wins. It
also deletes that directory after launch. `ravel.host` instead names a file
outside the launcher's managed natives directory.

## Proving it without the game

```console
cargo make ravel:verify-jvm
```

This gate starts a JVM and runs the harness for every live-eligible tracked
version. It is part of `ravel:verify`. Minecraft 1.12.2 currently declares
`eligibility = "generation-only"`: no modern stand-in harness is run for it,
and reconciliation reports every live obligation as not yet run.
The harness tests agent startup and native crossings, but only a real game
launch proves attachment to the official client; one is recorded in
[`evidence/m2/prism-launch.json`](../evidence/m2/prism-launch.json).

The component commands are:

```console
cargo run -p ravel-xtask -- generate java
cargo build -p ravel-composed-host --release
./jvm/build/build-agent.sh
./jvm/harness/run-harness.sh
```

`ravel-composed-host` produces `libravel_bootstrap.so` for the fixed JVM harness test-mod set.
The generic `ravel-bootstrap-runtime` runtime is an rlib with no selected mod set.

### Generating the stand-in game

The adapter generator renders `harness/template/` into separate directories
under `target/ravel-jvm/generated-harness/`:

- `game/`: templates named after the role they declare. For example,
  `BLOCK.java.in` uses `{{package}}` and `{{BLOCK}}` from that role's linked
  class name. Nested classes become top-level sources with `$` in their names.
- `driver/`: Ravel-owned `main` classes in the unnamed package. A command line
  names them directly. They reference screens through placeholders such as
  `{{ravel RavelSettingsScreen}}`, since screens may themselves be unnamed.
- `sibling/`: stand-ins for the screen jar's persistence classes, compiled
  against the stand-in game instead of the generated stubs.

Screens use the linked game's package. Under an obfuscated version that may be
the unnamed package; `ScreenNames` tells the agent where they were generated.

Java cannot name an unnamed-package class from inside a named package. When a
version's stand-in game spans both, all stand-ins are written in the unnamed
package. `generated-harness/aliases.txt` records the differing names as
`written=linked` lines. The file exists even when no aliases are needed.
`ScreenNames` and the sibling templates use that same package decision.

After the last `javac` for each tree, `harness/remap/Remap.java` restores the
linked names in compiled class files. It rewrites class tokens in constant-pool
entries, descriptors, and signatures, copies the remaining bytes unchanged,
and moves renamed class files to their linked paths. An empty alias file makes
this a no-op. A matching name in a string constant is refused rather than
rewritten: it could be a `GameNames` value rather than a type reference.

Per-version templates live under
`minecraft/versions/<version>/harness/{game,driver,sibling}/`. An override
replaces a whole shared template, so edits to the shared file do not affect
versions that override it. Only `game/` may add version-only templates, and an
unreferenced addition is refused. References are checked across all three trees
and the screen sources. Driver, sibling, and screen overrides must replace an
existing shared template.

Signatures are hand-written Java with role placeholders for game types; only
names are substituted. The generated descriptor must match the member the
agent resolves. Guava and DataFixerUpper names are not mapped game names;
`harness/guava/` and `harness/datafixers/` supply their stand-ins.

### What each version proves, and what it does not

Every live-eligible version must run all thirteen sections. Eligibility defaults
to `live`; 1.21.1 and 26.1.2 cannot downgrade to generation-only. The explicit
1.12.2 generation-only declaration requires generated Java compilation and an
adapter build, not adapter tests, harness coverage, playability, or live support.

`cargo xtask version-matrix <minecraft>` prints `live`, `generation-only`, or
`unavailable: <first absent path> is absent` for the selected version. Acquire
the named pinned input and rerun to discover any next missing input. Matrix
tasks report missing local fixtures loudly as `SKIPPED`, not as a successful
compilation, test, or live run.

The generation-only harness is explicitly recorded as `NOT-YET-RUN`; adapter
tests and all thirteen live harness sections remain not-yet-met obligations.

The legacy adapter uses the official client and dedicated-server jars with
Feather build 31 mappings. `sourceJavaRelease = 8` describes the upstream
bytecode; Ravel's generated code still compiles and runs on JDK 27 through
`javaRelease = 27`. Legacy screen templates live under
`minecraft/versions/1.12.2/templates/screen/`; generation does not render the
modern game, driver, or sibling harness trees for that version.

Lock-only generation requires every declared game and library jar to be absent.
A partially available set is an error naming the missing jar, including a
missing DataFixerUpper jar beside a modern client jar. Either acquire the full
pinned set or remove all its local jars to use the tracked locks.

- **Transformer sections** (initialization, hierarchy, refusal, members,
  transformation, load, and side) use hand-written targets and stand-ins. Neither
  half of their comparison comes from version data.
- **Content, outward, settings, screens, and persistence** resolve generated
  names against per-version generated stand-ins. Settings and screen drivers
  press buttons and inspect widget lists across reinitialization. They read
  screen names from `ScreenNames`, title-menu row behavior from `MenuLayout`,
  and settings targets and expected method counts from the generated hook table.
  Persistence uses remapped class files where Java cannot express the linked
  signatures.
- **Cluster** installs packages, discovers and verifies them, then plays, saves,
  exits, and reloads a world across processes. The reload must write a successor
  record; a corrupted record must refuse the world.

For cluster runs, `RAVEL_HARNESS_PACKAGE_DIRS` supplies colon-separated absolute
package directories instead of the two under `test-mods/`. Every manifest and
declared digest is still verified. `RAVEL_HARNESS_HOST` selects a host for those
packages, and `RAVEL_HARNESS_COMPOSED_MODS` gives its member count. The other
sections use the compiled host. The reported `mods=` count comes from the host,
not from the installed package count.

The harness does not exercise `Members`' reflection walk against real
obfuscated class files. The generator checks declared members against the
pinned jar, but exercising the walk with real game classes requires a game run.

`harness/net/minecraft/util/StringUtil.java` is a hand-written stub with
invented bodies and the reviewed names and descriptors, not copied Minecraft
bytecode. Its transformer targets are also hand-written in
`harness/initialization/dev/ravel/jvm/Hook.java`, keeping the following
expectations independent of version data:

```text
allowed=true
filtered=a
b
simple=ab
valid=true
transformed=1 methods=4
installed=true unknown=false
[ravel] rewrote 1 classes and 4 methods; 5 hook invocations reached native code
```

The return values check that injected calls preserve method bodies and operand
stacks. The single loaded class has four rewritten methods. Five native calls
are expected because `filterText(String)` delegates to its two-argument overload
and both are rewritten. The shutdown count comes from `Hook.nativeInvocations()`;
`-1` means the library did not load.

Jar reproducibility can be checked across rebuilds with
`sha256sum target/ravel-jvm/ravel-agent.jar`. Native JNI exports track the
callable inventory plus the hand-written invocation counter; fields are not
call targets.

## Class loading and native boundaries

### Who loads the library, and from which class loader

`dev.ravel.jvm.Boot` appends the agent jar to the bootstrap search path before
naming other Ravel classes. The generated `Hook` class loads the native library
from its own initializer, which the agent forces eagerly. JNI binds the library
to that defining loader, so native methods and their library share one loader.

Rewritten classes resolve `dev.ravel.jvm.Hook` through their own loader's
bootstrap parent. This also works for the bundled server, whose game loader
bypasses the system loader. Game adapters use `Members.game` and the loader that
defined the distribution entry point, not the system loader. Screens use a
separate `URLClassLoader` parented by that game loader.

The pinned client measurement in
[`evidence/m3/class-loader-identity.json`](../evidence/m3/class-loader-identity.json)
found the rewritten game class's loader and the system loader to be the same
object. That observation does not apply to the bundled server.

Appending to bootstrap disables the JVM's shared class archive. The harness
passes `-Xshare:off` to keep that notice out of exact-output cases. Eager native
loading makes missing libraries fail at startup instead of during a hook call.

### What the panic guard and the veto path cost, and what shape they have

A panic unwinding out of `extern "system"` aborts the process. JNI crossings use
`EnvUnowned::with_env` and `resolve::<ThrowRuntimeExAndDefault>` to turn caught
panics and ordinary errors into `RuntimeException` on the calling game thread.
An exception unwinds the rewritten method without an injected branch.
Inventory-opted-in void targets also support boolean refusal: `true` returns
early from the host method without inventing a return value.

With no listener, `crate::guard::enter` performs a relaxed atomic increment and
load, without allocating or attaching an environment. Argument collection,
string copies, and observers run inside the guarded closure. The guarded path's
cost is unmeasured; the retired unguarded JVM-entry spike does not measure it.

### JVM artifact readers

The generator reads class files with `noak`, including Java modified UTF-8,
and parses ProGuard records with `proguard`. Ravel retains exact descriptors,
member selection and bijective namespace checks. `cafebabe` was evaluated but
cannot expose the raw descriptors needed to reject trailing malformed text.

Feather Tiny v2 artifacts are normalized at build time by the pinned Fabric
`mapping-io` JAR into the same validated class/member resolver. Runtime and
reviewed namespaces are explicit; missing or repeated names are refused.
Only a manifest declaring the legacy capability may omit DataFixerUpper;
modern versions still require their pinned DataFixerUpper input.
The normalizer executes `JAVA_HOME/bin/java`; `mise` supplies the pinned JDK 27.
An absent Java home or a runtime older than Java 11 is refused explicitly.

The normalizer pin is `net.fabricmc:mapping-io:0.9.1` (Apache-2.0),
262096 bytes, SHA-256
`aedc94b25a2fe7c09c898144ce33a1ef396680e984b07525d34916922d6ee3f4`,
from <https://maven.fabricmc.net/net/fabricmc/mapping-io/0.9.1/mapping-io-0.9.1.jar>.

`ravel:verify-jvm` verifies the normalizer pin and runs its regression harness
before any game-version harness. It can also run independently of game downloads:

```sh
mkdir -p target/mapping-io-tests
mise exec -- javac -Xlint:all -Werror \
  -cp tests/fixtures/local/tooling/mapping-io-0.9.1.jar \
  -d target/mapping-io-tests \
  crates/ravel-minecraft-generate/java/NormalizeMappings.java \
  crates/ravel-minecraft-generate/java/NormalizeMappingsTest.java
mise exec -- java \
  -cp tests/fixtures/local/tooling/mapping-io-0.9.1.jar:target/mapping-io-tests \
  NormalizeMappingsTest
```

JAR entry lookup uses `zip` with default features disabled and only the Rust
DEFLATE backend enabled. Entries are read without extraction, and decompression
is bounded by each entry's declared size and the original 32-bit size ceiling.
Ravel rejects unsupported compression, Unicode-path overrides and duplicate names
rather than checking bytes under a name the JVM would not load.

The Rust reader licenses are MIT/Apache-2.0 (`noak`), MIT (`zip`) and BSD-3-Clause
(`proguard`). Exact-version license exceptions in `deny.toml` cover `proguard`
and the Zlib-licensed archive dependencies `foldhash` and `zlib-rs`; they do not
broaden the repository's general allow list. `cargo deny check` remains required.

`jni` is built without `invocation`: Ravel runs inside a JVM rather than hosting
one.
