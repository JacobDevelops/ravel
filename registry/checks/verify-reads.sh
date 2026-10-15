#!/usr/bin/env bash
# Holds every claim about what this workspace opens to what it was seen opening.
#
# Round 5's review found `derivation.reads` derived from filenames: which checks
# a producer sources, unioned with a hand-written phrase list per filename. It
# injected a reader of 1,113 class-file bodies into `checks/surfaces-model.sh`,
# touched no vocabulary entry and no record, and the gate exited 0 with every
# record still saying `decompiled: false`, `restrictedContentRead: false` and
# `implementationMethod: metadata-only`. That is the sentence which makes reading
# 235 restricted-licence artifacts lawful, so it has to be true rather than
# merely well-formed, and a table of phrases cannot make it true.
#
# So this check does not read a table. It runs each producer, watches the two
# things in this workspace that can open an archive, and judges what it saw:
#
#   toc          a listing was taken
#   declaration  a mod declaration a loader reads was read whole
#   signature    four bytes or fewer of an entry, which tells a container from a
#                payload and retains nothing
#   container    an entry that is an archive by name or by where it sits, read so
#                that it could be listed
#   body         anything else: the bytes of a class, a resource, a data file
#
# The first four are what metadata-only means. A `body` observation is a failure
# here whatever any record says, and it cannot be argued away by editing a table,
# because the classification is over the entry that was opened and the number of
# bytes that came back. That is the whole point: the round 5 injection produces
# 1,113 `body` observations and fails on the first one.
#
# Two further things are held to the observation rather than to prose. Each
# record's `decompiled` and `restrictedContentRead` are compared to what was
# seen, so the booleans stop being assertions; and the set of kinds a producer
# was seen performing has to equal the set its vocabulary entries claim, so a
# producer that quietly stops reading declarations fails as loudly as one that
# starts reading class files.
#
# The producers are run against a copy of this workspace in a temporary
# directory. Nothing committed is touched by the run, and the copy carries the
# checks verbatim, so an injected reader is copied along with everything else.
#
# The limit, stated rather than left to be found, and it is not the limit this
# file used to state. The old sentence said the gap was non-archive reading — the
# pinned manifest, the acquisition log, this workspace's own tracked files, the
# pack override tree — which is true and let a reader finish the paragraph
# believing archive reading was covered. It was the archive reading that was
# open. What is watched is four methods of one standard-library class and one
# binary on PATH. A reader that opens an artifact with the builtin and inflates
# its entries with `zlib` reaches no classification here, and neither does one
# that uses `zipfile` verbatim under an interpreter started with -S. Round 6's
# review did both, and this check printed that it had classified 1,448,401
# archive reads and seen no entry body while 50,357 class files were being
# written to disk.
#
# What closes that is not in this file. `checks/verify-primitives.sh` reads the
# premise this check acts on out of the code — every file under checks/, every
# read primitive it uses, refused outright or named in a table that checker owns
# — and `checks/read-observation.sh` shims the interpreter so the isolation flags
# fail at run time. This file checks the third thing: that what was observed
# through the two channels is metadata and nothing more. All three are needed and
# none of them is a proof. A hand that adds a reader and adds its table row in
# the same edit still passes.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/public-repo.sh
. "$checks_dir/public-repo.sh"
# shellcheck source=checks/read-observation.sh
. "$checks_dir/read-observation.sh"

vocabulary="$checks_dir/derivation-vocabulary.json"
inventory="$registry_root/inventory/${FIXTURE_ID}"
record="$registry_root/provenance/${FIXTURE_ID}.json"

for path in "$vocabulary" "$record"; do
  if [ ! -f "$path" ]; then
    printf 'read observation failed:\n- missing input: %s\n' "$path" >&2
    exit 1
  fi
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

violations=()
note() { violations+=("$1"); }

# A copy of this workspace, so a producer writes its record into the copy and
# the committed one is untouched. The checks are copied verbatim.
#
# What is left out is the history and the trees .gitignore reserves for build
# output and acquisition material. Nothing under any of them is tracked, so the
# sandbox is still this workspace; carrying them is what filled a root
# filesystem and killed this check mid-run, because the cargo target directory
# alone is tens of gigabytes and the copy lands in a temporary directory.
sandbox="$work/registry"
mkdir -p "$sandbox"
tar -C "$registry_root" \
  --exclude=.jj \
  --exclude=.git \
  --exclude=target \
  --exclude=artifacts \
  --exclude=extracted \
  --exclude=scratch \
  --exclude=./tests/fixtures/local \
  -cf - . | tar -C "$sandbox" -xf -
ln -s "$repo_root/tests" "$work/tests"

# Which entry points are watched. The five producers are watched because their
# records make claims; `checks/observed-recount.sh` is watched because the
# recount opens archives and belongs to no producer, which is how round 5's
# review found it extracting whole entries that no record mentions.
producers=()
while IFS= read -r producer; do
  [ -n "$producer" ] && producers+=("$producer")
