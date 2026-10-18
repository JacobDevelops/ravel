#!/usr/bin/env bash
# Formats, lints and tests counterpart packages in the root workspace.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
registry_root="$(cd "$checks_dir/.." && pwd)"
repo_root="$(cd "$registry_root/.." && pwd)"

for tool in cargo jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "counterpart build: $tool is not on PATH, so nothing here was built or run" >&2
    echo "counterpart build: run this from the repository's mise environment" >&2
    exit 1
  fi
done

packages="$(
  find "$registry_root/mods" -mindepth 2 -maxdepth 2 -name Cargo.toml -printf '%h\n' |
    wc -l
)"

if [ "$packages" -eq 0 ]; then
  echo "counterpart build: no counterpart package was found to build" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

status=0

members=()
metadata="$(cargo metadata --offline --no-deps --format-version 1 \
  --manifest-path "$repo_root/Cargo.toml")"
while IFS= read -r name; do
  [ -n "$name" ] && members+=(-p "$name")
done < <(jq -r --arg prefix "$registry_root/mods/" \
  '.packages[] | select(.manifest_path | startswith($prefix)) | .name' <<<"$metadata")

if [ "${#members[@]}" -ne "$((packages * 2))" ]; then
  echo "counterpart build: workspace members do not match counterpart manifests" >&2
  exit 1
fi

if ! (cd "$repo_root" && cargo fmt "${members[@]}" -- --check >"$work/fmt" 2>&1); then
  echo "counterpart build: the formatter reports a difference:" >&2
  sed 's/^/- /' "$work/fmt" >&2
  status=1
fi

if ! (cd "$repo_root" && cargo clippy --offline "${members[@]}" --all-targets --quiet -- -D warnings \
  >"$work/clippy" 2>&1); then
  echo "counterpart build: clippy refuses the workspace:" >&2
  tail -40 "$work/clippy" >&2
  status=1
fi

# Attribute each test run directly to its selected package. Cargo's human-readable
# "Running" banners may be suppressed by workspace settings, while doc-test
# banners alone cannot identify the unit-test results that preceded them.
tests=0
for ((member = 1; member < ${#members[@]}; member += 2)); do
  package="${members[member]}"
  log="$work/test-$package"
  if ! (cd "$repo_root" && cargo test --offline -p "$package" --color never >"$log" 2>&1); then
    printf 'counterpart build: package %s tests do not pass:\n' "$package" >&2
    tail -40 "$log" >&2
    status=1
    continue
  fi
  counts="$(LC_ALL=C awk '
    /^test result:/ { passed += $4; failed += $6; rows++ }
    END {
      if (rows == 0) exit 1
      printf "%d %d\n", passed, failed
    }
  ' "$log")" || {
    printf 'counterpart build: package %s produced no test result summaries\n' "$package" >&2
    status=1
    continue
  }
  read -r passed failed <<<"$counts"
  tests=$((tests + passed))
  printf 'counterpart build: package %s: %d tests passed, %d failed\n' "$package" "$passed" "$failed"
done

if [ "$status" -ne 0 ]; then
  exit 1
fi

printf 'counterpart build: %s packages compiled, formatted and linted, %s tests passed\n' \
  "$packages" "$tests"
printf 'counterpart build: does not prove test quality, completeness of the counterpart set, or behaviour inside a running game; see each counterpart'"'"'s NOTES.md.\n'
