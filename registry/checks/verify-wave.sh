#!/usr/bin/env bash
# Holds the frozen M6 foundation wave to the inventory it claims to come from.
#
# The consumer for M6-W1. It does not run the selector and it does not compare
# against a recomputed selection: it re-reads plans.json and rights.json and asks
# whether what the manifest asserts is true of those files. So the manifest can
# be wrong in a way the selector could never produce -- an id that does not
# exist, an entry whose stated owner or rights position is not the one on the
# row, a part that is not dependency-closed, a rule cited over a row that does
# not satisfy it -- and this check still says so.
#
# What it cannot say: whether the chosen cluster is the smallest one, or whether
# a different tie-break would have been better. Rule C2 orders candidates and
# nothing here re-orders them. It does hold rule F1 exhaustively, because F1
# claims to admit every row that satisfies it, and that is a claim about the
# inventory rather than about a preference.
#
# It used to live under waves/ and to run on its own, because M5's closed public
# evidence binds checks/verify.sh by digest and publishes a count of the files
# under checks/, so adopting it into the gate rewrote a record M5 froze. What was
# wrong there was the freeze, not the adoption:
# docs/decisions/machinery-is-not-material.md in the public repository decides
# that a closed milestone's evidence freezes the material its figures describe and
# not the machinery that produced them, so M6 amends M5's machinery figures, says
# why in that record, and puts this check where a check that has to fail the gate
# belongs.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
registry_root="$(dirname "$checks_dir")"
wave_dir="$registry_root/waves"

fixture="${FIXTURE_ID:-${RAVEL_FIXTURE_ID:-atm10-tts-2.0.2}}"
plans="$registry_root/inventory/$fixture/plans.json"
rights="$registry_root/inventory/$fixture/rights.json"
manifest="$wave_dir/m6-foundation.json"

violations=()
note() { violations+=("$1"); }

for path in "$plans" "$rights" "$manifest"; do
  if [ ! -f "$path" ]; then
    printf 'wave verification failed:\n- missing input: %s\n' "$path" >&2
    exit 1
  fi
done

# The manifest names its producer and its verifier, and both are held to being
# files that are here. The verifier pointer went stale when this check moved out
# of waves/ and into the gate: it went on naming waves/verify.sh across every
# green run, because nothing read it.
for key in producedBy verifiedBy; do
  named="$(jq -r --arg k "$key" '.[$k] // empty' "$manifest")"
  if [ -z "$named" ]; then
    note "manifest carries no $key"
  elif [ ! -f "$registry_root/$named" ]; then
    note "manifest's $key names a file that is not here: $named"
  fi
done

# The manifest is frozen against the two inventory files by digest.
for input in plans rights; do
  declared="$(jq -r --arg k "$input" '.inputs[$k].sha256' "$manifest")"
  case "$input" in
    plans) actual="$(sha256sum "$plans" | cut -d' ' -f1)" ;;
    rights) actual="$(sha256sum "$rights" | cut -d' ' -f1)" ;;
  esac
  if [ "$declared" != "$actual" ]; then
    note "manifest is frozen against a different $input file than the one on disk"
  fi
done

if [ "$(jq -cS '.fixture' "$manifest")" != "$(jq -cS '.fixture' "$plans")" ]; then
  note "manifest names a different fixture than the inventory it cites"
fi

