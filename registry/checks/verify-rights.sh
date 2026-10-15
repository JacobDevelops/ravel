#!/usr/bin/env bash
# Holds the committed rights triage to the artifacts and to its own vocabulary.
#
# The consumer for M5-W5. It recomputes the triage and requires the committed
# file to be it, and then it checks the things a recomputation cannot: that every
# status word is one the public provenance schema allows, that the licence string
# in each record is the string the artifact actually declares, that the three
# inherited rules are applied to every record rather than to most of them, and
# that no artifact was quietly cleared for publication while a surface of it is
# unresolved or prohibited.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/public-repo.sh
. "$checks_dir/public-repo.sh"
mode="${1:---fixture}"
case "$mode" in --offline|--fixture) ;; *) echo "usage: $0 [--offline|--fixture]" >&2; exit 2 ;; esac

records="$registry_root/inventory/${FIXTURE_ID}/rights.json"
schema="$repo_root/schemas/provenance.schema.json"

violations=()
note() { violations+=("$1"); }

for path in "$records" "$schema"; do
  if [ ! -f "$path" ]; then
    printf 'rights verification failed:\n- missing input: %s\n' "$path" >&2
    exit 1
  fi
done
meta_work="$(mktemp -d)"
trap 'rm -rf "$meta_work"' EXIT
# Every status word comes from the public schema. This reads the enumeration out
# of the schema rather than repeating it, so the two cannot drift apart.
jq -r '.properties.rights.items.properties.status.enum[]' "$schema" | sort >"$meta_work/allowed-status"
jq -r '.artifacts[].rights[].status' "$records" | sort -u >"$meta_work/used-status"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "rights status is not in the public schema vocabulary: $bad"
done < <(comm -13 "$meta_work/allowed-status" "$meta_work/used-status")
jq -r '.properties.rights.items.properties.surface.enum[]' "$schema" | sort >"$meta_work/allowed-surface"
jq -r '.artifacts[].rights[].surface' "$records" | sort -u >"$meta_work/used-surface"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "rights surface is not in the public schema vocabulary: $bad"
done < <(comm -13 "$meta_work/allowed-surface" "$meta_work/used-surface")
statuses="$checks_dir/licence-status.tsv"
if [ ! -f "$statuses" ]; then
  note "missing input: checks/licence-status.tsv, which is where what a licence family permits is decided"
else
  jq -r '.artifacts[] | [.licence.family, ([.rights[] | select(.surface == "code") | .status] | first)] | @tsv' "$records" |
    LC_ALL=C sort -u >"$meta_work/recorded-status"
  cut -f1,2 "$statuses" | LC_ALL=C sort -u >"$meta_work/decided-status"
  while IFS=$'\t' read -r family status; do
    [ -n "$family" ] && note "the triage gives a licence family a code status no decision records: $family -> $status"
  done < <(LC_ALL=C comm -23 "$meta_work/recorded-status" "$meta_work/decided-status")
  while IFS=$'\t' read -r family status; do
    [ -n "$family" ] && note "a decided family status covers a family nothing in the fixture declares, so the decision is stale: $family -> $status"
  done < <(LC_ALL=C comm -13 "$meta_work/recorded-status" "$meta_work/decided-status")
  # Every decided status is a word the public schema admits, and every row says
  # why. A table with a blank reason column is a regex with extra steps.
  while IFS=$'\t' read -r family status reason; do
    [ -z "$family" ] && continue
    if ! grep -qxF "$status" "$meta_work/allowed-status"; then
      note "checks/licence-status.tsv decides a status the public schema does not admit: $family -> $status"
    fi
    if [ "${#reason}" -lt 40 ]; then
      note "checks/licence-status.tsv records no reason for what it decides about $family"
    fi
  done <"$statuses"
fi
if [ "$mode" = "--offline" ]; then
  if [ "${#violations[@]}" -ne 0 ]; then
    printf '%s\n' "${violations[@]}" >&2
    exit 1
  fi
  echo "rights schema vocabulary validated; SKIPPED: acquired fixture rights reconciliation (--offline)" >&2
  exit 0
fi
rm -rf "$meta_work"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
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

if [ "$(jq -r '.fixture.artifactSet.sha256' "$records")" != "$artifact_set_sha" ]; then
  note "triage is bound to a different artifact set than the one on disk"
fi

# One record per acquired artifact.
cut -f2 "$artifact_tsv" | sort >"$meta_work/acquired"
jq -r '.artifacts[].artifact' "$records" | sort >"$meta_work/triaged"
while IFS= read -r line; do
  [ -n "$line" ] && note "acquired artifact carries no rights record: $line"
done < <(comm -23 "$meta_work/acquired" "$meta_work/triaged")
while IFS= read -r line; do
  [ -n "$line" ] && note "rights record names an artifact that was not acquired: $line"
done < <(comm -13 "$meta_work/acquired" "$meta_work/triaged")



