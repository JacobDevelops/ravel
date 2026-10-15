#!/usr/bin/env bash
# Rebuilds the pinned-reference records from the fixture manifest.
#
# This is the producer `derivation.producedBy` names. It reads four metadata
# fields per entry and nothing else, so what it can write is bounded by what it
# reads: no record can carry restricted content because no restricted content
# is opened. `verify-inventory.sh` is the consumer and does not trust this
# script — it recomputes the same comparison from the fixture on disk.
set -euo pipefail

# shellcheck source=checks/public-repo.sh
. "$(dirname "${BASH_SOURCE[0]}")/public-repo.sh"
checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/derivation-reads.sh
. "$checks_dir/derivation-reads.sh"

records="$registry_root/inventory/${FIXTURE_ID}/references.json"
manifest="$repo_root/tests/fixtures/local/${FIXTURE_ID}/extracted/manifest.json"
archive="$repo_root/tests/fixtures/local/${FIXTURE_ID}/source/${FIXTURE_ID}.zip"

for path in "$manifest" "$archive"; do
  if [ ! -f "$path" ]; then
    echo "fixture input missing: ${path#"$repo_root/"}" >&2
    exit 1
  fi
done

corrections="$(jq -c '.corrections // []' "$records" 2>/dev/null || echo '[]')"

jq --sort-keys \
  --arg fixture_id "$FIXTURE_ID" \
  --argjson reads "$(derivation_reads "${BASH_SOURCE[0]}")" \
  --arg manifest_sha "$(sha256sum "$manifest" | cut -d' ' -f1)" \
  --arg archive_sha "$(sha256sum "$archive" | cut -d' ' -f1)" \
  --argjson corrections "$corrections" '
{
  schemaVersion: 1,
  inventoryKind: "pinned-reference",
  fixture: {
    id: $fixture_id,
    publicDescriptor: "tests/fixtures/descriptors/${FIXTURE_ID}.json",
    archive: {
      path: "tests/fixtures/local/${FIXTURE_ID}/source/${FIXTURE_ID}.zip",
      sha256: $archive_sha
    },
    manifest: {
      path: "tests/fixtures/local/${FIXTURE_ID}/extracted/manifest.json",
      sha256: $manifest_sha
    },
    name: .name,
    version: .version,
    author: .author,
    minecraft: .minecraft.version,
    modLoader: (.minecraft.modLoaders[] | select(.primary) | .id),
    expectedReferenceCount: (.files | length)
  },
  derivation: {
    reads: $reads,
    restrictedContentRead: false,
    producedBy: "checks/rebuild-references.sh"
  },
  corrections: $corrections,
  references: [
    .files
    | to_entries[]
    | {
        ordinal: .key,
        projectId: .value.projectID,
        fileId: .value.fileID,
        required: .value.required,
        locked: .value.isLocked
      }
  ]
}' "$manifest" >"$records.new"

mv "$records.new" "$records"
echo "rebuilt $(jq '.references | length' "$records") records from the fixture manifest"
