#!/usr/bin/env bash
# Holds the committed dependency graph to the artifacts it claims to describe.
#
# The consumer for M5-W3. It recomputes the model from the fixture and requires
# the committed file to be that model, and then it asks the file questions the
# model never answered: that every acquired artifact is somewhere in the graph,
# that the edge count is the number of declared dependencies counted directly,
# that every node and edge carries a verdict, and that the clusters really are
# the connected components — proved by walking each one, not by rerunning the
# union-find that produced them.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
# shellcheck source=checks/mod-metadata.sh
. "$checks_dir/mod-metadata.sh"
# shellcheck source=checks/graph-model.sh
. "$checks_dir/graph-model.sh"

records="$registry_root/inventory/${FIXTURE_ID}/graph.json"
if [ ! -f "$records" ]; then
  echo "graph verification failed:" >&2
  echo "- missing input: inventory/${FIXTURE_ID}/graph.json" >&2
  exit 1
fi

violations=()
note() { violations+=("$1"); }

graph_model >"$meta_work/model"
# shellcheck source=/dev/null
. "$meta_work/model"

# The fixture the file says it describes, recomputed.
if [ "$(jq -r '.fixture.artifactSet.sha256' "$records")" != "$artifact_set_sha" ]; then
  note "graph is bound to a different artifact set than the one on disk"
fi
if [ "$(jq -r '.fixture.artifactSet.count' "$records")" != "$(wc -l <"$artifact_tsv")" ]; then
  note "graph records a different artifact count than the acquired set holds"
fi

# The model, recomputed and compared field by field rather than believed.
jq -S '{nodes, edges, clusters: [.clusters[] | {size, members}] | sort_by(.members)}' "$records" \
  >"$meta_work/committed.json"
jq -Sn \
  --slurpfile nodes <(jq -R 'split("\t") | {
      id: .[0], kind: .[1], artifact: .[2], version: .[3], displayName: .[4],
      license: .[5], loader: .[6], classification: .[7]
    }' "$graph_nodes") \
  --slurpfile edges <(jq -R 'split("\t") | {
      from: .[0], to: .[1], type: .[2], versionRange: .[3], side: .[4],
      resolution: .[5], classification: .[6]
    }' "$graph_edges") \
  --slurpfile clusters <(jq -R 'split("\t") | {
      size: (.[1] | tonumber), members: (.[2] | split(",") | sort)
    }' "$graph_clusters") '{
    nodes: ($nodes | sort_by(.id)),
    edges: ($edges | sort_by(.from, .to, .type)),
    clusters: ($clusters | sort_by(.members))
  }' >"$meta_work/recomputed.json"
if ! diff -q "$meta_work/committed.json" "$meta_work/recomputed.json" >/dev/null; then
  note "graph does not match the model recomputed from the acquired artifacts"
fi

# Coverage. Every acquired file has to be reachable from some node, or the graph
# describes a pack that is not the one on disk.
cut -f2 "$artifact_tsv" | sort -u >"$meta_work/acquired-names"
jq -r '.nodes[].artifact' "$records" | sort -u >"$meta_work/graph-names"
while IFS= read -r missing; do
  [ -n "$missing" ] && note "acquired artifact appears in no node: $missing"
done < <(comm -23 "$meta_work/acquired-names" "$meta_work/graph-names")
while IFS= read -r extra; do
  [ -n "$extra" ] && note "node names an artifact that was not acquired: $extra"
done < <(comm -13 "$meta_work/acquired-names" "$meta_work/graph-names")

# Edge population, counted straight off the declarations rather than off the model.
declared_deps="$(awk -F'\t' '$2 == "DEP" || $2 == "NESTEDDEP"' "$meta_tsv" | wc -l)"
recorded_edges="$(jq '.edges | length' "$records")"
if [ "$declared_deps" -ne "$recorded_edges" ]; then
  note "graph records $recorded_edges edges for $declared_deps declared dependencies"
fi

