#!/usr/bin/env bash
set -euo pipefail

# Covers the probe's refusals and the one guarantee that matters when it fails
# partway: the instance config is left exactly as it was found. Needs no display.

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
probe="$script_dir/prism-nested-launch-probe.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

instance_id='probe-instance'
instance_dir="$work/data/instances/$instance_id"
mkdir -p "$instance_dir"
config="$instance_dir/instance.cfg"
printf 'OverrideCommands=true\nWrapperCommand=/keep/me\nJvmArgs=\n' >"$config"
config_before="$(cat "$config")"

launcher="$work/launcher"
printf '#!/bin/sh\nexit 0\n' >"$launcher"
chmod +x "$launcher"

# Stubs so the tool preflight passes without a display or a real X server.
stub_bin="$work/bin"
mkdir -p "$stub_bin"
for tool in xdotool openbox import wmctrl xdpyinfo glxinfo Xephyr; do
  printf '#!/bin/sh\nexit 0\n' >"$stub_bin/$tool"
  chmod +x "$stub_bin/$tool"
done
export PATH="$stub_bin:$PATH"

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

expect_config_untouched() {
  local description="$1"
  [[ "$(cat "$config")" == "$config_before" ]] || {
    echo "FAIL: $description left instance.cfg modified" >&2
    diff <(printf '%s\n' "$config_before") "$config" >&2 || true
    exit 1
  }
}

expect_status 2 'no arguments' "$probe"
expect_status 2 'too few arguments' "$probe" :97 3 "$launcher"
expect_status 2 'malformed display' \
  "$probe" 97 3 "$launcher" "$work/data" "$instance_id" world "$work/out"
expect_status 2 'malformed workspace' \
  "$probe" :97 main "$launcher" "$work/data" "$instance_id" world "$work/out"
expect_status 1 'unreadable launcher' \
  "$probe" :97 3 "$work/absent" "$work/data" "$instance_id" world "$work/out"
expect_status 1 'unknown instance' \
  "$probe" :97 3 "$launcher" "$work/data" absent-instance world "$work/out"
expect_config_untouched 'a refused invocation'

# A display start that fails happens after the config has been rewritten, which
# is the path that must still hand the operator's instance back intact.
failing_setup="$work/failing-nested-setup.sh"
printf '#!/bin/sh\nexit 1\n' >"$failing_setup"
chmod +x "$failing_setup"
RAVEL_NESTED_SETUP="$failing_setup" expect_status 1 'a failed display start' \
  "$probe" :97 3 "$launcher" "$work/data" "$instance_id" world "$work/out"
expect_config_untouched 'a failed display start'

# An interrupted run is the case that actually leaves OverrideCommands=false
# behind, so drive it: hold the probe open past the rewrite, then signal it.
slow_setup="$work/slow-nested-setup.sh"
printf '#!/bin/sh\nsleep 10\nexit 1\n' >"$slow_setup"
chmod +x "$slow_setup"

start_interrupted_probe() {
  RAVEL_NESTED_SETUP="$slow_setup" \
    "$probe" :97 3 "$launcher" "$work/data" "$instance_id" world "$work/out" \
    >/dev/null 2>&1 &
  probe_pid=$!
  for _ in {1..100}; do
    grep -q '^OverrideCommands=false$' "$config" && return 0
    sleep 0.1
  done
  echo "FAIL: the probe never reached its config rewrite" >&2
  kill -KILL "$probe_pid" 2>/dev/null || true
  exit 1
}

start_interrupted_probe
kill -TERM "$probe_pid"
wait "$probe_pid" 2>/dev/null || true
expect_config_untouched 'a probe killed with SIGTERM'

# SIGKILL cannot be trapped, so the guarantee there is recoverability rather
# than restoration: the backup must still be a faithful copy to repair from.
start_interrupted_probe
kill -KILL "$probe_pid"
wait "$probe_pid" 2>/dev/null || true
grep -q '^OverrideCommands=false$' "$config" || {
  echo "FAIL: expected SIGKILL to leave the rewritten config in place" >&2
  exit 1
}
[[ -f "$work/out/instance.cfg.backup" ]] || {
  echo "FAIL: SIGKILL left no backup to recover from" >&2
  exit 1
}
cp "$work/out/instance.cfg.backup" "$config"
expect_config_untouched 'the documented repair after SIGKILL'

echo "prism-nested-launch-probe: all cases passed"
