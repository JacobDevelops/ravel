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

This gate starts a JVM and runs the harness for every tracked version. It is
part of `ravel:verify`. It tests agent startup and the native crossings, but only
a real game launch proves attachment to the official client; one is recorded
in [`evidence/m2/prism-launch.json`](../evidence/m2/prism-launch.json).

The component commands are:

```console
cargo run -p ravel-xtask -- generate java
cargo build -p ravel-composed-host --release
./jvm/build/build-agent.sh
./jvm/harness/run-harness.sh
```

`ravel-composed-host` produces `libravel_bootstrap.so` for the selected mod set.
The generic `ravel-bootstrap` runtime is an rlib with no selected mod set.

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

Every tracked version runs all thirteen sections. `cargo xtask verify
gate-versions` rejects missing sections, and `cargo xtask verify scaffold`
rejects the retired `[harness.notRun]` table.

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

### Why `miniz_oxide` is the archive dependency

The adapter's archive reader parses the ZIP central directory and uses
`miniz_oxide` only for DEFLATE. It needs exact entry lookup, not archive writing,
encryption, or other compression formats. The dependency and its `adler2`
dependency are pure Rust; its licence is `MIT OR Zlib OR Apache-2.0`.

The reader accepts stored and deflated entries and rejects other compression
methods and Zip64. The pinned client jar fits classic ZIP limits. `jni` is
built without `invocation`: Ravel runs inside a JVM rather than hosting one.
