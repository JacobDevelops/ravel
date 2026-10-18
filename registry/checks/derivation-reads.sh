#!/usr/bin/env bash
# Computes a record's derivation.reads list from what its producer actually runs.
#
# The list used to be written by hand into each rebuild script and checked by
# nothing, which round 4's review found twice over: a record could claim it had
# disassembled every class and stay green, and rights.json omitted the input
# that decides three of its seven statuses. So the list is no longer written; it
# is derived. Walking the `. "$checks_dir/..."` lines of a producer gives the
# set of checks it runs, `checks/derivation-vocabulary.json` says what each of
# those reads, and the union is the answer. Adding an input to a model without
# naming it in the vocabulary fails here; naming one a producer does not run
# fails here; and the record's committed list is required to equal this, so a
# hand-edited reads list fails in verify-provenance.sh.
#
# Sourcing this defines derivation_reads, which prints a JSON array.
set -euo pipefail

derivation_reads() {
  local start="$1"
  local dir vocab name child
  dir="$(cd "$(dirname "$start")" && pwd)"
  vocab="$dir/derivation-vocabulary.json"

  if [ ! -f "$vocab" ]; then
    echo "derivation vocabulary missing: checks/derivation-vocabulary.json" >&2
    return 1
  fi

  local -a queue seen
  queue=("$(basename "$start")")
  seen=()
  while [ ${#queue[@]} -gt 0 ]; do
    name="${queue[0]}"
    queue=("${queue[@]:1}")
    if printf '%s\n' "${seen[@]+"${seen[@]}"}" | grep -qxF "$name"; then
      continue
    fi
    seen+=("$name")
    if [ ! -f "$dir/$name" ]; then
      echo "a producer sources a check that is not there: checks/$name" >&2
      return 1
    fi
    while IFS= read -r child; do
      [ -n "$child" ] && queue+=("$child")
    done < <(grep -oE '^\. ".*/[a-z0-9-]+\.sh"' "$dir/$name" |
      sed -E 's#.*/([a-z0-9-]+\.sh)"#\1#')
  done

  printf 'checks/%s\n' "${seen[@]}" |
    jq -R -s --slurpfile vocab "$vocab" '
      ($vocab[0]) as $v
      | split("\n") | map(select(length > 0))
      | (map(. as $n | select(($v.producers | has($n)) | not))) as $unknown
      | if ($unknown | length) > 0
        then error("the derivation vocabulary does not describe what these read: \($unknown | join(", "))")
        else map(. as $n | $v.producers[$n][]) | unique end'
}
