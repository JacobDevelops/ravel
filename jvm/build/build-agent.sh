#!/usr/bin/env bash
# Builds the premain agent jar from tracked and generated sources.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$root/target/ravel-jvm"
classes="$work/classes"
generated="$work/generated"
manifest="$work/agent.manifest"
jar="$work/ravel-agent.jar"

# Fix timestamps so identical inputs produce identical hashes.
stamp="${RAVEL_JAR_DATE:-1980-01-01T00:00:02Z}"

if [ ! -f "$generated/dev/ravel/jvm/Hook.java" ]; then
	echo "the generated hook class is missing; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi

# The generator pins the greater of the game's release and the transformer's floor.
release_file="$generated/dev/ravel/jvm/agent-release.txt"
if [ ! -f "$release_file" ]; then
	echo "the generated Java release is missing; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi
release="$(cat "$release_file")"

rm -rf "$classes"
mkdir -p "$classes"

# The agent is two disjoint adapters that share the transformer core. Do not
# compile one adapter against the other mode's generated Hook surface: legacy
# sources require legacy-only Hook methods and names that a modern inventory
# deliberately does not generate.
sources=()
adapter_sources=()
for source in Boot Agent Transformer HookTarget Members; do
	sources+=("$root/jvm/bootstrap/dev/ravel/jvm/$source.java")
done

# No game classes belong on this compile path; adapters resolve them at runtime.
legacy=false
if grep -q 'LEGACY_ADAPTER = true' "$generated/dev/ravel/jvm/Hook.java"; then
	legacy=true
	mapfile -d '' adapter_sources < <(find "$root/jvm/bootstrap/dev/ravel/jvm" -maxdepth 1 -name 'Legacy*.java' -print0 | sort -z)
	mapfile -d '' generated_sources < <(find "$generated" -name '*.java' -print0 | sort -z)
else
	for source in Content Datapack Outward Persistence Screens; do
		sources+=("$root/jvm/bootstrap/dev/ravel/jvm/$source.java")
	done
	mapfile -d '' generated_sources < <(find "$generated" -name '*.java' ! -name 'LegacyNames.java' -print0 | sort -z)
fi
sources+=("${adapter_sources[@]}" "${generated_sources[@]}")
javac --release "$release" -Xlint:all -Werror -d "$classes" "${sources[@]}"

printf 'Premain-Class: dev.ravel.jvm.Boot\nCan-Retransform-Classes: false\nAutomatic-Module-Name: dev.ravel.agent\nRavel-Legacy-Adapter: %s\n' "$legacy" >"$manifest"

# Explicit sorted entries avoid filesystem-dependent jar ordering.
mapfile -d '' compiled < <(cd "$classes" && find . -name '*.class' -print0 | sort -z)
entries=()
for class in "${compiled[@]}"; do
	entries+=(-C "$classes" "${class#./}")
done

jar --create --file "$jar" --manifest "$manifest" --date "$stamp" "${entries[@]}"
echo "$jar"
