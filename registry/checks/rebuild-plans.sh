#!/usr/bin/env bash
# Assigns a canonical Ravel-native id and a port plan to every pinned entry.
#
# The M5-W6 producer. One plan per acquired artifact: the slot it occupies in
# Ravel's inventory, the lane that owns the work, the method the entry's rights
# admit, the entries it depends on by their canonical ids, the tests its
# catalogued surfaces oblige, and a seven-component score.
#
# The plan says what work is intended. It never says how the entry does what it
# does, because nothing in this workspace has read that and nothing here may. A
# port plan is a statement about the inventory, not an extract of a mod.
#
# The vocabulary — the id scheme, the lanes, the methods, the test kinds and the
# rubric scales — lives in checks/plans-vocabulary.json and is copied into the
# record rather than written twice. The consumer reads the same file and requires
# the copy to be it, so the documentation of a rule and the rule cannot drift.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
# shellcheck source=checks/derivation-reads.sh
. "$checks_dir/derivation-reads.sh"
# shellcheck source=checks/mod-metadata.sh
. "$checks_dir/mod-metadata.sh"
# shellcheck source=checks/graph-model.sh
. "$checks_dir/graph-model.sh"
# shellcheck source=checks/surfaces-model.sh
. "$checks_dir/surfaces-model.sh"
# shellcheck source=checks/rights-model.sh
. "$checks_dir/rights-model.sh"
# shellcheck source=checks/plans-model.sh
. "$checks_dir/plans-model.sh"

for stage in graph surfaces rights plans; do
  "${stage}_model" >"$meta_work/$stage-model"
  # shellcheck source=/dev/null
  . "$meta_work/$stage-model"
done

records="$registry_root/inventory/${FIXTURE_ID}/plans.json"
vocabulary="$checks_dir/plans-vocabulary.json"

jq --sort-keys -n \
  --arg fixture_id "$FIXTURE_ID" \
  --argjson reads "$(derivation_reads "${BASH_SOURCE[0]}")" \
  --arg artifact_set_sha "$artifact_set_sha" \
  --argjson artifact_count "$(wc -l <"$artifact_tsv")" \
  --slurpfile vocabulary "$vocabulary" \
  --slurpfile plans <(jq -R 'split("\t") | {
      artifact: .[0],
      canonicalId: .[1],
      referenceOrdinal: (.[2] | tonumber),
      owner: .[3],
      method: { name: .[4], readsSource: (.[5] == "true") },
      sourceUse: .[6],
      dependencies: {
        entries: (if .[7] == "-" then [] else (.[7] | split(",") | sort) end),
        platform: (.[8] | tonumber),
        absentFromPack: (.[9] | tonumber),
        optionalOutsidePack: (.[10] | tonumber)
      },
      tests: (.[11] | split(",") | sort),
      rubric: {
        foundationValue: (.[12] | tonumber),
        dependencyCentrality: (.[13] | tonumber),
        integrationBreadth: (.[14] | tonumber),
        compatibilityUncertainty: (.[15] | tonumber),
        rightsRisk: (.[16] | tonumber),
        expectedMemoryImpact: (.[17] | tonumber),
        verticalSliceValue: (.[18] | tonumber)
      },
      riskScore: (.[19] | tonumber),
      rank: (.[20] | tonumber)
    }' "$plans_tsv") '
{
  schemaVersion: 1,
  inventoryKind: "port-plans",
  fixture: {
    id: $fixture_id,
    artifactSet: { count: $artifact_count, sha256: $artifact_set_sha }
  },
  derivation: {
    reads: $reads,
    decompiled: false,
    restrictedContentRead: false,
    handAssignedFields: 0,
    vocabulary: "checks/plans-vocabulary.json",
    producedBy: "checks/rebuild-plans.sh"
  },
  vocabulary: $vocabulary[0],
  totals: {
    plans: ($plans | length),
    owners: ($plans | group_by(.owner) | map({ key: .[0].owner, value: length }) | from_entries),
    methods: ($plans | group_by(.method.name) | map({ key: .[0].method.name, value: length }) | from_entries),
    methodsReadingSource: ([$plans[] | select(.method.readsSource)] | length),
    sourceUse: ($plans | group_by(.sourceUse) | map({ key: .[0].sourceUse, value: length }) | from_entries),
    riskScore: ($plans | group_by(.riskScore) | map({ key: (.[0].riskScore | tostring), value: length }) | from_entries),
    rank: { min: ([$plans[].rank] | min), max: ([$plans[].rank] | max) },
    dependencyEdges: ([$plans[] | .dependencies.entries | length] | add),
    canonicalIds: ([$plans[].canonicalId] | unique | length)
  },
  plans: ($plans | sort_by(.canonicalId))
}' >"$records.new"

mv "$records.new" "$records"
jq -r '"plans rebuilt: \(.totals.plans) entries, \(.totals.canonicalIds) canonical ids, \(.totals.methodsReadingSource) reading source"' "$records"
