#!/usr/bin/env bash
# Runs Ravel's agent, transformer and native library end to end against a real
# JVM, using a stub class in place of the game's.
#
# What this proves: the premain agent installs, the transformer matches a target
# by name *and* descriptor, the rewritten call reaches the JNI-mangled export,
# the original return value survives, and the shutdown report counts what
# happened. What it does not prove: anything about the actual client. That needs
# a launch, and `evidence/m2/prism-launch.json` is where that is recorded.
#
# The harness names no Minecraft asset. `net/minecraft/util/StringUtil.java` is a
# stub carrying the internal name and descriptors the hook inventory pins, and
# nothing here reads the client jar.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
jar="$root/target/ravel-jvm/ravel-agent.jar"
screen="$root/target/ravel-jvm/ravel-screen.jar"
library="$root/target/release"
work="$root/target/ravel-jvm/harness"
agent="$work/agent/ravel-agent.jar"

if [ ! -f "$jar" ]; then
	echo "the agent jar is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter && ./jvm/build/build-agent.sh" >&2
	exit 1
fi
"$root/jvm/build/build-screen.sh" >/dev/null
if [ ! -f "$screen" ]; then
	echo "the screen jar build produced no artifact" >&2
	exit 1
fi
if [ ! -f "$library/libravel_bootstrap.so" ]; then
	echo "the native library is missing; run:" >&2
	echo "  cargo build -p ravel-bootstrap --release" >&2
	exit 1
fi

rm -rf "$work"
mkdir -p "$(dirname "$agent")"
cp "$jar" "$agent"

javac -d "$work" "$root/jvm/harness/net/minecraft/util/StringUtil.java"
# `Main` needs the agent jar on its compile class path only for the mid-run
# `Agent.transformedClasses()` call. A real launch never compiles against it.
javac -cp "$work:$jar" -d "$work" "$root/jvm/harness/Main.java"
mapfile -d '' screen_stubs < <(find "$root/jvm/harness/net/minecraft/client" \
	"$root/jvm/harness/net/minecraft/network" -name '*.java' -print0 | sort -z)
javac -cp "$work" -d "$work" "${screen_stubs[@]}"
javac -cp "$work" -d "$work" \
	"$root/jvm/harness/TitleMenuMain.java" \
	"$root/jvm/harness/TitleMenuMissingMain.java"

# `java.library.path` is the directory holding the library, not the file.
stdout="$work/stdout"
stderr="$work/stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp . Main >"$stdout" 2>"$stderr")
observed="$(<"$stdout")"
diagnostic="$(grep '^\[ravel\]' "$stderr")"

expected='allowed=true
filtered=a
b
simple=ab
valid=true
transformed=1 methods=4
settings=null
saved=1:10:46:the settings the screen sent could not be read
mods=null
[ravel] rewrote 1 classes and 4 methods; 5 hook invocations reached native code'
expected_diagnostic='[ravel] ravel-screen.jar is missing or not a regular file beside the agent jar'

printf '%s\n' "$observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$stderr"

if [ "$observed" != "$expected" ]; then
	echo >&2
	echo "the harness did not produce the recorded output; expected:" >&2
	printf '%s\n' "$expected" >&2
	exit 1
fi

if [ "$diagnostic" != "$expected_diagnostic" ]; then
	echo >&2
	echo "the harness did not produce the recorded diagnostic; expected:" >&2
	printf '%s\n' "$expected_diagnostic" >&2
	exit 1
fi

missing_stdout="$work/missing-stdout"
missing_stderr="$work/missing-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp . TitleMenuMissingMain >"$missing_stdout" 2>"$missing_stderr")
missing_observed="$(<"$missing_stdout")"
missing_diagnostic="$(grep '^\[ravel\]' "$missing_stderr")"
missing_expected='result=120
buttons=0
[ravel] rewrote 2 classes and 2 methods; 1 hook invocations reached native code'

printf '%s\n' "$missing_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$missing_stderr"

if [ "$missing_observed" != "$missing_expected" ]; then
	echo >&2
	echo "the missing-screen title route did not produce the recorded output; expected:" >&2
	printf '%s\n' "$missing_expected" >&2
	exit 1
fi

if [ "$missing_diagnostic" != "$expected_diagnostic" ]; then
	echo >&2
	echo "the missing-screen title route did not preserve the diagnostic; expected:" >&2
	printf '%s\n' "$expected_diagnostic" >&2
	exit 1
fi

cp "$screen" "$(dirname "$agent")/ravel-screen.jar"

menu_stdout="$work/menu-stdout"
menu_stderr="$work/menu-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp . TitleMenuMain >"$menu_stdout" 2>"$menu_stderr")
menu_observed="$(<"$menu_stdout")"
menu_diagnostic="$(grep '^\[ravel\]' "$menu_stdout")"

menu_expected='result=130
buttons=1
label=Ravel Mods
buttonY=130
screen=dev.ravel.jvm.screen.RavelModsScreen
modWidgets=4
entry=Configuration Settings
settings=dev.ravel.jvm.screen.RavelSettingsScreen
entry=Profiling
profiling=dev.ravel.jvm.screen.RavelProfilingScreen
entry=gui.done
[ravel] rewrote 3 classes and 3 methods; 7 hook invocations reached native code'
menu_expected_diagnostic='[ravel] rewrote 3 classes and 3 methods; 7 hook invocations reached native code'

printf '%s\n' "$menu_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$menu_stderr"

if [ "$menu_observed" != "$menu_expected" ]; then
	echo >&2
	echo "the title-menu harness did not produce the recorded output; expected:" >&2
	printf '%s\n' "$menu_expected" >&2
	exit 1
fi

if [ "$menu_diagnostic" != "$menu_expected_diagnostic" ]; then
	echo >&2
	echo "the title-menu harness did not produce the recorded diagnostic; expected:" >&2
	printf '%s\n' "$menu_expected_diagnostic" >&2
	exit 1
fi
