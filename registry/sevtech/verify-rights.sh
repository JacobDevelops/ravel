#!/usr/bin/env bash
# WP0 byte-bound rights gate; the historical M5 verifier is not modified.
set -euo pipefail
export RAVEL_FIXTURE_ID="${RAVEL_FIXTURE_ID:-sevtech-ages-3.2.3}"
. "$(dirname "${BASH_SOURCE[0]}")/../checks/public-repo.sh"
mode="${1:---fixture}"
case "$mode" in --offline|--fixture) ;; *) echo "usage: $0 [--offline|--fixture]" >&2; exit 2 ;; esac
references="$registry_root/inventory/${FIXTURE_ID}/references.json"
if ! jq -e 'has("acquiredArtifacts") and has("bundledArtifacts")' "$references" >/dev/null; then
  exec bash "$registry_root/checks/verify-rights.sh" "$mode"
fi
if [ "$mode" = --offline ]; then
  python3 -B "$registry_root/sevtech/rights-static.py" "$repo_root" "$FIXTURE_ID"
  echo "WP0 rights: SKIPPED acquired-byte reconciliation (--offline); no grant bytes verified"
  exit 0
fi
. "$registry_root/sevtech/artifact-set.sh"
cat "$registry_root/checks/licence-families.tsv" \
  "$registry_root/inventory/${FIXTURE_ID}/licence-families.tsv" >"$artifact_work/licence-families.tsv"
python3 -B "$registry_root/sevtech/rights-evidence.py" \
  "$registry_root/inventory/${FIXTURE_ID}/rights-evidence.json" \
  "$artifact_paths_tsv" "$artifact_work/licence-families.tsv" \
  "$artifact_work/grants.tsv" "$artifact_work/acquisition.tsv" \
  "$fixture_root/${FIXTURE_ID}/rights-evidence/grants" \
  "$registry_root/inventory/${FIXTURE_ID}/rights.json" "$artifact_tsv" "$FIXTURE_ID" "$artifact_set_sha"
echo "WP0 rights: acquired hashes, host observations, cited grant bytes and rights decisions validated"
echo "NOT-YET-MET: reproducible legacy graph/surfaces derivation; no TOML metadata or observed implementation reads claimed"
