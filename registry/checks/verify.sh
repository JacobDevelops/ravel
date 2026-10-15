#!/usr/bin/env bash
# Offline checks by default; --fixture additionally requires acquired local inputs.
set -euo pipefail

checks="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mode="${1:---offline}"
if [ "$#" -gt 1 ] || { [ "$mode" != "--offline" ] && [ "$mode" != "--fixture" ]; }; then
  echo "usage: registry/checks/verify.sh [--offline|--fixture]" >&2
  exit 2
fi
status=0

"$checks/verify-boundary.sh" || status=1
"$checks/verify-primitives.sh" || status=1
"$checks/verify-proposals.sh" || status=1
"$checks/verify-wave.sh" || status=1
"$checks/verify-notes-rows.sh" || status=1
"$checks/verify-counterparts.sh" || status=1
"$checks/verify-counterpart-packages.sh" || status=1
"$checks/verify-counterpart-build.sh" || status=1
"$checks/verify-spec-identifiers.sh" || status=1
for check in verify-inventory.sh verify-rights.sh verify-provenance.sh verify-spec-source-names.sh; do
  "$checks/$check" "$mode" || status=1
done
fixture_checks=(
  verify-accounting.sh
  verify-graph.sh
  verify-surfaces.sh
  verify-reads.sh
  verify-plans.sh
)
if [ "$mode" = "--fixture" ]; then
  for check in "${fixture_checks[@]}"; do
    "$checks/$check" || status=1
  done
else
  printf 'REGISTRY FIXTURE CHECKS SKIPPED (--offline): acquired pack and clean-room source inputs were NOT verified.\n' >&2
  printf 'SKIPPED: %s\n' "${fixture_checks[@]}" >&2
  printf 'Run cargo make ravel:registry-verify-fixture to require every fixture obligation; missing inputs fail that task.\n' >&2
fi

exit "$status"