done < <(jq -r '.records | to_entries | map(.value) | unique | .[]' "$vocabulary")
recount="checks/observed-recount.sh"
watched=("${producers[@]}" "$recount")

observation="$work/observed.tsv"
: >"$observation"
isolated="$work/isolated.tsv"
: >"$isolated"
scripts="$work/scripts.tsv"
: >"$scripts"

for producer in "${watched[@]}"; do
  if [ ! -f "$sandbox/$producer" ]; then
    note "the vocabulary names a producer that is not there: $producer"
    continue
  fi
  if ! (
    read_observation_begin "$work/watch"
    "$sandbox/$producer"
  ) >"$work/run.log" 2>&1; then
    note "$producer could not be run under observation: $(tail -1 "$work/run.log")"
    continue
  fi
  # A producer that opened no archive leaves an empty log, and that is a
  # reading rather than a failure: `checks/rebuild-references.sh` reads the
  # pinned manifest and nothing else. What it may not do is claim otherwise,
  # which the kinds comparison below decides.
  awk -F'\t' -v p="$producer" 'BEGIN { OFS = "\t" } $1 == "READ" { print p, $2, $3, $4, $5, $6 }' \
    "$work/watch/log/"* >>"$observation"
  # The interpreter shim's own line. An isolation flag means the sitecustomize
  # never loaded, so silence from the Python half of the watch is a fact about
  # the interpreter and not about the producer.
  awk -F'\t' -v p="$producer" 'BEGIN { OFS = "\t" } $1 == "ISOLATED" { print p, $2 }' \
    "$work/watch/log/python3.log" >>"$isolated"
  awk -F'\t' -v p="$producer" 'BEGIN { OFS = "\t" } $1 == "SCRIPT" { print p, $2 }' \
    "$work/watch/log/python3.log" >>"$scripts"
done

while IFS=$'\t' read -r producer argv; do
  [ -n "$producer" ] || continue
  note "$producer started an interpreter with an isolation flag, so the watch never loaded and what it read is unknown: $argv"
done < <(LC_ALL=C sort -u "$isolated")

# Which script the interpreter was pointed at. `checks/verify-primitives.sh`
# accounts for the invocation and cannot account for a variable argument, so this
# is where the argument is held: an interpreted script has to be a file under
# checks/ in the sandbox, which is the set that scan reads.
while IFS=$'\t' read -r producer script; do
  [ -n "$producer" ] || continue
  # Reconstructed from its own basename and required to be equal, because a
  # case glob's * matches a slash: `checks/*.py` admits
  # `checks/../../elsewhere/reader.py`, which is the one thing this pattern
  # claims to refuse.
  base="${script##*/}"
  case "$base" in
    *.py)
      case "$script" in
        "$sandbox/checks/$base" | "checks/$base") ;;
        *) note "$producer ran an interpreter over something no scan of checks/ ever read: $script" ;;
      esac
      ;;
    *) note "$producer ran an interpreter over something no scan of checks/ ever read: $script" ;;
  esac
done < <(LC_ALL=C sort -u "$scripts")

if [ ! -s "$observation" ]; then
  printf 'read observation failed:\n- no producer was observed reading anything, so this check proved nothing\n' >&2
  exit 1
fi

# Both wrappers attached. A watch that quietly did not attach reads exactly like
# a workspace that opened nothing, and round 5's review lost a whole probe to
# precisely that — a gate that never ran, read as a green baseline. This
# workspace opens archives through Python and through unzip, so seeing neither
# is a failure of the observation rather than a fact about the producers.
for tool in zipfile unzip; do
  if ! awk -F'\t' -v t="$tool" '$2 == t { found = 1 } END { exit !found }' "$observation"; then
    note "the observation saw no read through $tool, so the wrapper for it did not attach"
  fi
done

