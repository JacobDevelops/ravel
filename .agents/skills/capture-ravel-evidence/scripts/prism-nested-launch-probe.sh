#!/usr/bin/env bash
set -euo pipefail

# Proves the pinned client reaches a world and accepts real keystrokes on a
# nested display, so functional verification never needs the operator's screen.
# Setup-only: the nested display renders through llvmpipe, so nothing measured
# here is timing evidence.

usage() {
  echo "usage: $0 DISPLAY WORKSPACE LAUNCHER DATA_DIR INSTANCE_ID WORLD OUT_DIR" >&2
  exit 2
}

[[ $# -eq 7 ]] || usage

display="$1"
workspace="$2"
launcher="$3"
data_dir="$4"
instance_id="$5"
world="$6"
out_dir="$7"

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# Overridable so a test can inject a failing display start and prove the
# instance config is restored on that path.
nested_setup="${RAVEL_NESTED_SETUP:-$script_dir/prism-nested-setup.sh}"
instance_dir="$data_dir/instances/$instance_id"
instance_config="$instance_dir/instance.cfg"
game_log="$instance_dir/minecraft/logs/latest.log"
probe_text="ravel nested probe"

[[ "$display" =~ ^:[0-9]+$ ]] || usage
[[ "$workspace" =~ ^[0-9]+$ ]] || usage
[[ -x "$launcher" ]] || {
  echo "launcher is not executable" >&2
  exit 1
}
[[ -f "$instance_config" ]] || {
  echo "instance config is missing for $instance_id" >&2
  exit 1
}

for tool in xdotool openbox import wmctrl xdpyinfo glxinfo Xephyr; do
  command -v "$tool" >/dev/null || {
    echo "missing $tool; install it with your system package manager (xdotool, openbox," \
      "imagemagick, wmctrl, xdpyinfo, mesa-utils, xorg-server)" >&2
    exit 1
  }
done

# Match the launcher's own executable, not its data directory: any shell that
# invoked this script carries the directory in its command line and would be
# mistaken for a running launcher.
launcher_pids() {
  pgrep -f -- '/bin/prismlauncher' || true
}

mapfile -t already_running < <(launcher_pids)
[[ "${#already_running[@]}" -eq 0 ]] || {
  echo "a Prism Launcher process is already running; close it first, because" \
    "this probe rewrites the instance configuration" >&2
  exit 1
}

mkdir -p "$out_dir"
chmod 700 "$out_dir"
config_backup="$out_dir/instance.cfg.backup"
cp "$instance_config" "$config_backup"
chmod 600 "$config_backup"

# Redact the operator's home from anything this script prints.
sanitize() {
  sed -e "s|$HOME|<home>|g" -e 's|/home/[^/ ]*|<home>|g'
}

game_pids() {
  pgrep -f "$data_dir/instances/$instance_id/minecraft" || true
}

restore_config() {
  cp "$config_backup" "$instance_config"
  cmp -s "$config_backup" "$instance_config" || {
    echo "FATAL: instance.cfg was not restored; copy it back from" \
      "$(printf '%s' "$config_backup" | sanitize)" >&2
    return 1
  }
  echo "instance.cfg restored"
}

cleanup() {
  local status=$?
  local pids
  for reaper in game_pids launcher_pids; do
    mapfile -t pids < <("$reaper")
    [[ "${#pids[@]}" -eq 0 ]] && continue
    kill -TERM "${pids[@]}" 2>/dev/null || true
    for _ in {1..30}; do
      mapfile -t pids < <("$reaper")
      [[ "${#pids[@]}" -eq 0 ]] && break
      sleep 1
    done
  done
  if [[ -n "${openbox_pid:-}" ]]; then
    kill -TERM "$openbox_pid" 2>/dev/null || true
  fi
  "$nested_setup" stop "$display" >/dev/null 2>&1 || true
  restore_config || status=1
  exit "$status"
}
trap cleanup EXIT

# The M0 capture wrapper owns process start; a setup launch must not run it.
sed -i 's/^OverrideCommands=true$/OverrideCommands=false/' "$instance_config"
if [[ -n "${RAVEL_PROBE_JVM_ARGS:-}" ]]; then
  sed -i 's/^OverrideJavaArgs=.*/OverrideJavaArgs=true/' "$instance_config"
  sed -i "s|^JvmArgs=.*|JvmArgs=${RAVEL_PROBE_JVM_ARGS//|/\\|}|" "$instance_config"
fi

"$nested_setup" start "$display" "$workspace"
export DISPLAY="$display"

renderer="$(glxinfo -B | awk -F': ' '/OpenGL renderer string/ { print $2; exit }')"
gl_version="$(
  glxinfo -B | awk -F': ' '/OpenGL core profile version string/ { print $2; exit }'
)"
echo "renderer=$renderer"
echo "gl=$gl_version"

# Without a window manager the nested focus is PointerRoot and keys go nowhere.
openbox >/dev/null 2>&1 &
openbox_pid=$!
for _ in {1..20}; do
  wmctrl -m >/dev/null 2>&1 && break
  sleep 0.5
done
wmctrl -m >/dev/null 2>&1 || {
  echo "no window manager answered on $display" >&2
  exit 1
}

: >"$game_log" 2>/dev/null || true
"$launcher" -d "$data_dir" -l "$instance_id" -w "$world" \
  >"$out_dir/launch.log" 2>&1 &
launcher_pid=$!

window=""
for second in {1..420}; do
  window="$(xdotool search --class minecraft 2>/dev/null | tail -1 || true)"
  [[ -n "$window" ]] && {
    echo "window_seconds=$second"
    break
  }
  kill -0 "$launcher_pid" 2>/dev/null || break
  sleep 1
done
[[ -n "$window" ]] || {
  echo "no client window appeared on $display" >&2
  grep -iE 'opengl|glx|error|exception' "$out_dir/launch.log" |
    head -5 | sanitize >&2
  exit 1
}

world_ready=no
for _ in {1..60}; do
  if grep -qE 'Loaded [0-9]+ advancements|Time elapsed|Started serving' \
    "$game_log" 2>/dev/null; then
    world_ready=yes
    break
  fi
  sleep 5
done
echo "world_ready=$world_ready"
[[ "$world_ready" == yes ]] || {
  echo "the client never reported a loaded world" >&2
  exit 1
}

# Let the first chunks finish rasterising before the screen is worth capturing.
sleep 20
import -display "$display" -window root "$out_dir/world.png"
echo "world_capture_bytes=$(wc -c <"$out_dir/world.png")"

# XTEST on the nested display only. A --window target would degrade to
# XSendEvent, which the client ignores.
xdotool search --sync --class minecraft windowactivate --sync
sleep 2
xdotool key --clearmodifiers t
sleep 2
xdotool type --clearmodifiers --delay 60 "$probe_text"
sleep 1
import -display "$display" -window root "$out_dir/chat.png"
xdotool key --clearmodifiers Return
sleep 5
import -display "$display" -window root "$out_dir/sent.png"

keystrokes_landed=no
grep -qF "$probe_text" "$game_log" 2>/dev/null && keystrokes_landed=yes
echo "keystrokes_landed=$keystrokes_landed"

wmctrl -c Minecraft
exited=no
for _ in {1..90}; do
  mapfile -t remaining < <(game_pids)
  [[ "${#remaining[@]}" -eq 0 ]] && {
    exited=yes
    break
  }
  sleep 1
done
echo "clean_exit=$exited"

grep -qE 'Stopping!|Stopping worker threads' "$game_log" 2>/dev/null &&
  echo "shutdown_logged=yes" || echo "shutdown_logged=no"

[[ "$keystrokes_landed" == yes && "$exited" == yes ]] || {
  echo "the probe did not prove input and clean exit" >&2
  exit 1
}
printf 'probe=pass display=%s renderer=%s setup_only=yes\n' \
  "$display" "$renderer"
