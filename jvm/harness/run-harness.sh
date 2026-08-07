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
library="$root/target/release"
work="$root/target/ravel-jvm/harness"
agent="$work/agent/ravel-agent.jar"

if [ ! -f "$jar" ]; then
	echo "the agent jar is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter && ./jvm/build/build-agent.sh" >&2
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
