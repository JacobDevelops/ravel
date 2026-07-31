# Ravel's JVM side

Two artefacts enter the game process, and this directory holds the Java half of
one of them.

- `bootstrap/` — the premain agent's tracked sources: the entry point, the class
  transformer, and the record the generator fills. Package `dev.ravel.jvm`.
- `build/build-agent.sh` — builds `target/ravel-jvm/ravel-agent.jar` from those
  sources plus the generated `Hook` class.

The generated `Hook` class is not tracked. `ravel-minecraft-adapter`'s build
script writes it to `target/ravel-jvm/generated/dev/ravel/jvm/Hook.java`, so a
build of that crate has to have happened before the jar can be built:

```console
cargo build -p ravel-minecraft-adapter
./jvm/build/build-agent.sh
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
-Djava.library.path=<instance natives>:<directory holding libravel_bootstrap.so>
```

The agent takes no arguments; what it rewrites comes from the generated class it
ships with. `java.library.path` must **append** to the instance's own natives
directory rather than replace it, because the launcher sets that property for
LWJGL and replacing it breaks the game. Removing these two arguments switches
Ravel off without uninstalling anything.

At shutdown the agent prints one line naming how many classes and methods it
rewrote and how many hook invocations reached native code. ADG-5's launch proved
the JVM accepted both arguments and the client ran; it could not prove the
rewrite happened, because nothing reported it. That line is what closes it.

## Three questions M2-W5 owed by name

### Who calls `System.loadLibrary`, and from which class loader

The generated `Hook` class calls it, from its own static initializer, and the
agent forces that initializer to run from `premain`.

JNI binds a loaded library to the class loader that loaded the class whose code
called `System.loadLibrary`, and resolves a native method only against that
loader's copy of the declaring class. If some other class loaded the library, a
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
  loaded the *rewritten* class. Stock Minecraft 26.1 runs on the application
  class loader, which delegates to the system loader, so it finds that same copy.
  A launcher that isolates the game behind a non-delegating loader would not, and
  that is a compatibility requirement rather than a bug to work around.

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
