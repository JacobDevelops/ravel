#!/usr/bin/env bash
# Runs the surface recount on its own, so that what it opens can be watched.
#
# The recount is a consumer: `verify-surfaces.sh` calls it and no record is
# derived from it, so it appears in no producer closure and nothing held it to
# what it reads. Round 5's review found the consequence — it extracts every entry
# named like an archive in full, including a resource an artifact merely ships,
# and no record anywhere mentions that. An entry point exists here so the recount
# is watched on the same terms as a producer: `checks/verify-reads.sh` runs this
# file, classifies every read, and holds the kinds it saw to the vocabulary entry
# for `checks/surface-recount.sh`.
#
# It writes nothing. Running it is the point.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/artifact-set.sh
. "$checks_dir/artifact-set.sh"
# shellcheck source=checks/mod-metadata.sh
. "$checks_dir/mod-metadata.sh"
# shellcheck source=checks/surface-recount.sh
. "$checks_dir/surface-recount.sh"

surface_recount >/dev/null
