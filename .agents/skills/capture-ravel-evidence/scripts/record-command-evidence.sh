#!/usr/bin/env bash
set -euo pipefail
umask 077
export LC_ALL=C

usage() {
  printf 'usage: record-command-evidence.sh LABEL LOG_PATH RECORD_JSON -- COMMAND [ARG...]\n' >&2
  exit 2
}

fail() {
  printf 'command evidence rejected: %s\n' "$1" >&2
  exit 1
}

test "$#" -ge 5 || usage
label="$1"
log_input="$2"
record_input="$3"
test "$4" = "--" || usage
shift 4
test "$#" -gt 0 || usage

for command in date jq ln realpath rg sha256sum stat; do
  command -v "$command" >/dev/null || fail "required command is unavailable"
done

repository_input="${RAVEL_EVIDENCE_ROOT:-}"
if test -z "$repository_input"; then
  repository_input="$(jj root 2>/dev/null)" || fail "Ravel repository root is unavailable"
fi
repository="$(realpath -e -- "$repository_input")"
private_root="$(realpath -e -- "$repository/evidence/private")"
log_parent="$(realpath -e -- "$(dirname -- "$log_input")")"
record_parent="$(realpath -e -- "$(dirname -- "$record_input")")"
log_path="$log_parent/$(basename -- "$log_input")"
record_path="$record_parent/$(basename -- "$record_input")"

require_owner_only_chain() {
  local current="$1"
  while true; do
    parent_mode="$(stat -c '%a' "$current")"
    case "$parent_mode" in
      *[1-7][0-7] | *[0-7][1-7]) fail "output directory is not owner-only" ;;
    esac
    test "$current" != "$private_root" || break
    current="$(dirname -- "$current")"
  done
}

for parent in "$log_parent" "$record_parent"; do
  case "$parent/" in
    "$private_root/"*) ;;
    *) fail "outputs must stay under the private evidence boundary" ;;
  esac
  require_owner_only_chain "$parent"
done

test "$log_path" != "$record_path" || fail "log and record paths must differ"
for output in "$log_path" "$record_path"; do
  test ! -e "$output" && test ! -L "$output" || fail "output already exists"
done

if printf '%s\n' "$label" \
  | rg --ignore-case --quiet \
    '/home/|/users/|evidence/private/|tests/fixtures/local/|token|authorization|bearer'; then
  fail "public command label contains forbidden local or credential data"
fi

if ! (set -o noclobber; : >"$log_path") 2>/dev/null; then
  fail "log output appeared before capture"
fi
chmod 600 "$log_path"
started_at="$(date --utc '+%Y-%m-%dT%H:%M:%SZ')"
started_ns="$(date '+%s%N')"
argv_sha="$(
  printf '%s\0' "$@" \
    | sha256sum \
    | cut -d' ' -f1
)"

set +e
"$@" >"$log_path" 2>&1
exit_status="$?"
set -e

ended_ns="$(date '+%s%N')"
duration_ms="$(( (ended_ns - started_ns) / 1000000 ))"
log_sha="$(sha256sum "$log_path" | cut -d' ' -f1)"
temporary="$(mktemp "$record_parent/.command-record.XXXXXX")"
trap 'if test -n "${temporary:-}" && test -e "$temporary"; then rm -f -- "$temporary"; fi' EXIT

jq -n \
  --arg command "$label" \
  --arg started_at "$started_at" \
  --argjson duration_ms "$duration_ms" \
  --argjson exit_status "$exit_status" \
  --arg log_sha "$log_sha" \
  --arg argv_sha "$argv_sha" \
  '{
    command: $command,
    argvSha256: $argv_sha,
    startedAt: $started_at,
    durationMs: $duration_ms,
    exitStatus: $exit_status,
    logSha256: $log_sha
  }' >"$temporary"
chmod 600 "$temporary"
ln -- "$temporary" "$record_path" || fail "record output appeared during capture"
rm -f -- "$temporary"
temporary=""

printf 'Recorded command evidence: %s (exit %s)\n' "$label" "$exit_status"
exit "$exit_status"
