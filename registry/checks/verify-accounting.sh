#!/usr/bin/env bash
# The gate half that holds the declared-metadata reader to what it parsed.
#
# The invariant and the reasoning are in checks/reader-accounting.sh. Sourcing
# mod-metadata.sh runs it, which is why every other half of this gate is already
# covered; this half exists so that the accounting is a verdict verify.sh prints
# rather than a precondition buried inside the halves that need declared
# metadata for something else.
#
# Round 3's review pointed out that printing is not checking: this file used to
# be a single printf of variables reader_accounting exported, so replacing that
# function's body with three assignments to zero produced "0 rows parsed at 0
# declaration sites ... 1591 emitted" and exit 0 — a line contradicting itself
# under a green gate. So the figures are recomputed here from the ledger and the
# record stream directly, which is a second opinion about the same files rather
# than a restatement of the first one's answer.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
# shellcheck source=checks/mod-metadata.sh
. "$checks_dir/mod-metadata.sh"

emitted="$(wc -l <"$meta_tsv")"
status=0

# A reading that parsed nothing is not a reading, and neither is one that found
# no library to read. These are the figures a reader would cite as evidence that
# the accounting ran, so they are bounded rather than believed.
if [ "$ledger_parsed" -le 0 ] || [ "$ledger_sites" -le 0 ] || [ "$emitted" -le 0 ]; then
  echo "declared metadata accounting is vacuous: $ledger_parsed rows parsed at $ledger_sites sites, $emitted emitted" >&2
  status=1
fi
if [ "$ledger_read" -le 0 ]; then
  echo "declared metadata accounting found no nested library in any acquired artifact" >&2
  status=1
fi

# The accounting equation, summed over artifacts and recomputed from the ledger
# rather than taken from the function that already checked it.
expected="$(awk -F'\t' '
  $1 == "PARSED" { n += $4 }
  $1 == "SYNTH"  { n += $4 }
  $1 == "REFUSED" { n-- }
  END { print n + 0 }' "$ledger_tsv")"
if [ "$emitted" -ne "$expected" ]; then
  echo "the declared-metadata ledger accounts for $expected rows and the record stream holds $emitted" >&2
  status=1
fi

if [ "$status" -ne 0 ]; then
  exit "$status"
fi

printf 'declared metadata accounts: %s rows parsed at %s declaration sites, %s refused by name, %s emitted; %s nested libraries read, %s archives declined by name\n' \
  "$ledger_parsed" "$ledger_sites" "$ledger_refused" "$emitted" "$ledger_read" "$ledger_declined"
