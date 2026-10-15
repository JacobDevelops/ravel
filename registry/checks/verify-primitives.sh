#!/usr/bin/env bash
# Reads the premise the read observation acts on, instead of stating it.
#
# `checks/read-observation.sh` wraps two channels and `checks/verify-reads.sh`
# judges what came back through them. Both files rested on a claim about this
# workspace's own code — that those two are the only ways an archive gets opened
# here — and that claim was written in the files that acted on it and nowhere
# else. Round 6's review broke it in one edit: a reader injected into
# `checks/surfaces-model.sh` opened all 235 artifacts with the builtin, walked
# the local file headers by hand and inflated 50,357 class entries, using nothing
# but the standard library and no wrapped channel at all. The gate exited 0 and
# printed that it had classified 1,448,401 archive reads and seen no entry body,
# which was true of everything that reached the classification and said nothing
# about what did not. The same review ran the injection a second way, verbatim
# through the wrapped channel but under an interpreter started with the flag that
# skips site initialisation, and the shim never loaded.
#
# So the premise is read here, from the code, as a second and independent reading
# of the same thing the observation assumes. Two questions are asked of every
# file under checks/:
#
#   1. Does it use a read primitive this workspace has no business using? Those
#      are refused outright and there is no table entry that admits one. A hand
#      that wants one edits `checks/read-primitive-vocabulary.tsv`, which is a
#      tracked decision with a reason attached rather than a data entry.
#   2. Does it use a read primitive this workspace legitimately uses? Then the
#      occurrence has to be named, counted and given a reason in
#      `checks/read-primitives.tsv`, which this checker owns and no producer
#      writes. The comparison is set equality in both directions, so a new
#      occurrence fails and a row whose occurrence has gone fails too.
#
# This file holds no primitive of its own. What counts as a way of reading is a
# judged claim, so it is data: `checks/read-primitive-vocabulary.tsv` names each
# one, carries the pattern and a probe the pattern must match, and says why the
# primitive is refused or accounted. The two are congruent both ways — a pattern
# that matches nothing fails as loudly as an occurrence nothing accounts for,
# which is the failure a bare list of regexes cannot report, because a mistyped
# pattern and a clean workspace print the same line.
#
# The file set is every file under checks/, not the transitive closure of what
# verify.sh runs. That is deliberate: a closure is computed by a rule, and a rule
# is exactly the thing this sequence of reviews keeps finding wrong. The whole
# directory is a superset of any closure, so a file the closure would have missed
# is scanned anyway, and `checks/verify-reads.sh` copies the whole directory into
# its sandbox and runs producers out of it, which makes the superset the right
# set rather than merely a safe one. Two further rules keep the set honest: an
# untracked file under checks/ is a failure, because the sandbox tar would carry
# it, and code may only be sourced or executed from under checks/.
#
# This file is scanned on the same terms as every other. The vocabulary's
# literals are written with one character bracketed, so each pattern still
# matches what it looks for and no longer matches its own source; that is the
# same device `checks/verify-boundary.sh` uses, and it is why this checker is not
# exempt from itself.
#
# What this cannot see is printed on every run, passing or failing, by the
# `limits` array below. That is not decoration and it is not a comment: the
# mechanism this file repairs was a correct mechanism whose reach was stated
# nowhere it would be read, which is how it came to be presented as proof. A
# gate that says what it does not cover is worth more than one that implies it
# covers everything, so the reach is printed beside the verdict rather than left
# for a reviewer to derive.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/public-repo.sh
. "$checks_dir/public-repo.sh"

vocabulary="$checks_dir/read-primitive-vocabulary.tsv"
table="$checks_dir/read-primitives.tsv"
for path in "$vocabulary" "$table"; do
  if [ ! -f "$path" ]; then
    printf 'read primitive check failed:\n- missing input: %s\n' "${path#"$registry_root/"}" >&2
    exit 1
  fi
done

# What this scan does not reach. Printed on every run; see the head of this file
# for why that is a requirement rather than a courtesy.
limits=(
  "it reads static text, so a name computed at run time is opaque to it: the interpreter invocation in checks/mod-metadata.sh is accounted here and the script it is pointed at is a variable, which checks/verify-reads.sh holds at run time instead"
  "it reads the files that are there, not the text a run generates: shell a model writes into the scratch directory is accounted by row and its generator is scanned, and the generated text itself is never read by anything"
  "it recognises the primitives checks/read-primitive-vocabulary.tsv names and no others, so a channel nobody has thought of is invisible until somebody adds a row"
  "the accounted half is a table, so a hand that adds a reader and writes its row in the same edit passes; this catches an accident and a careless hand and does not catch a determined one"
  "it strips whole-line comments only, and says nothing about the tools a primitive calls into or about anything outside checks/"
)

violations=()
note() { violations+=("$1"); }

