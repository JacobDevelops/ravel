#!/usr/bin/env bash
set -euo pipefail

# Drives the pinned 1.21.1 client through a Ravel proof on a virtual display, so
# the whole sequence runs without the operator's screen, pointer, keyboard,
# sound or Microsoft account.
#
# Setup-only: the virtual display renders through llvmpipe, so nothing measured
# here is timing evidence. Frame times, load times and the window-appearance
# budget below describe software rasterisation and describe nothing else.
#
# The display is Xvfb rather than Xephyr: Xephyr draws into a window on the
# operator's real display and this must touch it at all. Every xdotool call is
# made with DISPLAY pointing at the virtual server this script started, and the
# script refuses to start when that display already exists.
#
# The account is the second half of the isolation. This refuses any data
# directory whose accounts.json holds an entry that is not Offline, which is
# what keeps it off the real launcher root and off the operator's credentials.

usage() {
	cat >&2 <<'USAGE'
usage: prism-headless-launch.sh [OPTION...] DISPLAY LAUNCHER DATA_DIR INSTANCE_ID OUT_DIR

  DISPLAY      virtual display to create, like :99; must not already exist
  LAUNCHER     the Prism Launcher executable
  DATA_DIR     the headless launcher root, offline accounts only
  INSTANCE_ID  the instance directory name under DATA_DIR/instances
  OUT_DIR      private evidence directory for this run

options:
  --phase-mtime      also compare the written trees before and after a session
  --phase-reopen     also quit to the title and reopen the same world
  --phase-negative    also run the resourcePacks:["oops"] negative launch
  --stop-after STEP  stop after title, world, commands or close
  --keep-display     leave the display and any running client up after a
                     --stop-after halt, so a screen can be measured against it
USAGE
	exit 2
}

phase_mtime=no
phase_reopen=no
phase_negative=no
stop_after=none
keep_display=no
hold_client=no

while [[ $# -gt 0 ]]; do
	case "$1" in
	--phase-mtime)
		phase_mtime=yes
		shift
		;;
	--phase-reopen)
		phase_reopen=yes
		shift
		;;
	--phase-negative)
		phase_negative=yes
		shift
		;;
	--stop-after)
		[[ $# -ge 2 ]] || usage
		stop_after="$2"
		shift 2
		;;
	--keep-display)
		keep_display=yes
		shift
		;;
	-*)
		usage
		;;
	*)
		break
		;;
	esac
done

[[ $# -eq 5 ]] || usage

display="$1"
launcher="$2"
data_dir="$3"
instance_id="$4"
out_dir="$5"

case "$stop_after" in
none | title | world | commands | close) ;;
*) usage ;;
esac

[[ "$display" =~ ^:[0-9]+$ ]] || usage

instance_dir="$data_dir/instances/$instance_id"
instance_config="$instance_dir/instance.cfg"
game_dir="$instance_dir/minecraft"
options_file="$game_dir/options.txt"
game_log="$game_dir/logs/latest.log"
saves_dir="$game_dir/saves"
world_name="Ravel Headless"
world_dir="$saves_dir/$world_name"

screen_width=1920
screen_height=1080

# The interface is drawn at guiScale 2, so a logical pixel is two real ones and
# the logical viewport is half the screen. Every coordinate below is computed
# from the layout `TitleScreen.init` states rather than measured off a capture,
# because a measured one is only true at the size it was measured at.
gui_scale=2
logical_height=$((screen_height / gui_scale))
logical_width=$((screen_width / gui_scale))
column_top=$((logical_height / 4 + 48))
column_x=$((logical_width / 2))

# Row centres in logical pixels. `titleMenuRow = -24` in the version's hooks.toml
# puts Ravel's row one spacing above the column, so the vanilla rows keep the
# places they had.
ravel_row=$((column_top - 24 + 10))
singleplayer_row=$((column_top + 10))

# The world-select and create-world screens place their widgets through a layout
# no pinned input states, so unlike the title column these are measured off a
# capture rather than computed. Each is a logical "X Y" pair at 1920x1080 and
# guiScale 2, and the run refuses while any is still unmeasured: a guessed click
# lands on some other button and produces a failure that says nothing about
# Ravel. Take them from a run with `--stop-after title --keep-display`.
create_new_world_button=unmeasured
world_name_field=unmeasured
game_tab=unmeasured
game_mode_button=unmeasured
world_tab=unmeasured
world_type_button=unmeasured
create_button=unmeasured

