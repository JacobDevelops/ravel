#!/usr/bin/env bash
# Holds the committed port plans to the inventory, to the rights triage, and to
# their own published vocabulary.
#
# The consumer for M5-W6. It recomputes the plans and requires the committed file
# to be them, and then it asks the questions a recomputation cannot answer.
#
# The one that matters is the method. M5-W5 decided, per artifact, whether this
# workspace may read that artifact's source, and a port plan that quietly ignores
# that verdict makes the whole triage worthless. So the closure is checked
# against the committed rights.json rather than against the model this file just
# ran: a plan whose method reads source must carry a sourceUse of permitted in
# the sibling file, and the count of plans that read source without one is
# required to be zero rather than merely unobserved.
#
# The rest is coverage and independence. Every acquired artifact has exactly one
# plan and every plan names an acquired artifact; every canonical id matches the
# scheme, is unique, and is not any identifier upstream owns; every dependency
# names a plan that exists, and the dependency set is recounted from the
# committed graph by an expression that shares nothing with the model's; the
# derived rubric components are recomputed from the committed sibling files
# rather than believed; and every string in a plan record comes from a closed
# vocabulary, so a plan cannot grow a prose field describing what a mod does.
#
# The committed file is canonical, not merely equivalent: plans are ordered by
# canonical id and every list inside one is sorted, because a file two runs can
# disagree about the ordering of is a file no later milestone can diff.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
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

inventory="$registry_root/inventory/${FIXTURE_ID}"
records="$inventory/plans.json"
vocabulary="$checks_dir/plans-vocabulary.json"

violations=()
note() { violations+=("$1"); }

for path in "$records" "$vocabulary" "$inventory/references.json" "$inventory/graph.json" \
  "$inventory/surfaces.json" "$inventory/rights.json"; do
  if [ ! -f "$path" ]; then
    printf 'plan verification failed:\n- missing input: %s\n' "${path#"$registry_root/"}" >&2
    exit 1
  fi
done

for stage in graph surfaces rights plans; do
  "${stage}_model" >"$meta_work/$stage-model"
  # shellcheck source=/dev/null
  . "$meta_work/$stage-model"
done

if [ "$(jq -r '.fixture.artifactSet.sha256' "$records")" != "$artifact_set_sha" ]; then
  note "plans are bound to a different artifact set than the one on disk"
fi

# Coverage, both directions, against the artifacts rather than against a sibling
# file that could be wrong in the same way.
cut -f2 "$artifact_tsv" | sort >"$meta_work/acquired"
jq -r '.plans[].artifact' "$records" | sort >"$meta_work/planned"
while IFS= read -r line; do
  [ -n "$line" ] && note "acquired artifact carries no port plan: $line"
done < <(comm -23 "$meta_work/acquired" "$meta_work/planned")
while IFS= read -r line; do
  [ -n "$line" ] && note "port plan names an entry that was not acquired: $line"
done < <(comm -13 "$meta_work/acquired" "$meta_work/planned")

# The canonical ids: the scheme's own pattern, uniqueness, and the allocation
# recomputed through the committed reference records rather than the manifest.
pattern="$(jq -r '.idScheme.pattern' "$vocabulary")"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "canonical id does not match the scheme $pattern: $bad"
done < <(jq -r --arg p "$pattern" '.plans[].canonicalId | select(test($p) | not)' "$records")

duplicate_ids="$(jq -r '[.plans[].canonicalId] | group_by(.) | map(select(length > 1) | .[0]) | .[]' "$records")"
while IFS= read -r dup; do
  [ -n "$dup" ] && note "canonical id is not unique across the inventory: $dup"
done <<<"$duplicate_ids"

while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r -n \
  --slurpfile plans "$records" --slurpfile refs "$inventory/references.json" \
  --slurpfile surfaces "$inventory/surfaces.json" '
  ([$refs[0].references[] | {key: (.fileId | tostring), value: .ordinal}] | from_entries) as $ordinal
  | ([$surfaces[0].artifacts[] | {key: .artifact, value: (.reference.fileId | tostring)}] | from_entries) as $file
  | $plans[0].plans[]
  | . as $p
  | ($file[$p.artifact]) as $fid
  | if $fid == null then "port plan is bound to no pinned reference: \($p.artifact)"
    elif $ordinal[$fid] == null then "port plan names a file id that is not a pinned reference: \($p.artifact)"
    elif $p.referenceOrdinal != $ordinal[$fid]
      then "\($p.canonicalId) records reference ordinal \($p.referenceOrdinal); the pinned reference is \($ordinal[$fid])"
    elif $p.canonicalId != ("RVL-" + (("00" + (($ordinal[$fid] + 1) | tostring)) | .[-3:]))
      then "\($p.artifact) carries \($p.canonicalId); the slot it is bound to allocates RVL-\((("00" + (($ordinal[$fid] + 1) | tostring)) | .[-3:]))"
    else empty end')

