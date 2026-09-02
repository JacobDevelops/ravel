#!/usr/bin/env bash
# Builds Ravel's settings screen jar from the generated sources and the generated
# compile-time stubs.
#
# The jar is a build product and is never tracked. It sits beside
# `ravel-agent.jar`, because that is where `dev.ravel.jvm.Screens` looks for it.
#
# The sources are generated too, and for a different reason: a screen extends the
# game's `Screen` and overrides its methods, so every game name it uses is a
# constant pool entry the JVM links directly. Under an obfuscated version the
# reviewed name is not that name, so the sources are rendered from the templates
# in `jvm/screen/template` into whatever the selected version links.
#
# This compiles against generated stubs rather than the client jar, so it runs
# on a machine without the restricted fixture — the same property
# `build-agent.sh` protects. The stubs carry the erased descriptors the pinned
# jar declares, which is what the runtime links on; `minecraft/versions/*/
# stubs.lock` records them and states what that pinning is and is not worth.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$root/target/ravel-jvm"
generated="$work/generated-stubs"
screens="$work/generated-screen"
stubs="$work/screen-stubs"
classes="$work/screen-classes"
jar="$work/ravel-screen.jar"

for tree in "$generated" "$screens"; do
	if [ ! -d "$tree" ]; then
		echo "$tree is missing; run:" >&2
		echo "  cargo build -p ravel-minecraft-adapter" >&2
		exit 1
	fi
done

rm -rf "$stubs" "$classes"
mkdir -p "$stubs" "$classes"

# The stubs are generated and their shape is not ours to adjust, so a lint
# warning in one would block a build nobody could fix. Our own sources below
# keep `-Xlint:all -Werror`.
mapfile -d '' stub_sources < <(find "$generated" -name '*.java' -print0 | sort -z)
javac -nowarn -d "$stubs" "${stub_sources[@]}"

# The screen's two crossings are `dev.ravel.jvm.Hook`'s natives, so the generated
# hook class is on the compile path beside the stubs. It is not packaged here
# either: the agent jar carries it, and at runtime the screen resolves it from
# the same system loader the agent jar is already on.
hook="$work/generated"
if [ ! -f "$hook/dev/ravel/jvm/Hook.java" ]; then
	echo "the generated hook class is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
javac -Xlint:all -Werror -cp "$stubs" -d "$stubs" \
	"$hook/dev/ravel/jvm/Hook.java" "$root/jvm/bootstrap/dev/ravel/jvm/HookTarget.java"

mapfile -d '' sources < <(find "$screens" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -cp "$stubs" -d "$classes" "${sources[@]}"

# No stub goes in. A stub class inside this jar would shadow the game's own class
# on the loader they share, and the game would then draw against a method that
# throws. Compiling against the stubs and shipping none of them is what keeps
# that impossible rather than merely avoided.
#
# The stub tree is what says which names those are, rather than a package
# pattern: an obfuscated version puts the game in the unnamed package, and so a
# rule about `net/minecraft/` would pass every jar it was supposed to refuse.
while IFS= read -r -d '' stub; do
	class="${stub#"$generated/"}"
	if [ -f "$classes/${class%.java}.class" ]; then
		echo "the stub ${class%.java} reached the screen output tree; refusing to build a jar that would shadow the game" >&2
		exit 1
	fi
done < <(find "$generated" -name '*.java' -print0 | sort -z)

stamp="${RAVEL_JAR_DATE:-1980-01-01T00:00:02Z}"
mapfile -d '' compiled < <(cd "$classes" && find . -name '*.class' -print0 | sort -z)
entries=()
for class in "${compiled[@]}"; do
	entries+=(-C "$classes" "${class#./}")
done

jar --create --file "$jar" --date "$stamp" "${entries[@]}"
echo "$jar"
