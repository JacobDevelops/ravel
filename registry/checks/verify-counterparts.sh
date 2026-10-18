#!/usr/bin/env bash
# Rewrite integrity: holds every counterpart to a published lexicon and to the
# identifiers the M5 inventory knows.
#
# The consumer for M6-W5, and the property it exists for is the one ROADMAP.md
# states: a transliteration carries the original's names and a rewrite does not,
# so the names are what tells them apart mechanically. Nothing here reads a jar,
# decompiles anything, or produces Rust from Java. It reads names.
#
# Two sides that are not allowed to be the same thing:
#
#   The upstream identifier set is read out of the pinned pack inventory as data --
#   mod ids, graph node ids and display names, and artifact file stems. It is
#   whatever M5 recorded, and no rule here decides what belongs in it.
#
#   The lexicon in checks/counterpart-lexicon.txt is written by hand and says
#   what a counterpart may call itself. Nothing derives it.
#
# They meet in rule L2 below, which refuses a lexicon entry that is an upstream
# identifier or matches one under the documented normalization. That is the join
# where they can contradict each other, and a check whose two sides cannot
# contradict each other is the defect this project has spent eight review rounds
# on. Neither side is generated from the other and neither is generated from the
# counterparts.
#
# What it reads: every regular file under mods/, source and generated
# alike. ROADMAP.md says in as many words that a check enforcing the rule over
# source files only has not satisfied the obligation, and the reason is plain --
# a generated artefact carries names too, and it is the one a counterpart author
# does not type by hand.
#
# What it refuses to do: pass over nothing. An empty counterpart set is a
# failure here rather than a green line, and it names the directory it expected.
# This check was written before M6-W3 wrote a counterpart, on purpose, so that
# counterparts are written under it rather than audited against it afterwards;
# the cost of that choice is that this check fails the gate until the first
# counterpart exists, and it says so when it does.
#
# It used to be impossible to put a check here at all, because M5's closed public
# evidence binds checks/verify.sh by digest and publishes counts over this
# directory. docs/decisions/machinery-is-not-material.md decides that those are a
# measurement of the tree M5 certified rather than an invariant of every later
# tree, so M6 amends M5's machinery figures, records why there, and puts this
# check where a check that has to fail the gate belongs.
#
# What it cannot catch is printed on every run rather than left for a reader to
# work out. See the limits block at the end of this file and the line the check
# prints.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
registry_root="$(cd "$checks_dir/.." && pwd)"

inventory="$registry_root/inventory/${RAVEL_FIXTURE_ID:-atm10-tts-2.0.2}"
lexicon="$checks_dir/counterpart-lexicon.txt"
normalization="$checks_dir/counterpart-normalization.tsv"
fixtures_table="$checks_dir/counterpart-fixtures.tsv"
alias_fixtures_table="$checks_dir/counterpart-alias-fixtures.tsv"
collisions="$checks_dir/public-word-collisions.tsv"
counterpart_root="$registry_root/mods"
fixture_root="$registry_root/fixtures/counterparts"
alias_fixture_root="$registry_root/fixtures/counterpart-aliases"
specification_root="$registry_root/specifications/m6-foundation"

for tool in awk cat comm find grep sed sort tr; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "required tool missing: $tool" >&2
    exit 1
  fi
done
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is not on PATH" >&2
  exit 1
fi

for path in "$lexicon" "$normalization" "$fixtures_table" "$alias_fixtures_table" "$collisions" \
  "$inventory/surfaces.json" "$inventory/graph.json"; do
  if [ ! -f "$path" ]; then
    printf 'counterpart verification failed:\n- missing input: %s\n' \
      "${path#"$registry_root/"}" >&2
    exit 1
  fi
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

violations=()
note() { violations+=("$1"); }

fail_now() {
  printf 'counterpart verification failed:\n' >&2
  printf -- '- %s\n' "$@" >&2
  exit 1
}

