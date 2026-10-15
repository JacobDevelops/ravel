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
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi

# The generator pins the greater of the game's release and the transformer's floor.
release_file="$generated/dev/ravel/jvm/agent-release.txt"
if [ ! -f "$release_file" ]; then
	echo "the generated Java release is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
release="$(cat "$release_file")"

rm -rf "$classes"
mkdir -p "$classes"

# No game classes belong on this compile path; adapters resolve them at runtime.
mapfile -d '' sources < <(find "$root/jvm/bootstrap" "$generated" -name '*.java' -print0 | sort -z)
javac --release "$release" -Xlint:all -Werror -d "$classes" "${sources[@]}"

printf 'Premain-Class: dev.ravel.jvm.Boot\nCan-Retransform-Classes: false\n' >"$manifest"

# Explicit sorted entries avoid filesystem-dependent jar ordering.
mapfile -d '' compiled < <(cd "$classes" && find . -name '*.class' -print0 | sort -z)
entries=()
for class in "${compiled[@]}"; do
	entries+=(-C "$classes" "${class#./}")
done

jar --create --file "$jar" --manifest "$manifest" --date "$stamp" "${entries[@]}"
echo "$jar"
