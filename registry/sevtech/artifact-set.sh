#!/usr/bin/env bash
# Standalone acquisition boundary; historical launcher machinery stays immutable.
set -euo pipefail
export RAVEL_FIXTURE_ID="${RAVEL_FIXTURE_ID:-sevtech-ages-3.2.3}"
. "$(dirname "${BASH_SOURCE[0]}")/../checks/public-repo.sh"
references="$registry_root/inventory/${FIXTURE_ID}/references.json"
if ! jq -e 'has("acquiredArtifacts") and has("bundledArtifacts")' "$references" >/dev/null; then
  . "$registry_root/checks/artifact-set.sh"
  return 0
fi
acquisition_mode=standalone
fixture_root="$repo_root/tests/fixtures/local"
instance_root="$fixture_root/${FIXTURE_ID}/extracted/overrides"
pack_manifest="$fixture_root/${FIXTURE_ID}/extracted/manifest.json"
import_log=""
artifact_work="$(mktemp -d)"
trap 'rm -rf "$artifact_work"' EXIT
artifact_tsv="$artifact_work/artifacts.tsv"
artifact_paths_tsv="$artifact_work/artifact-paths.tsv"
python3 "$registry_root/sevtech/artifact-set.py" \
  "$repo_root" "$FIXTURE_ID" "$references" "$artifact_tsv" "$artifact_paths_tsv"
artifact_set_sha="$(cut -f1-3 "$artifact_tsv" | LC_ALL=C sort -t $'\t' -k2,2 | sha256sum | cut -d' ' -f1)"
export acquisition_mode fixture_root instance_root pack_manifest import_log artifact_tsv artifact_paths_tsv artifact_set_sha