alias_shape_valid() {
  jq -e '
    type == "object" and
    (keys | sort) == ["canonicalId", "mappings", "schemaVersion"] and
    .schemaVersion == 1 and
    (.canonicalId | type == "string" and test("^RVL-[0-9]{3}$")) and
    (.mappings | type == "array") and
    all(.mappings[];
      type == "object" and
      (keys | sort) == ["source", "target"] and
      (.source | type == "string" and test("^[a-z][a-z0-9]*(?:[-_.][a-z0-9]+)*:[a-z0-9][a-z0-9_./-]*$")) and
      (.target | type == "string" and test("^[a-z][a-z0-9]*(?:[-_.][a-z0-9]+)*:[a-z0-9][a-z0-9_./-]*$"))) and
    ([.mappings[].source] | length == (unique | length)) and
    ([.mappings[].target] | length == (unique | length))
  ' "$1" >/dev/null 2>&1
}

declare -A proved_alias_files=()

# Validates only the aliases.json at each immediate package root. A source value
# receives the U1/N1 exemption only after the whole declaration is structurally
# valid, canonically bound, and evidenced by exact accepted-spec lines.
validate_alias_packages() {
  local packages_root="$1" specs_root="$2" label="$3" out="$4"
  local packages=() package package_name aliases canonical declared_canonical
  local specification source target valid
  : >"$out"
  [ -d "$packages_root" ] || return 0
  mapfile -d '' -t packages < <(
    find "$packages_root" -mindepth 1 -maxdepth 1 -type d -print0 | LC_ALL=C sort -z
  )
  for package in "${packages[@]}"; do
    package_name="${package##*/}"
    aliases="$package/aliases.json"
    if [[ ! "$package_name" =~ ^rvl-([0-9]{3})- ]]; then
      printf '%s: package has no canonical slot in its directory name: %q\n' \
        "$label" "$package_name" >>"$out"
      continue
    fi
    canonical="RVL-${BASH_REMATCH[1]}"
    if [ ! -f "$aliases" ]; then
      printf '%s: counterpart %s has no aliases.json\n' "$label" "$package_name" >>"$out"
      continue
    fi
    if ! alias_shape_valid "$aliases"; then
      printf '%s: counterpart %s has a malformed aliases.json: only schemaVersion, canonicalId, and unique source/target mappings are admitted\n' \
        "$label" "$package_name" >>"$out"
      continue
    fi
    valid=1
    declared_canonical="$(jq -r '.canonicalId' "$aliases")"
    if [ "$declared_canonical" != "$canonical" ]; then
      printf '%s: counterpart %s aliases.json names %s instead of %s\n' \
        "$label" "$package_name" "$declared_canonical" "$canonical" >>"$out"
      valid=0
    fi
    specification="$specs_root/$canonical.md"
    if [ ! -f "$specification" ]; then
      printf '%s: counterpart %s has no accepted specification for %s\n' \
        "$label" "$package_name" "$canonical" >>"$out"
      valid=0
    else
      while IFS=$'\t' read -r source target; do
        [ -n "${source:-}" ] || continue
        if ! LC_ALL=C grep -Fxq -- "save-compatible alias: $source -> $target" "$specification"; then
          printf '%s: counterpart %s maps %s to %s without an exact save-compatibility requirement in %s\n' \
            "$label" "$package_name" "$source" "$target" "$canonical" >>"$out"
          valid=0
        fi
      done < <(jq -r '.mappings[] | [.source, .target] | @tsv' "$aliases")
    fi
    if [ "$valid" -eq 1 ]; then
      proved_alias_files["$aliases"]=1
    fi
  done
}

# ---------------------------------------------------------------------------
# The normalization, in one place, used by everything below.
# ---------------------------------------------------------------------------
normalize_stream() {
  LC_ALL=C tr '[:upper:]' '[:lower:]' | LC_ALL=C sed 's/[^a-z0-9]//g'
}

# The documented rule is held against the code that implements it. Every worked
# example in checks/counterpart-normalization.tsv is recomputed here, so the
# table and the implementation fail rather than drift.
examples=0
while IFS=$'\t' read -r raw expected _rest; do
  case "$raw" in '#'* | '') continue ;; esac
  [ -n "${expected:-}" ] || continue
  examples=$((examples + 1))
  actual="$(printf '%s' "$raw" | normalize_stream)"
  if [ "$actual" != "$expected" ]; then
    note "the documented normalization disagrees with the one this check applies: '$raw' is documented as '$expected' and normalizes to '$actual'"
  fi
