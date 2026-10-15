#!/usr/bin/env bash
# Holds the provenance record to the inventory it is provenance for.
#
# M5-W7's review found this record asserting things M5-W3 to M5-W5 had already
# falsified — that no artifact had been acquired or examined, and that behaviour
# rights were unresolved pending artifacts the workspace did not hold — while
# every check in the workspace passed. Nothing was wrong with the checks: no
# check read the record. It was validated against the public JSON schema and
# mined for its status vocabulary, and a schema says what shape a statement has,
# never whether it is true.
#
# A record that only has to be well-formed is not evidence, and this one is the
# artifact a later audit would trust first. So every statement it makes about the
# fixture is recomputed here from the fixture or from the inventory:
#
#   - the artifact set it binds itself to, digested from the files on disk;
#   - the two fixture digests, recomputed from the fixture on disk;
#   - every rights status, as the worst of the 235 per-artifact verdicts M5-W5
#     recorded for that surface, because a fixture-level status over artifacts
#     that disagree can only be the strictest of them;
#   - every count written into a basis, matched against the verdicts it counts,
#     so the prose goes stale loudly rather than quietly;
#   - implementationMethod, against what every producer in the workspace declares
#     it read;
#   - the three surfaces the per-artifact triage does not reach — quests, scripts
#     and other — each against what this workspace's own tree and inventory hold,
#     because round 2's review rewrote one of their bases into the opposite of
#     the milestone's central privacy claim and the gate still passed;
#   - the attribution notices, against the subject they credit and every
#     distributor the triage names, because they too could be deleted whole.
#
# A statement this file cannot check is a statement that does not belong in the
# record.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/public-repo.sh
. "$checks_dir/public-repo.sh"
mode="${1:---fixture}"
case "$mode" in --offline|--fixture) ;; *) echo "usage: $0 [--offline|--fixture]" >&2; exit 2 ;; esac
if [ "$mode" = "--fixture" ]; then
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
else
  artifact_work="$(mktemp -d)"
  trap 'rm -rf "$artifact_work"' EXIT
  artifact_tsv="$artifact_work/artifacts.tsv"
  jq -r '.artifacts[] | [.class, .artifact] | @tsv' \
    "$registry_root/inventory/$FIXTURE_ID/rights.json" >"$artifact_tsv"
  artifact_set_sha="$(jq -r '.fixture.artifactSet.sha256' "$registry_root/inventory/$FIXTURE_ID/rights.json")"
  echo "SKIPPED: provenance acquired-byte reconciliation (--offline); committed record consistency remains required" >&2
fi
# shellcheck source=checks/derivation-reads.sh
. "$checks_dir/derivation-reads.sh"

inventory="$registry_root/inventory/${FIXTURE_ID}"
record="$registry_root/provenance/${FIXTURE_ID}.json"
manifest="$repo_root/tests/fixtures/local/${FIXTURE_ID}/extracted/manifest.json"
archive="$repo_root/tests/fixtures/local/${FIXTURE_ID}/source/${FIXTURE_ID}.zip"

violations=()
note() { violations+=("$1"); }

required=("$record" "$inventory/references.json" "$inventory/graph.json"
  "$inventory/surfaces.json" "$inventory/rights.json" "$inventory/plans.json")
if [ "$mode" = "--fixture" ]; then required+=("$manifest" "$archive"); fi
for path in "${required[@]}"; do
  if [ ! -f "$path" ]; then
    printf 'provenance verification failed:\n- missing input: %s\n' "$path" >&2
    exit 1
  fi
done

fixture_id="$(jq -r '.subject.id' "$record")"

# The subject is the fixture every inventory record says it describes.
for file in references graph surfaces rights plans; do
  recorded="$(jq -r '.fixture.id' "$inventory/$file.json")"
  if [ "$recorded" != "$fixture_id" ]; then
    note "the record is provenance for $fixture_id but $file.json describes $recorded"
  fi
done

# The digests, recomputed rather than read back.
declare -A digest
if [ "$mode" = "--fixture" ]; then
  manifest_sha="$(sha256sum "$manifest" | cut -d' ' -f1)"
  archive_sha="$(sha256sum "$archive" | cut -d' ' -f1)"
else
  manifest_sha="$(jq -r '.fixture.manifest.sha256' "$inventory/references.json")"
  archive_sha="$(jq -r '.fixture.archive.sha256' "$inventory/references.json")"