# Each record covers every surface exactly once.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  ["code","textures","models","sounds","names","branding","behaviour"] as $required
  | .artifacts[]
  | . as $a
  | ([$a.rights[].surface] | sort) as $got
  | if $got != ($required | sort)
    then "rights record for \($a.artifact) covers \($got | join(", ")) rather than every surface"
    else empty end' "$records")

# The licence string is the artifact own declaration, checked against the jar.
awk -F'\t' '$2 == "LICENSE" { print $1 "\t" $3 }' "$meta_tsv" | sort >"$meta_work/declared-licences"
jq -r '.artifacts[] | select(.licence.declared != "") | "\(.artifact)\t\(.licence.declared)"' "$records" |
  sort >"$meta_work/recorded-licences"
if ! diff -q "$meta_work/declared-licences" "$meta_work/recorded-licences" >/dev/null; then
  note "a recorded licence is not the licence the artifact declares"
fi

# Every rights decision a licence drives, against tables this file owns.
#
# Nothing about a licence is measurable from the fixture. `bucket()`,
# `reserves_assets()` and `code_status()` in rights-model.sh are run by the
# producer and again by this consumer, and no other reading of a licence exists
# in this workspace, so all three agree by construction: they are the same
# sentences run twice. A second opinion about a licence can only be a person who
# wrote the answer down, so the answers are written down, and the tables are read
# here rather than computed.
#
# Round 4 wrote down the first of the three, and round 5's review found that it
# was the least valuable of them. A family is not a verdict. `code_status()`
# turns a family into one of the five schema statuses one function below
# `bucket()`, it was written once, and changing that one line moved 65 artifacts
# from source-prohibited to source-cleared and 65 port plans from black-box
# respecification to reading the source with the whole gate still exiting 0 —
# because no family had moved, so the family table stayed silent. The same was
# true of `reserves_assets()`, which decides whether an author reserved their
# assets apart from their code and so decides three statuses per artifact.
#
# So all three are tabled now, in two files:
#
#   checks/licence-families.tsv   declared string -> family, assets reserved
#   checks/licence-status.tsv     family -> the code status it carries, and why
#
# and the set of tuples the records hold has to equal the set the tables hold,
# in both directions. Changing a licence verdict is then a tracked edit to a file
# of licence decisions with a reason beside each one, which is a thing a reviewer
# reads, rather than a regex nobody diffs. What the tables cannot do is tell a
# right decision from a wrong one: they record the decision, and a hand that
# edits the model and the table together has changed a recorded decision in the
# open. That is the boundary, and it is stated in the README rather than implied.
families="$checks_dir/licence-families.tsv"
if [ ! -f "$families" ]; then
  note "missing input: checks/licence-families.tsv, which is where a licence family is decided"
else
  jq -r '.artifacts[] | [.licence.declared, .licence.family, (.licence.assetsReservedSeparately | tostring)] | @tsv' "$records" |
    LC_ALL=C sort -u >"$meta_work/recorded-families"
  LC_ALL=C sort -u "$families" >"$meta_work/decided-families"
  while IFS=$'\t' read -r declared family reserved; do
    [ -n "$family" ] && note "the triage reads a declared licence in a way no decision records: ${declared:-(no licence declared)} -> $family, assets reserved separately: $reserved"
  done < <(LC_ALL=C comm -23 "$meta_work/recorded-families" "$meta_work/decided-families")
  while IFS=$'\t' read -r declared family reserved; do
    [ -n "$family" ] && note "a recorded licence decision matches nothing the fixture declares, so it is stale: ${declared:-(no licence declared)} -> $family, assets reserved separately: $reserved"
  done < <(LC_ALL=C comm -13 "$meta_work/recorded-families" "$meta_work/decided-families")
fi


# The asset surfaces, restated here rather than taken from the model.
#
# `asset_status()` says an artifact that ships none of a thing is not-applicable
# for it, that a reserved asset is prohibited whatever the code licence allows,
# and that everything else follows the code. That is three decisions, and until
# round 5 they were written once. They are written twice now: the sentence below
# is this file's own, and the counts it asks about come from the surfaces model
# rather than from the triage that is being checked.
awk -F'\t' 'BEGIN { OFS = "\t" } { print $1, $8, $9, $10 }' "$surfaces_tsv" |
  LC_ALL=C sort >"$meta_work/asset-presence"
