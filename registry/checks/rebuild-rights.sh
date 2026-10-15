#!/usr/bin/env bash
# Records a per-artifact rights and provenance verdict.
#
# The M5-W5 producer. One record per acquired artifact, each carrying the licence
# the artifact declares about itself, how the artifact was obtained, and a status
# per surface in the public provenance schema's vocabulary.
#
# The two verdicts that matter downstream are sourceUse and publication, and they
# answer different questions. sourceUse says whether this workspace may read the
# artifact's source at all; publication says whether anything derived from it
# could ever be released. Neither of them gates the inventory: behaviour is
# cleared everywhere, because Ravel re-specifies observed behaviour rather than
# copying code, and that is the whole reason a pack of mostly reserved licences
# is still a lawful compatibility target.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
# shellcheck source=checks/derivation-reads.sh
. "$checks_dir/derivation-reads.sh"
# shellcheck source=checks/mod-metadata.sh
. "$checks_dir/mod-metadata.sh"
# shellcheck source=checks/surfaces-model.sh
. "$checks_dir/surfaces-model.sh"
# shellcheck source=checks/rights-model.sh
. "$checks_dir/rights-model.sh"

surfaces_model >"$meta_work/surfaces-model"
# shellcheck source=/dev/null
. "$meta_work/surfaces-model"
rights_model >"$meta_work/rights-model"
# shellcheck source=/dev/null
. "$meta_work/rights-model"

records="$registry_root/inventory/${FIXTURE_ID}/rights.json"

jq --sort-keys -n \
  --arg fixture_id "$FIXTURE_ID" \
  --argjson reads "$(derivation_reads "${BASH_SOURCE[0]}")" \
  --arg artifact_set_sha "$artifact_set_sha" \
  --argjson artifact_count "$(wc -l <"$artifact_tsv")" \
  --slurpfile artifacts <(jq -R 'split("\t") | {
      artifact: .[0],
      class: .[1],
      reference: { projectId: (.[2] | tonumber), fileId: (.[3] | tonumber) },
      mods: (if .[4] == "-" then [] else (.[4] | split(",") | sort) end),
      licence: { declared: .[5], family: .[6], assetsReservedSeparately: (.[7] == "true") },
      acquisition: .[8],
      rights: [
        { surface: "code", status: .[9] },
        { surface: "textures", status: .[10] },
        { surface: "models", status: .[11] },
        { surface: "sounds", status: .[12] },
        { surface: "names", status: .[13] },
        { surface: "branding", status: .[14] },
        { surface: "behaviour", status: .[15] }
      ],
      sourceUse: .[16],
      publication: .[17]
    }' "$rights_tsv") '
{
  schemaVersion: 1,
  inventoryKind: "rights-triage",
  fixture: {
    id: $fixture_id,
    artifactSet: { count: $artifact_count, sha256: $artifact_set_sha }
  },
  derivation: {
    reads: $reads,
    decompiled: false,
    restrictedContentRead: false,
    statusVocabulary: "schemas/provenance.schema.json rights[].status",
    producedBy: "checks/rebuild-rights.sh"
  },
  inherited: {
    names: "unresolved for every artifact. Whether an original mod id may be advertised is an interoperability question SPEC.md 7.3 defers, and M5-W1 already recorded it that way for the pack.",
    branding: "prohibited wherever a mod declares a logo. A mod logo belongs to the author who drew it, and this workspace reproduces none of them. The pack branding, quests and scripts were prohibited by M5-W1 for the same reason: they name material nothing here extracts, so no later review clears them.",
    behaviour: "cleared for every artifact. The method is observation and respecification, not copying, so a licence that forbids source use narrows the method rather than blocking the work."
  },
  totals: {
    artifacts: ($artifacts | length),
    licenceFamilies: ($artifacts | group_by(.licence.family)
      | map({ key: .[0].licence.family, value: length }) | from_entries),
    acquisition: ($artifacts | group_by(.acquisition)
      | map({ key: .[0].acquisition, value: length }) | from_entries),
    sourceUse: ($artifacts | group_by(.sourceUse)
      | map({ key: .[0].sourceUse, value: length }) | from_entries),
    publication: ($artifacts | group_by(.publication)
      | map({ key: .[0].publication, value: length }) | from_entries),
    sourceUseProhibited: ($artifacts | map(select(.sourceUse == "prohibited")) | length),
    sourceUseUnresolved: ($artifacts | map(select(.sourceUse == "unresolved")) | length),
    publicationCleared: ($artifacts | map(select(.publication == "cleared")) | length)
  },
  artifacts: ($artifacts | sort_by(.artifact))
}' >"$records.new"

mv "$records.new" "$records"
jq -r '"rights rebuilt: \(.totals.artifacts) artifacts, \(.totals.sourceUseProhibited) source-prohibited, \(.totals.sourceUseUnresolved) source-unresolved"' "$records"
