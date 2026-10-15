#!/usr/bin/env bash
# Applies checks/verify-counterparts.sh's identifier rules (U1 and N1) to a
# directory of prose specifications.
#
# The upstream identifier set and the normalization are extracted the same way
# verify-counterparts.sh extracts them: the same four jq queries over the same
# two inventory records, minus the same hand-excused ordinary words, and the
# same fold-to-lower-then-drop-non-alphanumeric normalization. The lexicon rules
# L1/L2/L3 are not applied, because they hold a Rust package's own names to a
# published list and a prose specification carries no package.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
registry_root="$(cd "$checks_dir/.." && pwd)"
target="${1:-$registry_root/specifications}"
inventory="$registry_root/inventory/${RAVEL_FIXTURE_ID:-atm10-tts-2.0.2}"
collisions="$registry_root/checks/public-word-collisions.tsv"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

normalize_stream() {
  LC_ALL=C tr '[:upper:]' '[:lower:]' | LC_ALL=C sed 's/[^a-z0-9]//g'
}

{
  jq -r '.artifacts[].mods[]' "$inventory/surfaces.json"
  jq -r '.artifacts[].artifact | sub("\\.(jar|zip)$"; "")' "$inventory/surfaces.json"
  jq -r '.nodes[].id' "$inventory/graph.json"
  jq -r '.nodes[] | .displayName // empty' "$inventory/graph.json"
} | LC_ALL=C tr '[:upper:]' '[:lower:]' |
  { grep -v '^$' || true; } | LC_ALL=C sort -u >"$work/upstream-raw"

grep -v '^#' "$collisions" | cut -f1 | LC_ALL=C tr '[:upper:]' '[:lower:]' |
  { grep -v '^$' || true; } | LC_ALL=C sort -u >"$work/excused"

LC_ALL=C comm -23 "$work/upstream-raw" "$work/excused" >"$work/upstream-literals"
normalize_stream <"$work/upstream-literals" |
  { grep -v '^$' || true; } | LC_ALL=C sort -u >"$work/upstream-normals"

if [ ! -s "$work/upstream-literals" ]; then
  echo "no upstream identifier was read from the inventory" >&2
  exit 1
fi

mapfile -t files < <(find -L "$target" -type f | LC_ALL=C sort)
if [ "${#files[@]}" -eq 0 ]; then
  echo "no file to check under $target" >&2
  exit 1
fi

: >"$work/out"

LC_ALL=C grep -oiwHFf "$work/upstream-literals" -- "${files[@]}" 2>/dev/null |
  LC_ALL=C sort -u | while IFS= read -r hit; do
  [ -n "$hit" ] || continue
  printf 'rule U1: %s carries an upstream identifier: %s\n' "${hit%%:*}" "${hit#*:}"
done >>"$work/out" || true

LC_ALL=C awk -v LITF="$work/upstream-literals" -v NORMF="$work/upstream-normals" '
  function norm(s) { gsub(/[^A-Za-z0-9]/, "", s); return tolower(s) }
  BEGIN {
    while ((getline l < LITF) > 0) { if (l != "") lit[tolower(l)] = 1 }
    while ((getline l < NORMF) > 0) {
      if (l == "") continue
      nrm[l] = 1
      if (length(l) >= 10) longn[++nlong] = l
    }
  }
  function examine(c,   n, i) {
    gsub(/^[^A-Za-z0-9]+/, "", c)
    gsub(/[^A-Za-z0-9]+$/, "", c)
    if (c == "" || c ~ /^[0-9]+$/) return
    n = norm(c)
    if (n == "" || length(n) < 2) return
    if (n in nrm) {
      if (tolower(c) in lit) return
      if (seen[n "\t" FILENAME]++) return
      printf "rule N1: %s carries a name that matches an upstream identifier under the documented normalization: %s\n", FILENAME, c
      return
    }
    if (length(n) >= 11) {
      for (i = 1; i <= nlong; i++) {
        if (index(n, longn[i]) > 0) {
          if (seen[longn[i] "\t" FILENAME]++) return
          printf "rule N1: %s carries a name with an upstream identifier inside it: %s\n", FILENAME, c
          return
        }
      }
    }
  }
  {
    line = $0
    while (match(line, /[A-Za-z0-9_][A-Za-z0-9_.:\/-]*/)) {
      t = substr(line, RSTART, RLENGTH)
      line = substr(line, RSTART + RLENGTH)
      examine(t)
      m = split(t, part, /[^A-Za-z0-9]+/)
      if (m > 1) for (k = 1; k <= m; k++) examine(part[k])
    }
  }
' "${files[@]}" >>"$work/out"

if [ -s "$work/out" ]; then
  printf 'specification identifier check failed:\n' >&2
  sed 's/^/- /' "$work/out" >&2
  exit 1
fi

printf 'specifications: %s files read against %s upstream identifiers, no hit under rules U1 or N1\n' \
  "${#files[@]}" "$(wc -l <"$work/upstream-literals" | tr -d ' ')"
printf 'specifications: not applied here: lexicon rules L1, L2 and L3, which hold a package name to checks/counterpart-lexicon.txt and have no counterpart in prose. Same limits as the counterpart check otherwise: a rewrite that renamed everything is invisible, and M5 recorded no Java member or field name, so only mod ids, node ids, display names and artifact file stems are reachable at all.\n'