[[ -x "$launcher" ]] || {
	echo "launcher is not executable" >&2
	exit 1
}
[[ -f "$instance_config" ]] || {
	echo "instance config is missing for $instance_id" >&2
	exit 1
}
[[ -f "$options_file" ]] || {
	echo "the instance has no options.txt; launch it once by hand first" >&2
	exit 1
}

for tool in xdotool openbox import wmctrl xdpyinfo glxinfo Xvfb jq; do
	command -v "$tool" >/dev/null || {
		echo "missing $tool; provision it from nixpkgs (xdotool, openbox," \
			"imagemagick, wmctrl, xorg.xdpyinfo, mesa-demos, xorg.xvfb, jq)" >&2
		exit 1
	}
done

# Refusal 1: the display must not already exist. Attaching to a display this
# script did not create is how a run reaches the operator's screen.
xdpyinfo -display "$display" >/dev/null 2>&1 && {
	echo "$display already has an X server; this script only drives one it started" >&2
	exit 1
}

# Refusal 2: offline accounts only. This is the whole point of the separate
# launcher root and belongs here rather than in a procedure someone remembers.
accounts="$data_dir/accounts.json"
[[ -f "$accounts" ]] || {
	echo "no accounts.json under the headless launcher root; add an offline account first" >&2
	exit 1
}
jq -e '[.accounts[] | select(.type != "Offline")] | length == 0' \
	"$accounts" >/dev/null || {
	echo "the headless launcher root still holds a non-offline account; refusing" >&2
	exit 1
}

# Refusal 3: every sound category silent in the instance's own options, on top of
# the null OpenAL driver exported below. Either one alone is a single point of
# failure over a sleeping operator.
mapfile -t loud < <(grep '^soundCategory_' "$options_file" | grep -v ':0.0$' || true)
[[ "${#loud[@]}" -eq 0 ]] || {
	echo "these sound categories are not 0.0 in the instance options:" >&2
	printf '  %s\n' "${loud[@]}" >&2
	exit 1
}
[[ "$(grep -c '^soundCategory_' "$options_file")" -gt 0 ]] || {
	echo "the instance options declare no sound category at all; refusing" >&2
	exit 1
}

# Match the launcher's own executable and nothing else. Two near misses both
# match a shell that merely invoked this script: its data directory, and the
# unanchored name, because the AppImage this is normally handed is called
# `.../bin/prismlauncher-11.0.3.AppImage`. Anchoring on end-of-argument leaves
# only the real process, which the AppImage runs as `<mount>/bin/prismlauncher`;
# the AppImage wrapper exits with it.
launcher_pids() {
	pgrep -f -- '/bin/prismlauncher($| )' || true
}

mapfile -t already_running < <(launcher_pids)
[[ "${#already_running[@]}" -eq 0 ]] || {
	echo "a Prism Launcher process is already running; close it first, because" \
		"this proof rewrites the instance configuration" >&2
	exit 1
}

mkdir -p "$out_dir"
chmod 700 "$out_dir"
config_backup="$out_dir/instance.cfg.backup"
cp "$instance_config" "$config_backup"
chmod 600 "$config_backup"
options_backup="$out_dir/options.txt.backup"
cp "$options_file" "$options_backup"
chmod 600 "$options_backup"

verdicts="$out_dir/verdicts.txt"
: >"$verdicts"

# Redact the operator's home from anything this script prints.
sanitize() {
	sed -e "s|$HOME|<home>|g" -e 's|/home/[^/ ]*|<home>|g'
}

verdict() {
	printf '%s=%s\n' "$1" "$2" | tee -a "$verdicts"
}

failed=no
expect() {
	local name="$1" value="$2"
	verdict "$name" "$value"
	[[ "$value" == pass || "$value" == yes ]] || failed=yes
}

game_pids() {
	pgrep -f "$game_dir" || true
}

display_pids() {
	pgrep -f -- "^Xvfb ${display//./\\.} " || true
}

