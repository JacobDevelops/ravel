# Prism, authentication, and display recovery

## Microsoft callback does not open Prism

Symptoms include a browser completing Microsoft sign-in while the desktop says
no application can open the launcher callback URI.

1. Keep the isolated Prism data root and its account files private.
2. Generate and inspect an isolated handler without touching desktop defaults:

   ```console
   .agents/skills/capture-ravel-evidence/scripts/prism-uri-handler.sh render \
     PrismLauncher.AppImage ISOLATED_DATA_ROOT handler.desktop
   ```

3. Register it only when the user explicitly wants the desktop callback wired:

   ```console
   .agents/skills/capture-ravel-evidence/scripts/prism-uri-handler.sh register \
     PrismLauncher.AppImage ISOLATED_DATA_ROOT
   ```

   The script reads Prism's embedded desktop metadata and refuses to guess the
   URI scheme. It never accepts or logs a callback URL.

4. Register or invoke the launcher-provided URI handler without copying the
   callback into logs or chat.
5. Let Prism consume the callback, then verify entitlement by launching the
   pinned instance.
6. Never expose `accounts.json`, access/refresh tokens, profile names,
   authorization URLs, callback URLs, or OAuth state.

Do not treat successful browser sign-in alone as proof. The isolated launcher
must retain the account and Minecraft must pass entitlement checks.

## Virtual display has no OpenGL

An Xvfb session may start Prism but fail Minecraft or renderer setup because
GLX/OpenGL is absent.

1. Check the candidate display's renderer before use.
2. If GLX/OpenGL is unavailable, stop the virtual-display path.
3. Use direct/background capture on the real display, or ask the user for a
   quiet window when unavoidable UI setup remains.
4. Never admit virtual-display performance numbers as native baseline evidence.

Xvfb is the wrong server for this, and the symptom is specific enough to
recognise without re-diagnosing it. `glxinfo -B` against an Xvfb display reports
`Error: couldn't find RGB GLX visual or fbconfig`, with or without
`LIBGL_ALWAYS_SOFTWARE=1`, and the client then dies in
`com.mojang.blaze3d.platform.Window.<init>` with `Failed to create backend
OpenGL`. That is the X server, not the game: Xvfb and Xnest support software
rendering only, and this build advertises no usable GLX visual. Xephyr on the
same host exposes OpenGL 4.5 core through llvmpipe, because it is built from the
same X server as the session's Xorg. Choose Xephyr and check the renderer rather
than tuning llvmpipe against Xvfb.

For focus-free world creation or other setup-only UI, Xephyr may be used when
it exposes working GLX:

```console
.agents/skills/capture-ravel-evidence/scripts/prism-nested-setup.sh start :97 3
DISPLAY=:97 glxinfo -B
.agents/skills/capture-ravel-evidence/scripts/prism-nested-setup.sh check :97
```

The workspace argument is zero-indexed. The helper detaches Xephyr from the
calling tool session, uses `-noreset`, moves its host window without activating
it, and rejects startup if Xephyr becomes the host active window. It reports
whether the active-window ID stayed identical but permits the operator to
switch between their own windows during startup. A tool-bound foreground
Xephyr can be terminated when the tool session expires, which breaks the
game's X11 connection and invalidates unsaved setup.

Launch Prism and Minecraft on the nested display only for setup. Never admit
its software-rendered timing or memory output as evidence. If the instance's
capture wrapper cannot tolerate setup load, close Prism, temporarily disable
the wrapper, perform setup, then restore and deduplicate `OverrideCommands`,
`PreLaunchCommand`, `PostExitCommand`, and `WrapperCommand`. Reopen and close
Prism once and require exactly one authoritative value for each key before
capture.

When sending input inside the nested display:

- verify `xdpyinfo -display :97` before every batch;
- keep batches short and inspect the resulting screen state;
- invoke `xdotool type` separately from the following `xdotool` command,
  because `type` can consume later tokens as text;
- never point `xdotool` at the host display.

One probe drives that whole sequence and proves it, so the mechanics do not have
to be reassembled by hand:

```console
.agents/skills/capture-ravel-evidence/scripts/prism-nested-launch-probe.sh \
  :97 3 LAUNCHER DATA_DIR INSTANCE_ID WORLD PRIVATE_OUT_DIR
```

It backs the instance configuration up, disables `OverrideCommands` for the
setup launch, starts the nested display, runs a window manager inside it, waits
on log conditions rather than sleeps, captures the screen, types into chat over
XTEST, and requires both a landed keystroke and a clean exit before reporting
`probe=pass`. It restores the configuration on every exit path, including
failure, and verifies the restored file byte-for-byte. Set `RAVEL_PROBE_JVM_ARGS`
to attach an agent. Refuse to run it while any Prism process is alive, which it
enforces. Keep its output directory private: the captures contain world content.

