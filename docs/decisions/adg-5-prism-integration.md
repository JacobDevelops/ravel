# ADG-5 — Prism integration

Status: accepted at M1-W6. The mechanism is decided, the launcher surface is
verified, and the pinned game has been observed launching with Ravel's two
arguments attached through a stock launcher. Corrected after M2's launch, which
found the second argument does not work, and amended after M3 to choose the fix
that note deferred. The amendment is at "Amendment: `System.load` on a
Ravel-owned path"; the decision — two stock instance settings, no launcher
change — is what it restores rather than what it changes.

## Decision

Ravel reaches the official game through two stock Prism instance settings and no
launcher change: `-javaagent` for the entry mechanism ADG-1 selected, and
`-Djava.library.path` for the boundary library. Prism keeps account management,
downloads, Java selection, and the launch itself. Ravel adds two arguments to one
instance and owns no part of the launcher.

## Context

`SPEC.md` puts account authentication, downloads, and launcher UI outside Ravel.
That only works if the entry mechanism can be expressed in what a stock launcher
already offers. If it could not, the alternative was a patch — and a patch means
either persuading Prism upstream to carry it or shipping a fork, which is a
maintenance obligation for as long as Ravel exists.

## Evidence

Reproduce with:

```console
cargo make ravel:spike-adg-5
```

The result is in
[`evidence/m1/spikes/adg-5/`](../../evidence/m1/spikes/adg-5/) and is checked by
`ravel:verify-spikes`.

What was observed, from the pinned Prism 11.0.3 AppImage verified against the
hash in
[`runtime-inputs-26.1.2.json`](../../tests/fixtures/descriptors/runtime-inputs-26.1.2.json):

- **`OverrideJavaArgs` and `JvmArgs` exist.** An instance can be told to use its
  own JVM arguments and what those arguments are. That is exactly what the
  selected mechanism needs, and it is the whole of what it needs.
- **`OverrideCommands` and `WrapperCommand` also exist.** The hosted-VM candidate
  ADG-1 rejected would not have needed a patch either. Recording that matters:
  ADG-1's choice rests on compatibility surface, not on launcher support, and this
  confirms it was not deciding between possible and impossible.
- **No patch is required.** `changedLines` is zero and patch ownership is
  `none-required`.

Then the launch itself, on the pinned instance with an authenticated session:

- **Both arguments reached the JVM command line.** The instance carried
  `-javaagent` pointing at the agent jar and `-Djava.library.path` carrying the
  boundary library directory appended to the instance's own natives directory.
  Appending matters: the launcher sets that property itself for LWJGL, so
  replacing it rather than extending it would break the game.
- **The client ran.** It reached a world and exited when its window was closed.
  A `-javaagent` the JVM cannot load is fatal at startup, so a running client is
  itself proof the agent attached. The log records no instrumentation error and no
  exception.
- **No other loader.** Nothing in the launch named NeoForge, Fabric, Quilt, or
  ModLauncher, and M0's supervisor wrapper was disabled so the launch was a stock
  one.

The agent was pointed at `net.minecraft.util.StringUtil.isAllowedChatCharacter`,
one of the reviewed targets in
[`hooks.toml`](../../minecraft/versions/26.1.2/hooks.toml), with the Java callback
rather than the native one — nothing in the game calls `System.loadLibrary`, so
binding the native side needs Ravel's own bootstrap and is M2-W5's evidence rather
than this gate's.

M0-W6 recorded the neighbouring fact for the unmodified client: title screen,
fresh world, clean exit, no loader present. See
[`evidence/m0/linux-vanilla-launch.json`](../../evidence/m0/linux-vanilla-launch.json).
This gate adds the two arguments to that same launch.

## How to reproduce the launch

The launch needs a signed-in session, so it is run by hand rather than by the
gate:

1. In the pinned Prism instance for 26.1.2, enable **Override Java arguments** and
   set `-javaagent:<agent jar>=net.minecraft.util.StringUtil:isAllowedChatCharacter:onJavaCallback`
   together with `-Djava.library.path=<instance natives>:<library directory>`. Take
   the natives directory from an unmodified launch rather than guessing it, and
   append rather than replace.
2. Launch the instance and reach a world. `--launch <instance> --world <name>`
   does that without any keyboard input.
3. Point the runner at the launcher's output:

   ```console
   ./target/release/ravel-spike-adg-5-prism --launch-log <instance log>
   ```

   It redacts the account's session arguments, records only the hash of the
   redacted form, and sets `prismIntegration.stockLaunch` true when the command
   line carried `-javaagent` and named no other mod loader. The library path is
   recorded but not required, for the reason the correction at the end of this
   file gives. If neither holds, the result says which of the two it was —
   no agent, or a loader it named — rather than claiming a launch. The log
   itself is never copied into the result and never tracked, and it is deleted
   after recording.

`ravel:verify-milestone:m1` refuses to close M1 unless an ADG-5 result records
`stockLaunch: true`, so this evidence cannot be skipped by assertion.

## Consequences

- Ravel ships no launcher, no fork, and no patch, and it does not read or store
  credentials. Anything Ravel needs to know about an instance comes from what the
  operator configured on it.
- Ravel is switchable off by removing two arguments, without uninstalling
  anything. That is a genuinely useful property for diagnosing whether a problem
  is Ravel's, and M2-W7's failure reporting should say so when it fires.
- A launcher that does not expose per-instance JVM arguments cannot host Ravel.
  That is worth stating as a compatibility requirement rather than an assumption:
  other launchers are supported exactly when they expose this.
- The instance log already contains the full launch command, so an operator can
  see what Ravel added without Ravel reporting it. Evidence captures must keep
  redacting that log, because it also contains the account's session arguments.

## Limits of this evidence

- The launcher's settings are read as names in its own executable. That proves
  the settings exist. The launch is what proves the values are accepted.
- The launch proves the JVM took both arguments and the client ran with the agent
  attached. It does not prove the class rewrite happened: nothing in the spike
  agent reports whether it transformed the target, so the evidence here is that a
  transformer installed against a real Minecraft class broke nothing. ADG-1
  measured the rewrite itself, and M2-W5 owns proving it in the running game.

  What would make this direct is small and worth writing down, because the next
  person to want it will otherwise rediscover it. The agent already tracks both
  facts — `Agent.transformed()` and `Hook.javaInvocations()` — and never reports
  them, because only a trial harness ever asks and no harness exists inside a real
  game launch. A shutdown hook printing one line would turn an in-game launch into
  direct evidence that the target was rewritten and the hook fired. It is not
  retrofitted here because every ADG-1 result hashes `agent.jar`: changing the
  agent invalidates four recorded results and requires re-running the gate, which
  would move measurements that are currently correct. M2-W5 builds its own
  bootstrap and should build the reporting into it rather than amend the spike.
- One launch on one machine, recorded as the hash of a redacted log. That is
  reproducible by the steps above, not by the gate.
- Only Linux and only Prism 11.0.3. Other launchers and other platforms are
  M10-and-later questions, and the compatibility requirement above is how they
  will be judged.

## What M2's launch found — the second argument does not work

This note is a correction to the evidence above, not a change to the decision.
The mechanism stands: stock launcher, no patch, and Ravel still switches off by
removing arguments. What does not hold is the `-Djava.library.path` half of it.

M2-W5's launch was the first to attach a native library, and it aborted at
startup with `UnsatisfiedLinkError: no ravel_bootstrap in java.library.path`,
naming only the instance's own natives directory. Prism 11.0.3 places the
instance's `JvmArgs` on the command line *before* its own
`-Djava.library.path=<instance>/natives`, and the last one wins, so appending a
directory through the instance setting has no effect. The launcher also deletes
that natives directory when a launch ends, so a library copied there lasts
exactly one launch. M1's launch could not have found either, because its agent
loaded no library.

