#!/usr/bin/env bash
# Reconciles the pinned-reference records against the fixture they came from.
#
# Nothing here reads a number this repository wrote down. The count, the
# project and file IDs, and the two digests are all recomputed from the fixture
# on disk and from the public descriptor that pins it, so the reconciliation is
# something a reader reruns rather than something a reader is told.
set -euo pipefail

# shellcheck source=checks/public-repo.sh
. "$(dirname "${BASH_SOURCE[0]}")/public-repo.sh"

records="$registry_root/inventory/${FIXTURE_ID}/references.json"
provenance="$registry_root/provenance/${FIXTURE_ID}.json"
descriptor="$repo_root/tests/fixtures/descriptors/${FIXTURE_ID}.json"
manifest="$repo_root/tests/fixtures/local/${FIXTURE_ID}/extracted/manifest.json"
archive="$repo_root/tests/fixtures/local/${FIXTURE_ID}/source/${FIXTURE_ID}.zip"

violations=()
note() { violations+=("$1"); }
# The provenance instance is held to the public schema rather than to a
# paraphrase of it, which is the whole reason it is an instance of that schema.
if command -v check-jsonschema >/dev/null 2>&1; then
  if ! check-jsonschema --schemafile "$repo_root/schemas/provenance.schema.json" \
    "$provenance" >/dev/null 2>&1; then
    note "provenance record does not validate against schemas/provenance.schema.json"
  fi
else
  note "check-jsonschema is not on PATH; run this from the repository's mise environment"
fi
mode="${1:---fixture}"
case "$mode" in --offline|--fixture) ;; *) echo "usage: $0 [--offline|--fixture]" >&2; exit 2 ;; esac

if [ "$mode" = "--offline" ]; then
  if [ "${#violations[@]}" -ne 0 ]; then
    printf '%s\n' "${violations[@]}" >&2
    exit 1
  fi
  echo "inventory provenance schema validated; SKIPPED: acquired fixture reconciliation (--offline)" >&2
  exit 0
fi
for path in "$records" "$provenance" "$descriptor" "$manifest" "$archive"; do
  if [ ! -f "$path" ]; then
    note "missing input: $path"
  fi