The instance configuration is shared state, so **never run this probe
concurrently with another agent session or another capture**. It rewrites
`OverrideCommands`, and the launcher is not the only thing that can be mid-run:
the working copy and the launcher directory are shared. One probe at a time, and
no other session driving the same instance while it runs.

Interruption is covered but not perfectly. A probe killed with `SIGTERM`,
`SIGINT`, or a closing tool session restores the configuration through its exit
trap, and the test proves it. `SIGKILL` cannot be trapped, so it leaves
`OverrideCommands=false` behind. The repair is one command, using the backup the
probe writes before it changes anything:

```console
cp PRIVATE_OUT_DIR/instance.cfg.backup INSTANCE_DIR/instance.cfg
```

After that, confirm `OverrideCommands=true` and exactly one `WrapperCommand`
line before any capture. If the backup is gone, the instance has to be rebuilt
from the runbook rather than guessed at.

On the reference Linux machine the pinned vanilla 26.1.2 instance produced a
client window 13 seconds after launch, reached a loaded world, accepted typed
chat, and exited cleanly, rendering through `llvmpipe` at OpenGL 4.5 core. Those
seconds are a liveness signal, not a measurement.

Bare XTEST on an isolated display is therefore a proven input route, while a
directed event is not. Do not assume `xdotool key --window MINECRAFT_WINDOW`
makes native operation focus-free. Minecraft/GLFW can ignore synthetic key events while another window
owns X11 focus even though `xdotool` returns success. Prove both the resulting
game state and the unchanged focus owner in an isolated display before relying
on directed events. The ATM11 0.2.1 setup test rejected this path: Escape was
ignored while an `xmessage` window retained focus. Native operator-run routes
therefore still require a bounded quiet window unless an accepted in-world or
protocol-level automation path is added to the frozen profile.

The nested session may be muted only when it is setup-only. Resolve the exact
audio stream by matching its process ID and Minecraft working directory; do not
mute an evidence run unless the audio setting is part of the frozen profile.
Stop the setup display with:

```console
.agents/skills/capture-ravel-evidence/scripts/prism-nested-setup.sh stop :97
```

Minecraft 26.1 stores canonical world state across multiple NBT files. Verify
the saved state on disk rather than trusting chat feedback:

- seed and generator: `data/minecraft/world_gen_settings.dat`;
- time: `data/minecraft/world_clocks.dat`;
- weather: `data/minecraft/weather.dat`;
- Overworld border:
  `dimensions/minecraft/overworld/data/minecraft/world_border.dat`;
- difficulty, game type, and `allowCommands`: `level.dat`;
- generated Overworld chunks:
  `dimensions/minecraft/overworld/region/*.mca`.

For a 16-chunk radius, inspect the Anvil location tables and require every
expected coordinate, not merely a total chunk count.

## An attached agent cannot find its native library

Symptom: the client never opens a window, and the game process aborts with
`FATAL ERROR in native method: processing of -javaagent failed`, caused by
`java.lang.UnsatisfiedLinkError: no <library> in java.library.path: <instance>/natives`.

The instance's `JvmArgs` are placed on the command line *before* the launcher's
own `-Djava.library.path=<instance>/natives`, and the last one wins, so an
instance setting that appends a directory to that path has no effect. The
launcher also removes `<instance>/natives` when a launch ends, so a library
copied there survives exactly one launch.

Put the library back immediately before every launch:

```console
mkdir -p INSTANCE_DIR/natives
cp target/release/libravel_bootstrap.so INSTANCE_DIR/natives/
```

Keep the `-Djava.library.path` argument set anyway — it is what ADG-5 decided
and it is inert rather than wrong — and record the override as a deviation
wherever the launch is admitted. Both halves of this are the launcher's
behaviour rather than a Ravel bug, and a launch record that does not mention it
overstates how stock the arrangement is.

## The agent's shutdown line is nowhere on disk

Ravel's premain agent prints what it rewrote from a JVM shutdown hook, after the
game's own logging has stopped, so the line reaches raw stdout and never enters
`<instance>/minecraft/logs/`. The launcher shows it in an instance console
window, writes that window's contents to no file, and — when launched with `-l`
and no `--show-window` — quits with the game and takes the window with it.

Capture it with a stock `WrapperCommand` that tees the process's own output:

```bash
#!/usr/bin/env bash
"$@" 2>&1 | tee PRIVATE_OUT_DIR/game-stdout.log
```

`tee` rather than a redirect, because the launcher reads that stream. The
wrapper changes no launch argument and starts nothing else, which is what keeps
the launch admissible as a stock one. Set `OverrideCommands=true` for it, and
restore the instance configuration afterwards.

A launch driven this way must not use the nested probe, which disables
`OverrideCommands` for its own setup launch. Keep the two paths separate.

