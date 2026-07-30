#!/usr/bin/env bash
set -euo pipefail

skill_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
codex_root="${CODEX_HOME:-"$HOME/.codex"}"
validator="$codex_root/skills/.system/skill-creator/scripts/quick_validate.py"

for script in "$skill_root"/scripts/*.sh; do
  bash -n "$script"
done
for test_script in "$skill_root"/scripts/test-*.sh; do
  "$test_script"
done

# The frontmatter validator ships with Codex; skip it where only Claude Code is installed.
if [[ ! -f "$validator" ]]; then
  printf 'skipped frontmatter validation: %s not found\n' "$validator" >&2
  exit 0
fi

printf -v validation_command 'python3 %q %q' "$validator" "$skill_root"
exec nix-shell \
  -p 'python3.withPackages (packages: [ packages.pyyaml ])' \
  --run "$validation_command"
