# Ravel's JVM side

Three artefacts enter the game process — the premain agent jar, the native
library, and the screen jar — and this directory holds the Java sources of the
first and the third.

[ADG-6](../docs/decisions/adg-6-configuration-ui.md) draws Ravel's screens in
Java, and [ADG-1's amendment](../docs/decisions/adg-1-jvm-entry.md) settles that
the screen is a separate artefact rather than something the agent jar grows: a
screen names game types, and the agent jar deliberately compiles without the
client jar. M3-W3 answered how it reaches a loader that resolves those types —
the agent opens the screen jar under a `URLClassLoader` of its own whose parent
is the loader that defined the distribution entry point, which is where the game
is.

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
- `harness/` — the stand-ins the transformer sections supply themselves, and the
  side markers, which run the whole path against a real JVM without the game. See
  [Proving it without the game](#proving-it-without-the-game).
- `harness/remap/Remap.java` — puts a split version's linked class names back
  into the compiled stand-ins, which is the one thing no Java source can be
  written to do. See [Generating the stand-in game](#generating-the-stand-in-game).
- `harness/template/` — the shared half of the stand-in game, the drivers that
  exercise it and the stand-in half of the sibling artifact, one template per
  class, rendered per version into `target/ravel-jvm/generated-harness/`. The
  per-version half lives with the rest of a version's data, under
  `minecraft/versions/<version>/harness/`. See
  [Generating the stand-in game](#generating-the-stand-in-game).

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

`ravel-screen.jar` has to sit in the same directory as the agent jar, on a
dedicated server as much as on a client. The agent takes no arguments, so it
finds the screen jar by resolving that name beside the jar it was itself loaded
from, and both are built into `target/ravel-jvm/`. Copy one to an instance
without the other and the launch prints `ravel-screen.jar is missing or not a
regular file beside the agent jar`, then runs with no settings screen, no mod
list, no world persistence and no registered content. A server draws no screen
and still needs the jar: the class that registers a mod's blocks and items is the
screen jar's sibling half, and a server without it prints `content registration
has no component to register through`. The derivation is exact; what is fragile
is that nothing checks the two jars travelled together.

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

### Generating the stand-in game

The reader that renders the screen templates renders the harness's stand-in game
too, and for the reason one step further in: a stand-in exists to be resolved
against a generated name, so under an obfuscated version it has to *be* that name.
`jvm/harness/template/` holds three trees and the same `{{…}}` placeholders the
screen templates use. Each renders into a directory of its own below the
generated root, because the run script sweeps a tree whole rather than naming the
files it wants.

- `template/game/` — one template per stand-in game class, **named after the role
  whose class it declares**: `BLOCK.java.in` declares whatever this version binds
  `BLOCK` to. Its `{{package}}` and the simple name `{{BLOCK}}` renders both come
  from that role's linked name, and the file lands at the linked internal name —
  the same rule the generated stubs are placed by, so a nested class arrives as a
  top-level one whose name carries the `$` its binary name has.
- `template/driver/` — Ravel's own `main` classes. They keep the name their
  template gives them and are drawn in the unnamed package under every version,
  because a command line names them. A driver that opens one of Ravel's own
  screens writes `{{ravel RavelSettingsScreen}}` for it rather than importing
  one: the screens move with the game, an unnamed-package class can be imported
  by nothing, and the whole name is legal to spell under either case.
- `template/sibling/` — the stand-in half of the sibling artifact, which the
  persistence section builds a `ravel-screen.jar` out of. These are rendered
  exactly the way the real screens are, name and package alike, because the agent
  jar opens both through the constants `ScreenNames` carries; what differs is that
  they compile against the stand-in game rather than against the stubs, and stand
  in for the codec stack the harness has no game to encode against.

A version that obfuscates need not put its whole game in one place: it may leave
a handful of classes in `net.minecraft` and move the rest out of every package,
and a class inside a package cannot name one outside every package, because Java
has no syntax for it. A stand-in game split that way cannot be written down at
all. So where a version's stand-in roles span both, every one of them is written
in the unnamed package — the one name space both halves can be spelled in, and
the one all but a handful of the classes are already in — and the pairs whose
written name is not the name the version links are recorded in
`generated-harness/aliases.txt`, one `written=linked` line each. The file is
written whichever way the version falls, so an unsplit version leaves it empty
rather than absent and a reader can tell a version that needs no alias from a
generator that never ran. Ravel's own classes follow the same answer, which is
read off the stand-in game once and rendered under by every tree: the name
`ScreenNames` publishes for a screen has to be the name the sibling half beside
the stand-ins was written under, and a tree that decided from its own roles
would publish one name and write the other.

A class an alias moved compiles under a name the agent jar's adapters do not
resolve, so the names go back before anything runs against them, and the class
file is where they can go back: only the constant pool spells a name, and
everything after it reaches the pool by index. `harness/remap/Remap.java` takes
`aliases.txt` and a directory of compiled classes, rewrites every pool entry that
is a written name or carries one as a class token inside a descriptor or a
signature, copies the rest byte for byte, and moves the renamed class file to the
path its new name gives. `run-harness.sh` runs it over each tree after the last
javac that writes into it, which is a no-op where the file is empty. So the
compiled stand-in every section meets carries the name its version links, whether
or not that is the name javac could be given, and a name a stand-in spells as a
string constant rather than as a type is the one thing the pass will not touch:
it refuses the run instead, because a `GameNames` value and a class name are the
same characters and only one of them is a name to put back.

What a version renders instead of a shared template lives under
`minecraft/versions/<version>/harness/game/`, `.../harness/driver/` and
`.../harness/sibling/` — one directory per shared tree, and all three are read —
because a source only one version renders is that version's data and a version's
data is kept in one place. Such a template replaces the shared one of its own name and,
unlike the screen tree, may also carry a template no shared one replaces: a
stand-in only one version has is what a version whose game gained a class means,
while a screen only one version draws is one the agent jar cannot open under the
others. One that replaces nothing *and* that no source the version renders names
is refused as an override that outlived the template it shadowed — asked across
every tree at once, because the stand-in game exists for the screen jar's
generated sources to compile against and the source reaching for a stand-in is
often not in the stand-in's own tree.

An override is whole-file: a template renders one class, and Java has no way to
write down half of one. So an override re-declares every member of its class,
the ones that differ and the ones that do not, and a later edit to the shared
template reaches only the versions that do not override it. That is the cost of
the mechanism and it is accepted with eyes open: the storage stand-in and the
sibling world data are shared under one version's shape and overridden in full
by the other, and each override's class comment names the member that made it
one. Where no version's shape is a reasonable shared one — the stand-in client,
whose world-load seam takes the optional rule overrides under one tracked version
and none under the other — no shared template exists and every version writes
the class in full.

Signatures are hand-written Java and only names are substituted. A stand-in
declares the member the agent jar looks up, so the descriptor javac writes has to
be the one this version links — which it is by construction, because every game
type in a signature is a role and renders as that version's name. Deriving a
signature from an erased descriptor instead would put an unchecked cast in every
body and `-Xlint:all -Werror` would refuse it.

Guava and DataFixerUpper are the names a template spells: they are libraries
beside the game rather than part of it, no published mapping renames them, and
`harness/guava/` and `harness/datafixers/` stand in for the three types the
templates name once each.

### What each version proves, and what it does not

The harness stands in for the game rather than loading it. Its stand-in classes
are generated per version where a section resolves a generated name against them,
and written under Mojang names where they are not. That divides the sections in
two, and the division is the honest limit of what this directory can prove.

The **transformer sections** — initialization, hierarchy, refusal, members,
transformation, load and side — reach the game only through the class transformer, and the harness
supplies the transform targets as well as the stand-in classes. Neither side of
what they compare is version data, so they run at every tracked version and no
version may declare its way out of one. A version that stopped running one of
them did not meet an obstacle; something broke.

Every other section — content, outward, settings, screens and persistence —
compares a generated name against a stand-in, and their stand-in game is
`template/game/`'s: it is generated from the same version data the name is, so the
two names are one string by construction rather than by luck, and the obfuscated
case is the one they read rather than the one they skip. All of them run at
every version.

The settings and screen sections were the last to join them, and what they needed
was more than a name to coincide: they press buttons and read widget lists that
survive a re-init, so they need stand-ins that behave rather than the generated
stubs' empty bodies. Three things about a version reach their recorded output and
none is written in the script. Where this version drew Ravel's own screens is read
from `ScreenNames`, because a screen extends a game class and goes where that
class is. Whether the title menu's column builder hands a row back is read from
`MenuLayout`, because a version whose builder returns void declares Ravel's row
instead of deriving it. And the class the settings round trip rewrites is read
from the generated hook table, by the seam it names.

The persistence section is the one that stopped being written down as Java. A
version that obfuscates leaves a handful of classes in `net.minecraft` because
they carry `@DontObfuscate` and moves the rest into the unnamed package, and Java
has no syntax for naming a class outside every package from inside one. The save
seam's receiver is one of the classes left behind, and the two members the adapter
reaches on it are typed by classes that moved, so no Java source declares it under
the name the adapter resolves. The remap above closes that in the class files
instead, and the section runs at every version like the four beside it.

The settings section's stand-in carries every member the version's own hook
table declares on the class its targets sit on: the shared template holds the
members both tracked versions pin, and a version that pins more overrides it in
full. The section's expected method count is read from that same generated table,
so it is version data and says so, while the transformer sections keep supplying
their own targets and their own hand-written copy of that class under the
reviewed names, which is where four methods and five crossings stay pinned
unmoved.

The thirteenth is the odd one and says so: `cluster` names no game class at all. It copies packages
into a game directory the way an install writes them, lets the launch discover and verify them there,
and then plays, saves, exits and reloads one world across separate processes through the two
persistence crossings the game itself calls. The second visit has to write a successor of the record
the first left, and a single flipped byte in that record has to refuse the world.

`RAVEL_HARNESS_PACKAGE_DIRS` names colon-separated absolute package directories for that section to
install instead of the two under `test-mods/`, so a set that lives outside this repository runs through
the same scenario without being copied into it. Discovery is unchanged: every manifest is parsed and
every declared digest verified, so a directory that is not a package fails the section rather than being
skipped. The report's `mods=` count is the launch's own surface, which the host library decides and the
installed set does not, so it stays what the composed host holds; a caller supplying its own
`ravel.host` beside its own packages names that host's member count in `RAVEL_HARNESS_COMPOSED_MODS`.

Every tracked version runs all thirteen sections, and nothing a version writes can
excuse one. `cargo xtask verify gate-versions` refuses a section missing from a
version's line in the harness record. The `[harness.notRun]` table a version once
declared a section away in is retired, and `cargo xtask verify scaffold` refuses
it in any tracked version's `adapter.toml`: the remap closed the last obstacle
there was, so a section that stops running is a break, and one a version turns
out not to be able to run is a work package in `STATUS.md` and `ROADMAP.md`
rather than a line somebody can write on their own.

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

It stays hand-written where the rest of the stand-in game did not, and that is
the point of it: the transformer sections match it against targets that are
hand-written too, in the stand-in `Hook` under `harness/initialization/`, so
neither side of the comparison is version data and the arithmetic below holds
unchanged at every version. The settings section is the one that drives the
shipped agent jar, and it meets a generated stand-in for the same class instead,
on a class path of its own.

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
agent forces that initializer to run as it starts.

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

Two consequences follow, and both hold for both published distributions:

- `dev.ravel.jvm.Boot` appends the agent jar to the *bootstrap* loader's search
  path before it names any other Ravel class, so `Hook` is defined by the
  bootstrap loader and the library binds there. The system class path the JVM put
  the jar on is not enough: Mojang's server jar is a bundler that runs the game
  under `new URLClassLoader(urls, getClass().getClassLoader().getParent())`,
  whose parent is the platform loader, and the first real dedicated server died
  with `NoClassDefFoundError: dev/ravel/jvm/Agent` on the prologue for exactly
  that reason. Bootstrap is every loader's furthest parent, so after the append
  there is one copy of every Ravel class and every loader delegates to it.
- The rewritten call site resolves `dev.ravel.jvm.Hook` through the loader that
  loaded the *rewritten* class, which delegates to bootstrap and finds that one
  copy. On the client that loader is also the system one:
  [`evidence/m3/class-loader-identity.json`](../evidence/m3/class-loader-identity.json)
  measured a rewritten `net.minecraft` class and `ClassLoader.getSystemClassLoader()`
  to be one object, `jdk.internal.loader.ClassLoaders$AppClassLoader`. On the
  bundled server it is not, and no game class is on the system loader at all —
  which is why every game name an adapter resolves goes through `Members.game`
  and the loader that defined the entry point, and never through the system one.

Appending to the bootstrap search turns the JVM's shared class archive off, and
the JVM says so once per launch on stderr. That is cosmetic and unavoidable; the
harness passes `-Xshare:off` so its exact-output cases do not each carry the
notice.

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
