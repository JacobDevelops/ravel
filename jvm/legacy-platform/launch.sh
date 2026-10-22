#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
module="$root/jvm/legacy-platform"
if [[ ! -v ALSOFT_DRIVERS ]]; then
	source "$module/launcher.env"
fi
export ALSOFT_DRIVERS
work="$root/tests/fixtures/local/legacy-platform"
mkdir -p "$work"
java=(mise exec -- java)
surface=("${java[@]}" "-Dravel.legacy.owners=$module/src/main/resources/dev/ravel/legacy/adapter-owners.txt")
javac=(mise exec -- javac)
parent_cp="$work/classes"
module_path=""
modules=org.lwjgl,org.lwjgl.glfw,org.lwjgl.opengl,org.lwjgl.openal
action="${1:-}"
agent_options=()
suffix=""
client_native_modules="$modules"
server_native_options=()
if [[ "$action" == client-agent || "$action" == server-agent ]]; then
    case "${RAVEL_LEGACY_RENDERER:-vanilla}" in
        vanilla|geometry-proof) ;;
        *) echo 'RAVEL_LEGACY_RENDERER must be vanilla or geometry-proof' >&2; exit 1 ;;
    esac
    action="${action%-agent}"
    suffix="-agent"
    agent="${RAVEL_LEGACY_AGENT:-$root/target/ravel-jvm/ravel-agent.jar}"
    host="${RAVEL_LEGACY_HOST:-$root/target/release/libravel_bootstrap.so}"
    [[ -f "$agent" && -f "$host" ]] || { echo 'Build the generated 1.12.2 agent and composed host first' >&2; exit 1; }
    python3 "$module/tools/agent.py" "$agent"
    module_path="$agent"
    client_native_modules="$modules,dev.ravel.agent"
    server_native_options=(--module-path "$agent" --enable-native-access=dev.ravel.agent)
    agent_options=(--add-modules dev.ravel.agent "-javaagent:$agent"
        "-Dravel.host=$host" -Dravel.report.targets=true -Dravel.report.loader=true
        -Dravel.legacy.acceptance=true -Dravel.legacy.proof=true)
    if [[ "$action" == client && "${RAVEL_LEGACY_RENDERER:-vanilla}" == geometry-proof ]]; then
        agent_options+=(-Dravel.renderer=geometry-proof)
    fi
    if [[ -n "${RAVEL_LEGACY_PROBES:-}" ]]; then
        agent_options+=(--patch-module "dev.ravel.agent=$RAVEL_LEGACY_PROBES"
            -Dravel.legacy.serverProbe=dev.ravel.jvm.LegacyServerLive
            -Dravel.legacy.clientProbe=dev.ravel.jvm.LegacyClientLive)
    fi
    if [[ -n "${RAVEL_LEGACY_SERVER:-}" ]]; then
        agent_options+=("-Dravel.legacy.server=$RAVEL_LEGACY_SERVER")
    fi
    if [[ -n "${RAVEL_PACKET_LIVE:-}" ]]; then
        agent_options+=("-Dravel.packet.live=$RAVEL_PACKET_LIVE")
    fi
