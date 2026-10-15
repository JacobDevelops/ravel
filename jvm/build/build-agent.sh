#!/usr/bin/env bash
# Builds Ravel's premain agent jar from the tracked sources and the generated
# hook class.
#
# The jar is a build product and is never tracked. It is built here rather than
# by a cargo-make task because it needs a JDK's `javac` and `jar`, which the pinned
# game runtime does not ship, and because nothing in the verify gate consumes it.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$root/target/ravel-jvm"
classes="$work/classes"
generated="$work/generated"
manifest="$work/agent.manifest"
jar="$work/ravel-agent.jar"

# A fixed timestamp, because `jar` otherwise stamps every entry with the time it
# ran and two identical inputs would produce two different hashes.
stamp="${RAVEL_JAR_DATE:-1980-01-01T00:00:02Z}"

if [ ! -f "$generated/dev/ravel/jvm/Hook.java" ]; then
	echo "the generated hook class is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi

# The Java release this jar is built for, written by the generator beside the
# Java it generated. Read from there rather than from the environment: a script
# that asked the shell could compile for a release the generator did not generate
# for. It is the game's declared release or the transformer's floor, whichever is
# higher, because the transformer is written on the JDK's Class-File API.
release_file="$generated/dev/ravel/jvm/agent-release.txt"
if [ ! -f "$release_file" ]; then
	echo "the generated Java release is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
release="$(cat "$release_file")"

rm -rf "$classes"
mkdir -p "$classes"

# No source in this jar declares a game type: the transformer manipulates byte
# arrays and descriptor strings, and the persistence, content and outward adapters resolve the
# game classes they need by name at runtime. So nothing here needs the client jar on the
# class path, and the absent `-cp` is what enforces that rather than records it —
# which is also what lets this run on a machine without the restricted fixture.
mapfile -d '' sources < <(find "$root/jvm/bootstrap" "$generated" -name '*.java' -print0 | sort -z)
javac --release "$release" -Xlint:all -Werror -d "$classes" "${sources[@]}"

printf 'Premain-Class: dev.ravel.jvm.Boot\nCan-Retransform-Classes: false\n' >"$manifest"

# Entries are passed explicitly and in sorted order, because `jar -C dir .` walks
# the directory in whatever order the filesystem reports.
mapfile -d '' compiled < <(cd "$classes" && find . -name '*.class' -print0 | sort -z)
entries=()
for class in "${compiled[@]}"; do
	entries+=(-C "$classes" "${class#./}")
done

jar --create --file "$jar" --manifest "$manifest" --date "$stamp" "${entries[@]}"
echo "$jar"
