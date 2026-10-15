#!/usr/bin/env bash
# Derives each capability proposal's motivation from the committed inventory.
#
# The mechanical half of M5-W7, shared by the producer and the consumer. It
# answers one question per proposal: which entries does the catalogued surface
# evidence actually put behind it, and how many.
#
# It reads the committed inventory rather than the fixture. Everything a
# proposal rests on was already established and gated by M5-W4 and M5-W6, and
# reopening the archives to ask again would only add a second chance to read
# something. The fixture binding still travels: the digest this stage records is
# required to equal the one both sibling records carry, so a proposal set cannot
# be motivated by one inventory and filed against another.
#
# Nothing here decides whether a proposal is admissible. The threshold, the
# vocabulary and the public-surface question are the consumer's, because a
# producer that applied its own admission rule would only ever produce records
# that pass it.
#
# Sourcing this defines: meta_work and proposals_tsv, a file of
# id/capability/anchor/rule/families/entries/count/restsOnEstimate rows.
set -euo pipefail

proposals_model() {
  local checks_dir registry_dir inventory vocabulary work
  checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  registry_dir="$(cd "$checks_dir/.." && pwd)"
  inventory="$registry_dir/inventory/${FIXTURE_ID}"
  vocabulary="$checks_dir/proposals-vocabulary.json"

  local path
  for path in "$vocabulary" "$inventory/surfaces.json" "$inventory/plans.json"; do
    if [ ! -f "$path" ]; then
      echo "proposal model input missing: ${path#"$registry_dir/"}" >&2
      return 1
    fi
  done

  work="$(mktemp -d)"

  # The fixture the motivation is read from, taken from the sibling records and
  # required to be one digest rather than two.
  local surfaces_sha plans_sha
  surfaces_sha="$(jq -r '.fixture.artifactSet.sha256' "$inventory/surfaces.json")"
  plans_sha="$(jq -r '.fixture.artifactSet.sha256' "$inventory/plans.json")"
  if [ "$surfaces_sha" != "$plans_sha" ]; then
    echo "the catalogued surfaces and the port plans are bound to different artifact sets" >&2
    return 1
  fi

  jq -r --slurpfile v "$vocabulary" --slurpfile p "$inventory/plans.json" '
    ($v[0].surfaceFamilies) as $fam
    | ([$p[0].plans[] | {key: .artifact, value: .canonicalId}] | from_entries) as $id
    | ($v[0].capabilities) as $caps
    | .artifacts as $arts
    | $caps
    | to_entries[]
    | .key as $capability
    | .value as $c
    | ( [ $arts[]
          | . as $a
          | ( [ $c.motivation.families[]
                | ($fam[.].path | split(".")) as $p
                | (($a | getpath($p)) // 0) > 0 ] ) as $hits
          | select(if $c.motivation.rule == "all" then ($hits | all) else ($hits | any) end)
          | $id[$a.artifact] ]
        | map(select(. != null)) | unique ) as $entries
    | [ $c.id,
        $capability,
        $c.anchor,
        $c.motivation.rule,
        ($c.motivation.families | sort | join(",")),
        (if ($entries | length) == 0 then "-" else ($entries | join(",")) end),
        ($entries | length),
        (if [ $c.motivation.families[] | $fam[.].estimate ] | any then "true" else "false" end)
      ] | @tsv
  ' "$inventory/surfaces.json" | LC_ALL=C sort >"$work/proposals.tsv"

  printf 'meta_work=%q\n' "$work"
  printf 'proposals_tsv=%q\n' "$work/proposals.tsv"
  printf 'inventory_sha=%q\n' "$surfaces_sha"
}
