#!/usr/bin/env bash
# Catalogues each acquired artifact's behaviour surfaces.
#
# The M5-W4 producer. One record per acquired artifact, so the 235 pinned
# references and the 235 records line up one to one, and the record says what a
# counterpart would have to reproduce: how much translated text, how many models
# and textures, how many recipes, loot tables, tags, advancements and worldgen
# files, how many mixin configurations and service providers, and which side the
# layout leans to.
#
# Three of the columns are estimates and are labelled where they appear. Packets
# and saved data are registered in code, which this workspace does not read, so
# what is recorded is how many classes sit on a path that names them; the side is
# read off the layout for the same reason. They are inputs to a port plan, not
# findings.
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

surfaces_model >"$meta_work/model"
# shellcheck source=/dev/null
. "$meta_work/model"

records="$registry_root/inventory/${FIXTURE_ID}/surfaces.json"

jq --sort-keys -n \
  --arg fixture_id "$FIXTURE_ID" \
  --argjson reads "$(derivation_reads "${BASH_SOURCE[0]}")" \
  --arg artifact_set_sha "$artifact_set_sha" \
  --argjson artifact_count "$(wc -l <"$artifact_tsv")" \
  --argjson overlay_config "$overlay_config_files" \
  --argjson overlay_scripts "$overlay_script_files" \
  --slurpfile artifacts <(jq -R 'split("\t") | {
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
    }' "$surfaces_tsv") '
{
  schemaVersion: 1,
  inventoryKind: "behaviour-surfaces",
  fixture: {
    id: $fixture_id,
    artifactSet: { count: $artifact_count, sha256: $artifact_set_sha }
  },
  derivation: {
    reads: $reads,
    decompiled: false,
    restrictedContentRead: false,
    estimates: {
      "indicative.networkingClasses": "classes on a network-shaped package path; packets are registered in code, which is not read",
      "indicative.persistenceClasses": "classes on a saved-data-shaped path; the same limit applies",
      "side.layout": "read off whether the archive carries assets, data, or both; no mod declares a side"
    },
    producedBy: "checks/rebuild-surfaces.sh"
  },
  packOverlay: {
    owner: "ATMTeam",
    rights: "prohibited",
    configFiles: $overlay_config,
    scriptFiles: $overlay_scripts,
    modsWithConfigOverride: ($artifacts | map(select(.packOverlay.config)) | length),
    modsWithScriptOverride: ($artifacts | map(select(.packOverlay.scripts)) | length),
    note: "Counted, never transcribed. The pack overrides are ATM-owned, so no name from either tree is recorded here; the only thing that travels is a flag on a mod id the mod itself declared. Every capability these files provide has to be respecified from observed behaviour rather than ported."
  },
  totals: {
    artifacts: ($artifacts | length),
    catalogued: ($artifacts | map(select(.classification == "catalogued")) | length),
    unclassified: ($artifacts | map(select(.classification == "catalogued" or (.classification | startswith("dead:")) or (.classification | startswith("unreachable:")) | not)) | length),
    lang: ($artifacts | map(.resources.lang) | add),
    textures: ($artifacts | map(.resources.textures) | add),
    models: ($artifacts | map(.resources.models) | add),
    sounds: ($artifacts | map(.resources.sounds) | add),
    shaders: ($artifacts | map(.resources.shaders) | add),
    recipes: ($artifacts | map(.data.recipes) | add),
    lootTables: ($artifacts | map(.data.lootTables) | add),
    tags: ($artifacts | map(.data.tags) | add),
    advancements: ($artifacts | map(.data.advancements) | add),
    worldgen: ($artifacts | map(.data.worldgen) | add),
    mixinConfigs: ($artifacts | map(.integration.mixinConfigs) | add),
    serviceProviders: ($artifacts | map(.integration.serviceProviders) | add),
    classFiles: ($artifacts | map(.indicative.classFiles) | add)
  },
  artifacts: ($artifacts | sort_by(.artifact))
}' >"$records.new"

mv "$records.new" "$records"
jq -r '"surfaces rebuilt: \(.totals.artifacts) artifacts, \(.totals.recipes) recipes, \(.totals.lang) language files"' "$records"
