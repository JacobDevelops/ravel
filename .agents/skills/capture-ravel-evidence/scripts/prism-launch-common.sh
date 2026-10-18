#!/usr/bin/env bash
# Shared process teardown and window-manager readiness for isolated Prism launches.

reap_launch_processes() {
	local reaper pids
	for reaper in "$@"; do
		mapfile -t pids < <("$reaper")
		[[ "${#pids[@]}" -eq 0 ]] && continue
		kill -TERM "${pids[@]}" 2>/dev/null || true
		for _ in {1..30}; do
			mapfile -t pids < <("$reaper")
			[[ "${#pids[@]}" -eq 0 ]] && break
			sleep 1
		done
	done
}

wait_for_window_manager() {
	local display="$1"
	for _ in {1..20}; do
		wmctrl -m >/dev/null 2>&1 && break
		sleep 0.5
	done
	wmctrl -m >/dev/null 2>&1 || {
		echo "no window manager answered on $display" >&2
		return 1
	}
}
