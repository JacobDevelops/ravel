#!/usr/bin/env bash
# Shared command-status assertion for the isolated launch regression scripts.

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
