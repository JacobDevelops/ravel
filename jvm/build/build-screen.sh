#!/usr/bin/env bash
# Builds the screen jar against generated compile-time stubs, without the client jar.
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
		echo "  cargo run -p ravel-xtask -- generate java" >&2
		exit 1
	fi
done

# Use the generated tree's release, not a possibly different environment selection.
release_file="$work/generated/dev/ravel/jvm/java-release.txt"
if [ ! -f "$release_file" ]; then
	echo "the generated Java release is missing; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi
release="$(cat "$release_file")"

# A refused build must not leave a previous build's jar behind.
rm -rf "$stubs" "$classes"
rm -f "$jar"
mkdir -p "$stubs" "$classes"

# Generated stubs erase generics; keep all lints fatal for our own sources below.
mapfile -d '' stub_sources < <(find "$generated" -name '*.java' -print0 | sort -z)
javac --release "$release" -nowarn -d "$stubs" "${stub_sources[@]}"

# Hook is compiled against but shipped only in the agent jar.
hook="$work/generated"
if [ ! -f "$hook/dev/ravel/jvm/Hook.java" ]; then
	echo "the generated hook class is missing; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi
javac --release "$release" -Xlint:all -Werror -cp "$stubs" -d "$stubs" \
	"$hook/dev/ravel/jvm/Hook.java" "$root/jvm/bootstrap/dev/ravel/jvm/HookTarget.java"

mapfile -d '' sources < <(find "$screens" -name '*.java' -print0 | sort -z)
javac --release "$release" -Xlint:all -Werror -cp "$stubs" -d "$classes" "${sources[@]}"

# Shipping stubs would shadow game classes, including names in the unnamed package.
while IFS= read -r -d '' stub; do
	class="${stub#"$generated/"}"
	if [ -f "$classes/${class%.java}.class" ]; then
		echo "the stub ${class%.java} reached the screen output tree; refusing to build a jar that would shadow the game" >&2
		exit 1
	fi
done < <(find "$generated" -name '*.java' -print0 | sort -z)

# ScreenNames, unlike a stale source tree, pins this version's allowed outer classes.
mapfile -d '' compiled < <(cd "$classes" && find . -name '*.class' -print0 | sort -z)
names="$work/generated/dev/ravel/jvm/ScreenNames.java"
if [ ! -f "$names" ]; then
	echo "the generated screen names are missing; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java" >&2
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