# Everything below is one jq program over the three files, so that every
# violation is found rather than only the first.
jq -r -n \
  --slurpfile plans "$plans" \
  --slurpfile rights "$rights" \
  --slurpfile manifest "$manifest" '
  ($plans[0].plans | INDEX(.canonicalId)) as $row
  | ($rights[0].artifacts | INDEX(.artifact)) as $triage
  | $manifest[0] as $m
  | ($m.entries | map(.canonicalId)) as $ids
  | ["permitted","conditional"] as $ok
  | ["client-presentation","content-data","integration-runtime"] as $gameplay
  | (reduce $m.entries[] as $e ({};
      reduce $e.admission[] as $a (.; .[$a.part] += [$e.canonicalId]))) as $part
  | ($part.foundation // []) as $foundation
  | ($part.cluster // []) as $cluster
  | [

  # No id is claimed twice, and no entry is in the wave without a reason.
    ( $ids | group_by(.) | map(select(length > 1) | .[0])
      | map("entry appears more than once: " + .) )
  , ( $m.entries | map(select((.admission | length) == 0)
      | "entry carries no admission rule: " + .canonicalId) )
  , ( $m.entries | map(.admission[] | select((.part | IN("foundation","cluster")) | not)
      | "admission names an unknown part: " + .part) )
  , ( $m.entries | map(.admission[] | select((.rule | IN(
        "foundation-seed","foundation-closure","cluster-leaf","cluster-closure")) | not)
      | "admission cites an unknown rule: " + .rule) )

  # Every id exists, and every field the manifest restates is the field on the row.
  , ( $m.entries | map(select($row[.canonicalId] == null)
      | "wave names an id that is not in the inventory: " + .canonicalId) )
  , ( $m.entries | map(select($row[.canonicalId] != null) | . as $e
      | [ (if $e.owner != $row[$e.canonicalId].owner
             then "stated owner is not the owner on the row: " + $e.canonicalId else empty end)
        , (if $e.sourceUse != $row[$e.canonicalId].sourceUse
             then "stated source use is not the one on the row: " + $e.canonicalId else empty end)
        , (if $e.method != $row[$e.canonicalId].method.name
             then "stated method is not the one on the row: " + $e.canonicalId else empty end)
        , (if $e.rank != $row[$e.canonicalId].rank
             then "stated rank is not the one on the row: " + $e.canonicalId else empty end)
        , (if $e.riskScore != $row[$e.canonicalId].riskScore
             then "stated risk score is not the one on the row: " + $e.canonicalId else empty end)
        , (if $e.foundationValue != $row[$e.canonicalId].rubric.foundationValue
             then "stated foundation value is not the one on the row: " + $e.canonicalId else empty end)
        , (if ($e.dependencies | sort) != ($row[$e.canonicalId].dependencies.entries | sort)
             then "stated dependencies are not the dependencies on the row: " + $e.canonicalId else empty end)
        , (if ($e.tests | sort) != ($row[$e.canonicalId].tests | sort)
             then "stated tests are not the tests on the row: " + $e.canonicalId else empty end)
        ]) | flatten )

  # Rule R0, checked against the rights triage rather than against the plan row,
  # and the two are required to agree.
  , ( $m.entries | map(select($row[.canonicalId] != null) | . as $e
      | $row[$e.canonicalId].artifact as $art
      | [ (if $triage[$art] == null
             then "no rights record for the artifact behind " + $e.canonicalId else empty end)
        , (if $triage[$art] != null and $triage[$art].sourceUse != $row[$e.canonicalId].sourceUse
             then "plans and rights disagree on the source use of " + $e.canonicalId else empty end)
        , (if $triage[$art] != null and ($triage[$art].sourceUse | IN($ok[]) | not)
             then "rights violation: " + $e.canonicalId + " has source use "
                  + $triage[$art].sourceUse + ", which rule R0 excludes" else empty end)
        ]) | flatten )

  # Rule R1, per part, read off the inventory rows and not off the manifest.
  , ( [ {p:"foundation", m:$foundation}, {p:"cluster", m:$cluster} ][] as $g
      | $g.m[] as $id
      | ($row[$id].dependencies.entries // [])[] as $dep
      | select(($g.m | index($dep)) == null)
      | "broken dependency edge: " + $g.p + " entry " + $id
        + " depends on " + $dep + ", which is not in the " + $g.p + " part" )

  # Rule F1, in both directions: every seat cited is earned, and every row that
  # earns one has it.
  , ( $m.entries | map(.admission[] | select(.rule == "foundation-seed") | .) as $_
      | $m.entries | map(select(.admission | any(.rule == "foundation-seed")) | . as $e
      | select($row[$e.canonicalId] != null)
      | select($row[$e.canonicalId].owner != "sdk-platform"
               or $row[$e.canonicalId].rubric.foundationValue < 3)
      | "rule foundation-seed cited over a row that does not satisfy it: " + $e.canonicalId) )
  , ( [ $plans[0].plans[]
        | select(.owner == "sdk-platform" and .rubric.foundationValue >= 3
                 and (.sourceUse | IN($ok[])))
        | .canonicalId ]
      - $foundation
      | map("rule foundation-seed is exhaustive but the foundation part omits " + .) )

  # Rules F2 and C2-closure: a closure seat is earned by an entry in the same
  # part that actually depends on it, on the row.
  , ( $m.entries[] as $e
      | $e.admission[]
      | select(.rule == "foundation-closure" or .rule == "cluster-closure")
      | . as $a
      | (if $a.rule == "foundation-closure" then $foundation else $cluster end) as $members
      | ( (if ($a.requiredBy // []) == [] then
             ["closure admission names no dependent: " + $e.canonicalId] else [] end)
        + ( ($a.requiredBy // [])[] as $by
            | [ (if ($members | index($by)) == null
                   then "closure admission for " + $e.canonicalId + " cites " + $by
                        + ", which is not in the " + $a.part + " part" else empty end)
              , (if (($row[$by].dependencies.entries // []) | index($e.canonicalId)) == null
                   then "closure admission for " + $e.canonicalId + " cites " + $by
                        + ", which does not depend on it" else empty end) ] )
        ) | .[] )

  # Rule C1: exactly two leaves, in gameplay lanes, sharing a dependency.
  , ( [ $m.entries[] | select(.admission | any(.rule == "cluster-leaf")) | .canonicalId ] as $leaves
      | ( (if ($leaves | length) != 2
             then ["cluster does not have exactly two leaves"] else [] end)
        + ($leaves | map(select(($row[.].owner | IN($gameplay[])) | not)
             | "cluster leaf is not in a gameplay lane: " + .))
        + (if ($leaves | length) == 2
             and ( ($row[$leaves[0]].dependencies.entries // [])
                   - (($row[$leaves[0]].dependencies.entries // [])
                      - ($row[$leaves[1]].dependencies.entries // [])) | length) == 0
             then ["cluster leaves share no dependency"] else [] end)
        ) | .[] )
  , ( if ($m.scenario.leaves | sort)
         != ([ $m.entries[] | select(.admission | any(.rule == "cluster-leaf")) | .canonicalId ] | sort)
       then ["scenario names leaves the cluster part does not admit"] else [] end | .[] )

  # The published arithmetic, recounted.
  , ( $m.arithmetic as $a
      | ($plans[0].plans | length) as $n
      | ([ $plans[0].plans[] | select(.sourceUse | IN($ok[])) ] | length) as $adm
      | [ (if $a.plans != $n then "arithmetic: plans is not the number of rows" else empty end)
        , (if $a.admissible != $adm then "arithmetic: admissible is not the number of admissible rows" else empty end)
        , (if $a.excludedByRights != ($n - $adm) then "arithmetic: excludedByRights does not complete the total" else empty end)
        , (if $a.sdkPlatformLane != ([ $plans[0].plans[] | select(.owner == "sdk-platform") ] | length)
             then "arithmetic: sdkPlatformLane is not the size of the lane" else empty end)
        , (if $a.sdkPlatformAdmissible != ([ $plans[0].plans[] | select(.owner == "sdk-platform" and (.sourceUse | IN($ok[]))) ] | length)
             then "arithmetic: sdkPlatformAdmissible is not the admissible part of the lane" else empty end)
        , (if $a.foundation != ($foundation | unique | length) then "arithmetic: foundation is not the size of the foundation part" else empty end)
        , (if $a.cluster != ($cluster | unique | length) then "arithmetic: cluster is not the size of the cluster part" else empty end)
        , (if $a.wave != ($ids | unique | length) then "arithmetic: wave is not the number of entries" else empty end)
        ] )
  ] | flatten | .[]
' >"${TMPDIR:-/tmp}/wave-violations.$$"

while IFS= read -r line; do
  [ -n "$line" ] && note "$line"
done <"${TMPDIR:-/tmp}/wave-violations.$$"
rm -f "${TMPDIR:-/tmp}/wave-violations.$$"

if [ "${#violations[@]}" -gt 0 ]; then
  printf 'wave verification failed:\n' >&2
  printf -- '- %s\n' "${violations[@]}" >&2
  exit 1
fi

printf 'wave: %s entries verified against the inventory\n' \
  "$(jq -r '.entries | length' "$manifest")"
