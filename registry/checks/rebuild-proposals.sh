#!/usr/bin/env bash
# Files one public-safe SDK capability proposal per published capability.
#
# The M5-W7 producer. A proposal says what Ravel's own SDK would have to provide
# for mods of a kind the inventory counts to be portable at all. It never says
# what any entry does: the record carries an id, a capability term, an anchor,
# the surface families that motivate it, the canonical ids of the entries whose
# counted surfaces put them behind it, and the test cases written against it.
#
# Every prose sentence lives in checks/proposals-vocabulary.json and is copied
# into the record rather than written twice, exactly as M5-W6 did with its
# rubric. The vocabulary is the published document; this file is arithmetic over
# it. Nothing here is hand-assigned, which is why the record says so and the
# consumer requires it.
#
# The proposals are meant to be published. That is why no field in one can be an
# acquired filename, a project id, a file id or an upstream word: what travels is
# a Ravel-native id and a count.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
registry_root="$(cd "$checks_dir/.." && pwd)"
# The pinned pack this run targets, overridable to retarget the whole gate.
FIXTURE_ID="${RAVEL_FIXTURE_ID:-atm10-tts-2.0.2}"
export FIXTURE_ID
# shellcheck source=checks/proposals-model.sh
. "$checks_dir/proposals-model.sh"

eval "$(proposals_model)"
trap 'rm -rf "$meta_work"' EXIT

records="$registry_root/proposals/sdk-gaps.json"
if [ "$(jq -r '.fixture.id' "$records")" != "$FIXTURE_ID" ]; then
  echo "refusing to rewrite $records: it is M5 material filed against $(jq -r '.fixture.id' "$records"), not $FIXTURE_ID" >&2
  exit 1
fi
vocabulary="$checks_dir/proposals-vocabulary.json"

# The digest of every test case, so the record is bound to the case it names and
# a case cannot be swapped for another after the fact.
digests="$meta_work/test-digests.tsv"
: >"$digests"
while IFS= read -r file; do
  [ -n "$file" ] || continue
  if [ ! -f "$registry_root/$file" ]; then
    echo "test case named by the vocabulary is missing: $file" >&2
    exit 1
  fi
  printf '%s\t%s\n' "$file" "$(sha256sum "$registry_root/$file" | cut -d' ' -f1)" >>"$digests"
done < <(jq -r '.capabilities[].tests[].file' "$vocabulary" | sort -u)

jq --sort-keys -n \
  --arg fixture_id "$FIXTURE_ID" \
  --arg inventory_sha "$inventory_sha" \
  --slurpfile vocabulary "$vocabulary" \
  --slurpfile motivation <(jq -R 'split("\t") | {
      id: .[0],
      capability: .[1],
      anchor: .[2],
      rule: .[3],
      families: (.[4] | split(",")),
      entries: (if .[5] == "-" then [] else (.[5] | split(",")) end),
      entryCount: (.[6] | tonumber),
      restsOnEstimate: (.[7] == "true")
    }' "$proposals_tsv" | jq -s .) \
  --slurpfile digests <(jq -R 'split("\t") | {key: .[0], value: .[1]}' "$digests" | jq -s from_entries) '
  ($vocabulary[0]) as $v
  | ($motivation[0] | INDEX(.id)) as $m
  | ($digests[0]) as $sha
  | [ $v.capabilities | to_entries[]
      | .key as $capability
      | .value as $c
      | ($m[$c.id]) as $motive
      | { id: $c.id,
          capability: $capability,
          motivation: {
            surfaceFamilies: ($motive.families | sort),
            rule: $motive.rule,
            entries: ($motive.entries | sort),
            entryCount: $motive.entryCount,
            restsOnEstimate: $motive.restsOnEstimate
          },
          publicSurface: { anchor: $c.anchor, borders: ($c.borders | sort), shipped: false },
          tests: [ $c.tests[] | { file: .file, kind: .kind, sha256: $sha[.file] } ] | sort_by(.file)
        } ]
    | sort_by(.id) as $proposals
  | { schemaVersion: 1,
      inventoryKind: "sdk-gap-proposals",
      fixture: { id: $fixture_id, inventoryDigest: $inventory_sha },
      derivation: {
        producedBy: "checks/rebuild-proposals.sh",
        reads: [
          "inventory/\($fixture_id)/surfaces.json model",
          "inventory/\($fixture_id)/plans.json model",
          "checks/proposals-vocabulary.json",
          "proposals/tests digests"
        ],
        prose: "checks/proposals-vocabulary.json",
        handAssignedFields: 0,
        decompiled: false,
        restrictedContentRead: false,
        readsUpstreamTests: false
      },
      proposals: $proposals,
      totals: {
        proposals: ($proposals | length),
        testCases: ([$proposals[].tests[].file] | unique | length),
        motivatingEntries: ([$proposals[].motivation.entries[]] | unique | length),
        restingOnEstimate: ([$proposals[] | select(.motivation.restsOnEstimate)] | length),
        familiesProposed: ([$proposals[].motivation.surfaceFamilies[]] | unique | length)
      },
      vocabulary: $v
    }' >"$records"

printf 'wrote %s proposals over %s entries and %s test cases to %s\n' \
  "$(jq '.totals.proposals' "$records")" \
  "$(jq '.totals.motivatingEntries' "$records")" \
  "$(jq '.totals.testCases' "$records")" \
  "${records#"$registry_root/"}"