done <"$normalization"
if [ "$examples" -eq 0 ]; then
  note "checks/counterpart-normalization.tsv states no worked example, so the documented normalization is not held to anything"
fi

# ---------------------------------------------------------------------------
# The upstream identifier set, read from the M5 inventory as data.
#
# These are the names that identify an original implementation: what a mod calls
# itself, what the graph calls it, what it is displayed as, and what its file is
# called. Loader names, version numbers and the other separator-delimited debris
# in a file name are not identity and are not read as identity, which is why the
# stem is taken whole rather than chopped up. A name a counterpart legitimately
# shares with an ordinary English word is excused by checks/public-word-
# collisions.tsv, which M5 already published for this exact reason and which is
# written by hand.
# ---------------------------------------------------------------------------
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

# The same set in normal form. Kept separate rather than derived on the fly so
# that both the verbatim rule and the normalized rule read the same source.
normalize_stream <"$work/upstream-literals" |
  { grep -v '^$' || true; } | LC_ALL=C sort -u >"$work/upstream-normals"

if [ ! -s "$work/upstream-literals" ] || [ ! -s "$work/upstream-normals" ]; then
  fail_now "no upstream identifier was read from the inventory; every rule below would pass by having nothing to compare against"
fi

# ---------------------------------------------------------------------------
# The lexicon, and rule L2: it may not carry an upstream name.
# ---------------------------------------------------------------------------
grep -v '^#' "$lexicon" | LC_ALL=C tr '[:upper:]' '[:lower:]' |
  sed 's/[[:space:]]*$//' | { grep -v '^$' || true; } |
  LC_ALL=C sort -u >"$work/lexicon"
if [ ! -s "$work/lexicon" ]; then
  note "the published lexicon is empty, so no counterpart name would be held to anything"
fi
normalize_stream <"$work/lexicon" | LC_ALL=C sort -u >"$work/lexicon-normals"

while IFS= read -r bad; do
  [ -n "$bad" ] && note "rule L2: the published lexicon carries an upstream identifier: $bad"
done < <(LC_ALL=C comm -12 "$work/lexicon" "$work/upstream-literals")
while IFS= read -r bad; do
  [ -n "$bad" ] && note "rule L2: the published lexicon carries a name that matches an upstream identifier under the documented normalization: $bad"
done < <(LC_ALL=C comm -12 "$work/lexicon-normals" "$work/upstream-normals")

# ---------------------------------------------------------------------------
# The roots to scan. counterparts/ is the real one; the fixtures are tracked
# counterparts with a declared verdict, so that the rules above are proved to
# discriminate on every run rather than only on the day somebody mutated one.
# ---------------------------------------------------------------------------
: >"$work/carried-words"

