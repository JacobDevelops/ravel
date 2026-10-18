#!/usr/bin/env bash
# WP0 acquisition/declaration gate; frozen M5 machinery stays in ../checks.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
checks="$here/../checks"
fixture="${RAVEL_FIXTURE_ID:-sevtech-ages-3.2.3}"
mode=--offline
phase=""
mode_seen=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --offline|--fixture)
      if [ "$mode_seen" -ne 0 ]; then
        echo "only one registry mode may be selected" >&2
        exit 2
      fi
      mode="$1"
      mode_seen=1
      shift
      ;;
    --phase)
      if [ -n "$phase" ] || [ "$#" -lt 2 ]; then
        echo "usage: registry/sevtech/verify.sh [--offline|--fixture] [--phase wp0|full]" >&2
        exit 2
      fi
      phase="$2"
      shift 2
      ;;
    *)
      echo "usage: registry/sevtech/verify.sh [--offline|--fixture] [--phase wp0|full]" >&2
      exit 2
      ;;
  esac
done
if [ -z "$phase" ]; then
  if [ "$fixture" = "sevtech-ages-3.2.3" ]; then phase=wp0; else phase=full; fi
fi
case "$phase" in
  full)
    if [ "$fixture" = "sevtech-ages-3.2.3" ]; then
      echo "SevTech full phase is unavailable; WP0 does not supply later-phase evidence" >&2
      for obligation in \
        "reproducible legacy graph derivation (mcmod.info / @Mod extraction)" \
        "surfaces inventory" "implementation plans" "observed reads reconciliation"; do
        echo "NOT-YET-MET: $obligation" >&2
      done
      exit 1
    fi
    exec env RAVEL_FIXTURE_ID="$fixture" bash "$checks/verify.sh" "$mode"
    ;;
  wp0)
    if [ "$fixture" != "sevtech-ages-3.2.3" ]; then
      echo "WP0 phase requires RAVEL_FIXTURE_ID=sevtech-ages-3.2.3" >&2
      exit 2
    fi
    ;;
  *) echo "unknown registry phase: $phase (expected wp0 or full)" >&2; exit 2 ;;
esac
status=0
# These are the historical/global contracts. Their inventory-dependent portions
# must replay their own ATM10 workload, never pretend WP0 supplied later evidence.
for check in verify-boundary.sh verify-primitives.sh verify-proposals.sh verify-wave.sh \
  verify-notes-rows.sh verify-counterparts.sh verify-counterpart-packages.sh \
  verify-counterpart-build.sh verify-spec-identifiers.sh; do
  env RAVEL_FIXTURE_ID=atm10-tts-2.0.2 bash "$checks/$check" || status=1
done
env RAVEL_FIXTURE_ID=atm10-tts-2.0.2 bash "$checks/verify-spec-source-names.sh" "$mode" || status=1
# WP0 uses the publisher-named archive; the frozen inventory replay assumes
# the historical launcher's archive alias and remains exclusive to full phase.
check-jsonschema --schemafile "$here/../../schemas/provenance.schema.json" \
  "$here/../provenance/$fixture.json" || status=1
env RAVEL_FIXTURE_ID="$fixture" bash "$here/verify-rights.sh" "$mode" || status=1
python3 -B "$here/verify-wp0.py" "${mode#--}" --fixture-id "$fixture" || status=1
# Exercise evidence refusal paths as well as benchmark reconstruction.
python3 -B -m unittest discover -s "$here" -p 'test_*.py' || status=1
python3 -B -m unittest discover -s "$here/bench" -p 'test_*.py' || status=1
if [ "$mode" = "--fixture" ]; then
  # Reuse standalone acquisition binding, including publisher override bytes
  # and the exact local jar population. No legacy metadata extractor is implied.
  env RAVEL_FIXTURE_ID="$fixture" bash -c '. "$1/artifact-set.sh"' -- "$here" || status=1
fi
exit "$status"
