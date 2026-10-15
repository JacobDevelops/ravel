#!/usr/bin/env bash
# Counts the behaviour surfaces again, from a walk this file makes itself.
#
# Round 4's review found the defect this file exists to make impossible. The
# behaviour catalogue's model was passed the outer archives' tables of contents
# and nothing else, and `verify-surfaces.sh` recounted three of its totals from
# the very same listing while its comment said it was recounting "from the
# archives". Producer and consumer therefore shared a depth rule, agreed
# perfectly, and 12,571 class files inside jar-in-jar libraries were counted by
# neither. That is the shape of every blocking finding this workspace has had: a
# rule written once and read twice, with the rows that would contradict it never
# built.
#
# So the consumer's recount does not read either reader's listing. It opens the
# artifacts again and walks them itself, and it does so under its own rule:
#
#   descend into every entry that sits in a jar-in-jar directory *or* is named
#   like an archive, at any depth
#
# which is neither reader's rule. `checks/modmeta.py` discovers a library by
# location and everything else by its first four bytes; this walk adds the
# conventional names and takes no notice of bytes. Its *discovery* is therefore
# broader than the discovery it is checking: it descends into archives an
# artifact merely ships, so that a producer which stopped early is contradicted
# by a walk that went further rather than by a second reading of where the
# producer stopped.
#
# What it counts is narrower than what it walks, and the difference is the
# attribution decision M5-W4 records. An entry counts as a behaviour surface
# only when every container between it and the acquired artifact is a jar-in-jar
# library, because that is the chain a loader actually loads. An archive an
# artifact merely carries is walked, found, and counted nowhere.
#
# Round 5's review made the necessary correction to the paragraph above, which
# used to claim the breadth without saying which half of the work it applied to.
# Two rules here are the producer's rules and not second opinions, and saying so
# is worth more than a sentence that sounds stronger:
#
#   * which directories hold a loaded library. `META-INF/jarjar/` and
#     `META-INF/jars/` is the same pair `checks/modmeta.py` uses, so an entry
#     outside them contributes to no family in either reading. The extra breadth
#     buys a check on where the producer stopped walking and none on where it
#     stopped counting. Nothing in this fixture ships a library outside those two
#     directories, so it costs nothing today; the second opinion that would
#     settle it is a loader, and this workspace does not run one.
#   * which suffix names which family. The path segments below are this file's
#     own sentences, and the suffixes ask the same question the model asks. Round
#     5 found the one entry where that mattered, an icon named `.PNG` that landed
#     in no family in either reading because both tests were case-sensitive.
#
# The walk is independent. The attribution shares those two rules, and a defect
# in either is a defect both readings have.
#
# Sourcing this defines surface_recount, which writes one file of
# `artifact <TAB> family <TAB> count` rows and prints the assignment naming it.
set -euo pipefail

recount_listing=""
recount_violations=()

