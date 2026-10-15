#!/usr/bin/env bash
# Holds the committed behaviour-surface catalogue to the artifacts it counts.
#
# The consumer for M5-W4. It recomputes the catalogue and requires the committed
# file to be it, then it recounts every counted family — per artifact, and split
# into the part the artifact itself holds and the part its shipped libraries do —
# from a walk of the archives that `checks/surface-recount.sh` makes on its own
# terms, checks that every acquired artifact has exactly one record bound to the
# reference it was acquired for, and closes the catalogue against the graph: a
# record may only name mod ids that M5-W3 admitted as nodes.
#
# The recount used to be three totals taken off the producer's own table of
# contents, which round 4's review correctly called a second reading rather than
# a second opinion: both sides stopped at the outer archive, so both missed
# 12,571 class files and neither could say so. It is now a separate walk with a
# separate descent rule; see the head of surface-recount.sh.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
# shellcheck source=checks/mod-metadata.sh
. "$checks_dir/mod-metadata.sh"
# shellcheck source=checks/surfaces-model.sh
. "$checks_dir/surfaces-model.sh"
# shellcheck source=checks/surface-recount.sh
. "$checks_dir/surface-recount.sh"

records="$registry_root/inventory/${FIXTURE_ID}/surfaces.json"
graph="$registry_root/inventory/${FIXTURE_ID}/graph.json"

violations=()
note() { violations+=("$1"); }

for path in "$records" "$graph"; do
  if [ ! -f "$path" ]; then
    printf 'surface verification failed:\n- missing input: %s\n' "${path#"$registry_root/"}" >&2
    exit 1
  fi
done

surfaces_model >"$meta_work/model"
# shellcheck source=/dev/null
. "$meta_work/model"

if [ "$(jq -r '.fixture.artifactSet.sha256' "$records")" != "$artifact_set_sha" ]; then
  note "catalogue is bound to a different artifact set than the one on disk"
fi

# One record per acquired artifact, bound to the reference it was acquired for.
cut -f2,4,5 "$artifact_tsv" | sort >"$meta_work/acquired-refs"
jq -r '.artifacts[] | "\(.artifact)\t\(.reference.projectId)\t\(.reference.fileId)"' "$records" |
  sort >"$meta_work/record-refs"
while IFS= read -r line; do
  [ -n "$line" ] && note "acquired artifact has no matching record: ${line%%$'\t'*}"
done < <(comm -23 "$meta_work/acquired-refs" "$meta_work/record-refs")
while IFS= read -r line; do
  [ -n "$line" ] && note "record does not match an acquired artifact: ${line%%$'\t'*}"
done < <(comm -13 "$meta_work/acquired-refs" "$meta_work/record-refs")

record_count="$(jq '.artifacts | length' "$records")"
distinct_records="$(jq -r '.artifacts[].artifact' "$records" | sort -u | wc -l)"
if [ "$record_count" -ne "$distinct_records" ]; then
  note "catalogue holds $record_count records for $distinct_records distinct artifacts"
fi

# Every counted family, recounted from a walk this consumer makes itself, per
# artifact and split by depth. This is the check the model cannot satisfy by
# being consistent with itself.
surface_recount >"$meta_work/recount"
# shellcheck source=/dev/null
. "$meta_work/recount"

while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done <"$recount_failures"

jq -r '.artifacts[] | . as $a
  | [ ["lang", $a.resources.lang, $a.nested.lang],
      ["textures", $a.resources.textures, $a.nested.textures],
      ["models", $a.resources.models, $a.nested.models],
      ["sounds", $a.resources.sounds, $a.nested.sounds],
      ["shaders", $a.resources.shaders, $a.nested.shaders],
      ["recipes", $a.data.recipes, $a.nested.recipes],
      ["lootTables", $a.data.lootTables, $a.nested.lootTables],
      ["tags", $a.data.tags, $a.nested.tags],
      ["advancements", $a.data.advancements, $a.nested.advancements],
      ["worldgen", $a.data.worldgen, $a.nested.worldgen],
      ["functions", $a.data.functions, $a.nested.functions],
      ["serviceProviders", $a.integration.serviceProviders, $a.nested.serviceProviders],
      ["classFiles", $a.indicative.classFiles, $a.nested.classFiles] ]
  | .[] | [$a.artifact, .[0], .[1], .[2]] | @tsv' "$records" |
  sort >"$meta_work/record-counts.tsv"