# Emptying the list above would turn a gate that discloses its reach back into
# one that implies it has none, silently and with everything else still green.
if [ ${#limits[@]} -lt 3 ]; then
  printf 'read primitive check failed:\n- this check states less than it must about what it cannot reach\n' >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The vocabulary, read rather than restated. Nothing below knows the name of a
# primitive that is not in this file.
declare -A class_of pattern_of probe_of
order=()
fragment=""
while IFS=$'\t' read -r primitive class pattern probe reason; do
  case "$primitive" in '' | '#'*) continue ;; esac
  case "$class" in
    refused | accounted | fragment) ;;
    *)
      note "checks/read-primitive-vocabulary.tsv gives $primitive a class this checker does not define: $class"
      continue
      ;;
  esac
  if [ -z "$pattern" ] || [ -z "$probe" ]; then
    note "checks/read-primitive-vocabulary.tsv gives $primitive no pattern or no probe, so nothing shows the pattern matches anything"
    continue
  fi
  if [ "${#reason}" -lt 40 ]; then
    note "checks/read-primitive-vocabulary.tsv records no reason for what it decides about $primitive"
  fi
  if [ "$class" = fragment ]; then
    fragment="$pattern"
  fi
  class_of["$primitive"]="$class"
  pattern_of["$primitive"]="$pattern"
  probe_of["$primitive"]="$probe"
  order+=("$primitive")
done <"$vocabulary"

if [ -z "$fragment" ]; then
  note "checks/read-primitive-vocabulary.tsv declares no command-position fragment, and the patterns that build on it would match nothing"
fi

# Each pattern against its own probe. A pattern that matches nothing watches
# nothing and reads exactly like a clean workspace, which is the one thing a list
# of regexes can be wrong about without saying so.
#
# The fragment carries an ampersand, because a command after `&&` is a command.
# Bash 5.2 made a bare ampersand in a substitution's replacement stand for the
# text that matched, so the substitution below silently replaced that ampersand
# with the marker it was expanding and every pattern built on the fragment lost
# its `&` alternative. Nothing failed: every invocation in this workspace at the
# time sat at the start of a line, so the surviving `^` alternative matched all
# of them, and a refused primitive placed after `&&` would have passed unseen.
# It is escaped here, and the assertion below refuses a pattern the marker
# survived, so the next way this can break is a failure rather than a silence.
fragment_replacement="${fragment//&/\\&}"
for primitive in ${order[@]+"${order[@]}"}; do
  pattern="${pattern_of[$primitive]//%CMD%/$fragment_replacement}"
  pattern_of["$primitive"]="$pattern"
  case "$pattern" in
    *%CMD%*)
      note "the pattern for $primitive still carries the command-position marker after expansion, so it is not the pattern this vocabulary declares"
      ;;
  esac
  if ! printf '%s\n' "${probe_of[$primitive]}" | LC_ALL=C grep -qE -- "$pattern"; then
    note "the pattern checks/read-primitive-vocabulary.tsv gives $primitive does not match the probe beside it, so it matches nothing and watches nothing"
  fi
done

if [ ${#order[@]} -eq 0 ]; then
  printf 'read primitive check failed:\n- checks/read-primitive-vocabulary.tsv names no primitive, so this check would pass by looking for nothing\n' >&2
  exit 1
fi

# Which files are code. A data file is skipped, and is required to be one: a
# .json or .tsv that is executable or carries a shebang is a script wearing a
# data file's name, and this is where that is refused rather than scanned past.
mapfile -t present < <(find "$checks_dir" -type f -printf '%P\n' | LC_ALL=C sort)
# Symlinks are named separately rather than left to fall out of the difference
# between two listings. Round 7 put one under checks/ pointing outside the
# directory and it was refused — but as "is tracked and is not there", because
# `find -type f` does not see a symlink and `jj file list` does. The route was
# closed by an accident of two tools disagreeing, and the sentence a maintainer
# read named the wrong reason. A symlink is refused here, as itself, because
# what it points at is not a file this scan read.
mapfile -t linked < <(find "$checks_dir" -type l -printf '%P\n' | LC_ALL=C sort)
if ! tracked_files registry/checks >"$work/tracked"; then
  printf 'read primitive check failed: registry tracked files could not be listed\n' >&2
  exit 1
fi
tracked=()
while IFS= read -r -d '' path; do
  tracked+=("${path#registry/checks/}")
done <"$work/tracked"

for name in ${linked[@]+"${linked[@]}"}; do
  note "checks/$name is a symlink, so what would be scanned is a path outside this listing rather than the file the sandbox copies"
done

existing=(${present[@]+"${present[@]}"} ${linked[@]+"${linked[@]}"})
for name in "${present[@]}"; do
  if ! grep -qxF "$name" <<<"$(printf '%s\n' "${tracked[@]}")"; then
    note "checks/$name is not tracked, and the read observation copies this directory into its sandbox and runs producers out of it"
  fi
done
for name in "${tracked[@]}"; do
  if ! grep -qxF "$name" <<<"$(printf '%s\n' "${existing[@]}")"; then
    note "checks/$name is tracked and is not there"
  fi
done

code=()
for name in "${present[@]}"; do
  case "$name" in
    *.json | *.tsv | *.txt | *.md)
      if [ -x "$checks_dir/$name" ] || head -c 2 "$checks_dir/$name" | grep -qF '#!'; then
        note "checks/$name is named as data and is executable or carries a shebang, so it is code that this scan would have skipped"
      fi
      ;;
    *) code+=("$name") ;;
  esac
