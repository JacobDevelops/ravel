#!/usr/bin/env bash
# Holds the SDK capability proposals to the inventory, to the public SDK, and to
# a closed lexicon.
#
# The consumer for M5-W7. It recomputes the proposals and requires the committed
# file to be them, and then it asks the questions a recomputation cannot answer.
#
# Two of those questions are the milestone's:
#
# Generality. A capability proposal is a statement about Ravel, so it has to be
# carried by enough of the inventory that no single entry could have produced it.
# The motivating entry set is recomputed here from the committed surfaces by an
# expression that shares nothing with the model's, so a proposal cannot cite an
# entry that does not carry the family it claims, and the count is required to
# clear the published threshold rather than merely to look large.
#
# Public safety. These records are meant to be published, and prose is a leak
# shape nothing earlier in M5 committed. The rule here is therefore positive
# rather than prohibitive: every word in every proposal file has to appear in
# checks/proposals-lexicon.txt, and that lexicon is required to share nothing
# with the identifiers the private inventory knows. An upstream name cannot
# arrive by being overlooked, because arriving at all means arriving as a word
# nobody published; and it cannot arrive by being added to the lexicon, because
# the lexicon is checked against the same identifier set. The two halves close
# each other, which is why neither is a list of forbidden words.
#
# The third question is whether a gap is a gap, or was one and was closed. Each
# proposal names an anchor — the module a shipped answer would occupy — and the
# borders it sits between. Every border is required to be a module of the public
# SDK, so a proposal cannot claim the SDK is emptier than it is.
#
# The anchor used to be required absent, full stop, on the reasoning that a
# capability which ships later should stop passing here rather than quietly stay
# a gap. That was right about the failure and wrong about where it lands. M5's
# finding is that these capabilities were absent when M5 measured, which is
# permanently true and is bound as material by the public M5 evidence; a gap
# being resolved is the expected outcome and not a defect in the finding, and
# there was no way to say so. The frozen record carried a lifecycle it could not
# move.
#
# So the finding stays frozen and the disposition lives in
# resolutions/sdk-gaps.json, which M6 owns and which may grow. What ships is read
# out of the public SDK tree; who resolved it is read out of the ledger; neither
# is generated from the other, so they can disagree, and each direction is a
# failure:
#
#   * an anchor that now ships with no ledger row is a gap closed by nobody;
#   * a ledger row whose anchor does not ship is a resolution nobody delivered;
#   * a ledger row naming a proposal that does not exist is a resolution of
#     nothing;
#   * a ledger row naming no work package is a claim with no author.
#
# publicSurface.shipped is superseded by all of that. It is M5's measurement,
# it is false in every row, it is frozen false twice over — the recomputation at
# the foot of this file writes it false and the record is bound by digest — and
# nothing below reads it as the answer to whether a capability ships today.
#
# This reads the committed inventory rather than the fixture. What a proposal
# rests on was established and gated by M5-W4 and M5-W6, and reopening the
# archives would only add another chance to read something.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
registry_root="$(cd "$checks_dir/.." && pwd)"
# shellcheck source=checks/public-repo.sh
. "$checks_dir/public-repo.sh"
# shellcheck source=checks/proposals-model.sh
. "$checks_dir/proposals-model.sh"

for tool in awk comm cut diff grep sed sort tr; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "required tool missing: $tool" >&2
    exit 1
  fi
done

records="$registry_root/proposals/sdk-gaps.json"
# These are M5 material, frozen against the pack M5 measured, so they are recomputed from that pack and not from whichever pack is pinned now.
FIXTURE_ID="$(jq -r '.fixture.id' "$records")"
inventory="$registry_root/inventory/${FIXTURE_ID}"
post_m5_records="$registry_root/proposals/sdk-gaps-m6.json"
ledger="$registry_root/resolutions/sdk-gaps.json"
vocabulary="$checks_dir/proposals-vocabulary.json"
lexicon="$checks_dir/proposals-lexicon.txt"
sdk_src="$repo_root/crates/ravel-sdk/src"

for path in "$records" "$post_m5_records" "$ledger" "$vocabulary" "$lexicon" \
  "$inventory/surfaces.json" "$inventory/plans.json" "$inventory/graph.json"; do
  if [ ! -f "$path" ]; then
    printf 'proposal verification failed:\n- missing input: %s\n' "${path#"$registry_root/"}" >&2
    exit 1
  fi
done
if [ ! -d "$sdk_src" ]; then
  printf 'proposal verification failed:\n- the SDK source is missing: %s\n' \
    "${sdk_src#"$repo_root/"}" >&2
  exit 1
fi

eval "$(proposals_model)"
trap 'rm -rf "$meta_work"' EXIT

