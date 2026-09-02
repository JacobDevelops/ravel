# Ravel's JVM side

Three artefacts enter the game process — the premain agent jar, the native
library, and the screen jar — and this directory holds the Java sources of the
first and the third.

[ADG-6](../docs/decisions/adg-6-configuration-ui.md) draws Ravel's screens in
Java, and [ADG-1's amendment](../docs/decisions/adg-1-jvm-entry.md) settles that
the screen is a separate artefact rather than something the agent jar grows: a
screen names game types, and the agent jar deliberately compiles without the
client jar. M3-W3 answered how it reaches a loader that resolves those types —
the agent appends the screen jar to the system class loader's search path through
the `Instrumentation` it already holds, and that loader is the one that loads the
game, which ADG-1's second amendment measured rather than assumed.

- `bootstrap/` — the premain agent's tracked sources: the entry point, the class
  transformer, and the record the generator fills. Package `dev.ravel.jvm`.
- `build/build-agent.sh` — builds `target/ravel-jvm/ravel-agent.jar` from those
  sources plus the generated `Hook` class.
- `screen/template/` — the screen artefact's tracked sources, one template per
  Java class. They name game classes and members inside `{{…}}` rather than
  spelling them, because a screen extends the game's `Screen` and overrides its
  methods: every game name it uses is a constant-pool entry the JVM links
  directly, so unlike the agent jar's adapters it cannot reach one through a
  runtime string. `ravel-minecraft-adapter`'s build script renders each template
  into `target/ravel-jvm/generated-screen/`, in the name space the selected
  version links, in the same pass that emits the stubs they compile against.
  Under an unobfuscated version that is the reviewed name space and the generated
  source is the template with its placeholders filled in; under an obfuscated one
  the game is in the unnamed package, so the screens are generated there too and
  `ScreenNames` tells the agent jar where they went.
- `build/build-screen.sh` — builds the screen jar from those generated sources.
- `harness/` — a stub class and a `main` that run the whole path against a real
  JVM without the game. See [Proving it without the game](#proving-it-without-the-game).

The generated `Hook` class is not tracked, and neither are the generated screen
sources or the stubs they compile against. `ravel-minecraft-adapter`'s build
script writes them under `target/ravel-jvm/`, so a build of that crate has to
have happened before either jar can be built:

```console
cargo build -p ravel-minecraft-adapter
./jvm/build/build-agent.sh
./jvm/build/build-screen.sh
```

Neither step needs the restricted client jar. The generator falls back to the
tracked `mappings.lock` when the jar is absent, and the transformer names no game
type, so it compiles without the game on its class path.

The jar is never tracked — `.jar` is a forbidden tracked extension and
`ravel-verify-scaffold` enforces it. It is built deterministically (fixed entry
timestamps, sorted entries) so its hash can be recorded as evidence.

## Launching with it

ADG-5 decided Ravel reaches the game through two stock Prism instance settings
and no launcher change:

```text
-javaagent:<path to ravel-agent.jar>
-Dravel.host=<absolute path to libravel_bootstrap.so>
```

A host `ravel-cli host build` produced sits at
`<instance>/ravel/host/libravel_bootstrap.so`, and that is the path a launch
names. Ravel is still switched off by removing the `-javaagent` argument.

`ravel-screen.jar` has to sit in the same directory as the agent jar. The agent
takes no arguments, so it finds the screen jar by resolving that name beside the
jar it was itself loaded from, and both are built into `target/ravel-jvm/`. Copy
one to an instance without the other and the launch prints `ravel-screen.jar is
missing or not a regular file beside the agent jar`, then runs with no settings
screen, no mod list and no world persistence. The derivation is exact; what is
fragile is that nothing checks the two jars travelled together.

The agent takes no arguments; what it rewrites comes from the generated class it
ships with. The library is named rather than searched: the generated `Hook`
reads `ravel.host` in its own static initializer and calls `System.load` on the
file it names. A launch that sets no property, or one whose property names
something that is not a file, throws from that initializer before the
transformer is installed and says which property and which path. Removing these
two arguments switches Ravel off without uninstalling anything.

At shutdown the agent prints one line naming how many classes and methods it
rewrote and how many hook invocations reached native code. ADG-5's launch proved
the JVM accepted the `-javaagent` and the client ran with it attached; it could
not prove the rewrite happened, because nothing reported it. That line is what closes it, and
`evidence/m2/prism-launch.json` records the launch where it did.

### Why the second argument is a property and not a library path

ADG-5 first described the second argument as `-Djava.library.path` appending the
directory holding the library, and the first real launch showed it does not
work. Prism 11.0.3 puts the instance's `JvmArgs` on the command line *before*
its own `-Djava.library.path=<instance>/natives`, and the last one wins, so
appending a directory through the instance setting has no effect. The launcher
also deletes that natives directory when a launch ends, so a library placed
there lasts one launch.

M2 and M3 launched by staging `libravel_bootstrap.so` into `<instance>/natives`
immediately before starting, which ADG-5 recorded as a workaround and told
nobody to build on. Its amendment chose the fix: the generated `Hook` calls
`System.load` on a path a `-D` property supplies. `ravel.host` is that property.
It is Ravel's own name in Ravel's own namespace, so nothing about the library
depends on a directory the launcher owns, populates on launch, and deletes on
exit.

## Proving it without the game

A launch is the only thing that proves Ravel reaches the official client, and
`evidence/m2/prism-launch.json` records one. Everything short of the client is
provable without it, and `harness/` is where:

```console
devenv tasks run ravel:verify-jvm
```

That runs all four steps for every tracked version and is inside `ravel:verify`,
so the harness is a gate rather than something to remember. It did not used to
be, and M3-W7 is what changed it: the harness sat here passing nothing, and a
null `System.getProperty("ravel.profiling")` reaching a native method aborted
every launch for a whole work package without a single check going red. This is
the only thing in the gate graph that starts a JVM, so it is the only thing that
runs `premain` at all.

### What each version proves, and what it does not

The harness stands in for the game rather than loading it, and its stand-in
classes are written under Mojang names. That divides the sections in two, and the
division is the honest limit of what this directory can prove.

The **transformer sections** — initialization, hierarchy, refusal, members,
transformation, load and side — reach the game only through the class transformer, and the harness
supplies the transform targets as well as the stand-in classes. Neither side of
what they compare is version data, so they run at every tracked version and no
version may declare its way out of one. A version that stopped running one of
them did not meet an obstacle; something broke.

The **reflective and screen sections** — persistence, content, outward, the
settings round trip and the screens — compare a *generated* name against a stand-in. `GameNames` and
`GameMembers` carry the names each version links, and those are the Mojang names
only under a version that does not obfuscate. Where the two differ, the adapter
asks for `m` while the stand-in declares `getUiState`, and the section would be
measuring the mismatch rather than the adapter. `run-harness.sh` decides which
case it is by reading the generated class itself, where every role carries the
Mojang name in its comment and the linked name in its value, so no version is
named in the script.

A section a version does not run has to say so in that version's `adapter.toml`,
under `[harness.notRun]`, with a reason naming the work package that closes it.
`cargo xtask verify gate-versions` refuses an undeclared absence, and refuses a
declaration that outlived its reason. Closing the remaining ones means generating
the stand-in game per version from templates, which is its own work package.

**What no version of this harness proves** is `Members` against real obfuscated
class files. That class walks a class's supertypes and rebuilds erased descriptors
from reflection objects, and here it always meets a stand-in whose linked name is
its Mojang name. The generator checks that every declared member exists in the
pinned jar, which covers the names; the walk itself is exercised only by a real
jar, which the headless client run and the planned server harness are what cover.

The four steps, when one of them is what you want on its own:

```console
cargo build -p ravel-minecraft-adapter
cargo build -p ravel-composed-host --release
./jvm/build/build-agent.sh
./jvm/harness/run-harness.sh
```

`ravel-composed-host` is the player-local generated boundary that produces
`libravel_bootstrap.so`; the generic `ravel-bootstrap` runtime is an rlib and
contains no selected mod set.

`harness/net/minecraft/util/StringUtil.java` is a stub carrying the internal
name, member names and exact descriptors the reviewed inventory pins, with
invented bodies. It is not a Minecraft asset — no game bytecode, resource or jar
is copied, and `minecraft/versions/*/hooks.toml` and `mappings.lock` already
track the same strings. The transformer declares no game type — it compares
target classes against internal names it holds as string literals — so nothing
here needs the client.

The script refuses any output but this one:

```text
allowed=true
filtered=a
b
simple=ab
valid=true
transformed=1 methods=4
[ravel] rewrote 1 classes and 4 methods; 5 hook invocations reached native code
```

Every line of it is a claim:

- **Return values survive.** `filtered` keeps the newline and `simple` strips it,
  so the transformer prepended the hook call and left the body intact. A
  clobbered operand stack or a swallowed return shows up here first.
- **One class, four methods.** The inventory pins 31 callable targets across 15
  classes; the harness stubs only `StringUtil`, so nothing else is ever loaded
  and nothing else is rewritten. That is correct behaviour rather than a miss,
  and it is why these two numbers do not move when the inventory grows — they
  count what the harness loads, not what the inventory pins.
- **Five invocations from four calls.** `filterText(String)` delegates to
  `filterText(String, boolean)` and both are rewritten, so the delegating call
  fires a second hook. Four would mean the one-arg overload matched and the
  two-arg one did not — exactly the failure a name-only match produces, and the
  reason the two overloads are in the stub.
- **The `[ravel]` line is a real JNI call.** The shutdown hook reads the counter
  through `Hook.nativeInvocations()`; `-1` means the library never loaded.

Two further assertions hold outside the script and are recorded with it:

```console
sha256sum target/ravel-jvm/ravel-agent.jar   # stable across rebuilds
nm -D --defined-only target/release/libravel_bootstrap.so | grep -c ' T Java_'
```

The jar hash is unchanged by rebuilding, which is what `jar --date` and the
sorted entry list buy. The symbol count is 32 and every name is a `Hook` method:
31 generated exports and one hand-written counter. It tracks the reviewed
inventory, so it moves whenever `hooks.toml` gains or loses a method entry —
fields are not call targets and never appear here.

## Three questions M2-W5 owed by name

### Who loads the library, and from which class loader

The generated `Hook` class loads it, from its own static initializer, and the
agent forces that initializer to run from `premain`.

JNI binds a loaded library to the class loader that loaded the class whose code
called `System.load`, and resolves a native method only against that
loader's copy of the declaring class. What binds it is which class's code
called, not which overload it called, so naming the file rather than searching
for it changes nothing here. If some other class loaded the library, a
second copy of `Hook` under a different loader would fail at the first hook
invocation with `UnsatisfiedLinkError` — at launch, not at build. Putting the
call in the declaring class's own initializer makes the loading loader and the
declaring loader the same one by construction rather than by convention, so the
failure mode cannot be reached by accident.

Two consequences follow, and both hold for the pinned game:

- The agent jar is appended to the system class path by the JVM before `premain`
  runs, so `Hook` is loaded by the system class loader and the library binds
  there.
- The rewritten call site resolves `dev.ravel.jvm.Hook` through the loader that
  loaded the *rewritten* class, and for the pinned launch that is the same loader
  again, so it finds that same copy. No delegation is involved:
  [`evidence/m3/class-loader-identity.json`](../evidence/m3/class-loader-identity.json)
  measured a rewritten `net.minecraft` class and `ClassLoader.getSystemClassLoader()`
  to be one object, `jdk.internal.loader.ClassLoaders$AppClassLoader`, whose only
  parents are the platform and bootstrap loaders. A launcher that isolates the
  game behind a non-delegating loader would break this, and that is a
  compatibility requirement rather than a bug to work around.

Loading eagerly from `premain` rather than lazily at the first hook is
deliberate: an absent or unloadable library then fails at startup, where it is
attributable, instead of mid-game on the render thread.

### What the panic guard and the veto path cost, and what shape they have

A Rust panic that unwinds out of `extern "system"` aborts the process. `jni`'s
answer is `EnvUnowned::with_env`, which wraps its closure in `catch_unwind` and
returns an outcome that `resolve::<ThrowRuntimeExAndDefault>` turns into a thrown
`RuntimeException` — for a caught panic and for an ordinary error alike.

That is also the whole veto path. An observer that refuses becomes an `Err`,
which becomes an exception on the game's own thread, which unwinds the method the
agent rewrote. The injected sequence needs no branch, no discarded return value,
and no invented contract for what a returned value would have meant while ADG-2
is undecided.

The guard is not paid on every crossing. `crate::guard::enter` does two things
before it decides: one relaxed atomic add for the invocation count, and one
relaxed atomic load asking whether anything is listening. Neither allocates,
indexes, or calls anything fallible, so neither can unwind, and when nothing is
listening the call returns without ever attaching an environment or entering a
`catch_unwind`. Everything that can panic — argument collection, the string copy,
the observer itself — happens inside the guarded closure.

What that costs is still unmeasured, and deliberately so: ADG-1's 10.02 ns is the
unguarded shape, measuring the guarded one needs a running JVM, and the verify
gate has none. M2-W8 owns the number. What M2-W5 owes is the shape, and the shape
is: two atomics on the path that observes nothing, an attach guard plus a
`catch_unwind` on the path that can reach mod code, and a thrown exception on the
path that refuses.

### Why `miniz_oxide` is the archive dependency

ADG-3's spike read jar entries by shelling out to `unzip`, one process per class,
and recorded that production picks a Rust archive dependency with its own
`cargo deny` review. It is `miniz_oxide`, used for inflate only, with the zip
central directory read by about a hundred lines in
`crates/ravel-minecraft-adapter/src/generate/jar.rs`.

The job is to pull two entries out of a thirty-thousand-entry archive by exact
name. A general zip crate brings a writer, several compression methods, encryption,
and a directory model none of that needs. `miniz_oxide` is the part that is
genuinely hard — a correct DEFLATE decoder — and it is:

- pure Rust and `#![forbid(unsafe_code)]` itself, so the dependency does not
  reintroduce unsafe under a crate that forbids it;
- one transitive dependency (`adler2`), also pure Rust;
- licensed `MIT OR Zlib OR Apache-2.0`, all on `deny.toml`'s allow list;
- already the backend every other Rust DEFLATE user reaches for, so it is not a
  novel piece of the supply chain.

`cargo deny check` passes on advisories, bans, licences, and sources with it and
with `jni` 0.22.4 added. `jni` is pinned without the `invocation` feature: that
feature exists for hosting a JVM, which ADG-1 rejected, and it is what would drag
`libloading` and a JVM locator into a production crate.

The archive reader is deliberately narrow. It handles stored and deflated entries
and refuses anything else by name, and it does not implement Zip64 — the pinned
client jar has 30675 entries and is 38 MB, both inside the classic format's
limits, and a jar that outgrows them should fail loudly rather than be guessed at.