done

if [ ${#code[@]} -eq 0 ]; then
  printf 'read primitive check failed:\n- no file under checks/ was scanned, so this check proved nothing\n' >&2
  exit 1
fi

# The reach limit the printed list did not state. The scan prints how many files
# it read and used to say nothing about the ones it did not, so a reader had to
# subtract two numbers this check never showed them together. It is computed
# rather than written down, which is the only way it does not go stale as files
# are added.
limits+=("it scans code and skips files named as data: of the ${#present[@]} files under checks/, ${#code[@]} are scanned and $((${#present[@]} - ${#code[@]})) are named .json, .tsv, .txt or .md and are not, though one of those that is executable or carries a shebang fails above rather than being skipped")

# Full-line comments are removed before scanning, and replaced by blank lines so
# the line numbers still point at the file. Prose in this workspace names every
# one of these primitives constantly and none of it is an invocation; what is
# left after this is code, plus the strings and here-documents inside it, and
# those are code too.
observed="$work/observed.tsv"
: >"$observed"
for name in "${code[@]}"; do
  stripped="$work/stripped"
  sed 's/^[[:space:]]*#.*$//' "$checks_dir/$name" >"$stripped"
  for primitive in "${order[@]}"; do
    [ "${class_of[$primitive]}" = fragment ] && continue
    # Occurrences, not lines. This counted matching lines until round 7, and the
    # table beside it was written from that output, so the two agreed and the row
    # that would have contradicted them — a second occurrence on one line — was
    # never built. Two such rows were in the tree and the published total was
    # wrong by two. A reader added to a line that already carried one moved
    # nothing and passed, which the disclosed limits did not describe.
    hits="$( { LC_ALL=C grep -oE -- "${pattern_of[$primitive]}" "$stripped" || true; } | wc -l)"
    [ "${hits:-0}" -gt 0 ] || continue
    printf '%s\t%s\t%s\n' "$name" "$primitive" "$hits" >>"$observed"
  done
done

# A refused primitive fails where it stands. The line is quoted so the failure
# names the thing rather than the file.
for primitive in "${order[@]}"; do
  [ "${class_of[$primitive]}" = refused ] || continue
  while IFS=$'\t' read -r name _ _; do
    [ -n "$name" ] || continue
    line="$(sed 's/^[[:space:]]*#.*$//' "$checks_dir/$name" |
      LC_ALL=C grep -nE -- "${pattern_of[$primitive]}" | head -1)"
    note "checks/$name uses $primitive, which no accounting admits and which nothing here watches: $line"
  done < <(awk -F'\t' -v l="$primitive" '$2 == l' "$observed")
done

# The accounted half, held to the table in both directions.
declared="$work/declared.tsv"
awk -F'\t' 'NF >= 4 && $1 !~ /^#/ && $1 != "" { printf "%s\t%s\t%s\n", $1, $2, $3 }' "$table" |
  LC_ALL=C sort >"$declared"
if [ ! -s "$declared" ]; then
  note "checks/read-primitives.tsv accounts for nothing, so the rule would pass by having no row to compare"
fi

for primitive in "${order[@]}"; do
  [ "${class_of[$primitive]}" = accounted ] || continue
  awk -F'\t' -v l="$primitive" '$2 == l' "$observed"
done | LC_ALL=C sort >"$work/accountable.tsv"

while IFS=$'\t' read -r name label count; do
  [ -n "$name" ] || continue
  note "checks/$name uses $label $count time(s) and checks/read-primitives.tsv accounts for no such use"
done < <(LC_ALL=C comm -23 "$work/accountable.tsv" "$declared")

while IFS=$'\t' read -r name label count; do
  [ -n "$name" ] || continue
  note "checks/read-primitives.tsv accounts for $label $count time(s) in checks/$name and the scan does not see it there"
done < <(LC_ALL=C comm -13 "$work/accountable.tsv" "$declared")

# The two tables against each other. A vocabulary row nothing exercises is stale
# and an accounted row for a primitive the vocabulary does not name is a rule
# nobody wrote down, which is the shape this whole sequence of reviews is about.
for primitive in "${order[@]}"; do
  [ "${class_of[$primitive]}" = accounted ] || continue
  if ! awk -F'\t' -v l="$primitive" '$2 == l { found = 1 } END { exit !found }' "$work/accountable.tsv"; then
    note "checks/read-primitive-vocabulary.tsv accounts for $primitive and nothing under checks/ uses it, so the row is stale"
  fi
done
while IFS=$'\t' read -r label; do
  [ -n "$label" ] || continue
  if [ "${class_of[$label]:-}" != accounted ]; then
    note "checks/read-primitives.tsv accounts for $label and checks/read-primitive-vocabulary.tsv does not name it as an accounted primitive"
  fi
done < <(cut -f2 "$declared" | LC_ALL=C sort -u)

# Every row's disposition has to be one this file names, and the reason has to be
# written. A row with no reason is a row nobody decided.
while IFS=$'\t' read -r name label count disposition reason; do
  [ -n "$name" ] || continue
  case "$name" in \#*) continue ;; esac
  case "$disposition" in
    watched | observation | local | digest) ;;
    *) note "checks/read-primitives.tsv gives checks/$name $label a disposition this checker does not define: $disposition" ;;
  esac
  if [ "${#reason}" -lt 20 ]; then
    note "checks/read-primitives.tsv records no reason for $label in checks/$name"
  fi
  if [ ! -f "$checks_dir/$name" ]; then
    note "checks/read-primitives.tsv accounts for a file that is not there: checks/$name"
  fi
  case "$count" in
    '' | *[!0-9]*) note "checks/read-primitives.tsv gives checks/$name $label a count that is not a number: $count" ;;
  esac