recount_candidates() {
  # Everything this walk will try to open, from the archive's own table of
  # contents. The manifest in a jar-in-jar directory names libraries rather than
  # being one, so it is the one entry there that is not a candidate.
  unzip -Z1 "$1" </dev/null 2>/dev/null |
    awk '
      /\/$/ { next }
      {
        leaf = $0
        sub(/.*\//, "", leaf)
        if (/^META-INF\/jarjar\// || /^META-INF\/jars\//) {
          if (leaf != "metadata.json") print
          next
        }
        if (tolower($0) ~ /\.(jar|zip|jmod|war|ear|aar|apk|zipx)$/) print
      }'
}

recount_descend() {
  # $1 archive on disk, $2 the acquired artifact, $3 depth (scratch-file
  # discriminator only), $4 whether everything above this point was a jar-in-jar
  # library and so is loaded rather than merely shipped.
  local jar="$1" filename="$2" depth="$3" loaded="$4"
  local extracted="$meta_work/recount-$depth.jar"
  local name child

  while IFS= read -r name; do
    [ -n "$name" ] || continue
    case "$name" in
      META-INF/jarjar/* | META-INF/jars/*) child="$loaded" ;;
      *) child=0 ;;
    esac

    if ! unzip -p "$jar" "$name" </dev/null >"$extracted" 2>/dev/null; then
      recount_violations+=("$filename!$name: the recount could not extract an entry its own walk found")
      continue
    fi
    if ! unzip -Z1 "$extracted" </dev/null >/dev/null 2>&1; then
      # Named like an archive and not one. That is a fact about the entry, and
      # the only place it is allowed to pass quietly: inside a jar-in-jar
      # directory it is a library the recount cannot open, which is a failure
      # here as well as in the reader's own ledger.
      if [ "$child" = 1 ]; then
        recount_violations+=("$filename!$name: a jar-in-jar library the recount could not open")
      fi
      continue
    fi

    if [ "$child" = 1 ]; then
      unzip -Z1 "$extracted" </dev/null 2>/dev/null |
        awk -v f="$filename" 'BEGIN { OFS = "\t" } { print f, 1, $0 }' >>"$recount_listing"
    fi
    recount_descend "$extracted" "$filename" "$((depth + 1))" "$child"
  done < <(recount_candidates "$jar")
}

surface_recount() {
  local out="${meta_work:?surface_recount requires mod-metadata.sh}"
  recount_listing="$out/recount-entries.tsv"
  recount_violations=()
  : >"$recount_listing"

  local class filename jar
  while IFS=$'\t' read -r class filename _sha _project _file _binding <&6; do
    case "$class" in
      mod) jar="$instance_root/mods/$filename" ;;
      shaderpack) jar="$instance_root/shaderpacks/$filename" ;;
      resourcepack) jar="$instance_root/resourcepacks/$filename" ;;
      *) continue ;;
    esac
    unzip -Z1 "$jar" </dev/null 2>/dev/null |
      awk -v f="$filename" 'BEGIN { OFS = "\t" } { print f, 0, $0 }' >>"$recount_listing"
    recount_descend "$jar" "$filename" 1 1
  done 6<"$artifact_tsv"

  rm -f "$meta_work"/recount-[0-9]*.jar

  if [ ! -s "$recount_listing" ]; then
    echo "the surface recount walked the acquired artifacts and found no entries at all" >&2
    return 1
  fi

  # The families, classified by this file's own expressions rather than the
  # model's. A family is decided by the segment a path sits under and the suffix
  # it ends in, which is the same question the model answers and not the same
  # sentence, so a defect in either reading is a disagreement.
  awk -F'\t' -v counts_out="$out/recount.tsv" '
    function bump(f, deep, fam) { total[f SUBSEP fam]++; if (deep) nest[f SUBSEP fam]++ }
    function under(fam, n, seg) { return (n >= 4 && (seg[3] == fam)) }
    BEGIN {
      # Named here rather than taken from what the walk happened to see, so a
      # family with nothing in it is a row of zeroes and never a missing row.
      split("lang textures models sounds shaders recipes lootTables tags " \
            "advancements worldgen functions serviceProviders classFiles", known, " ")
      for (i in known) fams[known[i]] = 1
    }
    {
      f = $1; deep = ($2 == "1"); p = $3
      arts[f] = 1
      if (p ~ /\/$/) next
      n = split(p, seg, "/")
      leaf = seg[n]
      # The suffix names a kind of file whatever case its author wrote it in.
      # The round 5 review found the one entry in this fixture that says so — an
      # icon named `.PNG` — landing in no family here and in no family in the
      # model, because both suffix tests were case-sensitive and therefore
      # agreed. That is the shared-rule shape again, and the header above now
      # says plainly which rules this file shares with the model it checks.
      leaf = tolower(leaf)
      if (seg[1] == "assets" && n >= 2) {
        if (under("lang", n, seg)) bump(f, deep, "lang")
        else if (leaf ~ /\.png$|\.jpg$|\.jpeg$/) bump(f, deep, "textures")
        else if (under("models", n, seg) || under("geo", n, seg) || under("animations", n, seg))
          bump(f, deep, "models")
        else if (leaf ~ /\.ogg$|\.wav$/ || leaf == "sounds.json") bump(f, deep, "sounds")
        else if (under("shaders", n, seg)) bump(f, deep, "shaders")
      } else if (seg[1] == "data" && n >= 2) {
        if (under("recipe", n, seg) || under("recipes", n, seg)) bump(f, deep, "recipes")
        else if (under("loot_table", n, seg) || under("loot_tables", n, seg)) bump(f, deep, "lootTables")
        else if (under("tags", n, seg)) bump(f, deep, "tags")
        else if (under("advancement", n, seg) || under("advancements", n, seg)) bump(f, deep, "advancements")
        else if (under("worldgen", n, seg)) bump(f, deep, "worldgen")
        else if (under("function", n, seg) || under("functions", n, seg)) bump(f, deep, "functions")
      } else if (seg[1] == "META-INF" && seg[2] == "services" && n >= 3) {
        bump(f, deep, "serviceProviders")
      } else if (leaf ~ /\.class$/) {
        bump(f, deep, "classFiles")
      }
      # A shaderpack keeps its programs at the archive root.
      if (seg[1] == "shaders" && n >= 2) bump(f, deep, "shaders")
    }
    END {
      for (f in arts)
        for (fam in fams)
          printf "%s\t%s\t%d\t%d\n", f, fam, total[f SUBSEP fam] + 0, nest[f SUBSEP fam] + 0 >counts_out
    }
  ' "$recount_listing"

  sort -o "$out/recount.tsv" "$out/recount.tsv"
  : >"$out/recount-failures"
  if [ ${#recount_violations[@]} -ne 0 ]; then
    printf '%s\n' "${recount_violations[@]}" >"$out/recount-failures"
  fi
  printf 'recount_tsv=%q\n' "$out/recount.tsv"
  printf 'recount_failures=%q\n' "$out/recount-failures"
}
