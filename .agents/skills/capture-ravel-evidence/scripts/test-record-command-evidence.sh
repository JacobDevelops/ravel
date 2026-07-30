#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
subject="$script_dir/record-command-evidence.sh"
test_root="$(mktemp -d)"
trap 'rm -rf -- "$test_root"' EXIT

repository="$test_root/repository"
private_output="$repository/evidence/private/m0/commands"
mkdir -p "$private_output"
chmod 700 "$repository/evidence/private" "$repository/evidence/private/m0" "$private_output"

success_log="$private_output/success.log"
success_record="$private_output/success.json"
RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  'safe fixture command' "$success_log" "$success_record" -- \
  bash -c 'printf "private-secret-output\n"'

jq --exit-status '
  .command == "safe fixture command"
  and (.argvSha256 | test("^[0-9a-f]{64}$"))
  and (.startedAt | type == "string")
  and .durationMs >= 0
  and .exitStatus == 0
  and (.logSha256 | test("^[0-9a-f]{64}$"))
  and ([.. | strings | contains("private-secret-output")] | any | not)
' "$success_record" >/dev/null
test "$(jq -r '.logSha256' "$success_record")" \
  = "$(sha256sum "$success_log" | cut -d' ' -f1)"
test "$(stat -c '%a' "$success_log")" = 600
test "$(stat -c '%a' "$success_record")" = 600

failure_log="$private_output/failure.log"
failure_record="$private_output/failure.json"
set +e
RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  'expected failing command' "$failure_log" "$failure_record" -- \
  bash -c 'exit 7'
failure_status="$?"
set -e
test "$failure_status" -eq 7
test "$(jq -r '.exitStatus' "$failure_record")" -eq 7

if RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  'unsafe output' "$repository/outside.log" "$repository/outside.json" -- \
  true 2>/dev/null; then
  printf 'expected output outside the private evidence boundary to be rejected\n' >&2
  exit 1
fi
test ! -e "$repository/outside.log"
test ! -e "$repository/outside.json"

printf 'record-command-evidence tests passed\n'
