#!/usr/bin/env bash
# Keeps restricted artifacts and machine-local data out of tracked registry files.
set -euo pipefail

# shellcheck source=checks/public-repo.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/public-repo.sh"
cd "$registry_root"


violations=()
note() { violations+=("$1"); }

scratch=()
cleanup() { [ ${#scratch[@]} -gt 0 ] && rm -f "${scratch[@]}"; return 0; }
trap cleanup EXIT
tmpfile() { local t; t="$(mktemp)"; scratch+=("$t"); printf '%s' "$t"; }

# NUL delimiters preserve fixture filenames containing newlines.
tracked_file="$(tmpfile)"
if ! tracked_files registry >"$tracked_file"; then
  echo "registry tracked files could not be listed" >&2
  exit 1
fi
tracked=()
while IFS= read -r -d '' path; do
  tracked+=("${path#registry/}")
done <"$tracked_file"
if [ ${#tracked[@]} -eq 0 ]; then
  note "no tracked files"
fi

for path in "${tracked[@]}"; do
  case "${path,,}" in
    *.jar | *.zip | *.mrpack | *.pem | *.p12 | *.key | *.keystore | *.hprof | *.jfr)
      note "forbidden tracked path: $path"
      ;;
    # A Ravel mod crate under mods/ may legitimately carry an upstream mod id in
    # its directory name (e.g. the f-t-b-quests or kube-js mod) — that is the mod
    # this workspace reimplements, not the pack's ATM-authored override tree. The
    # override patterns below are about pack data, which never lives under mods/,
    # so mods/ is exempted from them and still subject to every other rule.
    mods/*) : ;;
    overrides/* | */overrides/* | *kubejs* | *ftbquests* | */saves/* | */logs/*)
      note "forbidden tracked path: $path"
      ;;
    .env | .env.* | *credentials.json | *secrets.json)
      note "forbidden tracked path: $path"
      ;;
  esac
done

for path in "${tracked[@]}"; do
  [ -f "$path" ] || continue
  # Reject machine-local paths and credential material.
  # Each literal is written with one character bracketed. The pattern still
  # matches what it is looking for, and no longer matches its own source, which
  # is the only reason this file is not its own first violation.
  if LC_ALL=C grep -naiE '/[h]ome/|/[u]sers/|[c]:\\[u]sers\\|/\.[m]inecraft|[p]rism[ _-]?(launcher|approot)|-----[B]EGIN [A-Z ]*PRIVATE KEY-----|([a]pi[_-]?key|[a]ccess[_-]?token|[p]assword|[c]lient[_-]?secret)[[:space:]]*[:=][[:space:]]*["'"'"']?[A-Za-z0-9_/+.-]{12,}' \
    -- "$path" >/dev/null; then
    note "tracked file carries a local path or credential material: $path"
  fi

  # Pack-owned override structure. M5-W4 reads the configuration and KubeJS
  # trees to count them, which is a leak shape the earlier rules did not cover:
  # those trees are ATM-authored, so a count of them may travel and a path
  # inside them may not. Bracketed for the same reason as above. A mod crate
  # under mods/ is exempt: its source legitimately references the mod's own
  # namespace (e.g. an f-t-b-quests recipe id), which is the mod being
  # reimplemented rather than a transcription of the pack's ATM-authored tree.
  case "$path" in mods/*) continue ;; esac
  if LC_ALL=C grep -naE '[k]ubejs/(startup|server|client)_scripts/|[k]ubejs/dynamic_data/|[f]tbquests/' \
    -- "$path" >/dev/null; then
    note "tracked file transcribes pack-owned override structure: $path"
  fi
done

# Upstream identifiers in the tree that is meant to be published.
#
# M5-W7 commits the first prose in this workspace, and prose is a leak shape the
# rules above do not describe: nothing in a sentence looks like a path or a key.
# It is also the first tree written to leave here, which is what makes the rule
# placeable. Everything under proposals/ and resolutions/, and the two files in
# checks/ that carry the published words of the first of them, must name no
# acquired entry — as a field, as a comment, or as a sentence.
#
# resolutions/ joined this rule when M6 wrote it, and it is held here rather than
# by the lexicon that holds proposals/. The lexicon is closed in both directions,
# so a word added to it is a word M5's published count of it stops describing,
# and that count is a figure about material. This rule costs no such figure.
#
# The rule stops at that tree rather than covering the whole workspace, and the
# reason is worth stating rather than leaving as an omission. One acquired entry
# chose an ordinary English word as its id, and this repository has used that
# word about its own contents since M5-W1. A workspace-wide rule would therefore
# fail on eighteen files that leak nothing, and the only way to keep it would be
# a list of excused words — which is the hole this check exists to not have. The
# publishable tree pays the cost instead: nothing there uses a word an entry has
# taken, which is cheap to hold for one tree and impossible to hold for all of
# them.
#
# The list is read from the inventory rather than written here, so it grows with
# the fixture and cannot go stale, and only identifiers long enough to be
# unmistakable are matched as substrings.
#
# The identifier list is required, not merely used when it happens to be there.
# An earlier revision of this file wrapped everything below in a test for the
# inventory and had no else, so hiding one tracked file turned the whole rule
# off and the check still printed that the boundary held. The file is tracked,
# so its absence is never legitimate and is a failure here.
inventory="$registry_root/inventory/${FIXTURE_ID}/surfaces.json"
if [ ! -f "$inventory" ]; then
  note "missing input: inventory/${FIXTURE_ID}/surfaces.json, which is where the identifiers to refuse are read from"
else
  if ! command -v jq >/dev/null 2>&1; then
    echo "jq is not on PATH" >&2
    exit 1
  fi
  upstream="$(tmpfile)"
  {
    jq -r '.artifacts[].artifact' "$inventory"
    jq -r '.artifacts[].artifact | sub("\\.(jar|zip)$"; "")' "$inventory"
    jq -r '.artifacts[].mods[]' "$inventory"
  } | tr '[:upper:]' '[:lower:]' | awk 'length($0) >= 5' | LC_ALL=C sort -u >"$upstream"

  published=0
  for path in "${tracked[@]}"; do
    [ -f "$path" ] || continue
    case "$path" in
      proposals/* | resolutions/* | checks/proposals-vocabulary.json | checks/proposals-lexicon.txt) ;;
      *) continue ;;
    esac
    published=$((published + 1))
    if LC_ALL=C grep -oiFf "$upstream" -- "$path" >/dev/null; then
      note "a file written to be published names an acquired entry: $path"
    fi
  done
  if [ "$published" -eq 0 ]; then
    note "no publishable file was checked for upstream identifiers"
  fi
  if [ ! -s "$upstream" ]; then
    note "no upstream identifier was read from the inventory; the rule would pass by having nothing to refuse"
  fi
fi

# Excused public-word collisions must equal the observed set in both directions.
# Only registry material is outside the public-word collision scan.
collisions="$registry_root/checks/public-word-collisions.tsv"
graph="$registry_root/inventory/${FIXTURE_ID}/graph.json"
if [ ! -f "$collisions" ] || [ ! -f "$graph" ]; then
  note "missing input: public-word collisions or their inventory graph"
else
  ids="$(tmpfile)"
  observed="$(tmpfile)"
  recorded="$(tmpfile)"
  public_files="$(tmpfile)"
  jq -r '.nodes[].id' "$graph" | tr '[:upper:]' '[:lower:]' | LC_ALL=C sort -u >"$ids"
  awk -F'\t' 'NF >= 2 && $1 != "" { print tolower($1) }' "$collisions" | LC_ALL=C sort -u >"$recorded"
  if ! tracked_files >"$public_files"; then
    note "repository tracked paths could not be listed for public-word collisions"
  else
    public_tracked=()
    while IFS= read -r -d '' path; do
      case "$path" in registry/*) continue ;; esac
      public_tracked+=("$path")
    done <"$public_files"
    if [ "${#public_tracked[@]}" -eq 0 ] || [ ! -s "$ids" ]; then
      note "public paths or inventory identifiers are empty; collisions cannot be checked"
    else
      raw="$(tmpfile)"
      searched=0
      (cd "$repo_root" && LC_ALL=C grep -d skip -oiwHFf "$ids" -- "${public_tracked[@]}") >"$raw" 2>/dev/null || searched=$?
      if [ "$searched" -gt 1 ]; then
        note "public tracked files could not be searched for inventory identifiers"
      fi
      printf '%s\n' "${public_tracked[@]}" |
        (LC_ALL=C grep -oiwFf "$ids" || true) >>"$raw"
      awk -F: '{ print tolower($NF) }' "$raw" | LC_ALL=C sort -u >"$observed"
      while IFS= read -r word; do
        [ -n "$word" ] && note "an inventory identifier appears outside registry without a recorded collision decision: $word"
      done < <(LC_ALL=C comm -23 "$observed" "$recorded")
      while IFS= read -r word; do
        [ -n "$word" ] && note "a recorded public-word collision no longer occurs: $word"
      done < <(LC_ALL=C comm -13 "$observed" "$recorded")
    fi
  fi
fi

# Material bindings remain immutable; machinery changes require an explicit
# machineryAmendments entry and refreshed digests in the milestone summary.
# Bound names remain registry-relative, including checks/*.
summary="$repo_root/evidence/m5/inventory-summary.json"
if [ ! -f "$summary" ]; then
  note "missing evidence/m5/inventory-summary.json, which binds registry files by digest"
else
  machinery_set="$(tmpfile)"
  find "$registry_root/checks" -type f -printf 'checks/%P\n' | LC_ALL=C sort >"$machinery_set"
  if [ ! -s "$machinery_set" ]; then
    note "the checks directory enumerates no file, so bound artifact classes cannot be checked"
  fi
  bound="$(tmpfile)"
  jq -r '.boundArtifacts[] | .name + "\t" + (.class // "absent") + "\t" + .sha256' "$summary" >"$bound"
  if [ ! -s "$bound" ]; then
    note "the milestone record binds no file by digest"
  fi
  bound_count=0
  material_count=0
  machinery_count=0
  while IFS=$'\t' read -r name class published; do
    [ -n "$name" ] || continue
    bound_count=$((bound_count + 1))
    is_machinery=no
    if grep -qxF "$name" "$machinery_set"; then
      is_machinery=yes
    fi
    case "$class" in
      material)
        material_count=$((material_count + 1))
        if [ "$is_machinery" = yes ]; then
          note "the record binds $name as material, but it is this gate's own machinery"
        fi
        ;;
      machinery)
        machinery_count=$((machinery_count + 1))
        if [ "$is_machinery" = no ]; then
          note "the record binds $name as machinery, but it is not this gate's own machinery"
        fi
        ;;
      absent) note "the record binds $name without declaring its material or machinery class" ;;
      *) note "the record binds $name under an unknown class: $class" ;;
    esac
    is_tracked=no
    for path in "${tracked[@]}"; do
      if [ "$path" = "$name" ]; then
        is_tracked=yes
        break
      fi
    done
    if [ "$is_tracked" = no ]; then
      note "the record binds $name by digest, but the registry does not track it"
      continue
    fi
    actual="$(sha256sum "$registry_root/$name" | cut -d' ' -f1)"
    if [ "$actual" != "$published" ]; then
      case "$class" in
        machinery) note "machinery $name differs from its bound digest; amend the machinery record with the milestone and reason, then rebind it" ;;
        *) note "material $name differs from its immutable milestone digest" ;;
      esac
    fi
  done <"$bound"
  echo "milestone record binds $bound_count registry files by digest ($material_count material, $machinery_count machinery), each recomputed and class checked"
fi


if [ ${#violations[@]} -ne 0 ]; then
  printf 'registry boundary check failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

echo "registry boundary holds across ${#tracked[@]} tracked files"