if ! diff -q "$meta_work/record-counts.tsv" "$recount_tsv" >/dev/null; then
  note "the catalogue's per-artifact counts are not what a walk of the archives holds; the rows below are the record's reading and then the walk's, as artifact, family, count at every depth, count inside shipped libraries"
  while IFS= read -r line; do
    note "$line"
  done < <(diff "$meta_work/record-counts.tsv" "$recount_tsv" | grep -E '^[<>]' | head -20)
fi

# mixinConfigs is declared rather than laid out, so it is recounted from the
# declaration stream instead of from a table of contents.
mixin_configs="$(awk -F'\t' '$2 == "MIXIN"' "$meta_tsv" | wc -l)"
if [ "$(jq -r '.totals.mixinConfigs' "$records")" != "$mixin_configs" ]; then
  note "totals.mixinConfigs disagrees with the declarations the readers parsed"
fi

# The pack-wide totals, summed over the recount rather than over the records.
while IFS=$'\t' read -r family expected; do
  actual="$(jq -r --arg f "$family" '.totals[$f] // "absent"' "$records")"
  [ "$actual" = "absent" ] && continue
  if [ "$actual" != "$expected" ]; then
    note "totals.$family is $actual but a walk of the archives holds $expected"
  fi
done < <(awk -F'\t' '{ n[$2] += $3 } END { for (f in n) printf "%s\t%d\n", f, n[f] }' "$recount_tsv")

lang_files="$(awk -F'\t' '$2 == "lang" { n += $3 } END { print n + 0 }' "$recount_tsv")"

# Every other total is arithmetic over the records.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  . as $s
  | [ ["textures", "resources", "textures"], ["models", "resources", "models"],
      ["sounds", "resources", "sounds"], ["shaders", "resources", "shaders"],
      ["recipes", "data", "recipes"], ["lootTables", "data", "lootTables"],
      ["tags", "data", "tags"], ["advancements", "data", "advancements"],
      ["worldgen", "data", "worldgen"], ["serviceProviders", "integration", "serviceProviders"] ]
  | .[]
  | . as [$total, $group, $field]
  | select($s.totals[$total] != ([$s.artifacts[][$group][$field]] | add))
  | "totals.\($total) disagrees with the records"' "$records")

# Every artifact carries a verdict, and a verdict is either catalogued or an
# evidenced reason it is not.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '.artifacts[]
  | select((.classification == "catalogued"
            or (.classification | startswith("dead:"))
            or (.classification | startswith("unreachable:"))) | not)
  | "artifact \(.artifact) carries no verdict: \(.classification)"' "$records")

# A catalogued artifact has to have something to catalogue, and an artifact
# called dead has to have nothing.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '.artifacts[]
  | ([.resources[], .data[], .integration.mixinConfigs, .integration.serviceProviders,
      .indicative.classFiles] | map(select(type == "number")) | add) as $weight
  | if .classification == "catalogued" and $weight == 0
    then "artifact \(.artifact) is called catalogued with no surface at all"
    elif (.classification | startswith("dead:")) and $weight > 0
    then "artifact \(.artifact) is called dead but carries \($weight) surface entries"
    else empty end' "$records")

# Closure against the graph. A surface record may only name a mod the graph
# admitted, or the two files disagree about what the pack contains.
jq -r '.nodes[].id' "$graph" | sort -u >"$meta_work/graph-ids"
jq -r '.artifacts[].mods[]' "$records" | sort -u >"$meta_work/record-ids"
while IFS= read -r stray; do
  [ -n "$stray" ] && note "catalogue names a mod the graph does not hold: $stray"
