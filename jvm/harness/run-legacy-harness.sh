#!/usr/bin/env bash
# Legacy-only drivers: each records only obligations its assertions genuinely cover.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
agent="${RAVEL_LEGACY_AGENT:-$root/target/ravel-jvm/ravel-agent.jar}"
host="${RAVEL_LEGACY_HOST:-$root/target/release/libravel_bootstrap.so}"
work="$root/tests/fixtures/local/legacy-platform/evidence-harness-agent"
mkdir -p "$work/classes"
run_root="$(mktemp -d "$work/run.XXXXXX")"
run_fixture() {
    local name="$1"
    shift
    mkdir -p "$run_root/$name"
    (cd "$run_root/$name" && "$@")
}
python3 "$root/jvm/legacy-platform/tools/agent.py" "$agent"
record="$root/target/ravel-jvm/legacy-harness-sections.txt"
: > "$record"
bash "$root/jvm/harness/legacy-generic/run.sh" "$agent" "$host" \
    2>&1 | tee "$work/generic.log"
cp "$root/target/ravel-jvm/legacy-generic-sections.txt" "$record"
java=(mise exec -- java --illegal-native-access=deny --module-path "$agent"
    --add-modules ALL-DEFAULT,dev.ravel.agent,java.instrument --enable-native-access=dev.ravel.agent -Dravel.legacy.proof=true
    --patch-module "dev.ravel.agent=$work/classes")
javac=(mise exec -- javac -Xlint:all -Werror)
mapfile -d '' sources < <(find "$root/jvm/harness" -maxdepth 1 -name 'Legacy*.java' ! -name 'LegacyPersistenceDataMain.java' -print0 | sort -z)
"${javac[@]}" -cp "$agent:$root/tests/fixtures/local/legacy-platform/classes" -d "$work/classes" "${sources[@]}" \
    "$root/jvm/harness/dev/ravel/jvm/LegacyCommandTest.java" \
    "$root/jvm/harness/dev/ravel/jvm/LegacyCommandLive.java"
game_cp=""
while IFS= read -r entry; do
    [[ -z "$entry" ]] || game_cp="${game_cp:+$game_cp:}$entry"
done < "$root/tests/fixtures/local/legacy-platform/server-classpath.txt"
run_fixture policy "${java[@]}" "-Dravel.host=$host" -m dev.ravel.agent/dev.ravel.jvm.LegacyRuntimeMain \
    2>&1 | tee "$work/semantics.log"
run_fixture commands "${java[@]}" -cp "$game_cp" -m dev.ravel.agent/dev.ravel.jvm.LegacyCommandTest \
    2>&1 | tee "$work/commands.log"
screen="$(dirname "$agent")/ravel-screen.jar"
# Match the live client launcher: Ravel's LWJGL 2 facade delegates to named LWJGL 3 modules.
platform="$root/tests/fixtures/local/legacy-platform"
client_cp="$platform/classes"
client_module_path="$agent"
client_modules="org.lwjgl,org.lwjgl.glfw,org.lwjgl.opengl,org.lwjgl.openal"
while read -r digest filename; do
    [[ -z "$digest" || "$digest" == \#* ]] && continue
    if [[ "$filename" == *-natives-linux.jar ]]; then
        client_cp="$client_cp:$platform/$filename"
    else
        client_module_path="$client_module_path:$platform/$filename"
    fi
done < "$root/jvm/legacy-platform/dependencies.sha256"
client_java=("${java[@]}" --module-path "$client_module_path"
    --add-modules "dev.ravel.agent,java.instrument,$client_modules"
    "--enable-native-access=dev.ravel.agent,$client_modules" "-Djava.library.path=$platform/natives")
for section in content members outward; do
    run_fixture "$section" "${client_java[@]}" "-javaagent:$agent" "-Dravel.host=$host" -cp "$client_cp" \
        -m dev.ravel.agent/dev.ravel.jvm.LegacyClientHarness \
        "$platform/client-classpath.txt" runtime 0 "$section" \
        2>&1 | tee "$work/$section.log"
    printf '%s\n' "$section" >> "$record"
done
mkdir -p "$run_root/settings-lock/ravel/config"
printf '%s\n' '{"schemaVersion":1,"owner":"configuration","values":{"dedicated.motd":"Pack","integrated.mob_limit":40}}' \
    > "$run_root/settings-lock/ravel/config/configuration.json"
for section in settings-lock settings screens; do
    side=0
    if [[ "$section" == settings-lock ]]; then side=1; fi
    run_fixture "$section" "${client_java[@]}" "-Dravel.host=$host" -cp "$client_cp" \
        -m dev.ravel.agent/dev.ravel.jvm.LegacyClientHarness \
        "$platform/client-classpath.txt" screens "$side" "$section" "$screen" \
        2>&1 | tee "$work/$section.log"
    if [[ "$section" != settings-lock ]]; then printf '%s\n' "$section" >> "$record"; fi
done
for side in dedicated integrated; do
    state="$(mktemp -d "$work/persistence-$side.XXXXXX")"
    run_fixture "persistence-$side" "${java[@]}" "-javaagent:$agent" "-Dravel.host=$host" -cp "$game_cp" \
        -m dev.ravel.agent/dev.ravel.jvm.LegacyPersistenceNativeMain "$state" "$side" "$screen" \
        2>&1 | tee "$work/persistence-$side.log"
done
printf 'persistence\n' >> "$record"
sections="$(paste -sd, "$record")"
printf '%s\n' "$sections" > "$record"