# Prints one violation per line for the tree at $1, with $2 naming it.
scan_root() {
  local root="$1" label="$2" out="$3"
  local files=()
  : >"$out"

  if [ ! -d "$root" ]; then
    printf '%s: the directory does not exist: %s\n' "$label" "${root#"$registry_root/"}" >>"$out"
    return 0
  fi
  mapfile -d '' -t files < <(find -L "$root" -type f -print0 | LC_ALL=C sort -z)
  if [ "${#files[@]}" -eq 0 ]; then
    printf '%s: no file to check under %s\n' "$label" "${root#"$registry_root/"}" >>"$out"
    return 0
  fi

  # Text only. A binary is not read as names, and it is counted and disclosed
  # rather than silently dropped.
  local text=() text_names=() named_text=() binary=0 source_files=0 generated_files=0
  local f rel sanitized probe_status
  for f in "${files[@]}"; do
    if [ ! -r "$f" ]; then
      printf '%s: unreadable file refused: %q\n' "$label" "${f#"$root/"}" >>"$out"
      continue
    fi
    set +e
    LC_ALL=C grep -qI '' -- "$f" 2>/dev/null
    probe_status=$?
    set -e
    if [ "$probe_status" -eq 0 ]; then
      rel="${f#"$root/"}"
      named_text+=("$f")
      if [ -n "${proved_alias_files["$f"]+proved}" ]; then
        sanitized="$work/aliases.$$.${#text[@]}.json"
        jq '(.mappings[]?.source) = ""' "$f" >"$sanitized"
        text+=("$sanitized")
      else
        text+=("$f")
      fi
      text_names+=("$rel")
      case "/$rel/" in
        */generated/* | */target/* | */build/*) generated_files=$((generated_files + 1)) ;;
        *) source_files=$((source_files + 1)) ;;
      esac
    elif [ "$probe_status" -eq 1 ]; then
      binary=$((binary + 1))
    else
      printf '%s: file probe failed closed for %q\n' "$label" "${f#"$root/"}" >>"$out"
    fi
  done
  if [ "${#text[@]}" -eq 0 ]; then
    printf '%s: every file under %s is binary, so no name was read\n' \
      "$label" "${root#"$registry_root/"}" >>"$out"
    return 0
  fi
  if [ "$source_files" -eq 0 ]; then
    printf '%s: no source file under %s, so the rules ran over generated output alone\n' \
      "$label" "${root#"$registry_root/"}" >>"$out"
  fi
  printf '%s\t%s\t%s\t%s\n' "$label" "$source_files" "$generated_files" "$binary" \
    >>"$work/read-counts"

  # Rule U1: the file carries an upstream identifier verbatim. Word boundaries,
  # case-insensitive, fixed strings. This is the branch a transliteration trips.
  local matched i
  for i in "${!text[@]}"; do
    while IFS= read -r matched; do
      [ -n "$matched" ] || continue
      printf '%s: rule U1: %q carries an upstream identifier: %s\n' \
        "$label" "${text_names[$i]}" "$matched" >>"$out"
    done < <(LC_ALL=C grep -oiwFf "$work/upstream-literals" -- "${text[$i]}" 2>/dev/null |
      LC_ALL=C sort -u || true)
  done

  # Rule N1: a name that is not an upstream identifier verbatim but becomes one
  # under the documented normalization. Case, separator style and package
  # segmentation are all folded, so a transliteration that renames by convention
  # is still a transliteration.
  for i in "${!text[@]}"; do
    LC_ALL=C awk -v LITF="$work/upstream-literals" -v NORMF="$work/upstream-normals" \
      -v DISPLAY="${text_names[$i]@Q}" -v LABEL="$label" '
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
      # Trailing punctuation is not part of a name. Without this a sentence
      # ending in an upstream name is reported twice, once verbatim and once as
      # a normalization hit, and the second report is an artefact of the comma.
      gsub(/^[^A-Za-z0-9]+/, "", c)
      gsub(/[^A-Za-z0-9]+$/, "", c)
      if (c == "" || c ~ /^[0-9]+$/) return
      n = norm(c)
      if (n == "" || length(n) < 2) return
      if (n in nrm) {
        if (tolower(c) in lit) return          # rule U1 already said so
        if (seen[n "\t" FILENAME]++) return
        printf "%s: rule N1: %s carries a name that matches an upstream identifier under the documented normalization: %s\n", \
          LABEL, DISPLAY, c
        return
      }
      if (length(n) >= 11) {
        for (i = 1; i <= nlong; i++) {
          if (index(n, longn[i]) > 0) {
            if (seen[longn[i] "\t" FILENAME]++) return
            printf "%s: rule N1: %s carries a name with an upstream identifier inside it: %s\n", \
              LABEL, DISPLAY, c
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
    ' "${text[$i]}" >>"$out"
  done

  # Rule L1: every name the package gives itself is a name the lexicon
  # publishes. The package directory, the directories under it and every file
  # stem, split on the separators a Rust tree uses.
  local names="$work/names.$$"
  : >"$names"
  for f in "${named_text[@]}"; do
    rel="${f#"$root/"}"
    printf '%s\0' "$rel"
  done | LC_ALL=C tr -cs 'A-Za-z0-9' '\n' | LC_ALL=C tr '[:upper:]' '[:lower:]' |
    { grep -v '^$' || true; } | { grep -vE '^[0-9]+$' || true; } |
    LC_ALL=C sort -u >"$names"
  while IFS= read -r bad; do
    [ -n "$bad" ] || continue
    printf '%s: rule L1: the package carries a name the published lexicon does not publish: %s\n' \
      "$label" "$bad" >>"$out"
  done < <(LC_ALL=C comm -23 "$names" "$work/lexicon")
  LC_ALL=C sort -u "$names" "$work/carried-words" -o "$work/carried-words"
  rm -f "$names"
  return 0
}

# Validate the exact package-root declarations before scanning so only fully
# proved source values can enter the exemption allowlist.
validate_alias_packages "$counterpart_root" "$specification_root" \
  "counterparts" "$work/out-real-aliases"
while IFS= read -r line; do
  [ -n "$line" ] && note "$line"
done <"$work/out-real-aliases"

# ---------------------------------------------------------------------------
# The real counterpart set. Empty is a failure and names the directory.
# ---------------------------------------------------------------------------
: >"$work/read-counts"
: >"$work/out-counterparts"
if [ -d "$counterpart_root" ] &&
  [ -n "$(find -L "$counterpart_root" -type f -print -quit 2>/dev/null)" ]; then
  scan_root "$counterpart_root" "counterparts" "$work/out-counterparts"
else
  note "there is no counterpart to check. This check refuses to pass over an empty set, because a check that passes over nothing says nothing. It expected counterpart packages under ${counterpart_root#"$registry_root/"}/ and found none; M6-W3 is the work package that writes the first one, and this check was deliberately built before it so that counterparts are written under it."
fi
while IFS= read -r line; do
  [ -n "$line" ] && note "$line"
done <"$work/out-counterparts"

# ---------------------------------------------------------------------------
# The tracked fixtures, each with a verdict declared by hand in
# checks/counterpart-fixtures.tsv. A rule that fires on nothing and a rule that
# fires on everything are the same useless rule, and this is what separates
# them: one fixture must pass and the others must fail, and the check fails if
# any fixture's verdict is not the one declared.
# ---------------------------------------------------------------------------
declared=0
while IFS=$'\t' read -r name expect reason; do
  case "$name" in '#'* | '') continue ;; esac
  [ -n "${expect:-}" ] || continue
  declared=$((declared + 1))
  if [ -z "${reason:-}" ]; then
    note "fixture $name declares no reason for its verdict"
  fi
  case "$expect" in
    pass | fail) ;;
    *)
      note "fixture $name declares the unknown verdict '$expect'"
      continue
      ;;
  esac
  scan_root "$fixture_root/$name" "fixture:$name" "$work/out-fixture"
  if [ -s "$work/out-fixture" ]; then
    actual=fail
  else
    actual=pass
  fi
  if [ "$actual" != "$expect" ]; then
    note "fixture $name is declared to $expect and the rules return $actual over it"
    if [ "$expect" = pass ]; then
      while IFS= read -r line; do
        [ -n "$line" ] && note "  $line"
      done <"$work/out-fixture"
    fi
  fi
done <"$fixtures_table"
if [ "$declared" -eq 0 ]; then
  note "checks/counterpart-fixtures.tsv declares no fixture, so nothing proves these rules discriminate"
fi
if [ -d "$fixture_root" ]; then
  while IFS= read -r -d '' fixture; do
    printf '%s\n' "${fixture##*/}"
  done < <(find -L "$fixture_root" -mindepth 1 -maxdepth 1 -type d -print0 | LC_ALL=C sort -z) \
    >"$work/fixtures-present"
  grep -v '^#' "$fixtures_table" | cut -f1 | { grep -v '^$' || true; } |
    LC_ALL=C sort -u >"$work/fixtures-declared"
  while IFS= read -r bad; do
    [ -n "$bad" ] && note "a fixture tree exists that no verdict is declared for: $bad"
  done < <(LC_ALL=C comm -23 "$work/fixtures-present" "$work/fixtures-declared")
  while IFS= read -r bad; do
    [ -n "$bad" ] && note "a verdict is declared for a fixture tree that does not exist: $bad"
  done < <(LC_ALL=C comm -13 "$work/fixtures-present" "$work/fixtures-declared")
