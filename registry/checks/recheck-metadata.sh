#!/usr/bin/env bash
# Reads the same artifacts again, with a different reader, and requires the two
# readings to agree.
#
# M5-W7's review found the defect this file exists to make impossible. Every
# producer and every consumer in this workspace derived its answer from one
# extraction, so a declaration the extractor did not recognise was absent from
# the producer's record and absent from the consumer's recomputation in exactly
# the same way. The consumer checked graph properties instead — partition,
# connectivity, no required internal edge crossing a boundary — and all of them
# hold on a graph that is missing an edge. Sharing the reader made the whole
# consumer side blind to the reader.
#
# So the readers are two, and they share nothing:
#
#   producer   checks/modmeta.py    Python's zipfile and tomllib
#   consumer   this file            unzip, taplo's Rust TOML parser, and jq
#
# Neither is a scanner; both are real parsers, so neither has the spelling blind
# spot that started this. What makes them evidence is that they are separate
# implementations of the same reading, compared byte for byte: a defect in either
# is a disagreement, and a disagreement is a gate failure with the differing
# records printed. Sourcing mod-metadata.sh runs this, so no producer and no
# consumer in this workspace can act on a single unchecked reading.
#
# What this cannot see is a rule both readers implement the same way, and round 2
# of the review found one: both parsed a nested library's dependencies in full
# and kept only its identity, so both agreed and 58 declarations vanished. That
# is checks/reader-accounting.sh's job, not this file's, and the two are wired
# side by side in mod-metadata.sh for that reason.
#
# Round 3 found the shared rule that decided which nested declarations exist at
# all: both readers took the list of nested libraries from the jar-in-jar
# manifest, so an artifact shipping two libraries and listing neither was read as
# shipping none by both of them. Neither reader reads that manifest now. This one
# takes the libraries from `unzip -Z1`, the archive's own table of contents,
# filtered to the directories a loader loads from, and it recurses, because a
# library a library ships loads on the same terms. It also writes the tables of
# contents of the archives it descends into, so reader-accounting.sh can require
# a site to exist from a listing neither reader's discovery rule produced.
set -euo pipefail