done
if [ ${#violations[@]} -ne 0 ]; then
  printf 'inventory reconciliation failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

# The fixture's own bytes, recomputed. A fixture that moved under the records
# invalidates every record derived from it, so this is checked before the
# records are read at all.
manifest_sha="$(sha256sum "$manifest" | cut -d' ' -f1)"
archive_sha="$(sha256sum "$archive" | cut -d' ' -f1)"

recorded_manifest_sha="$(jq -r '.fixture.manifest.sha256' "$records")"
recorded_archive_sha="$(jq -r '.fixture.archive.sha256' "$records")"
pinned_manifest_sha="$(jq -r '.manifest.sha256' "$descriptor")"
pinned_archive_sha="$(jq -r '.source.sha256' "$descriptor")"

if [ "$manifest_sha" != "$recorded_manifest_sha" ]; then
  note "fixture manifest digest changed under the records: fixture $manifest_sha, records $recorded_manifest_sha"
fi
if [ "$archive_sha" != "$recorded_archive_sha" ]; then
  note "fixture archive digest changed under the records: fixture $archive_sha, records $recorded_archive_sha"
fi
if [ "$manifest_sha" != "$pinned_manifest_sha" ]; then
  note "fixture manifest digest does not match the public descriptor pin: fixture $manifest_sha, pinned $pinned_manifest_sha"
fi
if [ "$archive_sha" != "$pinned_archive_sha" ]; then
  note "fixture archive digest does not match the public descriptor pin: fixture $archive_sha, pinned $pinned_archive_sha"
fi

# The count, from three independent statements of it.
fixture_count="$(jq '.files | length' "$manifest")"
record_count="$(jq '.references | length' "$records")"
declared_count="$(jq '.fixture.expectedReferenceCount' "$records")"
pinned_count="$(jq '.manifest.expectedReferenceCount' "$descriptor")"

if [ "$record_count" != "$fixture_count" ]; then
  note "record count does not reconcile: $record_count records for $fixture_count pinned references"
fi
if [ "$declared_count" != "$fixture_count" ]; then
  note "declared reference count does not reconcile: records declare $declared_count, fixture has $fixture_count"
fi
if [ "$pinned_count" != "$fixture_count" ]; then
  note "public descriptor pins $pinned_count references but the fixture manifest has $fixture_count"
fi

# Every correction has to name evidence, and has to still describe the fixture
# it corrects. A correction whose fixtureValue has drifted is stale, and a stale
# correction silently licenses a wrong record.
while IFS= read -r line; do
  [ -n "$line" ] && note "$line"
done < <(jq -r --slurpfile fixture "$manifest" '
  (.corrections // []) as $corrections
  | ($fixture[0].files
     | to_entries
     | map({
         ordinal: .key,
         projectId: .value.projectID,
         fileId: .value.fileID,
         required: .value.required,
         locked: .value.isLocked
       })) as $expected
  | [ $corrections[]
      | . as $c
      | ([ if (.ordinal | type) != "number" then "correction has no ordinal" else empty end,
           if (["projectId","fileId","required","locked"] | index($c.field)) == null
             then "correction \($c.ordinal): unknown field \($c.field // "null")" else empty end,
           if (($c.evidence.kind // "") | IN("source","hash")) | not
             then "correction \($c.ordinal) \($c.field): evidence.kind must be source or hash" else empty end,
           if (($c.evidence.locator // "") | length) == 0
             then "correction \($c.ordinal) \($c.field): evidence.locator is required" else empty end,
           if ($expected[$c.ordinal] // null) == null
             then "correction \($c.ordinal): no such pinned reference" else empty end,
           if ($expected[$c.ordinal] // null) != null
              and $expected[$c.ordinal][$c.field] != $c.fixtureValue
             then "correction \($c.ordinal) \($c.field) is stale: fixture now holds \($expected[$c.ordinal][$c.field])"
             else empty end
         ] | .[])
    ] | .[]' "$records")

# The reconciliation itself, in both directions and field by field.
while IFS= read -r line; do
  [ -n "$line" ] && note "$line"
done < <(jq -r --slurpfile fixture "$manifest" '
  (.corrections // []) as $corrections
  | ($fixture[0].files
     | to_entries
     | map({
         ordinal: .key,
         projectId: .value.projectID,
         fileId: .value.fileID,
         required: .value.required,
         locked: .value.isLocked
       })
     | map(. as $entry
           | reduce ($corrections[] | select(.ordinal == $entry.ordinal)) as $c
             ($entry; .[$c.field] = $c.recordValue))) as $expected
  | .references as $actual
  | ($expected | map(.ordinal)) as $expected_ordinals
  | ($actual | map(.ordinal)) as $actual_ordinals
  | [
      ( $expected[]
        | select((. .ordinal) as $o | ($actual_ordinals | index($o)) == null)
        | "no record for pinned reference \(.ordinal): project \(.projectId) file \(.fileId)" ),
      ( $actual[]
        | select((. .ordinal) as $o | ($expected_ordinals | index($o)) == null)
        | "record \(.ordinal) has no pinned reference: project \(.projectId) file \(.fileId)" ),
      ( $expected[]
        | . as $e
        | ($actual[] | select(.ordinal == $e.ordinal)) as $a
        | ["projectId","fileId","required","locked"][]
        | select($a[.] != $e[.]) as $field
        | "reference \($e.ordinal) \($field) does not reconcile: record \($a[$field]), fixture \($e[$field])" )
    ] | .[]' "$records")


if [ ${#violations[@]} -ne 0 ]; then
  printf 'inventory reconciliation failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

echo "inventory reconciles: $record_count/$fixture_count pinned references, fixture digests bound"