done < <(comm -13 "$meta_work/graph-ids" "$meta_work/record-ids")

# The pack overlay is counted and not named. This is the check on that promise
# rather than a restatement of it.
overlay_config="$(jq -r '.packOverlay.configFiles' "$records")"
overlay_scripts="$(jq -r '.packOverlay.scriptFiles' "$records")"
if [ "$overlay_config" != "$overlay_config_files" ] || [ "$overlay_scripts" != "$overlay_script_files" ]; then
  note "pack overlay counts do not match the override trees on disk"
fi
if jq -r '.artifacts[] | .packOverlay | keys[]' "$records" | sort -u |
  grep -qvE '^(config|scripts)$'; then
  note "a per-artifact pack overlay record carries more than the two flags"
fi

# The model, compared field by field.
jq -S '.artifacts' "$records" >"$meta_work/committed.json"
jq -S -R -s 'split("\n") | map(select(length > 0)) | map(split("\t") | {
    artifact: .[0],
    class: .[1],
    reference: { projectId: (.[2] | tonumber), fileId: (.[3] | tonumber) },
    mods: (if .[4] == "-" then [] else (.[4] | split(",") | sort) end),
    resources: {
      assetNamespaces: (.[5] | tonumber), lang: (.[6] | tonumber),
      textures: (.[7] | tonumber), models: (.[8] | tonumber),
      sounds: (.[9] | tonumber), shaders: (.[10] | tonumber)
    },
    data: {
      dataNamespaces: (.[11] | tonumber), recipes: (.[12] | tonumber),
      lootTables: (.[13] | tonumber), tags: (.[14] | tonumber),
      advancements: (.[15] | tonumber), worldgen: (.[16] | tonumber),
      functions: (.[17] | tonumber)
    },
    integration: {
      mixinConfigs: (.[18] | tonumber), accessTransformers: (.[19] | tonumber),
      serviceProviders: (.[20] | tonumber), nestedJars: (.[21] | tonumber)
    },
    indicative: {
      method: "class-path-heuristic",
      networkingClasses: (.[22] | tonumber),
      persistenceClasses: (.[23] | tonumber),
      classFiles: (.[24] | tonumber)
    },
    side: {
      method: "layout-heuristic",
      layout: .[25],
      declaredDependencySides: (if .[26] == "-" then [] else (.[26] | split(",") | sort) end)
    },
    nested: {
      attribution: "counted into the groups above and shown here as well",
      lang: (.[30] | tonumber), textures: (.[31] | tonumber),
      models: (.[32] | tonumber), sounds: (.[33] | tonumber),
      shaders: (.[34] | tonumber), recipes: (.[35] | tonumber),
      lootTables: (.[36] | tonumber), tags: (.[37] | tonumber),
      advancements: (.[38] | tonumber), worldgen: (.[39] | tonumber),
      functions: (.[40] | tonumber), serviceProviders: (.[41] | tonumber),
      classFiles: (.[42] | tonumber)
    },
    packOverlay: { config: (.[27] == "true"), scripts: (.[28] == "true") },
    classification: .[29]
  }) | sort_by(.artifact)' "$surfaces_tsv" >"$meta_work/recomputed.json"
if ! diff -q "$meta_work/committed.json" "$meta_work/recomputed.json" >/dev/null; then
  note "catalogue does not match the surfaces recomputed from the acquired artifacts"
fi

if [ ${#violations[@]} -ne 0 ]; then
  printf 'surface verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

printf 'surfaces reconcile: %s/%s acquired artifacts catalogued, %s recipes and %s language files recounted\n' \
  "$(jq '.totals.catalogued' "$records")" "$(wc -l <"$artifact_tsv")" \
  "$(jq '.totals.recipes' "$records")" "$lang_files"