fi
digest["sha256:$manifest_sha"]="the pinned client manifest"
digest["sha256:$archive_sha"]="the fixture archive"
digest["sha256:$artifact_set_sha"]="the acquired artifact set"
for revision in "${!digest[@]}"; do
  if [ "$(jq -r --arg r "$revision" '[.sources[] | select(.revision == $r)] | length' "$record")" -eq 0 ]; then
    note "no source binds ${digest[$revision]}: $revision is not among the recorded revisions"
  fi
done

# The artifact set the record binds itself to is the set on disk, counted by
# class the way artifact-set.sh counts it.
artifact_count="$(wc -l <"$artifact_tsv")"
mod_count="$(awk -F'\t' '$1 == "mod"' "$artifact_tsv" | wc -l)"
shader_count="$(awk -F'\t' '$1 == "shaderpack"' "$artifact_tsv" | wc -l)"
set_locator="$(jq -r --arg r "sha256:$artifact_set_sha" \
  '[.sources[] | select(.revision == $r) | .locator] | first // ""' "$record")"
for pair in "$artifact_count:files" "$mod_count:mods" "$shader_count:shaderpacks"; do
  if [[ "$set_locator" != *"${pair%%:*} ${pair##*:}"* ]]; then
    note "the artifact-set source does not say ${pair%%:*} ${pair##*:}; the acquired set holds that many"
  fi
done

# Every rights status, as the worst of the per-artifact verdicts.
jq -r '
  { "not-applicable": 0, cleared: 1, "permission-required": 2, unresolved: 3, prohibited: 4 } as $rank
  | [.artifacts[].rights[]]
  | group_by(.surface)
  | map({ surface: .[0].surface, status: (max_by($rank[.status]) | .status) })
  | .[] | [.surface, .status] | @tsv' "$inventory/rights.json" >"$artifact_work/rollup.tsv"

if [ ! -s "$artifact_work/rollup.tsv" ]; then
  note "the rights triage yields no per-surface verdict to hold the record to"
fi

while IFS=$'\t' read -r surface expected; do
  actual="$(jq -r --arg s "$surface" '[.rights[] | select(.surface == $s) | .status] | first // "absent"' "$record")"
  if [ "$actual" = "absent" ]; then
    note "the record states no status for $surface, which the triage decides for every artifact"
  elif [ "$actual" != "$expected" ]; then
    note "the record states $surface as $actual; the worst of the 235 per-artifact verdicts is $expected"
  fi
done <"$artifact_work/rollup.tsv"

# Every count written into a basis. A number beside a status word is a claim
# about the triage, so it is compared to the triage rather than read.
jq -r '.rights[] | [.surface, (.basis // "")] | @tsv' "$record" >"$artifact_work/basis.tsv"
jq -r '.artifacts[].rights[] | [.surface, .status] | @tsv' "$inventory/rights.json" |
  sort | uniq -c | awk '{ print $2 "\t" $3 "\t" $1 }' >"$artifact_work/counts.tsv"

while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(awk -F'\t' '
  FILENAME == counts { seen[$1 SUBSEP $2] = $3; next }
  {
    surface = $1; basis = $2
    while (match(basis, /[0-9]+ (cleared|permission-required|prohibited|unresolved|not-applicable)/)) {
      claim = substr(basis, RSTART, RLENGTH)
      basis = substr(basis, RSTART + RLENGTH)
      split(claim, part, " ")
      key = surface SUBSEP part[2]
      actual = (key in seen) ? seen[key] : 0
      if (actual != part[1] + 0)
        printf "the %s basis claims %s %s; the triage holds %d\n", surface, part[1], part[2], actual
      counted[surface] = 1
    }
  }
  END {
    split("code textures models sounds branding", decided, " ")
    for (i in decided)
      if (!(decided[i] in counted))
        printf "the %s basis carries no count, so nothing in it is held to the triage\n", decided[i]
  }' counts="$artifact_work/counts.tsv" "$artifact_work/counts.tsv" "$artifact_work/basis.tsv")

# The surfaces the triage does not decide.
#
# The rollup above reaches every surface the per-artifact triage records, and
# three more are stated in this record. Round 2's review rewrote the quests basis
# into a claim that ATM's quests had been extracted and transcribed into this
# workspace — the exact opposite of the milestone's central privacy claim — and
# the whole gate still exited 0, because nothing read those three. A statement
# this file cannot check does not belong in the record, so each one is bound
# below and a fourth would fail here rather than arrive unbound.
while IFS= read -r surface; do
  [ -n "$surface" ] || continue
  if ! cut -f1 "$artifact_work/rollup.tsv" | grep -qxF "$surface"; then
    case "$surface" in
      quests | scripts | other) ;;
      *) note "the record states $surface, which the triage does not decide and nothing here binds" ;;
    esac
  fi
