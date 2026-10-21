#!/usr/bin/env bash
# Run after generating/building 1.12.2; never builds production artifacts.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
jar="$(realpath "${1:?usage: run.sh AGENT_JAR NATIVE_LIBRARY}")"
host="$(realpath "${2:?usage: run.sh AGENT_JAR NATIVE_LIBRARY}")"
java="${JAVA:-java}"
javac="${JAVAC:-javac}"
jar_tool="${JAR:-jar}"
sources="$root/jvm/harness/legacy-generic"
mkdir -p "$root/target/ravel-jvm"
work="$(mktemp -d "$root/target/ravel-jvm/legacy-generic.XXXXXX")"
record="$root/target/ravel-jvm/legacy-generic-sections.txt"
: >"$record"
section() { printf '%s\n' "$1" >>"$record"; }
jvm=("$java" -Xshare:off -Xverify:all --illegal-native-access=deny -Dravel.legacy.proof=true)
mkdir -p "$work/classes" "$work/fixture" "$work/entries" "$work/missing"
"$javac" -cp "$jar" -d "$work/classes" "$sources/GenericMain.java" "$sources/NativeMain.java" \
    "$root/jvm/harness/LegacyGeometryProofMain.java" "$root/jvm/harness/LegacyRetainedMeshMain.java"
"$javac" -cp "$jar" -d "$work/fixture" "$sources/fixture/dev/ravel/jvm/Hook.java"
"$javac" -cp "$work/fixture:$jar" -d "$work/fixture" \
    "$sources/FixtureMain.java" "$sources/HierarchyFixture.java" \
    "$sources/InitTarget.java" "$sources/InitializationMain.java" \
    "$sources/ExecutableTarget.java" "$sources/ExecutableMain.java" \
    "$sources/PaletteFixture.java" "$sources/PaletteMain.java"
native=("${jvm[@]}" --module-path "$jar" --add-modules "dev.ravel.agent,java.instrument"
    --enable-native-access=dev.ravel.agent --patch-module "dev.ravel.agent=$work/classes")
for renderer in vanilla geometry-proof world; do
    "${native[@]}" "-Dravel.renderer=$renderer" \
        -m dev.ravel.agent/dev.ravel.jvm.LegacyGeometryProofMain
done
"${native[@]}" -m dev.ravel.agent/dev.ravel.jvm.LegacyRetainedMeshMain
(cd "$work" && "${native[@]}" -m dev.ravel.agent/dev.ravel.jvm.NativeMain prepare \
    "$work/entries" "$work/missing" "$work/entry-names")
mapfile -t entries <"$work/entry-names"
client="${entries[0]}"
server="${entries[1]}"

# Real generated inventory: every method must contain native and adapter calls.
(cd "$work" && "${native[@]}" -Dravel.host="$host" -m dev.ravel.agent/dev.ravel.jvm.GenericMain)
"${jvm[@]}" -cp "$work/fixture:$jar" dev.ravel.jvm.ExecutableMain
# ExecutableMain checks the actual legacy fire-damage refusal crossing.
section refusal
"${jvm[@]}" -cp "$work/fixture:$jar" dev.ravel.jvm.FixtureMain descriptors
"${jvm[@]}" -cp "$work/fixture:$jar" dev.ravel.jvm.PaletteMain
section transformation
"${jvm[@]}" -cp "$work/fixture:$jar" dev.ravel.jvm.FixtureMain hierarchy
section hierarchy

# Preserve production Boot/Agent/Transformer, replacing only Hook in a private artifact.
mkdir -p "$work/order-agent"
cp "$jar" "$work/order-agent/ravel-agent.jar"
"$jar_tool" --update --file "$work/order-agent/ravel-agent.jar" \
    -C "$work/fixture" dev/ravel/jvm/Hook.class
for side in client dedicated-server; do
    entry="$client"
    if [ "$side" = dedicated-server ]; then entry="$server"; fi
    directory="$work/order-$side"
    mkdir -p "$directory"
    (cd "$directory" && "${jvm[@]}" --module-path "$work/order-agent/ravel-agent.jar" \
        --add-modules dev.ravel.agent,java.instrument --enable-native-access=dev.ravel.agent \
        --patch-module "dev.ravel.agent=$work/classes:$work/fixture" \
        -javaagent:"$work/order-agent/ravel-agent.jar" \
        -cp "$work/entries" "$entry" "$side" initialization-fixture)
done
section initialization

# Each native process gets an empty game directory, never another launch's state.
launch() {
    local label="$1" entry="$2" side="$3" mode="$4"
    local directory="$work/$label"
    mkdir -p "$directory"
    printf 'version:1\n' >"$directory/options.txt"
    (cd "$directory" && "${native[@]}" -javaagent:"$jar" -Dravel.host="$host" \
        -Dravel.profiling=true -cp "$work/entries" "$entry" "$side" "$mode")
}
for side in client dedicated-server; do
    entry="$client"
    if [ "$side" = dedicated-server ]; then entry="$server"; fi
    launch "side-$side" "$entry" "$side" side
done
mkdir -p "$work/none" "$work/no-main"
(cd "$work/none" && "${native[@]}" -javaagent:"$jar" -Dravel.host="$host" \
    -m dev.ravel.agent/dev.ravel.jvm.NativeMain none >"$work/none.out")
cat "$work/none.out"
grep -qF 'no distribution entry point loaded, so no side was named and no mod set was installed' "$work/none.out"
(cd "$work/no-main" && "${native[@]}" -javaagent:"$jar" -Dravel.host="$host" \
    -cp "$work/missing" -m dev.ravel.agent/dev.ravel.jvm.NativeMain missing >"$work/missing.out")
cat "$work/missing.out"
grep -qF 'loaded and Ravel' "$work/missing.out"
grep -qF 'missing entry method' "$work/missing.out"
section side
launch load-client "$client" client load
launch load-server "$server" dedicated-server load
section load
launch cluster-client "$client" client cluster
launch cluster-server "$server" dedicated-server cluster
section cluster
launch renderer-client "$client" client renderer
printf 'legacy generic evidence: %s\n' "$work"