fi

# ---------------------------------------------------------------------------
# Package-root alias fixtures exercise structural, canonical, evidence, path,
# and selective-sanitization behavior against miniature tracked package trees.
# Each failing case also names the diagnostic that must remain observable.
# ---------------------------------------------------------------------------
alias_declared=0
while IFS=$'\t' read -r name expect required; do
  case "$name" in '#'* | '') continue ;; esac
  [ -n "${expect:-}" ] || continue
  alias_declared=$((alias_declared + 1))
  case "$expect" in
    pass | fail) ;;
    *) note "alias fixture $name declares the unknown verdict '$expect'"; continue ;;
  esac
  case_root="$alias_fixture_root/$name"
  validate_alias_packages "$case_root/counterparts" "$case_root/specifications" \
    "alias-fixture:$name" "$work/out-alias-validation"
  scan_root "$case_root/counterparts" "alias-fixture:$name" "$work/out-alias-scan"
  awk '1' "$work/out-alias-validation" "$work/out-alias-scan" >"$work/out-alias-fixture"
  if [ -s "$work/out-alias-fixture" ]; then actual=fail; else actual=pass; fi
  if [ "$actual" != "$expect" ]; then
    note "alias fixture $name is declared to $expect and the rules return $actual"
  elif [ "$expect" = fail ] && ! LC_ALL=C grep -Fq -- "$required" "$work/out-alias-fixture"; then
    note "alias fixture $name fails without its required diagnostic: $required"
  fi