done < <(jq -r '.rights[].surface' "$record")

# quests and scripts are ATM-owned material this workspace never holds, so what
# their basis claims is a fact about this tree rather than about the artifacts,
# and this tree is where it is checked. The literals are bracketed for the reason
# verify-boundary.sh brackets its own: the pattern still matches the material and
# no longer matches its own source.
overrides_held="$(LC_ALL=C grep -rlaE '[k]ubejs/(startup|server|client)_scripts/|[k]ubejs/dynamic_data/|[f]tbquests/' \
  --exclude-dir=.jj --exclude-dir=.git --exclude-dir=target --exclude-dir=mods -- "$registry_root" || true)"
for surface in quests scripts; do
  status="$(jq -r --arg s "$surface" '[.rights[] | select(.surface == $s) | .status] | first // "absent"' "$record")"
  basis="$(jq -r --arg s "$surface" '[.rights[] | select(.surface == $s) | .basis] | first // ""' "$record")"
  if [ -n "$overrides_held" ]; then
    held_here="$(printf '%s' "$overrides_held" | head -1)"
    note "the $surface basis says the pack's override material is never extracted, tracked or transcribed, and this workspace holds it: ${held_here#"$registry_root/"}"
  fi
  if [ "$status" != "prohibited" ]; then
    note "the record states $surface as $status; this workspace holds none of that material, so no reading of it is available to be cleared"
  fi
  if [[ "$basis" != *"never extracted, tracked, or transcribed"* ]]; then
    note "the $surface basis does not say the material is never extracted, tracked, or transcribed, which is what this workspace's own tree holds"
  fi
done

# other is the numeric project and file identifiers. What makes them citable is
# that they are numbers out of a distribution platform's catalogue rather than
# anyone's expression, so the record is held to the inventory carrying numbers.
other_status="$(jq -r '[.rights[] | select(.surface == "other") | .status] | first // "absent"' "$record")"
unnumbered="$(jq '[.artifacts[].reference | select((.projectId | type) != "number" or (.fileId | type) != "number")] | length' "$inventory/rights.json")"
if [ "$unnumbered" -ne 0 ]; then
  note "the other basis says the recorded references are numeric identifiers; $unnumbered of them are not"
elif [ "$other_status" != "cleared" ]; then
  note "the record states other as $other_status while every recorded reference is a numeric catalogue identifier"
fi

# Attribution. The notices are the whole reason the block exists, and until now
# every one of them could be deleted without failing anything: the public schema
# requires the key and sets no minimum, and no check read it. So the block is
# held to the subject it credits and to every distributor the triage names.
author="$(jq -r '.subject.originalAuthor' "$record")"
project="$(jq -r '.subject.originalProject' "$record")"
if [ "$(jq '[.attribution[] | select((.name // "") != "" and (.notice // "") != "" and (.url // "") != "")] | length' "$record")" \
  -ne "$(jq '.attribution | length' "$record")" ]; then
  note "an attribution entry is missing a name, a notice or a url"
fi
if ! jq -e --arg a "$author" --arg p "$project" \
  '[.attribution[] | select(.name == $a and (.notice | contains($p)))] | length > 0' "$record" >/dev/null; then
  note "the record names $author as the author of $project and carries no attribution notice crediting them for it"
fi
while IFS= read -r channel; do
  distributor="${channel#automated-}"
  [ "$distributor" = "$channel" ] && continue
  if ! jq -e --arg d "$distributor" \
    '[.attribution[] | select((.name | ascii_downcase) == $d)] | length > 0' "$record" >/dev/null; then
    note "the triage records artifacts acquired through $distributor and the record credits no such distributor"
  fi
done < <(jq -r '[.artifacts[].acquisition] | unique | .[]' "$inventory/rights.json")

