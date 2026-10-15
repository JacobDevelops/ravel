#!/usr/bin/env bash
# Reads the acquired artifacts' own declared metadata and nothing else.
#
# What is opened, per artifact: `META-INF/neoforge.mods.toml` or its Forge
# predecessor, the jar-in-jar manifest, and the archive's table of contents. No
# class file is decompiled, disassembled, or read as bytes, and no resource or
# data file is opened. The catalogues built on top of this are therefore bounded
# by what a loader is told about a mod plus how the mod lays its files out, which
# is the boundary M5 draws between inventorying a pack and porting one.
#
# `mods.toml` is TOML, so it is read with a TOML parser. M5-W7's review found
# what the alternative costs: a line scanner recognised one spelling of a
# dependency block, three mods in this fixture used another the grammar admits,
# and every one of their declarations was dropped with no node, no edge and no
# diagnostic.
#
# Reading is shared; believing a reading is not. Every producer and every
# consumer starts from these declared facts, and `recheck-metadata.sh` reads the
# same archives again with a different parser and requires the two to agree, so a
# defect in the reader is a gate failure rather than a hole every derived file
# inherits in the same shape.
#
# Sourcing this defines meta_tsv, entries_tsv, ledger_tsv and nested_entries_tsv.
# It requires artifact-set.sh to have been sourced first.
set -euo pipefail

if [ -z "${artifact_tsv:-}" ]; then
  echo "mod-metadata.sh requires artifact-set.sh to be sourced first" >&2
  exit 1
fi

meta_work="$(mktemp -d)"
meta_tsv="$meta_work/meta.tsv"
entries_tsv="$meta_work/entries.tsv"
ledger_tsv="$meta_work/ledger.tsv"
nested_entries_tsv="$meta_work/nested-entries.tsv"
trap 'rm -rf "${artifact_work:-}" "$meta_work"' EXIT
checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
reader="$checks_dir/modmeta.py"

if [ ! -f "$reader" ]; then
  echo "declared-metadata reader missing: checks/modmeta.py" >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "required tool missing: python3" >&2
  exit 1
fi

python3 "$reader" "$artifact_tsv" "$instance_root" "$meta_tsv" "$entries_tsv" "$ledger_tsv" \
  "$nested_entries_tsv"

if [ ! -s "$meta_tsv" ] || [ ! -s "$entries_tsv" ]; then
  echo "no declared metadata was read from the acquired mods" >&2
  exit 1
fi

# Two checks, and they are not the same check twice. The accounting holds the
# reader to the rows it parsed, which catches a rule the readers share; the
# comparison holds it to a second reader, which catches a rule only one of them
# has. Round 2's review found the first defect surviving the second check.
# shellcheck source=checks/reader-accounting.sh
. "$checks_dir/reader-accounting.sh"
reader_accounting

# shellcheck source=checks/recheck-metadata.sh
. "$checks_dir/recheck-metadata.sh"
recheck_metadata

export meta_tsv entries_tsv ledger_tsv nested_entries_tsv