violations=()
note() { violations+=("$1"); }

jq -s '{proposals: (.[0].proposals + [.[1].gaps[] | {id}])}' \
  "$records" "$post_m5_records" >"$meta_work/all-proposals.json"
all_records="$meta_work/all-proposals.json"

# The files a proposal set consists of. Every public-safety rule below runs over
# exactly this list, so a file added to the tree without being added here is a
# file the lexicon never sees — which is why the list is derived rather than
# written out.
mapfile -t proposal_files < <(
  {
    printf '%s\n' "$records" "$vocabulary" "$lexicon"
    find -L "$registry_root/proposals" -type f | sort
  } | sort -u
)

# ---------------------------------------------------------------------------
# The fixture the proposals are filed against.
# ---------------------------------------------------------------------------
if [ "$(jq -r '.fixture.inventoryDigest' "$records")" != "$inventory_sha" ]; then
  note "the proposals are filed against a different artifact set than the inventory they cite"
fi

# The vocabulary is the tracked file, copied rather than restated.
if ! diff -q <(jq -S . "$vocabulary") <(jq -S '.vocabulary' "$records") >/dev/null; then
  note "the recorded vocabulary is not checks/proposals-vocabulary.json"
fi

# ---------------------------------------------------------------------------
# Successor proposal schemas. M5 remains on its inventory-derived model above;
# implementation proof is a distinct source and is dispatched by its own schema.
# ---------------------------------------------------------------------------
post_m5_schema="$(jq -r '.schema // ""' "$post_m5_records")"
case "$post_m5_schema" in
  implementation-discovered-sdk-gap-list/v1)
    # A gap's own words are pinned here once it has been observed, so that a
    # later edit cannot restate what was found at a revision that has since
    # moved on. The pin is per gap rather than per file, because this file
    # exists to grow: holding every gap to the first gap's prose would have made
    # a second one unfileable. A gap with no pin yet is held to the shape below,
    # which is what a pin is checked against before it is written.
    #
    # Exact equality is what the four ServiceRefused/FailureKind/LifecycleState/
    # ServiceUnavailable substring rules used to approximate for RVL-GAP-07, and
    # is strictly stronger than they were, so they are not restated beside it.
    pins="$meta_work/post-m5-pins.json"
    jq -n \
      --arg dw_07 "Implementing RVL-097's integration discovery, which must skip absent targets rather than requiring them." \
      --arg imp_07 "At public Ravel jj change skluoolsplvu, commit 8983cab26baa, which was local-only and unpushed at observation time: ServiceRefused::NoProvider became FailureKind::MissingRequiredDependency at LifecycleState::Resolved and refused the whole set, while an undeclared call became ServiceUnavailable::Undeclared. Searches of crates/ravel-host-api/src/ and crates/ravel-sdk/src/ for fn installed, fn is_present, fn has_mod, fn present, installed_mods, and fn mods( found no presence query. Those two refusal paths therefore exhausted that revision's public options, so it could not express an optional target whose absence skips one integration without refusing the set." \
      '{"RVL-GAP-07": {discoveredWhile: $dw_07, impossibility: $imp_07}}' >"$pins"

    # A public symbol as Rust writes one. An impossibility that names none is a
    # claim about the public API that points at nothing in it.
    recognized_symbols='[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*'
    while IFS= read -r bad; do
      [ -n "$bad" ] && note "$bad"
    done < <(jq -r --arg symbols "$recognized_symbols" --arg work_package '^M[0-9]+-W[0-9]+$' --slurpfile pins "$pins" '
      ($pins[0]) as $pin |
      [ (if (keys | sort) != (["gaps", "idSequence", "schema", "sequencing", "whyThisFileExists"] | sort)
         then "the implementation-discovered gap list carries an unknown or missing top-level field" else empty end),
        (if (.sequencing | type) != "string" or .sequencing == ""
         then "the implementation-discovered gap list records no sequencing between its gaps" else empty end),
        (if (.whyThisFileExists | type) != "string"
            or (.whyThisFileExists | contains("docs/decisions/machinery-is-not-material.md") | not)
            or (.whyThisFileExists | contains("frozen inventory-derived M5") | not)
            or (.whyThisFileExists | contains("implementation-proof successor") | not)
         then "the implementation-discovered gap list does not explain the successor rule" else empty end),
        (if (.idSequence | type) != "string"
            or (.idSequence | contains("global RVL-GAP sequence") | not)
         then "the implementation-discovered gap list does not continue M5s global id sequence" else empty end),
        (.gaps[]
         | . as $g
         | (if (keys | sort) != ((["blockedEntries", "capability", "discoveredBy", "discoveredWhile", "id", "impossibility", "status"] + (if $g.status == "resolved" then ["resolvedBy"] else [] end)) | sort)
            then "\($g.id // "an implementation-discovered gap") carries an unknown or missing field" else empty end),
           (if ($g.capability | type) != "string" or $g.capability == ""
            then "\($g.id // "an implementation-discovered gap") names no Ravel capability" else empty end),
           (if ($g.discoveredBy | type) != "string" or ($g.discoveredBy | test($work_package) | not)
            then "\($g.id // "an implementation-discovered gap") has no valid discoveredBy attribution" else empty end),
           (if ($g.discoveredWhile | type) != "string" or $g.discoveredWhile == ""
            then "\($g.id // "an implementation-discovered gap") has no valid discoveredWhile context" else empty end),
           (if ($g.impossibility | type) != "string" or $g.impossibility == ""
            then "\($g.id // "an implementation-discovered gap") records no public-API impossibility" else empty end),
           (if ($g.impossibility | type) == "string" and ($g.impossibility | test($symbols) | not)
            then "\($g.id // "an implementation-discovered gap") impossibility names no recognized public symbol" else empty end),
           (if ($pin[$g.id // ""] // null) != null and $g.discoveredWhile != $pin[$g.id].discoveredWhile
            then "\($g.id) does not carry the exact discoveredWhile context pinned for it" else empty end),
           (if ($pin[$g.id // ""] // null) != null and $g.impossibility != $pin[$g.id].impossibility
            then "\($g.id) does not carry the exact public-API impossibility observed at the pinned revision" else empty end),
           (if (($g.blockedEntries // null) | type) != "array"
               or (($g.blockedEntries // []) | length) == 0
               or ([($g.blockedEntries // [])[] | select(type != "string" or (test("^RVL-[0-9]{3}$") | not))] | length) != 0
            then "\($g.id // "an implementation-discovered gap") names no blocked entry" else empty end),
           (if ($g.status != "open" and $g.status != "resolved")
            then "\($g.id // "an implementation-discovered gap") carries no known status" else empty end),
           (if $g.status == "resolved"
               and ((($g.resolvedBy // null) | type) != "object"
                    or (($g.resolvedBy.change // "") | test("^[k-z]{12}$") | not)
                    or (($g.resolvedBy.shipped // "") | type) != "string"
                    or ($g.resolvedBy.shipped // "") == "")
            then "\($g.id) is resolved without naming the public change that resolved it and what shipped there" else empty end)
        )
      ] | .[]' "$post_m5_records")
    ;;
  *)
    note "the post-M5 proposal file uses an unknown schema: ${post_m5_schema:--}"
    ;;
esac

# ---------------------------------------------------------------------------
# Coverage: one proposal per published M5 capability, and no M5 proposal without one.
# ---------------------------------------------------------------------------
jq -r '.capabilities | keys[]' "$vocabulary" | sort >"$meta_work/published"
jq -r '.proposals[].capability' "$records" | sort >"$meta_work/filed"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "the vocabulary publishes a capability no proposal files: $bad"
done < <(comm -23 "$meta_work/published" "$meta_work/filed")
while IFS= read -r bad; do
  [ -n "$bad" ] && note "a proposal files a capability the vocabulary does not publish: $bad"
done < <(comm -13 "$meta_work/published" "$meta_work/filed")

pattern="$(jq -r '.idScheme.pattern' "$vocabulary")"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "proposal id does not match the scheme $pattern: $bad"
done < <(jq -r --arg p "$pattern" '.proposals[].id | select(test($p) | not)' "$all_records")
while IFS= read -r dup; do
  [ -n "$dup" ] && note "proposal id is not unique across proposal files: $dup"
done < <(jq -r '[.proposals[].id] | group_by(.) | map(select(length > 1) | .[0]) | .[]' "$all_records")
next_gap_slot="$(jq -r '[.proposals[].id | capture("RVL-GAP-(?<slot>[0-9]{2})").slot | tonumber] | max + 1' "$records")"
while IFS= read -r id; do
  expected_id="$(printf 'RVL-GAP-%02d' "$next_gap_slot")"
  if [ "$id" != "$expected_id" ]; then
    note "implementation-discovered gap sequence expected $expected_id, found $id"
  fi
  next_gap_slot=$((next_gap_slot + 1))
done < <(jq -r '.gaps[].id' "$post_m5_records")
while IFS= read -r dup; do
  [ -n "$dup" ] && note "two proposals claim one anchor: $dup"
done < <(jq -r '[.proposals[].publicSurface.anchor] | group_by(.) | map(select(length > 1) | .[0]) | .[]' "$records")

# ---------------------------------------------------------------------------
# Generality. The motivating entry set is recomputed from the committed surfaces
# through a flat presence table and awk, which shares no expression with the
# model that produced it.
# ---------------------------------------------------------------------------
jq -r --slurpfile v "$vocabulary" --slurpfile p "$inventory/plans.json" '
  ($v[0].surfaceFamilies) as $fam
  | ([$p[0].plans[] | {key: .artifact, value: .canonicalId}] | from_entries) as $id
  | .artifacts[]
  | . as $a
  | $fam
  | to_entries[]
  | .key as $family
  | (.value.path | split(".")) as $path
  | select((($a | getpath($path)) // 0) > 0)
  | [$id[$a.artifact], $family] | @tsv
' "$inventory/surfaces.json" | LC_ALL=C sort -u >"$meta_work/presence.tsv"

jq -r '.proposals[] | [.id, .motivation.rule, (.motivation.surfaceFamilies | join(","))] | @tsv' \
  "$records" | LC_ALL=C sort >"$meta_work/claims.tsv"

awk -F'\t' '
  NR == FNR { carries[$1 "\t" $2] = 1; entries[$1] = 1; next }
  {
    split($3, wanted, ",")
    n = 0
    out = ""
    for (entry in entries) {
      hits = 0
      for (i in wanted) { if ((entry "\t" wanted[i]) in carries) hits++ }
      ok = ($2 == "all") ? (hits == length(wanted)) : (hits > 0)
      if (ok) { n++; out = out (out == "" ? "" : ",") entry }
    }
    print $1 "\t" n "\t" out
  }
' "$meta_work/presence.tsv" "$meta_work/claims.tsv" >"$meta_work/recomputed.tsv"

threshold="$(jq -r '.minimumMotivatingEntries.value' "$vocabulary")"
while IFS=$'\t' read -r id count entries; do
  committed_count="$(jq -r --arg i "$id" '.proposals[] | select(.id == $i) | .motivation.entryCount' "$records")"
  committed_entries="$(jq -r --arg i "$id" '.proposals[] | select(.id == $i) | .motivation.entries | sort | join(",")' "$records")"
  sorted_entries="$(printf '%s' "$entries" | tr ',' '\n' | grep -v '^$' | LC_ALL=C sort | paste -sd, -)"
  if [ "$committed_entries" != "$sorted_entries" ]; then
    note "$id cites an entry set the counted surfaces do not give: it claims $(printf '%s' "$committed_entries" | tr ',' ' ' | wc -w) entries, the surfaces put $count behind it"
  fi
  if [ "$committed_count" != "$count" ]; then
    note "$id records an entry count of $committed_count; the counted surfaces give $count"
  fi
  if [ "$count" -lt "$threshold" ]; then
    note "$id is motivated by $count entries, below the published minimum of $threshold; a pattern that small is one entry's habit rather than an SDK gap"
  fi
done <"$meta_work/recomputed.tsv"

# Every cited entry is a canonical id the port plans allocated.
jq -r '.plans[].canonicalId' "$inventory/plans.json" | LC_ALL=C sort -u >"$meta_work/allocated"
jq -r '.proposals[].motivation.entries[]' "$records" | LC_ALL=C sort -u >"$meta_work/cited"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "a proposal cites an entry no port plan allocated: $bad"
done < <(comm -13 "$meta_work/allocated" "$meta_work/cited")

# Whether a proposal rests on an estimate is recomputed, not believed.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r -n --slurpfile r "$records" --slurpfile v "$vocabulary" '
  ($v[0].surfaceFamilies) as $fam
  | $r[0].proposals[]
  | . as $p
  | ([$p.motivation.surfaceFamilies[] | $fam[.].estimate] | any) as $rests
  | [ ($p.motivation.surfaceFamilies[] | select($fam[.] == null)
       | "\($p.id) cites \(.), which is not a counted surface family"),
      (if $p.motivation.restsOnEstimate != $rests
       then "\($p.id) records restsOnEstimate \($p.motivation.restsOnEstimate); the families it cites give \($rests)"
       else empty end),
      (if ($v[0].motivationRules[$p.motivation.rule] // null) == null
       then "\($p.id) uses an unpublished motivation rule \($p.motivation.rule)" else empty end)
    ] | .[]')

# ---------------------------------------------------------------------------
# The eighteen families are dispositioned, and the disposition closes over the
# proposals rather than being asserted beside them.
# ---------------------------------------------------------------------------
jq -r '.surfaceFamilies | keys[]' "$vocabulary" | sort >"$meta_work/families"
jq -r '.familyDisposition | keys[]' "$vocabulary" | sort >"$meta_work/dispositioned"
if ! diff -q "$meta_work/families" "$meta_work/dispositioned" >/dev/null; then
  note "the counted surface families and the families with a disposition are not the same set"
fi
jq -r '.familyDisposition | to_entries[] | select(.value.disposition == "proposed") | .key' "$vocabulary" |
  sort >"$meta_work/claimed-proposed"
jq -r '.proposals[].motivation.surfaceFamilies[]' "$records" | sort -u >"$meta_work/actually-proposed"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "family $bad is dispositioned proposed but no proposal cites it"
done < <(comm -23 "$meta_work/claimed-proposed" "$meta_work/actually-proposed")
while IFS= read -r bad; do
  [ -n "$bad" ] && note "family $bad motivates a proposal but is not dispositioned proposed"
done < <(comm -13 "$meta_work/claimed-proposed" "$meta_work/actually-proposed")
while IFS= read -r bad; do
  [ -n "$bad" ] && note "family disposition uses an unpublished word: $bad"
done < <(jq -r '[.dispositions | keys[]] as $known | .familyDisposition[].disposition | select(IN($known[]) | not)' "$vocabulary")

# Every family's path resolves to a number on every catalogued entry, so a
# family cannot be a name nothing counts.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r --slurpfile v "$vocabulary" '
  ($v[0].surfaceFamilies) as $fam
  | [ .artifacts[] as $a
      | $fam | to_entries[]
      | .key as $family
      | (.value.path | split(".")) as $path
      | select((($a | getpath($path)) | type) != "number")
      | "surface family \($family) does not resolve to a count on every catalogued entry" ]
  | unique | .[]' "$inventory/surfaces.json")

# ---------------------------------------------------------------------------
# A gap is a gap, or the ledger says who closed it. Every border is present in
# the public SDK either way. See the header for why the anchor rule reads the
# ledger rather than publicSurface.shipped.
# ---------------------------------------------------------------------------
find -L "$sdk_src" -maxdepth 1 -name '*.rs' -printf '%f\n' | sed 's/\.rs$//' |
  { grep -v '^lib$' || true; } | LC_ALL=C sort -u >"$meta_work/sdk-modules"
# -R rather than -r, because a checkout reached through a mirrored tree is a tree
# of symlinks and -r walks past them. A grep that matches nothing is not an
# error here, but an SDK with no modules at all is, and is said below.
{
  cat "$meta_work/sdk-modules"
  { grep -RhoE 'pub use [^;]+;' "$sdk_src" || true; } |
    grep -oE '[A-Za-z_][A-Za-z0-9_]*' |
    { grep -vE '^(pub|use|as|crate|self)$' || true; }
  { grep -RhoE '^[[:space:]]*pub (mod|fn|struct|enum|trait|const|type) [A-Za-z_][A-Za-z0-9_]*' "$sdk_src" || true; } |
    awk '{ print $3 }'
} | tr '[:upper:]' '[:lower:]' | LC_ALL=C sort -u >"$meta_work/sdk-names"
if [ ! -s "$meta_work/sdk-modules" ]; then
  note "the public SDK exposes no module at all; a gap cannot be established against it"
fi

ships() { grep -qxF "$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')" "$meta_work/sdk-names"; }

# A missing or empty field is written as a dash rather than left empty, because
# tab is an IFS whitespace character and two adjacent tabs would collapse into
# one — an unsigned row would be read as though its anchor were the thing
# missing, and would fail for the wrong reason.
jq -r '.resolutions[]
  | [((.proposal // "") | if . == "" then "-" else . end),
     ((.workPackage // "") | if . == "" then "-" else . end),
     ((.anchor // "") | if . == "" then "-" else . end)]
  | @tsv' "$ledger" >"$meta_work/ledger.tsv"
while IFS= read -r dup; do
  [ -n "$dup" ] && note "the resolution ledger resolves one proposal more than once: $dup"
done < <(jq -r '[.resolutions[].proposal] | group_by(.) | map(select(length > 1) | .[0]) | .[]' "$ledger")
cut -f1 "$meta_work/ledger.tsv" | LC_ALL=C sort -u >"$meta_work/resolved"

# The ledger's side. Each row is held to the frozen record and to the SDK tree,
# and the work package is required to be one — a row whose author is a blank
# string is a resolution nobody signed.
work_package='^M[0-9]+-W[0-9]+$'
while IFS=$'\t' read -r id work anchor; do
  known="$(jq -r --arg i "$id" '[.proposals[] | select(.id == $i) | .id] | first // ""' "$all_records")"
  if [ -z "$known" ]; then
    note "the resolution ledger resolves $id, which is not a proposal any gap was filed under"
    continue
  fi
  filed="$(jq -r --arg i "$id" '[.proposals[] | select(.id == $i) | .publicSurface.anchor] | first // ""' "$records")"
  if [ -z "$filed" ]; then
    if ! printf '%s' "$work" | grep -qE "$work_package"; then
      note "the resolution ledger resolves $id by no work package; a resolution nobody owns is not a resolution"
    fi
    if [ "$anchor" = "-" ] || ! ships "$anchor"; then
      note "the resolution ledger resolves $id, but the public SDK ships no $anchor; that is a resolution nobody delivered"
    fi
    continue
  fi
  if [ "$anchor" != "$filed" ]; then
    note "the resolution ledger puts $id at $anchor; the proposal it closes anchors at $filed"
  fi
  if ! printf '%s' "$work" | grep -qE "$work_package"; then
    note "the resolution ledger resolves $id by no work package; a resolution nobody owns is not a resolution"
  fi
  if ! ships "$filed"; then
    note "the resolution ledger resolves $id, but the public SDK ships no $filed; that is a resolution nobody delivered"
  fi
done <"$meta_work/ledger.tsv"

# The SDK tree's side, asked independently of the ledger.
while IFS=$'\t' read -r id anchor; do
  ships "$anchor" || continue
  if grep -qxF "$id" "$meta_work/resolved"; then
    continue
  fi
  note "$id proposes $anchor, which the public SDK now ships, and no work package records having resolved it"
done < <(jq -r '.proposals[] | [.id, .publicSurface.anchor] | @tsv' "$records")

while IFS=$'\t' read -r id border; do
  if ! grep -qxF "$border" "$meta_work/sdk-modules"; then
    note "$id says it borders $border, which is not a module of the public SDK"
  fi
done < <(jq -r '.proposals[] | .id as $i | .publicSurface.borders[] | [$i, .] | @tsv' "$records")

while IFS=$'\t' read -r family module; do
  if ! grep -qxF "$module" "$meta_work/sdk-modules"; then
    note "family $family is dispositioned shipped by $module, which is not a module of the public SDK"
  fi
done < <(jq -r '.familyDisposition | to_entries[] | select(.value.module != null) | [.key, .value.module] | @tsv' "$vocabulary")

# publicSurface.shipped is superseded and frozen false. It is kept held rather
# than deleted because deleting it would move a record the public M5 evidence
# binds by digest as material, and it is worth saying here what a reader meeting
# it should conclude: it records what M5 measured, not what ships today, and the
# ledger above is what answers the second question.
if [ "$(jq '[.proposals[] | select(.publicSurface.shipped)] | length' "$records")" -ne 0 ]; then
  note "a proposal records itself as already shipped; that field is M5's frozen measurement and cannot become a status"
fi

# ---------------------------------------------------------------------------
# The test cases.
# ---------------------------------------------------------------------------
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r -n --slurpfile r "$records" --slurpfile v "$vocabulary" '
  ($v[0].testCaseOrigin) as $origin
  | ($v[0].testCaseKinds) as $kinds
  | $r[0].proposals[]
  | . as $p
  | [ (if ($p.tests | length) < $origin.minimumPerProposal
       then "\($p.id) carries \($p.tests | length) test cases; the published minimum is \($origin.minimumPerProposal)" else empty end),
      (if ([$p.tests[].kind] | unique | length) < $origin.minimumDistinctKindsPerProposal
       then "\($p.id) exercises \([$p.tests[].kind] | unique | length) test-case kinds; the published minimum is \($origin.minimumDistinctKindsPerProposal)" else empty end),
      ($p.tests[] | select($kinds[.kind] == null) | "\($p.id) names an unpublished test-case kind \(.kind)"),
      ($p.tests[] | select(.file | startswith("proposals/tests/") | not)
       | "\($p.id) names a test case outside the proposals tree: \(.file)")
    ] | .[]')

if [ "$(jq -r '.derivation.readsUpstreamTests' "$records")" != "false" ] ||
  [ "$(jq -r '.testCaseOrigin.readsUpstreamTests' "$vocabulary")" != "false" ]; then
  note "the proposals do not declare that no upstream test was read"
fi

while IFS=$'\t' read -r file sha; do
  if [ ! -f "$registry_root/$file" ]; then
    note "a named test case does not exist: $file"
    continue
  fi
  actual="$(sha256sum "$registry_root/$file" | cut -d' ' -f1)"
  if [ "$actual" != "$sha" ]; then
    note "test case $file does not hash to what the record binds it to"
  fi
done < <(jq -r '.proposals[].tests[] | [.file, .sha256] | @tsv' "$records")

jq -r '.proposals[].tests[].file' "$records" | sort -u >"$meta_work/named-tests"
find -L "$registry_root/proposals/tests" -type f -printf '%P\n' |
  sed 's#^#proposals/tests/#' | sort >"$meta_work/present-tests"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "the proposals tree carries a test case no proposal names: $bad"
done < <(comm -13 "$meta_work/named-tests" "$meta_work/present-tests")

jq -r '.testCaseKinds | keys[]' "$vocabulary" | sort >"$meta_work/kinds-published"
jq -r '.proposals[].tests[].kind' "$records" | sort -u >"$meta_work/kinds-used"
while IFS= read -r bad; do
  [ -n "$bad" ] && note "the vocabulary publishes a test-case kind no test case is: $bad"
done < <(comm -23 "$meta_work/kinds-published" "$meta_work/kinds-used")

# ---------------------------------------------------------------------------
# Public safety. Two halves that close each other.
# ---------------------------------------------------------------------------

# Keep capability proposals independent of upstream inventory identifiers.
{
  jq -r '.artifacts[].artifact' "$inventory/surfaces.json"
  jq -r '.artifacts[].artifact | sub("\\.(jar|zip)$"; "")' "$inventory/surfaces.json"
  jq -r '.artifacts[].mods[]' "$inventory/surfaces.json"
  jq -r '.artifacts[] | "\(.reference.projectId)", "\(.reference.fileId)"' "$inventory/surfaces.json"
  jq -r '.nodes[].id' "$inventory/graph.json"
} | tr '[:upper:]' '[:lower:]' | { grep -v '^$' || true; } | LC_ALL=C sort -u >"$meta_work/upstream"

# Half one: every word in every proposal file is a word the lexicon publishes.
# Hex digests are dropped rather than published, because a digest is arithmetic
# over a file and carries no word; an all-digit run long enough to be an upstream
# id is refused outright rather than admitted by any list.
#
# A resolved gap's change id is dropped for the same reason and by the same
# argument: it names a change in the public repository rather than saying
# anything, and publishing one as vocabulary would grow the lexicon by two words
# per resolution that no sentence ever uses. They are read out of the record
# itself rather than matched by shape, so only a value the schema half already
# held to ^[k-z]{12}$ is dropped, and a change id that is not in that position is
# still an unpublished word.
jq -r '[.gaps[]? | select(.status == "resolved") | .resolvedBy.change // empty] | .[]' \
  "$post_m5_records" | tr '[:upper:]' '[:lower:]' | { grep -v '^$' || true; } |
  LC_ALL=C sort -u >"$meta_work/change-ids"
LC_ALL=C grep -ohE '[A-Za-z_][A-Za-z0-9_]*' -- "${proposal_files[@]}" |
  tr '[:upper:]' '[:lower:]' |
  { grep -vE '^[0-9a-f]{32,}$' || true; } |
  { grep -vxF -f "$meta_work/change-ids" || true; } |
  LC_ALL=C sort -u >"$meta_work/used-words"
tr '[:upper:]' '[:lower:]' <"$lexicon" | { grep -v '^$' || true; } | LC_ALL=C sort -u >"$meta_work/lexicon"
if [ ! -s "$meta_work/lexicon" ]; then
  note "the published lexicon is empty; nothing would be checked against it"
fi
if [ ! -s "$meta_work/upstream" ]; then
  note "no upstream identifier was read from the inventory; the safety half would pass by having nothing to compare"
fi
while IFS= read -r bad; do
  [ -n "$bad" ] && note "a proposal file uses a word the published lexicon does not carry: $bad"
done < <(comm -13 "$meta_work/lexicon" "$meta_work/used-words")
while IFS= read -r bad; do
  [ -n "$bad" ] && note "the lexicon publishes a word no proposal file uses: $bad"
done < <(comm -23 "$meta_work/lexicon" "$meta_work/used-words")

# Half two: the lexicon shares nothing with what the inventory knows. This is
# what stops the first half from being satisfied by adding a name to the list.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "the published lexicon carries an identifier the inventory knows: $bad"
done < <(comm -12 "$meta_work/lexicon" "$meta_work/upstream")

# And no identifier long enough to be unmistakable survives as a substring, which
# is what catches one arriving inside a longer string rather than as a word.
awk 'length($0) >= 5' "$meta_work/upstream" >"$meta_work/upstream-long"
if [ -s "$meta_work/upstream-long" ]; then
  while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    note "a proposal file carries an upstream identifier: ${hit#"$registry_root/"}"
  done < <(LC_ALL=C grep -lFif "$meta_work/upstream-long" -- "${proposal_files[@]}" | sort -u)
fi

while IFS= read -r hit; do
  [ -n "$hit" ] && note "a proposal file carries a digit run long enough to be a project or file id: $hit"
done < <(LC_ALL=C grep -ohE '(^|[^0-9a-f])[0-9]{5,}([^0-9a-f]|$)' -- "${proposal_files[@]}" | sort -u)

# A proposal record itself carries identifiers and published terms and nothing
# else. The prose lives in the vocabulary, where it is one reviewed document
# rather than a field a later edit can grow.
while IFS= read -r bad; do
  [ -n "$bad" ] && note "proposal record carries a string that is neither an identifier nor a published term: $bad"
done < <(jq -r -n --slurpfile r "$records" --slurpfile v "$vocabulary" '
  ( [$r[0].proposals[].id] + [$r[0].proposals[].capability]
    + [$r[0].proposals[].motivation.entries[]]
    + [$r[0].proposals[].tests[].file]
    + ($v[0].surfaceFamilies | keys) + ($v[0].motivationRules | keys)
    + ($v[0].testCaseKinds | keys)
    + [$r[0].proposals[].publicSurface.anchor]
    + [$r[0].proposals[].publicSurface.borders[]] | INDEX(.) ) as $allowed
  | [$r[0].proposals[] | .. | strings] | unique | .[]
  | select(test("^[0-9a-f]{64}$") | not)
  | select($allowed[.] == null)')

# ---------------------------------------------------------------------------
# Totals, and the model compared field by field.
# ---------------------------------------------------------------------------
while IFS= read -r bad; do
  [ -n "$bad" ] && note "$bad"
done < <(jq -r '
  . as $r
  | [ if $r.totals.proposals != ($r.proposals | length) then "totals.proposals disagrees with the records" else empty end,
      if $r.totals.testCases != ([$r.proposals[].tests[].file] | unique | length)
        then "totals.testCases disagrees with the records" else empty end,
      if $r.totals.motivatingEntries != ([$r.proposals[].motivation.entries[]] | unique | length)
        then "totals.motivatingEntries disagrees with the records" else empty end,
      if $r.totals.restingOnEstimate != ([$r.proposals[] | select(.motivation.restsOnEstimate)] | length)
        then "totals.restingOnEstimate disagrees with the records" else empty end,
      if $r.totals.familiesProposed != ([$r.proposals[].motivation.surfaceFamilies[]] | unique | length)
        then "totals.familiesProposed disagrees with the records" else empty end,
      if $r.derivation.handAssignedFields != 0
        then "the proposals declare \($r.derivation.handAssignedFields) hand-assigned fields; every field is derived" else empty end,
      if $r.derivation.restrictedContentRead != false
        then "the proposals declare that restricted content was read" else empty end
    ] | .[]' "$records")

jq -S '.proposals' "$records" >"$meta_work/committed-proposals.json"
jq -S -R -s --slurpfile v "$vocabulary" --slurpfile r "$records" '
  ($v[0].capabilities) as $caps
  | ([$r[0].proposals[].tests[] | {key: .file, value: .sha256}] | from_entries) as $ignored
  | split("\n") | map(select(length > 0)) | map(split("\t") | {
      id: .[0],
      capability: .[1],
      motivation: {
        surfaceFamilies: (.[4] | split(",") | sort),
        rule: .[3],
        entries: (if .[5] == "-" then [] else (.[5] | split(",") | sort) end),
        entryCount: (.[6] | tonumber),
        restsOnEstimate: (.[7] == "true")
      },
      publicSurface: { anchor: .[2], borders: ($caps[.[1]].borders | sort), shipped: false },
      tests: [ $caps[.[1]].tests[] | .file ] | sort
    }) | sort_by(.id)' "$proposals_tsv" >"$meta_work/recomputed-proposals.json"
jq -S '[.[] | .tests = ([.tests[].file] | sort)]' "$meta_work/committed-proposals.json" \
  >"$meta_work/committed-shape.json"
if ! diff -q "$meta_work/committed-shape.json" "$meta_work/recomputed-proposals.json" >/dev/null; then
  note "the proposals do not match the proposals recomputed from the inventory"
fi

if [ ${#violations[@]} -ne 0 ]; then
  printf 'proposal verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

printf 'proposals reconcile: %s capabilities over %s of %s entries, %s families proposed, %s test cases, %s words published\n' \
  "$(jq '.totals.proposals' "$records")" \
  "$(jq '.totals.motivatingEntries' "$records")" \
  "$(jq '.plans | length' "$inventory/plans.json")" \
  "$(jq '.totals.familiesProposed' "$records")" \
  "$(jq '.totals.testCases' "$records")" \
  "$(wc -l <"$meta_work/lexicon")"
