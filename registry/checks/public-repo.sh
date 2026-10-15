#!/usr/bin/env bash
# Resolves the registry and its containing Ravel workspace.
set -euo pipefail
# All set comparisons must use the same byte ordering as their sorted inputs.
export LC_ALL=C

registry_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(cd "$registry_root/.." && pwd)"
export registry_root repo_root

# Emit root-relative tracked paths with NUL separators in local and CI checkouts.
tracked_files() {
  if [ -d "$repo_root/.jj" ]; then
    (cd "$repo_root" && jj --no-pager file list -T 'path ++ "\0"' "$@")
  else
    (cd "$repo_root" && git ls-files -z -- "$@")
  fi
}

# The pinned pack this workspace is built against. One place names it so a pack
# pivot is a single edit rather than a sweep across every check. Override
# RAVEL_FIXTURE_ID to point the whole gate at a different pinned pack.
FIXTURE_ID="${RAVEL_FIXTURE_ID:-atm10-tts-2.0.2}"
export FIXTURE_ID

# Every tool every check needs, refused here rather than in whichever check
# happens to reach for one first. A missing tool is a failure and never a skip:
# a check that quietly does not run reads exactly like one that passed.
for tool in jq sha1sum sha256sum unzip python3 taplo awk comm cut diff grep sed sort tr; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "required tool missing: $tool" >&2
    exit 1
  fi
done
