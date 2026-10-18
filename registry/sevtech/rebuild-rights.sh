#!/usr/bin/env bash
# Regenerate mutable WP0 triage only; historical results are never rewritten.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/artifact-set.sh"
if [ "${acquisition_mode:-launcher}" != standalone ]; then
  echo "WP0 rights regeneration refuses historical launcher inventories" >&2
  exit 1
fi
cat "$registry_root/checks/licence-families.tsv" \
  "$registry_root/inventory/${FIXTURE_ID}/licence-families.tsv" >"$artifact_work/licence-families.tsv"
python3 "$registry_root/sevtech/rights-evidence.py" \
  "$registry_root/inventory/${FIXTURE_ID}/rights-evidence.json" \
  "$artifact_paths_tsv" "$artifact_work/licence-families.tsv" \
  "$artifact_work/grants.tsv" "$artifact_work/acquisition.tsv" \
  "$fixture_root/${FIXTURE_ID}/rights-evidence/grants" \
  "$registry_root/inventory/${FIXTURE_ID}/rights.json" "$artifact_tsv" "$FIXTURE_ID" "$artifact_set_sha" --rebuild