fi
api_jars=()
while read -r digest filename; do
    [[ -z "$digest" || "$digest" == \#* ]] && continue
    artifact="${filename%%-3.3.6*}"
    if [[ ! -f "$work/$filename" ]]; then
        curl --fail --location --retry 3 "https://repo.maven.apache.org/maven2/org/lwjgl/$artifact/3.3.6/$filename" -o "$work/$filename.download"
        printf '%s  %s\n' "$digest" "$work/$filename.download" | sha256sum --check --status
        mv "$work/$filename.download" "$work/$filename"
    fi
    printf '%s  %s\n' "$digest" "$work/$filename" | sha256sum --check --status
    if [[ "$filename" == *-natives-linux.jar ]]; then
        parent_cp="$parent_cp:$work/$filename"
    else
        module_path="${module_path:+$module_path:}$work/$filename"
        api_jars+=("$work/$filename")
    fi
done < "$module/dependencies.sha256"
modular=("${java[@]}" --module-path "$module_path" --add-modules "$modules" --illegal-native-access=deny)
javac+=(--module-path "$module_path" --add-modules "$modules")
test_cp="$work/tests:$parent_cp"
if [[ "${1:-}" == build ]]; then
    python3 "$module/tools/acquire.py"
    rm -rf "$work/classes" "$work/tools" "$work/tests"
    mkdir -p "$work/classes" "$work/tools" "$work/tests"
    cp -R "$module/src/main/resources/." "$work/classes/"
    mapfile -d '' sources < <(find "$module/src/main/java" -name '*.java' -print0 | sort -z)
    "${javac[@]}" -Xlint:all -Werror -cp "$parent_cp" -d "$work/classes" "${sources[@]}"
    "${javac[@]}" -Xlint:all -Werror -cp "$parent_cp" -d "$work/tools" "$module/tools/LegacySurface.java"
    mapfile -d '' test_sources < <(find "$module/tests" -name '*.java' -print0 | sort -z)
    "${javac[@]}" -Xlint:all -Werror -cp "$parent_cp" -d "$work/tests" "${test_sources[@]}"
    mise exec -- jar --create --file "$work/platform.jar" -C "$work/classes" .
    "${surface[@]}" -cp "$work/tools" LegacySurface "$work/client.jar" > "$work/vanilla-inventory.tsv"
    cmp "$module/vanilla-1.12.2-lwjgl.tsv" "$work/vanilla-inventory.tsv"
    "${surface[@]}" -cp "$work/tools" LegacySurface --adapted "$work/client.jar" \
        "$work/platform.jar" "${api_jars[@]}" > "$work/vanilla-linkage.tsv"
    audio_library="$work/libraries/com/paulscode/librarylwjglopenal/20100824/librarylwjglopenal-20100824.jar"
    "${surface[@]}" -cp "$work/tools" LegacySurface --adapted "$audio_library" \
        "$work/platform.jar" "${api_jars[@]}" > "$work/audio-linkage.tsv"
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.ClientLinkageTest \
        "$work/client-classpath.txt" "$work/client.jar" "$work/vanilla-linkage.tsv" "$work/audio-linkage.tsv"
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.MatrixMathTest
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.opengl.MeshWireTest
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.launch.LegacyShutdownTest
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.SysTimerTest
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.input.KeyboardTranslationTest
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.input.MouseTranslationTest
    "${modular[@]}" -cp "$test_cp" dev.ravel.legacy.launch.LoaderRewriteTest \
        "$work/libraries/com/mojang/text2speech/1.10.3/text2speech-1.10.3.jar"
    exit 0
fi
if [[ ! -f "$work/classes/dev/ravel/legacy/launch/LegacyLauncher.class" ]]; then
    echo 'Run jvm/legacy-platform/launch.sh build first' >&2
    exit 1
fi
preflight=false
if [[ "${RAVEL_LEGACY_PREFLIGHT:-0}" == 1 ]]; then preflight=true; fi
jvm=("${modular[@]}" "--enable-native-access=$client_native_modules" "${agent_options[@]}" -Xmx2G
    -XX:ErrorFile="$work/hs_err_pid%p.log"
    "-Dravel.harness.settings=$module/harness-client.properties"
    "-Dravel.legacy.preflight=$preflight"
    -Djava.library.path="$work/natives" -Dlog4j2.formatMsgNoLookups=true -cp "$parent_cp"
    dev.ravel.legacy.launch.LegacyLauncher)
case "$action" in
    client)
        mode="${2:-xvfb}"
        [[ "$mode" == xvfb || "$mode" == display ]] || { echo 'client mode must be xvfb or display' >&2; exit 1; }
        python3 "$module/tools/acquire.py" --assets
        game="$work/client-$mode$suffix"
        evidence="$work/evidence-$mode$suffix${RAVEL_LEGACY_EVIDENCE_SUFFIX:-}"
        mkdir -p "$game" "$evidence"
        rm -f "$evidence/automation.complete"
        : > "$evidence/player-states.txt"
        : > "$evidence/ticks.txt"
        python3 "$module/tools/harness_settings.py" "$game/options.txt"
        while IFS=: read -r option value; do
            case "$option" in
                overrideWidth) width="$value" ;;
                overrideHeight) height="$value" ;;
            esac
        done < "$game/options.txt"
        command=("${jvm[@]}" client "$work/client-classpath.txt" "$evidence"
            --version 1.12.2 --gameDir "$game" --assetsDir "$work/assets" --assetIndex 1.12
            --username RavelLegacy --uuid 00000000000000000000000000000001 --accessToken 0
            --userType legacy --versionType release --width "$width" --height "$height")
        (
            if [[ -n "$suffix" ]]; then cd "$game"; fi
            if [[ "$mode" == xvfb ]]; then
                export LIBGL_ALWAYS_SOFTWARE=1
                timeout --kill-after=10s 300s xvfb-run -a -s '-screen 0 1280x720x24 +extension GLX' \
                    "${command[@]}"
            else
                timeout --kill-after=10s 300s "${command[@]}"
            fi
        ) 2>&1 | tee "$evidence/client.log"
        test -f "$evidence/automation.complete"
        if grep -Eq '/(ERROR|FATAL)\]|\[STDERR\]:' "$evidence/client.log"; then
            echo 'Client reported a runtime error' >&2
            exit 1
        fi
        ;;
    native-audit)
        evidence="$work/evidence-native-access"
        mkdir -p "$evidence"
        mise exec -- jar --extract --file "$work/lwjgl-3.3.6-natives-linux.jar" \
            --dir "$work/natives" linux/x64/org/lwjgl/liblwjgl.so
        native="$work/natives/linux/x64/org/lwjgl"
        if "${modular[@]}" -Dorg.lwjgl.librarypath="$native" -Dorg.lwjgl.util.Debug=true -Dorg.lwjgl.util.DebugLoader=true \
            -cp "$test_cp" dev.ravel.legacy.NativeAccessProbe > "$evidence/denied.log" 2>&1; then
            echo 'Native access unexpectedly succeeded without a grant' >&2
            exit 1
        fi
        grep -q 'module=org.lwjgl nativeAccess=false ravelNativeAccess=false' "$evidence/denied.log"
        grep -q 'UnsatisfiedLinkError: Failed to locate library: liblwjgl.so' "$evidence/denied.log"
        "${modular[@]}" "--enable-native-access=$modules" -Dorg.lwjgl.librarypath="$native" \
            -cp "$test_cp" dev.ravel.legacy.NativeAccessProbe | tee "$evidence/granted.log"
        "${modular[@]}" "--enable-native-access=$modules" \
            -cp "$test_cp" dev.ravel.legacy.openal.AudioLifecycleTest \
            2>&1 | tee "$evidence/audio.log"
        ;;
    server)
        game="$work/dedicated$suffix"
        evidence="$work/evidence-server$suffix${RAVEL_LEGACY_EVIDENCE_SUFFIX:-}"
        mkdir -p "$game"
        printf 'eula=true\n' > "$game/eula.txt"
        printf 'level-seed=8675309\nlevel-name=ravel-legacy\nonline-mode=false\nserver-ip=127.0.0.1\nserver-port=25586\nview-distance=2\nmax-tick-time=120000\n' > "$game/server.properties"
        RAVEL_LEGACY_EVIDENCE="$evidence" python3 "$module/tools/server.py" "$game" "${java[@]}" \
            --illegal-native-access=deny "${server_native_options[@]}" "${agent_options[@]}" -Xmx2G -Dlog4j2.formatMsgNoLookups=true \
            "-Xlog:library=info:file=$evidence/libraries.log" \
            -cp "$work/classes" dev.ravel.legacy.launch.LegacyLauncher \
            server "$work/server-classpath.txt" "$evidence" nogui
        ;;
    *) echo 'usage: launch.sh build | client xvfb|display | client-agent xvfb|display | native-audit | server | server-agent' >&2; exit 1 ;;
esac