# Ravel-native: an id may not be, or contain, an identifier upstream owns.
{
  jq -r '.nodes[].id' "$inventory/graph.json"
  jq -r '.plans[].artifact' "$records"
  jq -r '.plans[].artifact | sub("\\.(jar|zip)$"; "")' "$records"
  jq -r '.artifacts[] | "\(.reference.projectId)", "\(.reference.fileId)"' "$inventory/surfaces.json"
} | tr '[:upper:]' '[:lower:]' | sort -u >"$meta_work/borrowed"
jq -r '.plans[].canonicalId' "$records" | tr '[:upper:]' '[:lower:]' | sort -u >"$meta_work/assigned"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "canonical id is a borrowed upstream identifier, not a Ravel-native one: $bad"
done < <(comm -12 "$meta_work/borrowed" "$meta_work/assigned")
# The vocabulary is the tracked file, copied rather than restated.
if ! diff -q <(jq -S . "$vocabulary") <(jq -S '.vocabulary' "$records") >/dev/null; then
  note "the recorded vocabulary is not checks/plans-vocabulary.json"
fi

# Every term a plan uses exists, and every term the vocabulary publishes is used.
for kind in owners methods testKinds; do
  jq -r --arg k "$kind" '.[$k] | keys[]' "$vocabulary" | sort >"$meta_work/vocab-$kind"
done
jq -r '.plans[].owner' "$records" | sort -u >"$meta_work/used-owners"
jq -r '.plans[].method.name' "$records" | sort -u >"$meta_work/used-methods"
jq -r '.plans[].tests[]' "$records" | sort -u >"$meta_work/used-testKinds"
for kind in owners methods testKinds; do
  while IFS= read -r bad; do
    [ -n "$bad" ] && note "plan uses a $kind term the vocabulary does not define: $bad"
  done < <(comm -13 "$meta_work/vocab-$kind" "$meta_work/used-$kind")
  while IFS= read -r bad; do
    [ -n "$bad" ] && note "the vocabulary defines a $kind term no plan uses: $bad"
  done < <(comm -23 "$meta_work/vocab-$kind" "$meta_work/used-$kind")
done

# Every rubric component is published with a full 1-5 scale, and the plans carry
# exactly the components the vocabulary defines.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r -n --slurpfile v "$vocabulary" --slurpfile p "$records" '
  ($v[0].rubric | keys) as $defined
  | [ ($v[0].rubric | to_entries[]
       | select((.value.scale | keys) != ["1","2","3","4","5"])
       | "rubric component \(.key) does not publish a full 1-5 scale"),
      ($v[0].rubric | to_entries[]
       | select((.value.derivedFrom | length) == 0 or (.value.judgement | length) == 0)
       | "rubric component \(.key) does not say what it is derived from and what in it is judgement"),
      ($p[0].plans[] | select((.rubric | keys) != ($defined | sort))
       | "\(.canonicalId) scores \((.rubric | keys) | join(", ")) rather than the published components")
    ] | .[]')

# The method is the entry's rights verdict and nothing else. This reads the
# committed triage, not the model above, so the two files have to agree.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r -n --slurpfile p "$records" --slurpfile r "$inventory/rights.json" --slurpfile v "$vocabulary" '
  ([$r[0].artifacts[] | {key: .artifact, value: .sourceUse}] | from_entries) as $verdict
  | $v[0].methods as $methods
  | $p[0].plans[]
  | . as $plan
  | ($verdict[$plan.artifact]) as $triaged
  | [ if $triaged == null then "\($plan.canonicalId) has no rights verdict in the triage" else empty end,
      if $triaged != null and $plan.sourceUse != $triaged
        then "\($plan.canonicalId) records sourceUse \($plan.sourceUse); the triage says \($triaged)" else empty end,
      if $methods[$plan.method.name] == null
        then "\($plan.canonicalId) names an undefined method \($plan.method.name)" else empty end,
      if $methods[$plan.method.name] != null and $plan.method.readsSource != $methods[$plan.method.name].readsSource
        then "\($plan.canonicalId) claims readsSource \($plan.method.readsSource) for method \($plan.method.name)" else empty end,
      if $methods[$plan.method.name] != null and $triaged != null
         and ($methods[$plan.method.name].admissibleSourceUse | index($triaged)) == null
        then "\($plan.canonicalId) plans \($plan.method.name) for an entry whose source use is \($triaged)" else empty end,
      if $plan.method.readsSource and $triaged != "permitted"
        then "\($plan.canonicalId) plans a method that reads source for an entry whose source use is \($triaged)" else empty end
    ] | .[]')