## The shutdown report is missing and the log stops mid-startup

Symptom: a wrapper-captured launch ends with no `[ravel]` line, and the captured
log stops partway through startup with no error. The game process is often still
running afterwards, orphaned.

The cause is process selection, not the agent. Under a `WrapperCommand` the
launcher runs `wrapper.sh <java> <args...>`, so the wrapper shell's own command
line *contains the java path* and `pgrep -f <java-path>` matches both. The
wrapper is started first, so `| head -1` picks it. Terminating that kills the
capture pipe, leaves the JVM orphaned, and loses the shutdown report the launch
existed to produce.

Select on `comm` rather than on the command line:

```bash
pid=$(for candidate in $(pgrep -f "$JAVA_PATH"); do
  [ "$(cat "/proc/$candidate/comm" 2>/dev/null)" = "java" ] && echo "$candidate"
done | head -1)
```

Check for orphans before every run — a leftover JVM holds the instance and
silently invalidates the next launch.

## Prism 11.0.3 will not open a world from `JoinWorldOnLaunch`

The key exists in `instance.cfg` and setting it changes nothing: no
`quickPlay` argument reaches the client, with or without
`OverrideMiscellaneous=true`, and the game stops at the title screen. Do not
spend a launch on it.

There is no route to a loaded world that does not inject input. The nested probe
is the supported one, and note that it **cannot currently attach Ravel**: it sets
`OverrideCommands=false`, and the wrapper it thereby disables is the only thing
that stages `libravel_bootstrap.so` into the instance's natives directory after
the launcher rebuilds it. `RAVEL_PROBE_JVM_ARGS` with a `-javaagent` therefore
aborts the JVM on a missing library. Staging has to move into the probe before
that path works.

Do not confirm launcher capabilities with `strings` on the AppImage. It is a
compressed squashfs image, so a missing string proves nothing.

## Focus-free rule

Background process launch and localhost Tracy collection are allowed. Global
input injection, desktop-wide screenshots, focus stealing, and unannounced
window activation are not. If a UI-only step cannot be isolated, pause and ask
for a bounded quiet window.

## CurseForge reports a required file as Not Found

Prism may hand a mod to its manual-download watcher even when the corresponding
CurseForge CDN object remains available. Recover only when the import dialog or
Prism log supplies all three values: numeric CurseForge file ID, exact filename,
and expected SHA-1.

```console
.agents/skills/capture-ravel-evidence/scripts/recover-curseforge-file.sh \
  FILE_ID FILENAME EXPECTED_SHA1 "$HOME/Downloads"
```

The helper derives the official CDN path from the file ID, downloads over
HTTPS, verifies the expected SHA-1 before installing, uses mode `0600`, and
refuses to overwrite a mismatched or non-regular target. Prism's default
manual-download watcher should then show the file as found. Do not bypass the
hash check, use an unofficial mirror, or admit the downloaded JAR to the repo.

If Prism requires an acknowledgement and global input automation is
unavailable or would steal focus:

1. Move the Prism window to a non-active workspace without activating it.
2. Record the active X11 window ID.
3. Query `org.a11y.Bus.GetAddress`, find Prism's AT-SPI application, and inspect
   its accessible tree for the blocked-mod dialog's named `OK` button.
4. Confirm `org.a11y.atspi.Action.GetActions` exposes `Press`, then call
   `DoAction(0)` on that exact accessible object. Do not call `SetFocus`.
5. Re-read the active X11 window ID and require it to equal the value from step
   2. Stop if the object name, role, or active-window invariant is uncertain.

Rediscover the AT-SPI bus name and object path every time; both are ephemeral.
Treat the accessibility action as a narrow dialog acknowledgement, never as
general desktop automation.

## Skill validation cannot import YAML

The system validator imports `yaml`, which is not guaranteed to exist in the
ambient Python. Run
`.agents/skills/capture-ravel-evidence/scripts/validate.sh`; it supplies PyYAML
through an ephemeral Nix Python environment. A plain
`nix shell nixpkgs#python3Packages.pyyaml` is insufficient because it does not
place the module on that shell's Python import path.

The system `generate_openai_yaml.py` helper has the same import requirement.
Run it through `nix-shell -p 'python3.withPackages (packages:
[ packages.pyyaml ])' --run ...`, and shell-quote every `--interface` value so
the literal `$capture-ravel-evidence` in `default_prompt` is not expanded.

## Coverage gate hides the measured percentage

If `devenv tasks run ravel:verify` or `ravel:coverage` reports only an execution
failure after all tests pass, expose the workspace line result directly:

```console
cargo llvm-cov nextest --workspace --all-features --summary-only
```

Use the per-file totals and missing-line report to add meaningful regression
tests. Never lower or bypass the workspace's `--fail-under-lines 80` floor.
