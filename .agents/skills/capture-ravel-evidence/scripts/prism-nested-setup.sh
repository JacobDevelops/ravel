#!/usr/bin/env bash
set -euo pipefail

usage() {
	echo "usage: $0 start|check|stop DISPLAY [WORKSPACE]" >&2
	exit 2
}

[[ $# -ge 2 && $# -le 3 ]] || usage

action="$1"
display="$2"
workspace="${3:-}"

[[ "$display" =~ ^:[0-9]+$ ]] || {
	echo "DISPLAY must look like :97" >&2
	exit 2
}

find_pids() {
	pgrep -f "^Xephyr ${display//./\\.} " || true
}

check_display() {
	xdpyinfo -display "$display" >/dev/null 2>&1 || {
		echo "nested display is not ready: $display" >&2
		exit 1
	}
	DISPLAY="$display" glxinfo -B >/dev/null 2>&1 || {
		echo "nested display has no working GLX/OpenGL: $display" >&2
		exit 1
	}
}

case "$action" in
start)
	[[ $# -eq 3 && "$workspace" =~ ^[0-9]+$ ]] || usage
	[[ -z "$(find_pids)" ]] || {
		echo "nested display already has a Xephyr process: $display" >&2
		exit 1
	}

	focus_schema="org.cinnamon.desktop.wm.preferences"
	focus_key="focus-new-windows"
	restore_focus_policy=false
	if gsettings writable "$focus_schema" "$focus_key" >/dev/null 2>&1; then
		focus_policy_before="$(
			gsettings get "$focus_schema" "$focus_key" | tr -d "'"
		)"
		gsettings set "$focus_schema" "$focus_key" strict
		restore_focus_policy=true
	fi
	restore_policy() {
		if "$restore_focus_policy"; then
			gsettings set "$focus_schema" "$focus_key" "$focus_policy_before"
		fi
	}
	start_committed=false
	cleanup_start() {
		restore_policy
		if ! "$start_committed"; then
			mapfile -t failed_start_pids < <(find_pids)
			if [[ "${#failed_start_pids[@]}" -eq 1 ]]; then
				kill -TERM "${failed_start_pids[0]}"
			fi
		fi
	}
	trap cleanup_start EXIT
	active_before="$(xprop -root _NET_ACTIVE_WINDOW | awk '{print $5}')"

	setsid -f Xephyr "$display" \
		-screen 1280x720 -resizeable -noreset -nolisten tcp \
		>/dev/null 2>&1

	for _ in {1..20}; do
		xdpyinfo -display "$display" >/dev/null 2>&1 && break
		sleep 0.25
	done
	check_display

	# The display answers before the host window manager has mapped Xephyr's
	# window, and on a busy machine that gap is seconds rather than milliseconds.
	# Without this wait the start fails with a window that is about to exist.
	window_id=""
	for _ in {1..40}; do
		window_id="$(
			wmctrl -l -x |
				awk -v title="Xephyr on ${display}.0" 'index($0, title) { print $1; exit }'
		)"
		[[ -n "$window_id" ]] && break
		sleep 0.5
	done
	[[ -n "$window_id" ]] || {
		"$0" stop "$display"
		echo "could not identify the Xephyr host window" >&2
		exit 1
	}

	wmctrl -i -r "$window_id" -t "$workspace"
	active_after="$(xprop -root _NET_ACTIVE_WINDOW | awk '{print $5}')"
	[[ "${active_after,,}" != "${window_id,,}" ]] || {
		"$0" stop "$display"
		echo "Xephyr captured the host active window" >&2
		exit 1
	}
	if [[ "$active_before" == "$active_after" ]]; then
		active_unchanged=yes
	else
		active_unchanged=no
	fi
	start_committed=true

	printf \
		'display=%s workspace=%s focus_captured=no active_unchanged=%s setup_only=yes\n' \
		"$display" "$workspace" "$active_unchanged"
	;;
check)
	[[ $# -eq 2 ]] || usage
	check_display
	mapfile -t xephyr_pids < <(find_pids)
	[[ "${#xephyr_pids[@]}" -eq 1 ]] || {
		echo "expected exactly one Xephyr process for $display" >&2
		exit 1
	}
	printf 'display=%s pid=%s setup_only=yes\n' "$display" "${xephyr_pids[0]}"
	;;
stop)
	[[ $# -eq 2 ]] || usage
	mapfile -t xephyr_pids < <(find_pids)
	[[ "${#xephyr_pids[@]}" -le 1 ]] || {
		echo "refusing to stop multiple Xephyr processes for $display" >&2
		exit 1
	}
	if [[ "${#xephyr_pids[@]}" -eq 1 ]]; then
		kill -TERM "${xephyr_pids[0]}"
		printf 'stopped display=%s pid=%s\n' "$display" "${xephyr_pids[0]}"
	else
		printf 'display=%s already_stopped=yes\n' "$display"
	fi
	;;
*)
	usage
	;;
esac
