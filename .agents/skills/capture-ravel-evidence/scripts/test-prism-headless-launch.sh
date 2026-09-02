#!/usr/bin/env bash
set -euo pipefail

# Covers the headless proof's refusals and the guarantee that matters when it
# fails partway: the instance config and options are left exactly as they were
# found. Needs no display and no game.
#
# The two refusals worth the most here are the ones no procedure can be trusted
# to remember: a launcher root holding an account that is not Offline, and a
# sound category that is not silent.

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
proof="$script_dir/prism-headless-launch.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

instance_id='headless-instance'
data_dir="$work/data"
instance_dir="$data_dir/instances/$instance_id"
game_dir="$instance_dir/minecraft"
mkdir -p "$game_dir"
config="$instance_dir/instance.cfg"
printf 'OverrideCommands=false\nWrapperCommand=/keep/me\nJvmArgs=\n' >"$config"
config_before="$(cat "$config")"

options="$game_dir/options.txt"
write_options() {
	local master="$1"
	{
		printf 'resourcePacks:["file/ravel"]\n'
		printf 'soundCategory_master:%s\n' "$master"
		printf 'soundCategory_music:0.0\n'
	} >"$options"
}
write_options 0.0
options_before="$(cat "$options")"

accounts="$data_dir/accounts.json"
write_accounts() {
	printf '%s\n' "$1" >"$accounts"
}
offline_only='{"formatVersion":3,"accounts":[{"type":"Offline","profile":{"id":"0","name":"ravel-headless"}}]}'
with_msa='{"formatVersion":3,"accounts":[{"type":"MSA","profile":{"id":"0","name":"someone"}},{"type":"Offline","profile":{"id":"1","name":"ravel-headless"}}]}'
write_accounts "$offline_only"

launcher="$work/launcher"
printf '#!/bin/sh\nexit 0\n' >"$launcher"
chmod +x "$launcher"

# Stubs so the tool preflight passes with no X server anywhere. xdpyinfo answers
# from a marker file, so the same binary can report "no display yet" before the
# start and "display is up" after it, which is what the script's first refusal
# and its readiness loop each need.
stub_bin="$work/bin"
mkdir -p "$stub_bin"
display_marker="$work/display-up"
cat >"$stub_bin/xdpyinfo" <<EOF
#!/bin/sh
[ -f "$display_marker" ] || exit 1
exit 0
EOF
cat >"$stub_bin/Xvfb" <<EOF
#!/bin/sh
touch "$display_marker"
exit 0
EOF
printf '#!/bin/sh\necho "OpenGL renderer string: llvmpipe (stub)"\necho "OpenGL core profile version string: 4.5 (stub)"\nexit 0\n' \
	>"$stub_bin/glxinfo"
for tool in xdotool openbox import wmctrl; do
	printf '#!/bin/sh\nexit 0\n' >"$stub_bin/$tool"