restore_config() {
	cp "$config_backup" "$instance_config"
	cp "$options_backup" "$options_file"
	cmp -s "$config_backup" "$instance_config" || {
		echo "FATAL: instance.cfg was not restored; copy it back from" \
			"$(printf '%s' "$config_backup" | sanitize)" >&2
		return 1
	}
	cmp -s "$options_backup" "$options_file" || {
		echo "FATAL: options.txt was not restored; copy it back from" \
			"$(printf '%s' "$options_backup" | sanitize)" >&2
		return 1
	}
	echo "instance_config_restored=yes"
}

cleanup() {
	local status=$?
	local pids
	# A halt asked for with --keep-display is a measuring halt: a display with no
	# client on it shows nothing worth measuring. Every other exit reaps.
	if [[ "$hold_client" == yes ]]; then
		echo "client and display left running; kill them by pid when done"
		restore_config || status=1
		exit "$status"
	fi
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
	if [[ "$keep_display" == no ]]; then
		mapfile -t pids < <(display_pids)
		[[ "${#pids[@]}" -eq 0 ]] || kill -TERM "${pids[@]}" 2>/dev/null || true
	fi
	restore_config || status=1
	exit "$status"
}
trap cleanup EXIT

# The game is driven by this script, not by the launcher's own wrapper, but the
# wrapper is what captures the process's own stdout. It reads this.
export RAVEL_HEADLESS_LOG="$out_dir/game-stdout.log"
: >"$RAVEL_HEADLESS_LOG"
chmod 600 "$RAVEL_HEADLESS_LOG"

# Software rasterisation, named rather than inferred, and no audio device at all.
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
export ALSOFT_DRIVERS=null

setsid -f Xvfb "$display" -screen 0 "${screen_width}x${screen_height}x24" \
	-nolisten tcp >"$out_dir/xvfb.log" 2>&1
for _ in {1..40}; do
	xdpyinfo -display "$display" >/dev/null 2>&1 && break
	sleep 0.25
done
xdpyinfo -display "$display" >/dev/null 2>&1 || {
	echo "the virtual display never answered: $display" >&2
	exit 1
}
export DISPLAY="$display"

renderer="$(glxinfo -B | awk -F': ' '/OpenGL renderer string/ { print $2; exit }')"
gl_version="$(
	glxinfo -B | awk -F': ' '/OpenGL core profile version string/ { print $2; exit }'
)"
verdict renderer "$renderer"
verdict gl "$gl_version"
verdict timing_evidence no

# Without a window manager the focus is PointerRoot and keys go nowhere.
openbox >"$out_dir/openbox.log" 2>&1 &
openbox_pid=$!
for _ in {1..20}; do
	wmctrl -m >/dev/null 2>&1 && break
	sleep 0.5
done
wmctrl -m >/dev/null 2>&1 || {
	echo "no window manager answered on $display" >&2
	exit 1
}

# --- driving helpers -------------------------------------------------------
#
# Bare xdotool, which is XTEST. A --window target degrades to XSendEvent, which
# the client ignores; that was tried on the real display and does not work.

game_window=""

activate() {
	xdotool windowactivate --sync "$game_window"
	sleep 1
}

shot() {
	import -display "$display" -window root "$out_dir/$1.png"
}

# Clicks a point given in logical interface pixels.
click_logical() {
	local x="$1" y="$2"
	xdotool mousemove $((x * gui_scale)) $((y * gui_scale))
	sleep 0.4
	xdotool click 1
	sleep 1
}

# Clicks one of the measured "X Y" pairs above, by variable name. The check is
# here rather than up front so that a bring-up run reaches the screen whose
# coordinate is missing and captures it before it stops.
click_point() {
	local value="${!1}"
	[[ "$value" != unmeasured ]] || {
		echo "the coordinate '$1' is unmeasured; take it from the last capture in" \
			"$(printf '%s' "$out_dir" | sanitize)" >&2
		verdict world_created no
		exit 1
	}
	click_logical "${value%% *}" "${value##* }"
}