recheck_project='
  def s: if . == null then ""
         elif type == "boolean" then (if . then "true" else "false" end)
         elif type == "string" then .
         else tostring end;
  def at($t; $n): ($t | to_entries | map(select(.key | ascii_downcase == ($n | ascii_downcase)))
                      | if length == 0 then null else .[0].value end);
  . as $d
  | (at($d; "mods") | if . == null then [] elif type == "object" then [.] else . end) as $mods
  | ([$mods[] | at(.; "modId") | s]) as $modIds
  | (at($d; "dependencies")) as $deps
  | [
      (at($d; "modLoader") | select(. != null) | ["LOADER", s, "", "", "", ""]),
      (at($d; "license") | select(. != null) | ["LICENSE", s, "", "", "", ""]),
      ($mods[] | ["MOD", (at(.; "modId") | s), (at(.; "version") | s),
                  (at(.; "displayName") | s), (at(.; "logoFile") | s), ""]),
      ((at($d; "mixins") // []) | .[] | select(type == "object")
        | at(.; "config") | select(. != null) | ["MIXIN", s, "", "", "", ""]),
      ( if $deps == null then empty
        elif ($deps | type) == "object" then ($deps | to_entries[] | { owner: .key, entries: .value })
        elif ($deps | type) == "array" then
          (if ($modIds | length) == 1
           then { owner: $modIds[0], entries: $deps }
           else { unparsed: "dependencies are declared with no owner and the file declares \($modIds | length) mods, so no owner can be attributed" } end)
        else { unparsed: "dependencies are a \($deps | type), not a table or an array" } end
        | if .unparsed then ["UNPARSED", .unparsed, "", "", "", ""]
          else . as $g
          | ($g.entries | if type == "object" then [.] else . end) as $entries
          | if ($entries | type) != "array" then ["UNPARSED", "dependencies.\($g.owner) is a \($g.entries | type), not an array of tables", "", "", "", ""]
            else $entries[]
            | if type != "object" then ["UNPARSED", "a dependencies.\($g.owner) entry is a \(type), not a table", "", "", "", ""]
              elif (at(.; "modId") | s) == "" then ["UNPARSED", "a dependencies.\($g.owner) entry declares no modId", "", "", "", ""]
              else ["DEP", ($g.owner | s), (at(.; "modId") | s), (at(.; "type") | s | ascii_downcase),
                    (at(.; "versionRange") | s), (at(.; "side") | s | ascii_upcase)]
              end
            end
          end )
    ]
  | .[] | @tsv'

recheck_declaration() {
  # A mod declaration, under either of the two names a loader accepts.
  unzip -p "$1" META-INF/neoforge.mods.toml </dev/null 2>/dev/null ||
    unzip -p "$1" META-INF/mods.toml </dev/null 2>/dev/null ||
    true
}

recheck_nested_paths() {
  # The libraries an archive ships, read off its own table of contents. The rule
  # is a location, so no manifest can leave one out; the manifest itself is the
  # one entry in those directories that names libraries instead of being one.
  unzip -Z1 "$1" </dev/null 2>/dev/null |
    awk '
      /\/$/ { next }
      /^META-INF\/jarjar\// || /^META-INF\/jars\// {
        leaf = $0
        sub(/.*\//, "", leaf)
        if (leaf != "metadata.json") print
      }'
}

recheck_descend() {
  # $1 archive on disk, $2 the acquired artifact it belongs to, $3 the site path
  # so far ("" at the top), $4 the depth, used only to keep the scratch files of
  # one level from overwriting the level above it.
  local jar="$1" filename="$2" prefix="$3" depth="$4"
  local extracted="$meta_work/recheck-nested-$depth.jar"
  local toml="$meta_work/recheck-$depth.toml"
  local name path nested_rows

  while IFS= read -r name; do
    [ -n "$name" ] || continue
    if [ -n "$prefix" ]; then path="$prefix!$name"; else path="$name"; fi

    if ! unzip -p "$jar" "$name" </dev/null >"$extracted" 2>/dev/null ||
      ! unzip -Z1 "$extracted" </dev/null >/dev/null 2>&1; then
      printf '%s\tNESTED\t%s\t\t\t\t\n' "$filename" "$path" >>"$mine"
      continue
    fi

    unzip -Z1 "$extracted" </dev/null 2>/dev/null |
      awk -v f="$filename" -v p="$path" 'BEGIN { OFS = "\t" } { print f, p, $0 }' >>"$mine_nested"

    recheck_declaration "$extracted" >"$toml"
    if [ ! -s "$toml" ]; then
      printf '%s\tNESTED\t%s\t\t\t\t\n' "$filename" "$path" >>"$mine"
    else
      # A nested library's own dependencies travel with the artifact that ships
      # it; what it says about its loader, licence and mixins does not.
      nested_rows="$(recheck_records "$toml" "$filename!$path" |
        awk -v f="$filename" -v p="$path" '
          BEGIN { FS = OFS = "\t" }
          $1 == "UNPARSED" { print f, $1, $2, "", "", "", ""; next }
          $1 == "MOD" { print f, "NESTED", p, $2, $3, "", ""; next }
          $1 == "DEP" { print f, "NESTEDDEP", $2, $3, $4, $5, $6 }')"
      if [ -n "$nested_rows" ]; then
        printf '%s\n' "$nested_rows" >>"$mine"
      fi
      # The placeholder stands for a nested jar that named no mod, so it is the
      # mod row that decides it and never the dependency rows.
      if ! printf '%s\n' "$nested_rows" | grep -q $'\tNESTED\t'; then
        printf '%s\tNESTED\t%s\t\t\t\t\n' "$filename" "$path" >>"$mine"
      fi
    fi

    recheck_descend "$extracted" "$filename" "$path" "$((depth + 1))"
  done < <(recheck_nested_paths "$jar")
}

recheck_records() {
  # taplo parses the TOML; jq projects it. Neither step is this workspace's own
  # grammar, which is the point.
  local toml="$1" where="$2" json
  if ! json="$(taplo get -o json -f "$toml" 2>&1)"; then
    echo "second reader could not parse $where: $json" >&2
    return 1
  fi
  printf '%s' "$json" | jq -r "$recheck_project"
}

recheck_metadata() {
  local out="${meta_work:?recheck_metadata requires mod-metadata.sh}"
  local mine="$out/recheck-meta.tsv" mine_entries="$out/recheck-entries.tsv"
  local mine_nested="$out/recheck-nested-entries.tsv"
  local toml="$out/recheck.toml"

  for tool in unzip taplo jq; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      echo "second declared-metadata reader needs $tool, which is not on PATH" >&2
      return 1
    fi
  done

  : >"$mine"
  : >"$mine_entries"
  : >"$mine_nested"

  while IFS=$'\t' read -r class filename _sha _project _file _binding <&4; do
    [ "$class" = "mod" ] || continue
    local jar="$instance_root/mods/$filename"

    unzip -Z1 "$jar" </dev/null 2>/dev/null |
      awk -v f="$filename" 'BEGIN { OFS = "\t" } { print f, $0 }' >>"$mine_entries"

    recheck_declaration "$jar" >"$toml"
    if [ ! -s "$toml" ]; then
      printf '%s\tNOMETA\t\t\t\t\t\n' "$filename" >>"$mine"
    else
      recheck_records "$toml" "$filename" |
        awk -v f="$filename" 'BEGIN { FS = OFS = "\t" } { print f, $1, $2, $3, $4, $5, $6 }' >>"$mine"
    fi

    recheck_descend "$jar" "$filename" "" 1
  done 4<"$artifact_tsv"

  while IFS=$'\t' read -r class filename _sha _project _file _binding <&4; do
    [ "$class" = "shaderpack" ] || continue
    unzip -Z1 "$instance_root/shaderpacks/$filename" </dev/null 2>/dev/null |
      awk -v f="$filename" 'BEGIN { OFS = "\t" } { print f, $0 }' >>"$mine_entries"
    # A shaderpack declares no mod, but "what does this artifact ship" is asked
    # of every acquired artifact, so it is walked on the same terms.
    recheck_descend "$instance_root/shaderpacks/$filename" "$filename" "" 1
  done 4<"$artifact_tsv"

  while IFS=$'\t' read -r class filename _sha _project _file _binding <&4; do
    [ "$class" = "resourcepack" ] || continue
    unzip -Z1 "$instance_root/resourcepacks/$filename" </dev/null 2>/dev/null |
      awk -v f="$filename" 'BEGIN { OFS = "\t" } { print f, $0 }' >>"$mine_entries"
    # A resource pack declares no mod either, and is walked on the same terms as
    # a shaderpack: a pinned bundled resource the census still asks "what does
    # this ship" of.
    recheck_descend "$instance_root/resourcepacks/$filename" "$filename" "" 1
  done 4<"$artifact_tsv"

  rm -f "$toml" "$meta_work"/recheck-nested-*.jar "$meta_work"/recheck-[0-9]*.toml

  if [ ! -s "$mine" ] || [ ! -s "$mine_entries" ]; then
    echo "the second reader read no declared metadata from the acquired mods" >&2
    return 1
  fi

  # A declaration neither reader may guess at. It is classified rather than
  # dropped, and a classification is a failure rather than a record.
  if grep -q $'\tUNPARSED\t' "$mine"; then
    echo "declared metadata could not be interpreted:" >&2
    awk -F'\t' '$2 == "UNPARSED" { printf "- %s: %s\n", $1, $3 }' "$mine" >&2
    return 1
  fi

  # The comparison. Both readings are sorted, because only their content is
  # being compared and each reader walks the archives in its own order.
  sort -o "$mine" "$mine"
  sort -o "$mine_entries" "$mine_entries"
  sort -o "$mine_nested" "$mine_nested"
  sort "$meta_tsv" >"$out/recheck-theirs.tsv"
  sort "$entries_tsv" >"$out/recheck-theirs-entries.tsv"
  sort "$nested_entries_tsv" >"$out/recheck-theirs-nested.tsv"

  local failed=0
  if ! diff -q "$out/recheck-theirs.tsv" "$mine" >/dev/null; then
    echo "the two declared-metadata readers disagree:" >&2
    diff "$out/recheck-theirs.tsv" "$mine" | sed -n "1,40p" >&2
    failed=1
  fi
  if ! diff -q "$out/recheck-theirs-entries.tsv" "$mine_entries" >/dev/null; then
    echo "the two archive readers disagree about the tables of contents:" >&2
    diff "$out/recheck-theirs-entries.tsv" "$mine_entries" | sed -n "1,40p" >&2
    failed=1
  fi
  # The nested tables of contents. These are what reader-accounting.sh requires a
  # site to exist from, so a reader that never descended somewhere would
  # otherwise be the only witness that there was nowhere to descend to.
  if ! diff -q "$out/recheck-theirs-nested.tsv" "$mine_nested" >/dev/null; then
    echo "the two archive readers disagree about what the nested libraries hold:" >&2
    diff "$out/recheck-theirs-nested.tsv" "$mine_nested" | sed -n "1,40p" >&2
    failed=1
  fi
  return "$failed"
}
