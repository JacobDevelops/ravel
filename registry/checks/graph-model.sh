#!/usr/bin/env bash
# Turns declared metadata into nodes, edges and clusters.
#
# This is the mechanical half of M5-W3 and both the producer and the consumer
# run it, because a second hand-written copy of the same loop would be duplicated
# code rather than an independent opinion. What the consumer does not share is
# the checking: it holds this model to graph properties this file never computes
# — that the clusters partition the nodes, that no required internal edge crosses
# a cluster boundary, and that each cluster is connected by its own breadth-first
# walk rather than by the union-find below.
#
# Calling graph_model writes three tab-separated files and prints the shell
# assignments that name them.
set -euo pipefail

graph_model() {
  local out="${meta_work:?graph_model requires mod-metadata.sh}"

  awk -F'\t' -v nodes_out="$out/nodes.tsv" -v edges_out="$out/edges.tsv" \
    -v clusters_out="$out/clusters.tsv" '
    function root(x,   r, p) {
      r = x
      while (parent[r] != r) r = parent[r]
      while (parent[x] != r) { p = parent[x]; parent[x] = r; x = p }
      return r
    }
    function join(a, b,   ra, rb) {
      ra = root(a); rb = root(b)
      if (ra != rb) parent[ra] = rb
    }
    function addnode(id, kind, artifact, version, display, license, loader) {
      if (id in node_kind) return 0
      node_kind[id] = kind; node_artifact[id] = artifact
      node_version[id] = version; node_display[id] = display
      node_license[id] = license; node_loader[id] = loader
      parent[id] = id
      order[++node_count] = id
      return 1
    }

    # Pass one: the artifacts, so every acquired file is accounted for whether or
    # not it declares anything.
    FILENAME == artifacts {
      artifact_class[$2] = $1
      next
    }

    # Pass two: the declarations.
    $2 == "LICENSE" { license[$1] = $3; next }
    $2 == "LOADER"  { loader[$1] = $3; next }
    $2 == "MOD"     { modrec[++modn] = $1 "\t" $3 "\t" $4 "\t" $5; next }
    $2 == "NESTED"  { if ($4 != "") nestrec[++nestn] = $1 "\t" $4; next }
    $2 == "NOMETA"  { nometa[$1] = 1; next }
    # A mixin configuration is a behaviour surface, counted by surfaces-model.sh.
    # It says nothing about which mods depend on which, so it is answered here
    # and ignored on purpose rather than by omission.
    $2 == "MIXIN"   { next }
    # A jar-in-jar library declares dependencies like any other mod and loads
    # with the artifact that ships it, so its declarations are edges on the same
    # terms. Reading them and keeping only the identity of the library is the
    # defect round 2 of the review found.
    $2 == "DEP" || $2 == "NESTEDDEP" { deprec[++depn] = $1 "\t" $3 "\t" $4 "\t" $5 "\t" $6 "\t" $7; next }

    # Every record kind this model knows is answered above. One it does not know
    # is a reader that grew a row nothing consumes, which is how a declaration
    # goes missing quietly; it stops the model instead.
    { unknown[$2] = 1 }

    END {
      for (k in unknown) {
        printf "graph model has no rule for declared-metadata record kind: %s\n", k >"/dev/stderr"
        unhandled = 1
      }
      if (unhandled) exit 1

      platform["minecraft"] = 1; platform["neoforge"] = 1
      platform["forge"] = 1; platform["fml"] = 1; platform["java"] = 1

      for (i = 1; i <= modn; i++) {
        split(modrec[i], m, "\t")
        addnode(m[2], "mod", m[1], m[3], m[4], license[m[1]], loader[m[1]])
        provided[m[2]] = "internal"
      }
      # A nested library provides its mod id to the whole pack. When the pack
      # also ships that mod id as a mod of its own, the top-level one is the
      # node and the nested copy is redundant rather than a second node.
      for (i = 1; i <= nestn; i++) {
        split(nestrec[i], n, "\t")
        if (addnode(n[2], "nested-mod", n[1], "", "", license[n[1]], loader[n[1]]))
          provided[n[2]] = "nested"
      }
      for (f in artifact_class) {
        if (artifact_class[f] == "shaderpack")
          addnode("shaderpack:" f, "shaderpack", f, "", "", "", "")
        else if (artifact_class[f] == "resourcepack")
          addnode("resourcepack:" f, "resourcepack", f, "", "", "", "")
        else if (f in nometa)
          addnode("artifact:" f, "opaque-artifact", f, "", "", license[f], loader[f])
      }

      for (i = 1; i <= depn; i++) {
        split(deprec[i], d, "\t")
        from = d[2]; to = d[3]
        type = (d[4] == "" ? "required" : tolower(d[4]))
        range = d[5]
        side = (d[6] == "" ? "BOTH" : toupper(d[6]))

        if (!(from in node_kind)) { resolution = "orphan-owner" }
        else if (to in platform) { resolution = "platform" }
        else if (provided[to] == "internal") { resolution = "internal" }
        else if (provided[to] == "nested") { resolution = "nested" }
        else { resolution = "absent" }

        # An absent required dependency is a hole in the pack as declared. An
        # absent optional one is the ordinary case, and an absent incompatible
        # or discouraged one is the constraint being satisfied.
        if (resolution == "orphan-owner")
          class = "blocked:dependency-owner-is-not-a-declared-mod"
        else if (resolution == "absent" && type == "required")
          class = "blocked:required-dependency-absent-from-pack"
        else
          class = "resolved"

        edges[++edgen] = from "\t" to "\t" type "\t" range "\t" side "\t" resolution "\t" class
        if (class != "resolved") blocked_from[from] = 1
        if (type == "required" && (resolution == "internal" || resolution == "nested"))
          join(from, to)
      }

      for (i = 1; i <= node_count; i++) {
        id = order[i]
        if (node_kind[id] == "shaderpack" || node_kind[id] == "opaque-artifact")
          class = "blocked:no-declared-dependency-metadata"
        else if (id in blocked_from)
          class = "blocked:required-dependency-absent-from-pack"
        else
          class = "resolved"
        printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n", id, node_kind[id], node_artifact[id],
          node_version[id], node_display[id], node_license[id], node_loader[id], class >nodes_out
      }
      for (i = 1; i <= edgen; i++) print edges[i] >edges_out

      for (i = 1; i <= node_count; i++) {
        id = order[i]
        r = root(id)
        members[r] = (r in members) ? members[r] "," id : id
        size[r]++
      }
      cl = 0
      for (i = 1; i <= node_count; i++) {
        id = order[i]
        if (root(id) != id) continue
        printf "cluster-%03d\t%d\t%s\n", cl++, size[id], members[id] >clusters_out
      }
    }
  ' artifacts="$artifact_tsv" "$artifact_tsv" "$meta_tsv"

  for name in nodes edges clusters; do
    if [ ! -s "$out/$name.tsv" ]; then
      echo "graph model produced no $name" >&2
      return 1
    fi
    printf 'graph_%s=%q\n' "$name" "$out/$name.tsv"
  done
}