# derivation.reads, against the vocabulary that owns it.
#
# Round 4's review replaced "archive table of contents" with "every class file,
# fully disassembled" in two records and the gate exited 0, because nothing read
# the prose. The list is a vocabulary now, and it is not merely a membership
# test: it has to equal the union of what every check the record's producer runs
# declares it reads, so omitting an input is as much a failure as inventing one.
# That is the other half of the same finding — rights.json decided three of its
# seven statuses from the behaviour-surface counts and said it read no such
# thing.
vocabulary="$checks_dir/derivation-vocabulary.json"
if [ ! -f "$vocabulary" ]; then
  printf 'provenance verification failed:\n- missing input: checks/derivation-vocabulary.json\n' >&2
  exit 1
fi

method="$(jq -r '.implementationMethod' "$record")"
for file in references graph surfaces rights plans; do
  producer="$(jq -r --arg f "$file" '.records[$f] // ""' "$vocabulary")"
  if [ -z "$producer" ]; then
    note "the derivation vocabulary names no producer for $file.json"
    continue
  fi
  if ! expected="$(derivation_reads "$registry_root/$producer" 2>&1)"; then
    note "$file.json: the reads list could not be derived from $producer: $expected"
    continue
  fi
  actual="$(jq -c '.derivation.reads | sort' "$inventory/$file.json")"
  if [ "$actual" != "$(printf '%s' "$expected" | jq -c 'sort')" ]; then
    note "$file.json declares reading $actual and $producer reads $(printf '%s' "$expected" | jq -c 'sort')"
  fi

  # What the declared phrases imply, against the booleans beside them and
  # against the method the whole record claims.
  while IFS= read -r bad; do
    [ -n "$bad" ] && note "$file.json: $bad"
  done < <(jq -r --slurpfile vocab "$vocabulary" --arg method "$method" '
    ($vocab[0].reads) as $r
    | .derivation as $d
    | ($d.reads | map(. as $p | $r[$p] // {})) as $implied
    | [ ($d.reads[] | . as $p | select(($r | has($p)) | not)
          | "declares reading \($p), which the derivation vocabulary does not admit"),
        (select(($implied | map(.decompiled // false) | any) != ($d.decompiled // false))
          | "declares decompiled: \($d.decompiled) beside a reads list that says otherwise"),
        (select(($implied | map(.restrictedContentRead // false) | any) != ($d.restrictedContentRead // false))
          | "declares restrictedContentRead: \($d.restrictedContentRead) beside a reads list that says otherwise"),
        (select($method == "metadata-only" and ($implied | map(.readsSource // false) | any))
          | "reads a mod source under an implementation method of metadata-only") ]
    | .[]' "$inventory/$file.json")
done

# implementationMethod, against what every producer declares it read. Nothing in
# this workspace decompiles anything or opens restricted content, and the day one
# of them does, metadata-only stops being the truth here.
if [ "$method" = "metadata-only" ]; then
  for file in references graph surfaces rights plans; do
    if [ "$(jq -r '.derivation.decompiled // false' "$inventory/$file.json")" != "false" ]; then
      note "the record claims metadata-only but $file.json declares that something was decompiled"
    fi
    if [ "$(jq -r '.derivation.restrictedContentRead' "$inventory/$file.json")" != "false" ]; then
      note "the record claims metadata-only but $file.json declares that restricted content was read"
    fi
  done
else
  note "the record claims an implementation method of $method; nothing in this workspace implements anything from the fixture yet"
fi

# Publication. The record may not be more permissive than the triage it rests on.
cleared="$(jq -r '.totals.publicationCleared' "$inventory/rights.json")"
triaged="$(jq -r '.totals.artifacts' "$inventory/rights.json")"
status="$(jq -r '.publication.status' "$record")"
notes="$(jq -r '.publication.notes // ""' "$record")"
if [ "$cleared" -eq 0 ] && [ "$status" = "cleared" ]; then
  note "the record is cleared for publication while the triage clears none of the $triaged artifacts"
fi
if [ "$cleared" -eq 0 ] && [[ "$notes" != *"none of the $triaged artifacts"* ]]; then
  note "the publication notes do not say that none of the $triaged artifacts is cleared, which is what the triage holds"
fi
if [ "$status" != "cleared" ] &&
  [ "$(jq -r '[.rights[] | select(.status == "unresolved" or .status == "prohibited")] | length' "$record")" -eq 0 ]; then
  note "the record withholds publication while no surface it states is unresolved or prohibited"
fi

# Wave provenance. The fixture-level record above is provenance for one
# fixture; a wave under waves/ names entries drawn from many, and until now
# nothing read provenance/<wave>/<id>.json at all -- a wave entry with no
# record, or one missing a field, passed the gate silently. Every entry in
# every wave manifest is held to carrying one, with the same statement shape
# the fixture-level record above is held to: a method, a publication status,
# a rights status per surface drawn from the same status vocabulary, and at
# least one source-code source with a revision and a digest.
rights_status_vocab="not-applicable cleared permission-required unresolved prohibited"
wave_count=0
wave_entry_count=0
for wave_file in "$registry_root"/waves/*.json; do
  [ -f "$wave_file" ] || continue
  # A wave manifest declares manifestKind; waves/routing.json is the selector's
  # own working table over every plan row and carries no such field, so it is
  # not a wave and names no provenance directory of its own.
  jq -e '.manifestKind' "$wave_file" >/dev/null 2>&1 || continue
  wave_count=$((wave_count + 1))
  wave_name="$(basename "$wave_file" .json)"
  wave_prov_dir="$registry_root/provenance/$wave_name"

  while IFS= read -r id; do
    [ -n "$id" ] || continue
    wave_entry_count=$((wave_entry_count + 1))
    rec="$wave_prov_dir/$id.json"

    if [ ! -f "$rec" ]; then
      note "wave $wave_name entry $id has no provenance record: provenance/$wave_name/$id.json"
      continue
    fi
    if ! jq -e . "$rec" >/dev/null 2>&1; then
      note "provenance/$wave_name/$id.json is not valid JSON"
      continue
    fi

    if [ -z "$(jq -r '.implementationMethod // ""' "$rec")" ]; then
      note "provenance/$wave_name/$id.json carries no implementationMethod"
    fi
    if [ -z "$(jq -r '.publication.status // ""' "$rec")" ]; then
      note "provenance/$wave_name/$id.json carries no publication.status"
    fi

    if [ "$(jq '[.rights[]?] | length' "$rec")" -eq 0 ]; then
      note "provenance/$wave_name/$id.json carries no rights entries"
    fi
    while IFS=$'\t' read -r surface rstatus; do
      [ -n "$surface" ] || continue
      ok=0
      for v in $rights_status_vocab; do
        [ "$rstatus" = "$v" ] && ok=1 && break
      done
      if [ "$ok" -ne 1 ]; then
        note "provenance/$wave_name/$id.json: surface $surface carries status '$rstatus', outside the rights status vocabulary"
      fi
    done < <(jq -r '.rights[]? | [.surface, (.status // "")] | @tsv' "$rec")

    entry_method="$(jq -r '.implementationMethod // ""' "$rec")"
    if [ "$entry_method" = "black-box" ]; then
      if jq -e '[.sources[]? | select(.category == "source-code")] | length > 0' "$rec" >/dev/null 2>&1; then
        note "provenance/$wave_name/$id.json declares implementationMethod black-box and carries a source-code source; a black-box record read no source and must not claim one"
      fi
      if ! jq -e '[.sources[]? | select(.category == "compiled-artifact" and ((.revision // "") | test("[0-9a-fA-F]{64,}")))] | length > 0' \
        "$rec" >/dev/null 2>&1; then
        note "provenance/$wave_name/$id.json carries no compiled-artifact source with a revision and a digest"
      fi
    elif ! jq -e '[.sources[]? | select(.category == "source-code" and ((.revision // "") | test("[0-9a-fA-F]{64,}")))] | length > 0' \
      "$rec" >/dev/null 2>&1; then
      note "provenance/$wave_name/$id.json carries no source-code source with a revision and a digest"
    fi
  done < <(jq -r '.entries[].canonicalId' "$wave_file")
done

if [ ${#violations[@]} -ne 0 ]; then
  printf 'provenance verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

printf 'provenance reconciles: %s rights surfaces held to %s triaged artifacts, %s digests bound; %s wave entries across %s waves carry a complete record\n' \
  "$(jq '.rights | length' "$record")" "$triaged" "${#digest[@]}" "$wave_entry_count" "$wave_count"