# Every node and every edge carries a verdict, and a verdict is either the word
# resolved or a blocking classification that says what blocks it.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  [ (.nodes[] | select((.classification == "resolved" or (.classification | startswith("blocked:"))) | not)
      | "node \(.id) carries no verdict: \(.classification)"),
    (.edges[] | select((.classification == "resolved" or (.classification | startswith("blocked:"))) | not)
      | "edge \(.from) -> \(.to) carries no verdict: \(.classification)"),
    (.edges[] | select(.resolution == "absent" and .type == "required" and .classification == "resolved")
      | "edge \(.from) -> \(.to) is a required dependency that is absent yet called resolved")
  ] | .[]' "$records")

# Totals are arithmetic over the file, not a number the file was told.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  . as $g
  | [ if $g.totals.nodes != ($g.nodes | length) then "totals.nodes disagrees with the node list" else empty end,
      if $g.totals.edges != ($g.edges | length) then "totals.edges disagrees with the edge list" else empty end,
      if $g.totals.clusters != ($g.clusters | length) then "totals.clusters disagrees with the cluster list" else empty end,
      if $g.totals.blockedNodes != ([$g.nodes[] | select(.classification != "resolved")] | length)
        then "totals.blockedNodes disagrees with the node list" else empty end,
      if $g.totals.blockedEdges != ([$g.edges[] | select(.classification != "resolved")] | length)
        then "totals.blockedEdges disagrees with the edge list" else empty end,
      if $g.totals.largestCluster != ([$g.clusters[].size] | max)
        then "totals.largestCluster disagrees with the cluster list" else empty end,
      ($g.clusters[] | select(.size != (.members | length))
        | "cluster \(.id) states size \(.size) for \(.members | length) members")
    ] | .[]' "$records")

# The clusters, walked. A partition of the nodes in which no required internal
# edge crosses a boundary and every part is connected is the set of connected
# components, and this establishes all three properties without reusing the
# union-find that produced them.
jq -r '.clusters[] | .id as $c | .members[] | "M\t\($c)\t\(.)"' "$records" >"$meta_work/walk.tsv"
jq -r '.nodes[] | "N\t\(.id)"' "$records" >>"$meta_work/walk.tsv"
jq -r '.edges[] | select(.type == "required" and (.resolution == "internal" or .resolution == "nested"))
  | "E\t\(.from)\t\(.to)"' "$records" >>"$meta_work/walk.tsv"

while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(awk -F'\t' '
  $1 == "N" { node[$2] = 1; nodes++; next }
  $1 == "M" { if ($3 in member) print "node " $3 " is in more than one cluster"
              member[$3] = $2; size[$2]++; members[$2] = ($2 in members) ? members[$2] SUBSEP $3 : $3; next }
  $1 == "E" { adj[$2] = ($2 in adj) ? adj[$2] SUBSEP $3 : $3
              adj[$3] = ($3 in adj) ? adj[$3] SUBSEP $2 : $2
              elist[++en] = $2 SUBSEP $3; next }
  END {
    for (id in node) if (!(id in member)) print "node " id " is in no cluster"
    for (id in member) if (!(id in node)) print "cluster " member[id] " names an unknown node: " id
    for (i = 1; i <= en; i++) {
      split(elist[i], e, SUBSEP)
      if (member[e[1]] != member[e[2]])
        print "required dependency " e[1] " -> " e[2] " crosses a cluster boundary"
    }
    # Breadth-first walk from one member of each cluster.
    for (c in members) {
      split(members[c], m, SUBSEP)
      head = 1; tail = 0
      delete queue; delete visited
      queue[++tail] = m[1]; visited[m[1]] = 1; reached = 1
      while (head <= tail) {
        cur = queue[head++]
        if (!(cur in adj)) continue
        split(adj[cur], nb, SUBSEP)
        for (j in nb) {
          if (nb[j] in visited) continue
          visited[nb[j]] = 1; reached++
          queue[++tail] = nb[j]
        }
      }
      if (reached != size[c])
        print "cluster " c " is not connected: walked " reached " of " size[c] " members"
    }
  }' "$meta_work/walk.tsv")

if [ ${#violations[@]} -ne 0 ]; then
  printf 'graph verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

printf 'graph reconciles: %s nodes, %s edges, %s clusters over %s bound artifacts\n' \
  "$(jq '.nodes | length' "$records")" "$(jq '.edges | length' "$records")" \
  "$(jq '.clusters | length' "$records")" "$(wc -l <"$artifact_tsv")"
