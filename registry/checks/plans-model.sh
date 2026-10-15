#!/usr/bin/env bash
# Turns the graph, the behaviour catalogue and the rights triage into a port plan.
#
# The mechanical half of M5-W6, shared by the producer and the consumer for the
# same reason graph-model.sh and rights-model.sh are shared. Nothing here reads
# an artifact: every input is already a derived model, so a port plan is a
# statement about W3, W4 and W5 and can be no more intrusive than they were.
#
# Two properties of this file are the package, and both are structural rather
# than promised.
#
# The canonical id names a slot in Ravel's own inventory, never a thing upstream
# owns. It is `RVL-` and the zero-padded allocation number of the pinned
# reference the artifact is bound to, so it is a position in the frozen pin list
# and carries no upstream name, project id, file id or filename. An entry that is
# renamed upstream, or re-sourced from a different distributor, changes what
# fills the slot and not which slot it is; that is the whole reason the id is
# opaque. A readable id would have had to encode a name upstream can change.
#
# The method is chosen from the entry's own W5 sourceUse verdict and from nothing
# else. Only `permitted` admits a method that reads source. Every other verdict —
# conditional, unresolved, prohibited — gets black-box respecification, which is
# the method M5-W5 already found sufficient, because behaviour is cleared for all
# 235 and a licence that forbids source use narrows the method rather than
# blocking the work.
#
# Every rubric component below is computed by rule from a W3/W4/W5 field. No
# component is hand-assigned. What is judgement is the choice of proxy and the
# threshold between one band and the next, and each scale records that in its own
# words rather than leaving a reader to infer it.
#
# Calling plans_model writes one tab-separated file and prints the assignment
# that names it.
set -euo pipefail