# Walks the world-select and create-world screens to a Creative Superflat world.
# The two cycle buttons are clicked rather than set, because neither exposes a
# value to click directly: game mode cycles Survival, Hardcore, Creative from its
# default, and world type cycles Default, Superflat from its own.
create_world() {
	activate
	click_point create_new_world_button
	sleep 3
	shot 04a-create-world

	click_point world_name_field
	xdotool key --clearmodifiers ctrl+a
	xdotool type --clearmodifiers --delay 40 "$world_name"
	sleep 1
	shot 04b-world-named

	click_point game_tab
	sleep 1
	shot 04c-game-tab
	click_point game_mode_button
	click_point game_mode_button
	sleep 1
	shot 04d-game-mode

	click_point world_tab
	sleep 1
	shot 04e-world-tab
	click_point world_type_button
	sleep 1
	shot 04f-world-type

	click_point create_button
	sleep 3
	shot 04g-creating
}

# Types one chat line and waits for the game to act on it. Mouse-look is not
# available (the client grabs the pointer and GLFW reads raw motion, so a warp
# turns nothing), which is why every in-world action below is a command.
chat() {
	local line="$1"
	xdotool key --clearmodifiers t
	sleep 1
	xdotool type --clearmodifiers --delay 40 "$line"
	sleep 0.5
	xdotool key --clearmodifiers Return
	sleep 3
}

log_holds() {
	grep -qF "$1" "$RAVEL_HEADLESS_LOG" 2>/dev/null
}

log_matches() {
	grep -qE "$1" "$RAVEL_HEADLESS_LOG" 2>/dev/null
}

assert_log() {
	local name="$1" pattern="$2"
	if log_matches "$pattern"; then
		expect "$name" pass
	else
		expect "$name" fail
	fi
}

await_log() {
	local pattern="$1" seconds="$2"
	for _ in $(seq 1 "$seconds"); do
		log_matches "$pattern" && return 0
		sleep 1
	done
	return 1
}

# --- the launch ------------------------------------------------------------

sed -i 's/^OverrideCommands=false$/OverrideCommands=true/' "$instance_config"

: >"$game_log" 2>/dev/null || true
rm -rf "$world_dir"

"$launcher" -d "$data_dir" -l "$instance_id" \
	>"$out_dir/launch.log" 2>&1 &
launcher_pid=$!

# llvmpipe is slow to first frame and this budget says nothing about the game.
for second in {1..420}; do
	game_window="$(xdotool search --class minecraft 2>/dev/null | tail -1 || true)"
	[[ -n "$game_window" ]] && {
		verdict window_seconds "$second"
		break
	}
	kill -0 "$launcher_pid" 2>/dev/null || break
	sleep 1
done
[[ -n "$game_window" ]] || {
	echo "no client window appeared on $display" >&2
	{ grep -iE 'opengl|glx|error|exception' "$out_dir/launch.log" |
		head -5 | sanitize >&2; } || true
	exit 1
}

# EWMH fullscreen rather than a window size: it makes the client exactly the
# screen with no decoration, which is what the logical coordinates assume.
wmctrl -i -r "$game_window" -b add,fullscreen
sleep 3
geometry="$(xdotool getwindowgeometry "$game_window" | awk '/Geometry/ { print $2 }')"
if [[ "$geometry" == "${screen_width}x${screen_height}" ]]; then
	expect window_is_screen_sized pass
else
	verdict window_geometry "$geometry"
	expect window_is_screen_sized fail
fi

if await_log 'Loaded [0-9]+ recipes|Sound engine started|Created:.*textures' 300; then
	expect title_reached pass
else
	expect title_reached fail
fi
sleep 10
shot 01-title

# The agent has to have run before anything else is worth asserting.
assert_log agent_attached '\[ravel\] rewrote [0-9]+ classes and [0-9]+ methods|registered [0-9]+ of [0-9]+ admitted content entries'
assert_log content_registered 'registered 2 of 2 admitted content entries'

[[ "$stop_after" == title ]] && {
	verdict stopped_after title
	if [[ "$keep_display" == yes ]]; then hold_client=yes; fi
	exit 0
}

# --- the mods and settings screens ----------------------------------------

activate
click_logical "$column_x" "$ravel_row"
sleep 3
shot 02-mods-list
click_logical "$column_x" "$((logical_height - 27))"
sleep 2
shot 03-back-at-title

