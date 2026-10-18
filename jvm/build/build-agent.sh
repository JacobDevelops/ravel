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
source_manifest="$root/jvm/build/agent-sources.txt"
if [ ! -s "$source_manifest" ]; then
	echo "missing agent source manifest: $source_manifest" >&2
	exit 1
fi
core_sources=()
modern_sources=()
declare -A listed_sources=()
while read -r group source extra || [ -n "$group$source$extra" ]; do
	if [[ -n "$extra" || ! "$source" =~ ^[A-Za-z][A-Za-z0-9]*$ ]]; then
		echo "malformed agent source manifest: $source_manifest" >&2
		exit 1
	fi
	if [[ -v "listed_sources[$source]" ]]; then
		echo "duplicate agent source: $source" >&2
		exit 1
	fi
	listed_sources["$source"]=true
	case "$group" in
		core) core_sources+=("$root/jvm/bootstrap/dev/ravel/jvm/$source.java") ;;
		modern) modern_sources+=("$root/jvm/bootstrap/dev/ravel/jvm/$source.java") ;;
		*) echo "unknown agent source group: $group" >&2; exit 1 ;;
	esac
done <"$source_manifest"
if [ "${#core_sources[@]}" -eq 0 ] || [ "${#modern_sources[@]}" -eq 0 ]; then
	echo "agent source manifest needs core and modern sources" >&2
	exit 1
fi
sources+=("${core_sources[@]}")

# No game classes belong on this compile path; adapters resolve them at runtime.
legacy=false
if grep -q 'LEGACY_ADAPTER = true' "$generated/dev/ravel/jvm/Hook.java"; then
	legacy=true
	mapfile -d '' adapter_sources < <(find "$root/jvm/bootstrap/dev/ravel/jvm" -maxdepth 1 -name 'Legacy*.java' -print0 | sort -z)
	mapfile -d '' generated_sources < <(find "$generated" -name '*.java' -print0 | sort -z)
else
	sources+=("${modern_sources[@]}")
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
