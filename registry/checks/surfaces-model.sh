#!/usr/bin/env bash
# Turns declared structure and file layout into behaviour-surface records.
#
# The mechanical half of M5-W4, shared by the producer and the consumer for the
# same reason graph-model.sh is. Every count below comes from an archive's table
# of contents or from a mod's own declaration. Nothing is decompiled and no
# resource, data file or script is opened, so a count here says how much of a
# surface exists and never what it contains.
#
# A count is taken at every depth, and it is attributed to the acquired artifact
# that ships it. Round 4's review found the alternative: this model was passed
# the outer archive's table of contents alone, so 12,571 class files inside
# jar-in-jar libraries — 21% of the pack's — were counted by nothing, one
# artifact recorded four class files while shipping 6,728, and ten port plans
# carried the wrong size band. The reason attribution is to the container and
# not to a record of its own is that a nested library is not an acquired thing:
# no pinned reference names it, nothing downloaded it separately, and it loads
# only because the artifact around it ships it. A counterpart reimplementing
# that artifact has to reproduce what the library provides, so that is where the
# work sits. The split is not thrown away — every record carries the nested
# portion of each family beside the total, so a reader can see that an
# artifact's bulk is a shipped runtime rather than the mod's own code.
#
# Two of the columns are honest guesses and are labelled as such wherever they
# surface: networking and persistence are registered in code, so the only thing
# a file listing can offer is how many classes sit on a package path that names
# them. They are recorded as indicative counts and are never treated as an
# inventory of packets or of saved data.
#
# The pack's own overlay is handled differently again. ATM's configuration and
# KubeJS trees are ATM-owned material this workspace does not transcribe, so no
# name from either tree is written down. What is written down is a flag on a mod
# id the mod itself declared: whether the pack overrides that mod. The fact
# travels; ATM's authored structure does not.
set -euo pipefail