done <"$alias_fixtures_table"
if [ "$alias_declared" -eq 0 ]; then
  note "checks/counterpart-alias-fixtures.tsv declares no fixture"
fi
if [ -d "$alias_fixture_root" ]; then
  while IFS= read -r -d '' fixture; do
    printf '%s\n' "${fixture##*/}"
  done < <(find "$alias_fixture_root" -mindepth 1 -maxdepth 1 -type d -print0 | LC_ALL=C sort -z) \
    >"$work/alias-fixtures-present"
  grep -v '^#' "$alias_fixtures_table" | cut -f1 | { grep -v '^$' || true; } |
    LC_ALL=C sort -u >"$work/alias-fixtures-declared"
  while IFS= read -r bad; do
    [ -n "$bad" ] && note "an alias fixture tree exists that no verdict is declared for: $bad"
  done < <(LC_ALL=C comm -23 "$work/alias-fixtures-present" "$work/alias-fixtures-declared")
  while IFS= read -r bad; do
    [ -n "$bad" ] && note "an alias fixture verdict exists without a tree: $bad"
  done < <(LC_ALL=C comm -13 "$work/alias-fixtures-present" "$work/alias-fixtures-declared")
fi

# ---------------------------------------------------------------------------
# Rule L3: the lexicon may not be padded. A published name nothing carries is a
# permission granted ahead of the work it would excuse.
# ---------------------------------------------------------------------------
LC_ALL=C sort -u "$work/carried-words" -o "$work/carried-words"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "rule L3: the published lexicon publishes a name no counterpart carries: $bad"
done < <(LC_ALL=C comm -23 "$work/lexicon" "$work/carried-words")

if [ "${#violations[@]}" -gt 0 ]; then
  printf 'counterpart verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

src_total=0
gen_total=0
bin_total=0
while IFS=$'\t' read -r _label s g b; do
  [ -n "${s:-}" ] || continue
  src_total=$((src_total + s))
  gen_total=$((gen_total + g))
  bin_total=$((bin_total + b))
done <"$work/read-counts"

printf 'counterparts: %s source files and %s generated artefacts read against %s upstream identifiers and %s published names\n' \
  "$src_total" "$gen_total" \
  "$(wc -l <"$work/upstream-literals" | tr -d ' ')" \
  "$(wc -l <"$work/lexicon" | tr -d ' ')"
printf 'counterparts: what this does not catch: a rewrite that renamed everything; an upstream name M5 never recorded, and M5 recorded no Java member or field name, so only names carrying a mod id, a node id, a display name or a file stem are reachable here; a generated artefact absent from disk at the moment of this run (%s were present); %s binary files, which are not read as names; and the %s ordinary words checks/public-word-collisions.tsv excuses by hand.\n' \
  "$gen_total" "$bin_total" "$(wc -l <"$work/excused" | tr -d ' ')"