done
chmod +x "$stub_bin"/*
export PATH="$stub_bin:$PATH"

out_dir="$work/out"

expect_status() {
	local expected="$1" description="$2"
	shift 2
	local status=0
	"$@" >/dev/null 2>&1 || status=$?
	[[ "$status" -eq "$expected" ]] || {
		echo "FAIL: $description expected status $expected, got $status" >&2
		exit 1
	}
}

expect_untouched() {
	local description="$1"
	[[ "$(cat "$config")" == "$config_before" ]] || {
		echo "FAIL: $description left instance.cfg modified" >&2
		diff <(printf '%s\n' "$config_before") "$config" >&2 || true
		exit 1
	}
	[[ "$(cat "$options")" == "$options_before" ]] || {
		echo "FAIL: $description left options.txt modified" >&2
		exit 1
	}
}

run_proof() {
	"$proof" "$@" :99 "$launcher" "$data_dir" "$instance_id" "$out_dir"
}

expect_status 2 'no arguments' "$proof"
expect_status 2 'too few arguments' "$proof" :99 "$launcher"
expect_status 2 'malformed display' \
	"$proof" 99 "$launcher" "$data_dir" "$instance_id" "$out_dir"
expect_status 2 'an unknown stop-after step' \
	"$proof" --stop-after nowhere :99 "$launcher" "$data_dir" "$instance_id" "$out_dir"
expect_status 2 'an unknown option' \
	"$proof" --benchmark :99 "$launcher" "$data_dir" "$instance_id" "$out_dir"
expect_status 1 'an unreadable launcher' \
	"$proof" :99 "$work/absent" "$data_dir" "$instance_id" "$out_dir"
expect_status 1 'an unknown instance' \
	"$proof" :99 "$launcher" "$data_dir" absent-instance "$out_dir"
expect_untouched 'a refused invocation'

# The refusal the whole separate launcher root exists for.
write_accounts "$with_msa"
expect_status 1 'a launcher root holding a Microsoft account' run_proof
write_accounts "$offline_only"

mv "$accounts" "$work/accounts-away.json"
expect_status 1 'a launcher root with no account file' run_proof
mv "$work/accounts-away.json" "$accounts"

# The refusal that keeps a run silent over a sleeping operator.
write_options 1.0
expect_status 1 'a sound category that is not silent' run_proof
write_options 0.0

mv "$options" "$work/options-away.txt"
expect_status 1 'an instance with no options.txt' run_proof
mv "$work/options-away.txt" "$options"
expect_untouched 'the account and sound refusals'

# A display that already exists is never one this script started, so it is never
# safe to drive: it may be the operator's own.
touch "$display_marker"
expect_status 1 'a display that already exists' run_proof
rm -f "$display_marker"
expect_untouched 'the existing-display refusal'

# Past every refusal the config is rewritten, and the launcher stub exits at
# once, so no window ever appears. That is the ordinary failing path and it must
# still hand the instance back intact.
expect_status 1 'a launch that produces no window' run_proof
expect_untouched 'a launch that produces no window'
[[ -f "$out_dir/instance.cfg.backup" ]] || {
	echo "FAIL: the run left no instance.cfg backup" >&2
	exit 1
}
[[ -f "$out_dir/options.txt.backup" ]] || {
	echo "FAIL: the run left no options.txt backup" >&2
	exit 1
}

# The launcher argument is normally an AppImage called
# `.../bin/prismlauncher-11.0.3.AppImage`, so an unanchored pattern for a running
# launcher matches the very shell that invoked this and refuses every run.
appimage_dir="$work/local/bin"
mkdir -p "$appimage_dir"
appimage="$appimage_dir/prismlauncher-11.0.3.AppImage"
printf '#!/bin/sh\nexit 0\n' >"$appimage"
chmod +x "$appimage"
rm -f "$display_marker"
appimage_stderr="$work/appimage.stderr"
"$proof" :99 "$appimage" "$data_dir" "$instance_id" "$out_dir" \
	>/dev/null 2>"$appimage_stderr" || true
if grep -q 'already running' "$appimage_stderr"; then
	echo "FAIL: an AppImage launcher path was mistaken for a running launcher" >&2
	exit 1
fi
expect_untouched 'a launcher named like the AppImage'

# An interrupted run is the case that actually leaves OverrideCommands=true
# behind, so drive it: hold the proof open past the rewrite, then signal it.
slow_launcher="$work/slow-launcher"
printf '#!/bin/sh\nsleep 120\n' >"$slow_launcher"
chmod +x "$slow_launcher"

start_interrupted_proof() {
	rm -f "$display_marker"
	"$proof" :99 "$slow_launcher" "$data_dir" "$instance_id" "$out_dir" \
		>/dev/null 2>&1 &
	proof_pid=$!
	for _ in {1..300}; do
		grep -q '^OverrideCommands=true$' "$config" && return 0
		sleep 0.1
	done
	echo "FAIL: the proof never reached its config rewrite" >&2
	kill -KILL "$proof_pid" 2>/dev/null || true
	exit 1
}

start_interrupted_proof
kill -TERM "$proof_pid"
wait "$proof_pid" 2>/dev/null || true
expect_untouched 'a proof killed with SIGTERM'

# SIGKILL cannot be trapped, so the guarantee there is recoverability rather
# than restoration: the backups must still be faithful copies to repair from.
start_interrupted_proof
kill -KILL "$proof_pid"
wait "$proof_pid" 2>/dev/null || true
grep -q '^OverrideCommands=true$' "$config" || {
	echo "FAIL: expected SIGKILL to leave the rewritten config in place" >&2
	exit 1
}
cp "$out_dir/instance.cfg.backup" "$config"
cp "$out_dir/options.txt.backup" "$options"
expect_untouched 'the documented repair after SIGKILL'

echo "prism-headless-launch: all cases passed"
