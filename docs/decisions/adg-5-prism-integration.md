# ADG-5 — Prism integration

Status: accepted at M1-W6. The mechanism is decided, the launcher surface is
verified, and the pinned game has been observed launching with Ravel's two
arguments attached through a stock launcher.

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
devenv tasks run ravel:spike-adg-5
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
   devenv shell -- ./target/release/ravel-spike-adg-5-prism --launch-log <instance log>
   ```

   It redacts the account's session arguments, records only the hash of the
   redacted form, and sets `prismIntegration.stockLaunch` true when the command
   line carried both arguments and named no other mod loader. If it did not, the
   result says which argument was missing rather than claiming a launch. The log
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