reading_without_permission="$(jq -n --slurpfile p "$records" --slurpfile r "$inventory/rights.json" '
  ([$r[0].artifacts[] | {key: .artifact, value: .sourceUse}] | from_entries) as $verdict
  | [$p[0].plans[] | select(.method.readsSource) | select($verdict[.artifact] != "permitted")] | length')"
if [ "$reading_without_permission" -ne 0 ]; then
  note "$reading_without_permission plans read source without a permitted rights verdict"
fi

# Dependencies close over the plan set, and are recounted from the committed
# graph by an expression that shares nothing with the model's awk.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r -n --slurpfile p "$records" --slurpfile g "$inventory/graph.json" '
  ([$p[0].plans[] | {key: .artifact, value: .canonicalId}] | from_entries) as $id
  | ([$p[0].plans[].canonicalId] | INDEX(.)) as $known
  | ([$g[0].nodes[] | {key: .id, value: .artifact}] | from_entries) as $owner
  | ( [ $g[0].edges[]
        | select(.type == "required" and (.resolution == "internal" or .resolution == "nested"))
        | { from: $owner[.from], to: $owner[.to] }
        | select(.from != null and .to != null and .from != .to) ]
      | group_by(.from)
      | map({ key: .[0].from, value: ([.[].to] | map($id[.]) | unique) })
      | from_entries ) as $expected
  | $p[0].plans[]
  | . as $plan
  | [ ($plan.dependencies.entries[] | select($known[.] == null)
       | "\($plan.canonicalId) depends on \(.), which is not an entry"),
      ($plan.dependencies.entries[] | select(. == $plan.canonicalId)
       | "\($plan.canonicalId) depends on itself"),
      (if ($plan.dependencies.entries | sort) != (($expected[$plan.artifact] // []) | sort)
       then "\($plan.canonicalId) depends on [\($plan.dependencies.entries | join(" "))]; the graph gives [\((($expected[$plan.artifact] // []) | sort | join(" ")))]"
       else empty end)
    ] | .[]')

# The rubric, rechecked against the sibling files a component claims to come
# from, and the two scores rechecked as arithmetic over the components.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r -n --slurpfile p "$records" --slurpfile g "$inventory/graph.json" \
  --slurpfile s "$inventory/surfaces.json" --slurpfile r "$inventory/rights.json" '
  def band($v; $a; $b; $c; $d): if $v < $a then 1 elif $v < $b then 2 elif $v < $c then 3 elif $v < $d then 4 else 5 end;
  # Which lane owns the port work, restated here rather than taken from the
  # model. Round 5 went looking for the next rule a producer and its checker
  # share and this was it: plans-model.sh decides the owner from five thresholds,
  # this file checked only that the word it produced was in the vocabulary and
  # that the owners summed to 235, and nothing anywhere said which artifact
  # belongs in which lane. So the rule is written twice now, in two languages,
  # over the sibling records rather than over the scratch file the model wrote.
  # First match wins, as it does there.
  def lane($a; $dep):
    ($a.integration.mixinConfigs + $a.integration.accessTransformers + $a.integration.serviceProviders) as $integration
    | ($a.data.recipes + $a.data.lootTables + $a.data.tags + $a.data.advancements
       + $a.data.worldgen + $a.data.functions) as $data
    | ($a.resources.textures + $a.resources.models + $a.resources.sounds + $a.resources.lang) as $assets
    | if $a.class == "shaderpack" then "client-presentation"
      elif $integration >= 3 or $a.indicative.classFiles >= 2000 then "integration-runtime"
      elif $dep >= 3 or $a.side.layout == "code-only" then "sdk-platform"
      elif $a.side.layout == "server-leaning" or $data > $assets then "content-data"
      elif $a.side.layout == "client-leaning" or $assets > 0 then "client-presentation"
      else "content-data" end;
  ([$g[0].nodes[] | {key: .id, value: .artifact}] | from_entries) as $owner
  | ( [ $g[0].edges[]
        | select(.type == "required" and (.resolution == "internal" or .resolution == "nested"))
        | { from: $owner[.from], to: $owner[.to] }
        | select(.from != null and .to != null and .from != .to) ] ) as $links
  | ( $links | group_by(.to) | map({ key: .[0].to, value: ([.[].from] | unique | length) }) | from_entries ) as $dependents
  | ([$s[0].artifacts[] | {key: .artifact, value: .indicative.classFiles}] | from_entries) as $classes
  | ([$s[0].artifacts[] | {key: .artifact, value: .}] | from_entries) as $surface
  | ([$r[0].artifacts[] | {key: .artifact, value: .}] | from_entries) as $rights
  | $p[0].plans[]
  | . as $plan
  | ($plan.rubric) as $rb
  | ($rights[$plan.artifact]) as $right
  | ( if $right == null then null
      else ( if $right.sourceUse == "permitted" then 1
             elif $right.sourceUse == "conditional" then 3
             elif $right.sourceUse == "unresolved" then 4 else 5 end
             + (if ([$right.rights[] | select(.surface == "textures" or .surface == "models" or .surface == "sounds") | .status]
                    | any(. == "prohibited")) then 1 else 0 end)
             | if . > 5 then 5 else . end )
      end ) as $expected_rights
  | [ ($rb | to_entries[] | select((.value | type) != "number" or .value < 1 or .value > 5 or (.value | floor) != .value)
       | "\($plan.canonicalId) scores \(.key) as \(.value), outside the published 1-5 scale"),
      (if ($plan.riskScore | type) != "number" or $plan.riskScore < 1 or $plan.riskScore > 5 or ($plan.riskScore | floor) != $plan.riskScore
       then "\($plan.canonicalId) records a riskScore of \($plan.riskScore), outside the published 1-5 scale" else empty end),
      (if ($plan.rank | type) != "number" or $plan.rank < 7 or $plan.rank > 35 or ($plan.rank | floor) != $plan.rank
       then "\($plan.canonicalId) records a rank of \($plan.rank), outside the published 7-35 range" else empty end),
      (if $plan.riskScore != ((($rb.integrationBreadth + $rb.compatibilityUncertainty + $rb.rightsRisk + $rb.expectedMemoryImpact) + 3) / 4 | floor)
       then "\($plan.canonicalId) records riskScore \($plan.riskScore); its risk components round up to \(((($rb.integrationBreadth + $rb.compatibilityUncertainty + $rb.rightsRisk + $rb.expectedMemoryImpact) + 3) / 4 | floor))"
       else empty end),
      (if $plan.rank != ([$rb[]] | add)
       then "\($plan.canonicalId) records rank \($plan.rank); its components sum to \([$rb[]] | add)" else empty end),
      (if $expected_rights != null and $rb.rightsRisk != $expected_rights
       then "\($plan.canonicalId) scores rightsRisk \($rb.rightsRisk); the triage gives \($expected_rights)" else empty end),
      (if $rb.expectedMemoryImpact != band($classes[$plan.artifact]; 100; 500; 2000; 8000)
       then "\($plan.canonicalId) scores expectedMemoryImpact \($rb.expectedMemoryImpact); \($classes[$plan.artifact]) class files give \(band($classes[$plan.artifact]; 100; 500; 2000; 8000))" else empty end),
      (if $rb.foundationValue != band(($dependents[$plan.artifact] // 0); 1; 2; 5; 15)
       then "\($plan.canonicalId) scores foundationValue \($rb.foundationValue); \(($dependents[$plan.artifact] // 0)) dependents give \(band(($dependents[$plan.artifact] // 0); 1; 2; 5; 15))" else empty end),
      (if $surface[$plan.artifact] != null
          and $plan.owner != lane($surface[$plan.artifact]; ($dependents[$plan.artifact] // 0))
       then "\($plan.canonicalId) gives the work to \($plan.owner); its shape puts it in \(lane($surface[$plan.artifact]; ($dependents[$plan.artifact] // 0)))" else empty end)
    ] | .[]')

# A plan record carries identifiers and vocabulary terms and nothing else. This
# is a boundary rule as much as a schema one: a free prose field is where a
# description of what a mod does would arrive, and nothing here has read that.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "plan record carries a string that is neither an identifier nor a published term: $bad"
done < <(jq -r -n --slurpfile p "$records" --slurpfile v "$vocabulary" '
  ( [$p[0].plans[].artifact] + [$p[0].plans[].canonicalId]
    + ($v[0].owners | keys) + ($v[0].methods | keys) + ($v[0].testKinds | keys)
    + ["permitted", "conditional", "unresolved", "prohibited"] | INDEX(.) ) as $allowed
  | [$p[0].plans[] | .. | strings] | unique | .[] | select($allowed[.] == null)')

# Totals are arithmetic over the plans.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  . as $r
  | [ if $r.totals.plans != ($r.plans | length) then "totals.plans disagrees with the records" else empty end,
      if $r.totals.canonicalIds != ([$r.plans[].canonicalId] | unique | length)
        then "totals.canonicalIds disagrees with the records" else empty end,
      if $r.totals.methodsReadingSource != ([$r.plans[] | select(.method.readsSource)] | length)
        then "totals.methodsReadingSource disagrees with the records" else empty end,
      if ($r.totals.owners | to_entries | map(.value) | add) != ($r.plans | length)
        then "totals.owners does not cover every plan" else empty end,
      if ($r.totals.methods | to_entries | map(.value) | add) != ($r.plans | length)
        then "totals.methods does not cover every plan" else empty end,
      if ($r.totals.riskScore | to_entries | map(.value) | add) != ($r.plans | length)
        then "totals.riskScore does not cover every plan" else empty end,
      if ($r.totals.sourceUse | to_entries | map(.value) | add) != ($r.plans | length)
        then "totals.sourceUse does not cover every plan" else empty end,
      if $r.totals.dependencyEdges != ([$r.plans[] | .dependencies.entries | length] | add)
        then "totals.dependencyEdges disagrees with the records" else empty end,
      if $r.derivation.handAssignedFields != 0
        then "the plans declare \($r.derivation.handAssignedFields) hand-assigned fields; every field is derived" else empty end
    ] | .[]' "$records")

# The model, compared field by field.
jq -S '.plans' "$records" >"$meta_work/committed-plans.json"
jq -S -R -s 'split("\n") | map(select(length > 0)) | map(split("\t") | {
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
  }) | sort_by(.canonicalId)' "$plans_tsv" >"$meta_work/recomputed-plans.json"
if ! diff -q "$meta_work/committed-plans.json" "$meta_work/recomputed-plans.json" >/dev/null; then
  note "the plans do not match the plans recomputed from the inventory"
fi

# The sentence in the README that says how many lanes there are.
#
# Round 6's review found it saying five where there are four, and drew the lesson
# rather than just the correction: rounds 2 through 5 each found a false tracked
# sentence, and they kept finding them because nothing in this gate counts a word
# written in the README. This does not fix that in general — most of what the
# README says is not a figure, and a figure it does not repeat cannot be checked.
# It fixes it here, in the file that already computes the number, which is the
# only place the check could live without becoming a second table of prose.
#
# The pattern is required to match. Deleting the sentence fails, so the way out
# of a wrong figure is a right one.
readme="$registry_root/README.md"
lanes="$(jq -r '.owners | keys | length' "$vocabulary")"
planned="$(jq -r '.totals.plans' "$records")"
words=(zero one two three four five six seven eight nine ten)
claim="$(LC_ALL=C grep -oE 'the [a-z]+ lanes summed to [0-9]+' "$readme" | head -1)"
if [ -z "$claim" ]; then
  note "the README no longer says how many owner lanes there are, and that sentence is what binds the figure to this check"
elif [ "$claim" != "the ${words[$lanes]} lanes summed to $planned" ]; then
  note "the README says \"$claim\" and there are $lanes owner lanes over $planned planned entries"
fi

if [ ${#violations[@]} -ne 0 ]; then
  printf 'plan verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

printf 'plans reconcile: %s/%s entries planned, %s canonical ids, %s reading source, %s dependency edges closed\n' \
  "$(jq '.totals.plans' "$records")" "$(wc -l <"$artifact_tsv")" \
  "$(jq '.totals.canonicalIds' "$records")" "$(jq '.totals.methodsReadingSource' "$records")" \
  "$(jq '.totals.dependencyEdges' "$records")"