# What was seen, classified over the entry and the byte count and nothing else.
awk -F'\t' -v kinds_out="$work/kinds.tsv" -v bodies_out="$work/bodies.tsv" '
  function declaration(name) {
    return (name == "META-INF/neoforge.mods.toml" || name == "META-INF/mods.toml")
  }
  # An archive by name, or by sitting where a loader looks for one. Both are
  # properties of the entry rather than of any readers rule about it.
  function container(name,   lower) {
    lower = tolower(name)
    if (lower ~ /\.(jar|zip|jmod|war|ear|aar|apk|zipx|placebo)$/) return 1
    return (name ~ /^META-INF\/jarjar\//) || (name ~ /^META-INF\/jars\//)
  }
  {
    producer = $1; member = $4; extent = $5; count = $6
    if (extent == "toc") { seen[producer, "toc"] += count; next }
    if (extent == "unrecognised") {
      printf "%s\t%s\t%s\t%s\n", producer, "unrecognised", member, extent >bodies_out
      seen[producer, "unrecognised"] += count
      next
    }
    bytes = extent
    sub(/^bytes:/, "", bytes)
    if (bytes == "full") bytes = -1
    else bytes = bytes + 0
    if (declaration(member))    { seen[producer, "declaration"] += count; next }
    if (container(member))      { seen[producer, "container"] += count; next }
    if (bytes >= 0 && bytes <= 4) { seen[producer, "signature"] += count; next }
    seen[producer, "body"] += count
    printf "%s\t%s\t%s\t%s\n", producer, "body", member, extent >bodies_out
  }
  END {
    for (k in seen) {
      split(k, part, SUBSEP)
      printf "%s\t%s\t%d\n", part[1], part[2], seen[k] >kinds_out
    }
  }' "$observation"
[ -f "$work/bodies.tsv" ] || : >"$work/bodies.tsv"
[ -f "$work/kinds.tsv" ] || : >"$work/kinds.tsv"

# A read of anything that is not a listing, a declaration, a container or four
# bytes. No table can excuse one.
while IFS=$'\t' read -r producer kind member extent; do
  [ -n "$producer" ] || continue
  if [ "$kind" = "unrecognised" ]; then
    note "$producer opened an archive in a way the observation cannot classify, so what it read is not known: $member $extent"
  else
    note "$producer read $extent of $member, which is not a listing, a declaration, a container or a four-byte probe"
  fi
done < <(sort -u "$work/bodies.tsv" | sed -n '1,20p')

body_total="$(awk -F'\t' '$2 == "body" || $2 == "unrecognised" { n += $3 } END { print n + 0 }' "$work/kinds.tsv")"

# The booleans in every record, against what was seen rather than against prose.
decompiled_seen=false
restricted_seen=false
if awk -F'\t' '$2 == "body" { print $3 }' "$work/bodies.tsv" | grep -qi '\.class$'; then
  decompiled_seen=true
fi
if [ "$body_total" -gt 0 ]; then
  restricted_seen=true
fi

for file in references graph surfaces rights plans; do
  [ -f "$inventory/$file.json" ] || continue
  for pair in "decompiled:$decompiled_seen" "restrictedContentRead:$restricted_seen"; do
    key="${pair%%:*}"
    seen="${pair##*:}"
    claimed="$(jq -r --arg k "$key" '.derivation[$k] // false' "$inventory/$file.json")"
    if [ "$claimed" != "$seen" ]; then
      note "$file.json declares $key: $claimed and the producers were observed at $key: $seen"
    fi
  done
done

method="$(jq -r '.implementationMethod' "$record")"
if [ "$method" = "metadata-only" ] && [ "$body_total" -gt 0 ]; then
  note "the record claims metadata-only and $body_total reads of entry bodies were observed"
fi

# The kinds a producer was seen performing, against the kinds its vocabulary
# entries claim. This is the other direction: omitting an input fails here as
# much as inventing one, and both are decided from the run rather than the list.
# shellcheck source=checks/derivation-reads.sh
. "$checks_dir/derivation-reads.sh"
for producer in "${watched[@]}"; do
  [ -f "$sandbox/$producer" ] || continue
  if ! phrases="$(derivation_reads "$registry_root/$producer" 2>&1)"; then
    note "$producer: what it reads could not be derived: $phrases"
    continue
  fi
  expected="$(printf '%s' "$phrases" | jq -r --slurpfile vocab "$vocabulary" '
    ($vocab[0].reads) as $r
    | map(. as $p | ($r[$p].observedAs // []))
    | add // []
    | unique | .[]' | LC_ALL=C sort -u)"
  actual="$(awk -F'\t' -v p="$producer" '$1 == p && $2 != "body" && $2 != "unrecognised" { print $2 }' \
    "$work/kinds.tsv" | LC_ALL=C sort -u)"
  if [ "$expected" != "$actual" ]; then
    note "$producer was observed performing [$(echo "$actual" | tr '\n' ' ')] and its vocabulary entries claim [$(echo "$expected" | tr '\n' ' ')]"
  fi
done

if [ ${#violations[@]} -ne 0 ]; then
  printf 'read observation failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

# Every figure here is computed. `0 entry bodies read` used to be a literal,
# true only because a body observation raises a violation first and so makes the
# line unreachable with a non-zero count — a figure asserted rather than derived,
# in a workspace whose README says no figure is repeated where nothing would keep
# it true. And `producers watched` used to count six, which is the five producers
# plus the surface recount that the comment above is careful to say is not a
# producer.
printf 'reads observed: %s producers and the surface recount watched, %s archive reads classified, %s entry bodies read\n' \
  "${#producers[@]}" \
  "$(awk -F'\t' '{ n += $3 } END { print n + 0 }' "$work/kinds.tsv")" \
  "$body_total"