plans_model() {
  local out="${meta_work:?plans_model requires mod-metadata.sh}"
  local ordinals="$out/ordinals.tsv"

  # The allocation order: the position of each pinned reference in the frozen
  # manifest. M5-W2 recorded the same ordinal against the same reference.
  jq -r '.files | to_entries[] | "\(.value.fileID)\t\(.key)"' "$pack_manifest" >"$ordinals"
  if [ ! -s "$ordinals" ]; then
    echo "pinned manifest yields no reference ordinals" >&2
    return 1
  fi

  awk -F'\t' -v plans_out="$out/plans.tsv" '
    function bucket(v, a, b, c, d) {
      if (v < a) return 1
      if (v < b) return 2
      if (v < c) return 3
      if (v < d) return 4
      return 5
    }
    function cap(v) { return (v > 5) ? 5 : v }
    function isort(arr, n,   i, j, key) {
      for (i = 2; i <= n; i++) {
        key = arr[i]
        for (j = i - 1; j >= 1 && arr[j] > key; j--) arr[j + 1] = arr[j]
        arr[j + 1] = key
      }
    }
    function idof(f) { return sprintf("RVL-%03d", ordinal[f] + 1) }
    function addtest(f, name) { tests[f] = (f in tests) ? tests[f] "," name : name }

    FILENAME == ordinals_file { ord_by_file[$1] = $2; next }

    FILENAME == artifacts_file {
      klass[$2] = $1; project[$2] = $4; fileid[$2] = $5
      ordinal[$2] = ord_by_file[$5]
      next
    }

    # Which artifact owns each graph node. A nested library is owned by the jar
    # that ships it, so a dependency on it is a dependency on that jar.
    FILENAME == nodes_file { owner[$1] = $3; node_class[$1] = $8; next }

    FILENAME == clusters_file {
      n = split($3, members, ",")
      for (i = 1; i <= n; i++) csize[members[i]] = $2 + 0
      next
    }

    FILENAME == edges_file {
      from = $1; to = $2; type = $3; resolution = $6
      a = owner[from]
      if (a == "") next
      if (type == "required" && (resolution == "internal" || resolution == "nested")) {
        b = owner[to]
        if (b != "" && b != a) {
          if (!((a SUBSEP b) in seen_dep)) { seen_dep[a SUBSEP b] = 1
            deps[a] = (a in deps) ? deps[a] "," b : b }
          if (!((b SUBSEP a) in seen_dependent)) { seen_dependent[b SUBSEP a] = 1
            dependents[b]++ }
          if (!((a SUBSEP b) in seen_degree)) { seen_degree[a SUBSEP b] = 1; degree[a]++ }
          if (!((b SUBSEP a) in seen_degree)) { seen_degree[b SUBSEP a] = 1; degree[b]++ }
        }
      }
      if (type == "required" && resolution == "platform") platform_deps[a]++
      if (type == "required" && resolution == "absent") absent_deps[a]++
      if (type != "required" && resolution == "absent") outside_deps[a]++
      next
    }

    FILENAME == surfaces_file {
      f = $1
      assetns[f] = $6 + 0; lang[f] = $7 + 0; textures[f] = $8 + 0; models[f] = $9 + 0
      sounds[f] = $10 + 0; shaders[f] = $11 + 0; datans[f] = $12 + 0; recipes[f] = $13 + 0
      loot[f] = $14 + 0; tags[f] = $15 + 0; advancements[f] = $16 + 0; worldgen[f] = $17 + 0
      functions[f] = $18 + 0; mixins[f] = $19 + 0; transformers[f] = $20 + 0
      services[f] = $21 + 0; nested[f] = $22 + 0; net[f] = $23 + 0; persist[f] = $24 + 0
      classes[f] = $25 + 0; layout[f] = $26
      next
    }

    FILENAME == rights_file {
      f = $1
      tex_status[f] = $11; model_status[f] = $12; sound_status[f] = $13
      source_use[f] = $17
      next
    }

    END {
      for (f in klass) {
        if (!(fileid[f] in ord_by_file)) {
          printf "artifact is bound to no pinned reference ordinal: %s\n", f >"/dev/stderr"
          exit 1
        }

        # --- owner. The lane that owns the port work, from the shape of the
        # artifact rather than from what it is for. First match wins.
        integration = mixins[f] + transformers[f] + services[f]
        data_weight = recipes[f] + loot[f] + tags[f] + advancements[f] + worldgen[f] + functions[f]
        asset_weight = textures[f] + models[f] + sounds[f] + lang[f]
        if (klass[f] == "shaderpack") lane = "client-presentation"
        else if (integration >= 3 || classes[f] >= 2000) lane = "integration-runtime"
        else if (dependents[f] >= 3 || layout[f] == "code-only") lane = "sdk-platform"
        else if (layout[f] == "server-leaning" || data_weight > asset_weight) lane = "content-data"
        else if (layout[f] == "client-leaning" || asset_weight > 0) lane = "client-presentation"
        else lane = "content-data"

        # --- method. Decided by the W5 verdict and by nothing else.
        if (source_use[f] == "permitted") { method = "source-informed-reimplementation"; reads = "true" }
        else { method = "black-box-respecification"; reads = "false" }

        # --- dependencies, as canonical ids.
        dep_ids = "-"
        if (f in deps) {
          n = split(deps[f], d, ",")
          for (i = 1; i <= n; i++) sorted[i] = idof(d[i])
          isort(sorted, n)
          dep_ids = sorted[1]
          for (i = 2; i <= n; i++) dep_ids = dep_ids "," sorted[i]
          delete sorted
        }

        # --- tests. One kind per surface the catalogue counted, in a fixed order.
        delete tests[f]
        addtest(f, "registry-parity")
        addtest(f, "side-matrix")
        if (recipes[f] > 0) addtest(f, "recipe-parity")
        if (loot[f] > 0) addtest(f, "loot-parity")
        if (tags[f] > 0) addtest(f, "tag-parity")
        if (advancements[f] > 0) addtest(f, "advancement-parity")
        if (worldgen[f] > 0) addtest(f, "worldgen-parity")
        if (functions[f] > 0) addtest(f, "function-parity")
        if (lang[f] > 0) addtest(f, "language-parity")
        if (textures[f] + models[f] > 0) addtest(f, "asset-pipeline")
        if (sounds[f] > 0) addtest(f, "sound-parity")
        if (shaders[f] > 0) addtest(f, "shader-pipeline")
        if (net[f] > 0) addtest(f, "networking-probe")
        if (persist[f] > 0) addtest(f, "saved-data-round-trip")
        if (mixins[f] + transformers[f] > 0) addtest(f, "mixin-integration")
        if (services[f] > 0) addtest(f, "service-discovery")
        if (nested[f] > 0) addtest(f, "nested-library-provision")
        if (cluster_of(f) >= 2) addtest(f, "cluster-integration")
        if (source_use[f] != "permitted") addtest(f, "rights-method-audit")

        # --- rubric.
        foundation = bucket(dependents[f] + 0, 1, 2, 5, 15)
        centrality = bucket(degree[f] + 0, 1, 3, 7, 16)

        families = 0
        if (lang[f] > 0) families++
        if (textures[f] > 0) families++
        if (models[f] > 0) families++
        if (sounds[f] > 0) families++
        if (shaders[f] > 0) families++
        if (recipes[f] > 0) families++
        if (loot[f] > 0) families++
        if (tags[f] > 0) families++
        if (advancements[f] > 0) families++
        if (worldgen[f] > 0) families++
        if (functions[f] > 0) families++
        if (mixins[f] > 0) families++
        if (transformers[f] > 0) families++
        if (services[f] > 0) families++
        if (nested[f] > 0) families++
        if (classes[f] > 0) families++
        if (net[f] > 0) families++
        if (persist[f] > 0) families++
        breadth = bucket(families, 2, 4, 7, 10)

        uncertainty = 1
        if (blocked_of(f)) uncertainty++
        if (net[f] > 0 || persist[f] > 0) uncertainty++
        if (mixins[f] + transformers[f] > 0) uncertainty++
        if (absent_deps[f] > 0 || outside_deps[f] > 0) uncertainty++
        uncertainty = cap(uncertainty)

        if (source_use[f] == "permitted") rights_risk = 1
        else if (source_use[f] == "conditional") rights_risk = 3
        else if (source_use[f] == "unresolved") rights_risk = 4
        else rights_risk = 5
        if (tex_status[f] == "prohibited" || model_status[f] == "prohibited" || sound_status[f] == "prohibited")
          rights_risk++
        rights_risk = cap(rights_risk)

        memory = bucket(classes[f], 100, 500, 2000, 8000)

        slice = 1
        if (layout[f] == "both") slice++
        if (recipes[f] + loot[f] + advancements[f] > 0) slice++
        if (dependents[f] > 0) slice++
        if (cluster_of(f) >= 10) slice++
        slice = cap(slice)

        # riskScore is the risk half only, rounded up; rank orders the wave.
        risk_sum = breadth + uncertainty + rights_risk + memory
        risk = int((risk_sum + 3) / 4)
        rank = foundation + centrality + breadth + uncertainty + rights_risk + memory + slice

        printf "%s\t%s\t%d\t%s\t%s\t%s\t%s", f, idof(f), ordinal[f], lane, method, reads, source_use[f] >plans_out
        printf "\t%s\t%d\t%d\t%d", dep_ids, platform_deps[f] + 0, absent_deps[f] + 0,
          outside_deps[f] + 0 >plans_out
        printf "\t%s", tests[f] >plans_out
        printf "\t%d\t%d\t%d\t%d\t%d\t%d\t%d", foundation, centrality, breadth, uncertainty,
          rights_risk, memory, slice >plans_out
        printf "\t%d\t%d\n", risk, rank >plans_out
      }
    }

    function cluster_of(f,   nid, best) {
      best = 0
      for (nid in owner) if (owner[nid] == f && csize[nid] > best) best = csize[nid]
      return best
    }
    function blocked_of(f,   nid) {
      for (nid in owner) if (owner[nid] == f && node_class[nid] != "resolved") return 1
      return 0
    }
  ' ordinals_file="$ordinals" artifacts_file="$artifact_tsv" nodes_file="$graph_nodes" \
    clusters_file="$graph_clusters" edges_file="$graph_edges" surfaces_file="$surfaces_tsv" \
    rights_file="$rights_tsv" \
    "$ordinals" "$artifact_tsv" "$graph_nodes" "$graph_clusters" "$graph_edges" \
    "$surfaces_tsv" "$rights_tsv"

  if [ ! -s "$out/plans.tsv" ]; then
    echo "plan model produced no records" >&2
    return 1
  fi
  sort -o "$out/plans.tsv" "$out/plans.tsv"
  printf 'plans_tsv=%q\n' "$out/plans.tsv"
}