jq -r '.artifacts[] | [.artifact, (.licence.assetsReservedSeparately | tostring),
    ([.rights[] | select(.surface == "code") | .status] | first),
    ([.rights[] | select(.surface == "textures") | .status] | first),
    ([.rights[] | select(.surface == "models") | .status] | first),
    ([.rights[] | select(.surface == "sounds") | .status] | first)] | @tsv' "$records" |
  LC_ALL=C sort >"$meta_work/asset-status"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(awk -F'\t' -v presence="$meta_work/asset-presence" '
  function want(n, reserved, code) {
    if (n + 0 == 0) return "not-applicable"
    if (reserved == "true") return "prohibited"
    return code
  }
  FILENAME == presence { tex[$1] = $2; mod[$1] = $3; snd[$1] = $4; next }
  {
    if (!($1 in tex)) { printf "%s is triaged and the surfaces model catalogues no such artifact\n", $1; next }
    split("textures models sounds", surface, " ")
    have[1] = tex[$1]; have[2] = mod[$1]; have[3] = snd[$1]
    got[1] = $4; got[2] = $5; got[3] = $6
    for (i = 1; i <= 3; i++) {
      expected = want(have[i], $2, $3)
      if (got[i] != expected)
        printf "%s records %s as %s; it ships %d of them under a code status of %s with assets reserved separately: %s\n",
          $1, surface[i], got[i], have[i], $3, $2
    }
  }' "$meta_work/asset-presence" "$meta_work/asset-status")

# The three inherited rules, applied to every record rather than to most.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  .artifacts[]
  | . as $a
  | ([$a.rights[] | select(.surface == "names") | .status] | first) as $names
  | ([$a.rights[] | select(.surface == "behaviour") | .status] | first) as $behaviour
  | [ if $names != "unresolved" then "\($a.artifact) records names as \($names); M5-W1 fixed it at unresolved" else empty end,
      if $behaviour != "cleared" then "\($a.artifact) records behaviour as \($behaviour); the method is respecification and is cleared everywhere" else empty end
    ] | .[]' "$records")

logo_artifacts="$(awk -F'\t' '$2 == "MOD" && $6 != "" { print $1 }' "$meta_tsv" | sort -u | wc -l)"
branding_prohibited="$(jq '[.artifacts[] | select(.rights[] | .surface == "branding" and .status == "prohibited")] | length' "$records")"
if [ "$logo_artifacts" -ne "$branding_prohibited" ]; then
  note "branding is prohibited for $branding_prohibited artifacts but $logo_artifacts declare a logo"
fi

# No record is cleared for publication while one of its own surfaces is not.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  .artifacts[]
  | select(.publication == "cleared")
  | select([.rights[].status] | any(. == "unresolved" or . == "prohibited"))
  | "\(.artifact) is cleared for publication while one of its surfaces is not"' "$records")

# sourceUse follows the code status and nothing else.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  { cleared: "permitted", "permission-required": "conditional",
    prohibited: "prohibited", unresolved: "unresolved" } as $expected
  | .artifacts[]
  | . as $a
  | ([$a.rights[] | select(.surface == "code") | .status] | first) as $code
  | select($a.sourceUse != $expected[$code])
  | "\($a.artifact) records sourceUse \($a.sourceUse) for code status \($code)"' "$records")

# Totals are arithmetic over the records.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  . as $r
  | [ if $r.totals.artifacts != ($r.artifacts | length) then "totals.artifacts disagrees with the records" else empty end,
      if $r.totals.sourceUseProhibited != ([$r.artifacts[] | select(.sourceUse == "prohibited")] | length)
        then "totals.sourceUseProhibited disagrees with the records" else empty end,
      if $r.totals.sourceUseUnresolved != ([$r.artifacts[] | select(.sourceUse == "unresolved")] | length)
        then "totals.sourceUseUnresolved disagrees with the records" else empty end,
      if $r.totals.publicationCleared != ([$r.artifacts[] | select(.publication == "cleared")] | length)
        then "totals.publicationCleared disagrees with the records" else empty end,
      if ($r.totals.licenceFamilies | to_entries | map(.value) | add) != ($r.artifacts | length)
        then "totals.licenceFamilies does not cover every record" else empty end,
      if ($r.totals.acquisition | to_entries | map(.value) | add) != ($r.artifacts | length)
        then "totals.acquisition does not cover every record" else empty end
    ] | .[]' "$records")

# The model, compared field by field.
jq -S '.artifacts' "$records" >"$meta_work/committed.json"
jq -S -R -s 'split("\n") | map(select(length > 0)) | map(split("\t") | {
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
  }) | sort_by(.artifact)' "$rights_tsv" >"$meta_work/recomputed.json"
if ! diff -q "$meta_work/committed.json" "$meta_work/recomputed.json" >/dev/null; then
  note "triage does not match the rights recomputed from the acquired artifacts"
fi

if [ ${#violations[@]} -ne 0 ]; then
  printf 'rights verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

printf 'rights reconcile: %s artifacts triaged, %s source-prohibited, %s source-unresolved, %s publishable\n' \
  "$(jq '.totals.artifacts' "$records")" "$(jq '.totals.sourceUseProhibited' "$records")" \
  "$(jq '.totals.sourceUseUnresolved' "$records")" "$(jq '.totals.publicationCleared' "$records")"
