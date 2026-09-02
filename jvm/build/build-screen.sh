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

# The Java release this version's game refuses anything newer than, written by
# the generator beside the Java it generated. Read from there rather than from
# the environment, for the reason `build-agent.sh` gives.
release_file="$work/generated/dev/ravel/jvm/java-release.txt"
if [ ! -f "$release_file" ]; then
	echo "the generated Java release is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
release="$(cat "$release_file")"

# The jar goes with them. Every refusal below exits with no jar at this path
# rather than with the one a previous build left, so a step that reads the path
# reads this build's verdict rather than the last build's product.
rm -rf "$stubs" "$classes"
rm -f "$jar"
mkdir -p "$stubs" "$classes"

# The stubs are generated and their shape is not ours to adjust, so a lint
# warning in one would block a build nobody could fix. Our own sources below
# keep `-Xlint:all -Werror`.
mapfile -d '' stub_sources < <(find "$generated" -name '*.java' -print0 | sort -z)
javac --release "$release" -nowarn -d "$stubs" "${stub_sources[@]}"

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
javac --release "$release" -Xlint:all -Werror -cp "$stubs" -d "$stubs" \
	"$hook/dev/ravel/jvm/Hook.java" "$root/jvm/bootstrap/dev/ravel/jvm/HookTarget.java"

mapfile -d '' sources < <(find "$screens" -name '*.java' -print0 | sort -z)
javac --release "$release" -Xlint:all -Werror -cp "$stubs" -d "$classes" "${sources[@]}"

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

# Every class about to go in the jar has to be one this version generated. The
# trees this is built from are shared by every version, so a build after a
# version switch shipped the previous version's screens beside this one's — and
# both compiled, because the stubs for both were there too, and both went in the
# jar.
#
# Checked before the jar is written rather than after. The check used to read the
# finished jar, so a refusal exited with the wrong version's jar sitting at the
# path every later step reads: the refusal was loud and what it left behind was
# indistinguishable from a build that had passed.
#
# The list compared against is `ScreenNames`, which the generator writes from the
# templates it actually rendered. Comparing against the source tree instead would
# be comparing the jar with the thing that filled it, and a stale source is
# exactly what filled it wrongly. The compiled tree is not that: it is the set
# the entries below are built from, class for class.
#
# Nested and anonymous classes are named after the source that declares them, so
# the outer name is what is looked up.
mapfile -d '' compiled < <(cd "$classes" && find . -name '*.class' -print0 | sort -z)
names="$work/generated/dev/ravel/jvm/ScreenNames.java"
if [ ! -f "$names" ]; then
	echo "the generated screen names are missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
generated_names="$(sed -n 's/.*static final String [A-Z_]* = "\(.*\)";/\1/p' "$names")"
for class in "${compiled[@]}"; do
	entry="${class#./}"
	outer="${entry%.class}"
	outer="${outer%%\$*}"
	outer="${outer//\//.}"
	if ! printf '%s\n' "$generated_names" | grep -qxF "$outer"; then
		echo "$entry was compiled for the screen jar and this version's ScreenNames records no such class; refusing to build a jar that would ship another version's classes" >&2
		exit 1
	fi
done

stamp="${RAVEL_JAR_DATE:-1980-01-01T00:00:02Z}"
entries=()
for class in "${compiled[@]}"; do
	entries+=(-C "$classes" "${class#./}")
done

jar --create --file "$jar" --date "$stamp" "${entries[@]}"

echo "$jar"