surfaces_model() {
  local out="${meta_work:?surfaces_model requires mod-metadata.sh}"
  local overlay="$out/overlay.tsv"

  : >"$overlay"
  awk -F'\t' '$2 == "MOD" || ($2 == "NESTED" && $4 != "") { print ($2 == "MOD" ? $3 : $4) }' "$meta_tsv" |
    sort -u |
    while IFS= read -r mod_id; do
      [ -n "$mod_id" ] || continue
      config_hit=false; script_hit=false
      if [ -d "$instance_root/config" ]; then
        for candidate in "$instance_root/config/$mod_id" "$instance_root/config/$mod_id".*; do
          [ -e "$candidate" ] && config_hit=true
        done
        for suffix in common client server; do
          for candidate in "$instance_root/config/$mod_id-$suffix".*; do
            [ -e "$candidate" ] && config_hit=true
          done
        done
      fi
      if [ -d "$instance_root/kubejs" ] &&
        find -L "$instance_root/kubejs" -mindepth 1 -maxdepth 3 -type d -name "$mod_id" -print -quit |
        grep -q .; then
        script_hit=true
      fi
      printf '%s\t%s\t%s\n' "$mod_id" "$config_hit" "$script_hit" >>"$overlay"
    done

  if [ ! -s "$overlay" ]; then
    echo "surface model derived no mod ids to attribute the pack overlay to" >&2
    return 1
  fi

  awk -F'\t' -v surfaces_out="$out/surfaces.tsv" '
    function count(a, k) { return (k in a) ? a[k] : 0 }

    FILENAME == artifacts { klass[$2] = $1; project[$2] = $4; file[$2] = $5; next }

    FILENAME == overlay_file { config_overlay[$1] = $2; script_overlay[$1] = $3; next }

    function addmod(f, id) {
      # An id declared at top level and again by a library the artifact ships is
      # one mod, not two. graph-model.sh has always deduplicated; this file used
      # to append, so `sodium` was listed twice and the two files disagreed about
      # what "the mods this artifact declares" means.
      if ((f SUBSEP id) in seenmod) return
      seenmod[f SUBSEP id] = 1
      mods[f] = (f in mods) ? mods[f] "," id : id
    }

    # One entry of one archive, counted against the acquired artifact that ships
    # it. `deep` is 1 when the entry was found inside a jar-in-jar library rather
    # than in the artifact itself; the family counters are the same either way
    # and the deep half is additionally kept on its own so the record can state
    # the split.
    function tally(f, p, deep,   ns, lower) {
      if (p ~ /\/$/) {
        if (p ~ /^assets\/[^\/]+\/$/) { ns = p; sub(/^assets\//, "", ns); sub(/\/$/, "", ns)
                                        if (!((f SUBSEP ns) in seenasset)) { seenasset[f SUBSEP ns] = 1; assetns[f]++ } }
        if (p ~ /^data\/[^\/]+\/$/)   { ns = p; sub(/^data\//, "", ns); sub(/\/$/, "", ns)
                                        if (!((f SUBSEP ns) in seendata)) { seendata[f SUBSEP ns] = 1; datans[f]++ } }
        return
      }
      files[f]++
      # A suffix names a kind of file and does not name it in one case. The
      # round 5 review found `assets/croptopia/icon.PNG` counted by no family in
      # either reading, because this test and the one in the recount tested the
      # suffix case-sensitively and so agreed. A file kind is the same kind
      # however its author spelled the extension, so every suffix here is tested
      # against the lowered path.
      lower = tolower(p)
      if (p ~ /^assets\//) {
        hasassets[f] = 1
        if (p ~ /^assets\/[^\/]+\/lang\//) { lang[f]++; if (deep) nlang[f]++ }
        else if (lower ~ /\.(png|jpg|jpeg)$/) { textures[f]++; if (deep) ntextures[f]++ }
        else if (p ~ /^assets\/[^\/]+\/(models|geo|animations)\//) { models[f]++; if (deep) nmodels[f]++ }
        else if (lower ~ /\.(ogg|wav)$/ || lower ~ /sounds\.json$/) { sounds[f]++; if (deep) nsounds[f]++ }
        else if (p ~ /^assets\/[^\/]+\/shaders\//) { shaders[f]++; if (deep) nshaders[f]++ }
      } else if (p ~ /^data\//) {
        hasdata[f] = 1
        if (p ~ /^data\/[^\/]+\/(recipe|recipes)\//) { recipes[f]++; if (deep) nrecipes[f]++ }
        else if (p ~ /^data\/[^\/]+\/(loot_table|loot_tables)\//) { loot[f]++; if (deep) nloot[f]++ }
        else if (p ~ /^data\/[^\/]+\/tags\//) { tags[f]++; if (deep) ntags[f]++ }
        else if (p ~ /^data\/[^\/]+\/(advancement|advancements)\//) { advancements[f]++; if (deep) nadvancements[f]++ }
        else if (p ~ /^data\/[^\/]+\/worldgen\//) { worldgen[f]++; if (deep) nworldgen[f]++ }
        else if (p ~ /^data\/[^\/]+\/(function|functions)\//) { functions[f]++; if (deep) nfunctions[f]++ }
      } else if (p ~ /^META-INF\/services\//) {
        services[f]++; if (deep) nservices[f]++
      } else if (p ~ /\.cfg$/ && p ~ /^META-INF\//) {
        transformers[f]++
      } else if (lower ~ /\.class$/) {
        classes[f]++; if (deep) nclasses[f]++
        if (lower ~ /\/(network|networking|packet|packets|msg|message|messages)\//) net[f]++
        if (lower ~ /(saveddata|savedata|worlddata|worldsaveddata|persist)/) persist[f]++
      }
      # Shaderpacks carry their programs at the archive root.
      if (p ~ /^shaders\//) { shaders[f]++; if (deep) nshaders[f]++; hasshader[f] = 1 }
      if (p ~ /^META-INF\/accesstransformer.*\.cfg$/) transformers[f]++
    }

    FILENAME == meta_file {
      if ($2 == "MOD")    { addmod($1, $3) }
      else if ($2 == "NESTED") { nested[$1]++; if ($4 != "") addmod($1, $4) }
      else if ($2 == "MIXIN")  { mixins[$1]++ }
      # The same field as the edges in graph.json, read the same way: a
      # dependency that names no side is declared for both, which is what a
      # loader does with it. The two models used to default it differently over
      # one field, so one file said BOTH where the other said nothing.
      else if ($2 == "DEP" || $2 == "NESTEDDEP") {
                                 s = ($7 == "" ? "BOTH" : toupper($7))
                                 if (!(($1 SUBSEP s) in seenside)) { seenside[$1 SUBSEP s] = 1
                                   sides[$1] = ($1 in sides) ? sides[$1] "," s : s } }
      next
    }

    # The two tables of contents. The acquired archive supplies the first; the
    # second is the table of contents of every jar-in-jar library the readers
    # descended into, at any depth, written twice and compared byte for byte.
    # Directory entries end in a slash and are not files.
    FILENAME == entries_file { tally($1, $2, 0); next }
    FILENAME == nested_file  { tally($1, $3, 1); next }

    END {
      for (f in klass) {
        modlist = (f in mods) ? mods[f] : "-"
        sidelist = (f in sides) ? sides[f] : "-"

        if (klass[f] == "shaderpack") layout = "shader-pipeline"
        else if (klass[f] == "resourcepack") layout = "resource-pack"
        else if ((f in hasassets) && (f in hasdata)) layout = "both"
        else if (f in hasassets) layout = "client-leaning"
        else if (f in hasdata) layout = "server-leaning"
        else if (f in classes) layout = "code-only"
        else layout = "empty"

        cfg = "false"; scr = "false"
        if (modlist != "-") {
          n = split(modlist, ids, ",")
          for (i = 1; i <= n; i++) {
            if (config_overlay[ids[i]] == "true") cfg = "true"
            if (script_overlay[ids[i]] == "true") scr = "true"
          }
        }

        total = 0
        total += count(lang, f) + count(textures, f) + count(models, f) + count(sounds, f)
        total += count(shaders, f) + count(recipes, f) + count(loot, f) + count(tags, f)
        total += count(advancements, f) + count(worldgen, f) + count(functions, f)
        total += count(mixins, f) + count(services, f) + count(classes, f)
        if (total > 0) verdict = "catalogued"
        else if (count(files, f) == 0) verdict = "dead:archive-holds-no-files"
        else verdict = "dead:no-behaviour-surface-declared-or-laid-out"

        printf "%s\t%s\t%s\t%s\t%s", f, klass[f], project[f], file[f], modlist >surfaces_out
        printf "\t%d\t%d\t%d\t%d\t%d\t%d", count(assetns, f), count(lang, f), count(textures, f),
          count(models, f), count(sounds, f), count(shaders, f) >surfaces_out
        printf "\t%d\t%d\t%d\t%d\t%d\t%d\t%d", count(datans, f), count(recipes, f), count(loot, f),
          count(tags, f), count(advancements, f), count(worldgen, f), count(functions, f) >surfaces_out
        printf "\t%d\t%d\t%d\t%d", count(mixins, f), count(transformers, f), count(services, f),
          count(nested, f) >surfaces_out
        printf "\t%d\t%d\t%d", count(net, f), count(persist, f), count(classes, f) >surfaces_out
        printf "\t%s\t%s\t%s\t%s\t%s", layout, sidelist, cfg, scr, verdict >surfaces_out
        # The nested half of the same families, so the split the attribution
        # decision folds together stays recoverable from the record itself.
        printf "\t%d\t%d\t%d\t%d\t%d", count(nlang, f), count(ntextures, f), count(nmodels, f),
          count(nsounds, f), count(nshaders, f) >surfaces_out
        printf "\t%d\t%d\t%d\t%d\t%d\t%d", count(nrecipes, f), count(nloot, f), count(ntags, f),
          count(nadvancements, f), count(nworldgen, f), count(nfunctions, f) >surfaces_out
        printf "\t%d\t%d\n", count(nservices, f), count(nclasses, f) >surfaces_out
      }
    }
  ' artifacts="$artifact_tsv" overlay_file="$overlay" meta_file="$meta_tsv" \
    entries_file="$entries_tsv" nested_file="$nested_entries_tsv" \
    "$artifact_tsv" "$overlay" "$meta_tsv" "$entries_tsv" "$nested_entries_tsv"

  sort -o "$out/surfaces.tsv" "$out/surfaces.tsv"

  # The pack's own overlay, as totals only. A count of ATM's files is a fact
  # about the pack; the files' names are ATM's to keep.
  # Both trees are required by artifact-set.sh, so these are counts rather than
  # counts-if-present: a missing tree is a failure there and never a zero here.
  local config_files script_files
  config_files="$(find -L "$instance_root/config" -type f | wc -l)"
  script_files="$(find -L "$instance_root/kubejs" -type f -name '*.js' | wc -l)"

  if [ ! -s "$out/surfaces.tsv" ]; then
    echo "surface model produced no records" >&2
    return 1
  fi
  printf 'surfaces_tsv=%q\n' "$out/surfaces.tsv"
  printf 'overlay_config_files=%q\n' "$config_files"
  printf 'overlay_script_files=%q\n' "$script_files"
}