M2 got its launch by putting the library into the instance's natives directory
immediately before starting, keeping both arguments set, and recording the
override as a deviation in `evidence/m2/prism-launch.json`. That is a workaround
rather than a mechanism. Two proper fixes exist — the generated `Hook` calling
`System.load` on a path given by a `-D` property, which keeps Ravel to arguments
alone, or an installer that owns the instance's natives directory, which does
not — and choosing between them amends this ADR. Nothing should be built on the
appended library path in the meantime.

The reporting this record asked M2-W5 to build now exists and works, with one
correction of its own: the game replaces `System.out` with a logger that has
stopped by the time a shutdown hook runs, so the line has to be written to the
process's own standard output. `evidence/m2/prism-launch.json` records what it
printed — two classes and five methods rewritten inside the official client, and
144 hook invocations reaching native code.

## Amendment: `System.load` on a Ravel-owned path

Amended after M3, on what M3's launches actually did rather than on a new
measurement. The note above left this ADR in the worst available state: it
classified the natives-staging path as a workaround, told nobody to build on it,
named two proper fixes, and said choosing between them amends this ADR. Nothing
chose. Every native M3 launch then used the workaround, and it went further than
M2's did — the library was staged into the instance's natives directory *from the
instance's own `WrapperCommand`*, which is Ravel-supplied code running inside the
launcher's launch sequence, with `OverrideCommands` and `ShowConsole` set beside
it and a third `-D` argument past the two this decision names. So the native half
of this decision has had no accepted mechanism at all since M2-W5.

**The mechanism this ADR selects is `System.load` on an explicit path supplied by
a `-D` property.** The generated `Hook` reads a Ravel-owned absolute path out of a
system property and loads the library from there. The appended
`-Djava.library.path` and the natives staging that propped it up are both
withdrawn: the appended path does not work for the reason the note above records,
and the staging is what this amendment exists to stop.

**Why this one and not the installer.** Staging writes a Ravel file into a
directory the launcher owns, populates on launch, and deletes on exit, so the file
can be wiped between launches and the failure mode is an `UnsatisfiedLinkError` in
a launch that worked yesterday. The workaround is fragile precisely because it
borrows the launcher's storage. An installer that owns the instance's natives
directory makes that borrowing permanent: it spends "Ravel adds two arguments to
one instance and owns no part of the launcher" — the property in "Decision" above,
and the property that made this gate worth having — to solve a problem an absolute
path solves. `System.load` restores it, and returns the instance to two settings:
`-javaagent`, and one `-D` naming the library.

### What survives the change and what has to

The load stays in `Hook`'s own static initializer. The comment the generator emits
there is the part that must not be lost: JNI binds a loaded library to the loader
of the class whose code called, and resolves native methods only against that
loader's copy of the declaring class, so loading from the declaring class's own
initializer makes those two loaders the same one by construction. `System.load`
called from the same initializer keeps that property exactly — what binds the
library is which class's code called, not which overload it called. ADG-1's "What
is still forbidden" also continues to hold: no second class binds its own library.

### Nothing is built yet, and no work package owns it

`crates/ravel-minecraft-adapter/src/generate/render.rs` still emits
`System.loadLibrary(LIBRARY)`, and this amendment does not change it. The change
is launcher-integration work — a generator edit, a property name, and a launch
argument set that has to be proven in a real launch — and **no work package owns
it.** M3's implementation packages are closed and none of them named it; M4-W1 to
M4-W8 are SDK breadth and none of them names the launcher either. The obligation
is recorded here and in `ROADMAP.md`'s open-questions row rather than assigned to
a package that does not exist, and until a package does own it and run it, no
milestone may claim conformance with the native half of this decision. M3 does
not: its manifest records the staging as a deviation naming this ADR.