# --- the world -------------------------------------------------------------

activate
click_logical "$column_x" "$singleplayer_row"
sleep 4
shot 04-world-select

create_world

if await_log 'Starting integrated minecraft server version' 300; then
	expect world_started pass
else
	expect world_started fail
fi
if await_log 'Time elapsed|Started serving|Loaded [0-9]+ advancements' 300; then
	expect world_loaded pass
else
	expect world_loaded fail
fi
sleep 20
shot 05-in-world

if [[ -f "$world_dir/datapacks/ravel/pack.mcmeta" ]]; then
	expect world_datapack_written pass
else
	expect world_datapack_written fail
fi

if [[ "$phase_mtime" == yes ]]; then
	find "$game_dir/resourcepacks/ravel" "$world_dir/datapacks/ravel" \
		-printf '%T@ %p\n' 2>/dev/null | sort >"$out_dir/trees-after-first-session.txt"
fi

[[ "$stop_after" == world ]] && {
	verdict stopped_after world
	if [[ "$keep_display" == yes ]]; then hold_client=yes; fi
	exit 0
}

# --- the first session's assertions ---------------------------------------

activate
chat "/datapack list"
sleep 2
shot 06-datapack-list
if log_matches 'data pack\(s\) enabled:.*file/ravel \(world\)'; then
	expect first_session_datapack_enabled pass
else
	expect first_session_datapack_enabled fail
fi

# 1291 is the single strongest line in the proof: the pack was written, selected,
# loaded, and the item it names exists. 1290 is a different, named failure — the
# pack loaded and registration did not — rather than a generic mismatch.
if log_holds 'Loaded 1291 recipes'; then
	expect first_session_recipes pass
elif log_holds 'Loaded 1290 recipes'; then
	verdict first_session_recipes vanilla_count_only
	failed=yes
else
	expect first_session_recipes fail
fi

chat "/give @s content:copper_lantern 8"
sleep 2
shot 07-lantern-given
assert_log lantern_given 'Gave 8 \[Copper Lantern\]'

chat "/setblock ~ ~ ~2 content:copper_lantern"
sleep 2
shot 08-lantern-placed
assert_log lantern_placed 'Changed the block at'

chat "/gamemode survival"
chat "/difficulty peaceful"
chat "/clear"
chat "/setblock ~ ~ ~2 air destroy"
sleep 3
shot 09-lantern-drop
if log_matches 'Dropped 1 \[Copper Lantern\]|Copper Lantern'; then
	expect lantern_drops pass
else
	expect lantern_drops fail
fi

chat "/gamemode creative"
chat "/give @s minecraft:copper_ingot 8"
chat "/give @s minecraft:torch 1"
chat "/recipe give @s content:crafting/copper_lantern"
sleep 3
shot 10-recipe-unlocked
assert_log recipe_unlocked 'Unlocked 1 recipes'

if grep -qE 'Unable to load model|Missing model|Exception|Caused by' \
	"$RAVEL_HEADLESS_LOG" 2>/dev/null; then
	expect no_model_or_exception_warnings fail
	{ grep -nE 'Unable to load model|Missing model|Exception' \
		"$RAVEL_HEADLESS_LOG" | head -5 | sanitize >&2; } || true
else
	expect no_model_or_exception_warnings pass
fi

[[ "$stop_after" == commands ]] && {
	verdict stopped_after commands
	if [[ "$keep_display" == yes ]]; then hold_client=yes; fi
	exit 0
}

# --- optional phases -------------------------------------------------------

if [[ "$phase_reopen" == yes ]]; then
	activate
	xdotool key --clearmodifiers Escape
	sleep 2
	shot 11-pause-menu
	click_logical "$column_x" "$((logical_height / 4 + 120))"
	if await_log 'Stopping worker threads|Saving chunks for level' 120; then
		expect quit_to_title pass
	else
		expect quit_to_title fail
	fi
	sleep 10
	shot 12-title-after-quit
	activate
	click_logical "$column_x" "$singleplayer_row"
	sleep 5
	shot 13-world-list
	xdotool key --clearmodifiers Down
	sleep 1
	xdotool key --clearmodifiers Return
	if await_log 'Found new data pack file/ravel, loading it automatically' 300; then
		verdict reopen_found_pack_new yes
	else
		verdict reopen_found_pack_new no
	fi
	sleep 20
	shot 14-reopened
	if [[ "$(grep -c 'Loaded 1291 recipes' "$RAVEL_HEADLESS_LOG")" -ge 1 ]]; then
		expect reopen_recipes pass
	else
		expect reopen_recipes fail
	fi
