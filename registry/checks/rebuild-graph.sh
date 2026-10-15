#!/usr/bin/env bash
# Builds the pack's dependency and integration graph from declared metadata.
#
# This is the M5-W3 producer. Nodes are the mod ids the artifacts declare, plus
# one node for every artifact that declares nothing, because an artifact with no
# declaration is a hole in the graph and a hole has to be visible. Edges are the
# dependencies the mods declare on each other. Clusters are the connected
# components of the required-dependency subgraph: a cluster is the smallest set
# of entries that has to be ported together for any of them to load.
#
# Nothing is inferred from code. An edge exists here only because a mod's own
# metadata states it.
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

graph_model >"$meta_work/model"
# shellcheck source=/dev/null
. "$meta_work/model"

records="$registry_root/inventory/${FIXTURE_ID}/graph.json"

jq --sort-keys -n \
  --arg fixture_id "$FIXTURE_ID" \
  --argjson reads "$(derivation_reads "${BASH_SOURCE[0]}")" \
  --arg artifact_set_sha "$artifact_set_sha" \
  --argjson artifact_count "$(wc -l <"$artifact_tsv")" \
  --argjson reference_count "$(jq '.files | length' "$pack_manifest")" \
  --slurpfile nodes <(jq -R 'split("\t") | {
      id: .[0], kind: .[1], artifact: .[2], version: .[3], displayName: .[4],
      license: .[5], loader: .[6], classification: .[7]
    }' "$graph_nodes") \
  --slurpfile edges <(jq -R 'split("\t") | {
      from: .[0], to: .[1], type: .[2], versionRange: .[3], side: .[4],
      resolution: .[5], classification: .[6]
    }' "$graph_edges") \
  --slurpfile clusters <(jq -R 'split("\t") | {
      id: .[0], size: (.[1] | tonumber), members: (.[2] | split(",") | sort)
    }' "$graph_clusters") '
{
  schemaVersion: 1,
  inventoryKind: "dependency-graph",
  fixture: {
    id: $fixture_id,
    artifactSet: { count: $artifact_count, sha256: $artifact_set_sha },
    pinnedReferences: $reference_count
  },
  derivation: {
    reads: $reads,
    decompiled: false,
    restrictedContentRead: false,
    producedBy: "checks/rebuild-graph.sh"
  },
  totals: {
    nodes: ($nodes | length),
    blockedNodes: ($nodes | map(select(.classification != "resolved")) | length),
    edges: ($edges | length),
    blockedEdges: ($edges | map(select(.classification != "resolved")) | length),
    clusters: ($clusters | length),
    largestCluster: ($clusters | map(.size) | max)
  },
  nodes: ($nodes | sort_by(.id)),
  edges: ($edges | sort_by(.from, .to, .type)),
  clusters: ($clusters | sort_by(.id))
}' >"$records.new"

mv "$records.new" "$records"
jq -r '"graph rebuilt: \(.totals.nodes) nodes, \(.totals.edges) edges, \(.totals.clusters) clusters"' "$records"