done <"$table"

# Where sourced code may come from. Two answers are admissible and no third one
# is. A file under checks/ is scanned above. Shell written into the scratch
# directory during a run is not a file at all until a check writes it, and what
# writes it is a model under checks/ — which is scanned, and whose
# `source-generated` row in the table names the generator. Anything else is a
# path this scan never saw, and the set of files scanned is the whole guarantee.
for name in "${code[@]}"; do
  while IFS= read -r line; do
    case "$line" in
      *checks_dir* | *'/checks/'* | *BASH_SOURCE* | *meta_work*) ;;
      *) note "checks/$name sources code from a path this scan never read: $line" ;;
    esac
  done < <(sed 's/^[[:space:]]*#.*$//' "$checks_dir/$name" |
    LC_ALL=C grep -nE '^[[:space:]]*(\.|source)[[:space:]]+["$/]' || true)
done

refused_count=0
accounted_count=0
for primitive in "${order[@]}"; do
  case "${class_of[$primitive]}" in
    refused) refused_count=$((refused_count + 1)) ;;
    accounted) accounted_count=$((accounted_count + 1)) ;;
  esac
done

accounted_uses="$(awk -F'\t' '{ n += $3 } END { print n + 0 }' "$work/accountable.tsv")"
occurrence_rows="$(wc -l <"$declared")"

# Published machinery figures must match the current scan. Changes are recorded
# through the milestone summary's explicit machinery amendment mechanism.
summary="$repo_root/evidence/m5/inventory-summary.json"
if [ ! -f "$summary" ]; then
  note "missing evidence/m5/inventory-summary.json, which publishes this scan's figures"
else
  declare -A computed=(
    [filesScanned]="${#code[@]}"
    [primitivesDeclared]="$((refused_count + accounted_count))"
    [refused]="$refused_count"
    [accounted]="$accounted_count"
    [accountedUses]="$accounted_uses"
    [occurrenceRows]="$occurrence_rows"
  )
  for field in filesScanned primitivesDeclared refused accounted accountedUses occurrenceRows; do
    published="$(jq -r --arg f "$field" '.primitiveScan[$f] // "absent"' "$summary")"
    if [ "$published" != "${computed[$field]}" ]; then
      note "this scan computes $field ${computed[$field]} and the milestone record publishes $published; amend the machinery figures explicitly"
    fi
  done
fi
limits+=("it holds the milestone's figures to this scan, which stops drift but is not a second count: changing the scan and amending the record together passes")


if [ ${#violations[@]} -ne 0 ]; then
  printf 'read primitive check failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  printf 'and what this scan cannot reach, which is unchanged by the failures above:\n' >&2
  printf -- '- %s\n' "${limits[@]}" >&2
  exit 1
fi

printf 'read primitives accounted: %s files under checks/ scanned, %s primitives declared (%s refused, %s accounted, each matched against its own probe), %s accounted uses over %s rows\n' \
  "${#code[@]}" "$((refused_count + accounted_count))" "$refused_count" "$accounted_count" \
  "$accounted_uses" "$occurrence_rows"
printf 'and what this scan cannot reach:\n'
printf -- '- %s\n' "${limits[@]}"
