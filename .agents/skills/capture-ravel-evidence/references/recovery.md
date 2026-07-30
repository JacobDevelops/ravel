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

Do not assume `xdotool key --window MINECRAFT_WINDOW` makes native operation
focus-free. Minecraft/GLFW can ignore synthetic key events while another window
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