fi

if [[ "$phase_mtime" == yes ]]; then
	find "$game_dir/resourcepacks/ravel" "$world_dir/datapacks/ravel" \
		-printf '%T@ %p\n' 2>/dev/null | sort >"$out_dir/trees-after-run.txt"
	if cmp -s "$out_dir/trees-after-first-session.txt" "$out_dir/trees-after-run.txt"; then
		expect unchanged_trees_not_rewritten pass
	else
		expect unchanged_trees_not_rewritten fail
		{ diff "$out_dir/trees-after-first-session.txt" \
			"$out_dir/trees-after-run.txt" | head -10 | sanitize >&2; } || true
	fi
fi

# --- shutdown --------------------------------------------------------------

activate
xdotool key --clearmodifiers Escape
sleep 2
wmctrl -c Minecraft
exited=no
for _ in {1..120}; do
	mapfile -t remaining < <(game_pids)
	[[ "${#remaining[@]}" -eq 0 ]] && {
		exited=yes
		break
	}
	sleep 1
done
expect clean_exit "$exited"
assert_log shutdown_logged 'Stopping!'
assert_log transformer_reported '\[ravel\] rewrote [1-9][0-9]* classes and [1-9][0-9]* methods'

[[ "$stop_after" == close ]] && {
	verdict stopped_after close
	exit 0
}

# --- the negative -----------------------------------------------------------

if [[ "$phase_negative" == yes ]]; then
	sed -i 's|^resourcePacks:.*$|resourcePacks:["oops"]|' "$options_file"
	export RAVEL_HEADLESS_LOG="$out_dir/game-stdout-negative-oops.log"
	: >"$RAVEL_HEADLESS_LOG"
	chmod 600 "$RAVEL_HEADLESS_LOG"
	"$launcher" -d "$data_dir" -l "$instance_id" \
		>"$out_dir/launch-negative.log" 2>&1 &
	launcher_pid=$!
	game_window=""
	for _ in {1..420}; do
		game_window="$(xdotool search --class minecraft 2>/dev/null | tail -1 || true)"
		[[ -n "$game_window" ]] && break
		kill -0 "$launcher_pid" 2>/dev/null || break
		sleep 1
	done
	[[ -n "$game_window" ]] || {
		echo "the negative launch produced no window" >&2
		exit 1
	}
	wmctrl -i -r "$game_window" -b add,fullscreen
	await_log 'Reloading ResourceManager' 300 || true
	sleep 10
	shot 20-negative-title
	if log_matches 'Reloading ResourceManager: vanilla$'; then
		expect negative_pack_not_loaded pass
	else
		expect negative_pack_not_loaded fail
	fi
	activate
	wmctrl -c Minecraft
	for _ in {1..120}; do
		mapfile -t remaining < <(game_pids)
		[[ "${#remaining[@]}" -eq 0 ]] && break
		sleep 1
	done
	assert_log negative_clean_exit 'Stopping!'
fi

# --- the verdict ------------------------------------------------------------
#
# `devenv tasks run` shows neither stdout nor stderr for a task that succeeds, so
# a proof that only prints is invisible where it matters. This writes the file on
# success and never on start, the way ravel-test writes tested-versions.txt.

[[ "$failed" == no ]] || {
	echo "the headless proof did not hold; see $(printf '%s' "$verdicts" | sanitize)" >&2
	exit 1
}

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../../.." && pwd)"
mkdir -p "$repo_root/target"
{
	printf 'headless proof for %s on %s\n' "$instance_id" "$display"
	printf 'renderer: %s (setup only, not timing evidence)\n' "$renderer"
	cat "$verdicts"
} >"$repo_root/target/headless-launch-verdict.txt"

printf 'proof=pass display=%s renderer=%s setup_only=yes\n' "$display" "$renderer"
