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
	echo "  cargo build -p ravel-composed-host --release" >&2
	exit 1
fi

rm -rf "$work"
mkdir -p "$(dirname "$agent")"
printf 'version:1\n' >"$work/options.txt"
mkdir -p "$work/ravel-pack"
cat >"$work/ravel-pack/configuration.json" <<'EOF'
{
  "schemaVersion": 1,
  "owner": "configuration",
  "values": {
    "client.enabled": false,
    "integrated.mob_limit": 40,
    "dedicated.motd": "Pack"
  }
}
EOF
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
javac -cp "$work:$jar:$screen" -d "$work" "$root/jvm/harness/SettingsScreenMain.java"

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
client=false:pack:false
common=quiet:default:false
saved=1:0
invalid=1:114:client.enabled78:`client.enabled` was left unchanged: the setting holds a boolean, not a string
clientAfter=false:player:false
commonAfter=quiet:default:true
mods=1:16:client1:01:11:113:configuration7:running8:required4:true5:false1:0
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

printf 'not a jar\n' >"$(dirname "$agent")/ravel-screen.jar"
invalid_stdout="$work/invalid-stdout"
invalid_stderr="$work/invalid-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp . TitleMenuMissingMain >"$invalid_stdout" 2>"$invalid_stderr")
rm "$(dirname "$agent")/ravel-screen.jar"
invalid_observed="$(<"$invalid_stdout")"
invalid_diagnostic="$(grep '^\[ravel\]' "$invalid_stderr")"
invalid_expected_diagnostic='[ravel] ravel-screen.jar is unreadable or invalid (java.util.zip.ZipException)'

printf '%s\n' "$invalid_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$invalid_stderr"

if [ "$invalid_observed" != "$missing_expected" ]; then
	echo >&2
	echo "the invalid-screen title route did not preserve the original behavior; expected:" >&2
	printf '%s\n' "$missing_expected" >&2
	exit 1
fi

if [ "$invalid_diagnostic" != "$invalid_expected_diagnostic" ]; then
	echo >&2
	echo "the invalid-screen title route did not produce the stable diagnostic; expected:" >&2
	printf '%s\n' "$invalid_expected_diagnostic" >&2
	exit 1
fi

relaunch_stdout="$work/relaunch-stdout"
relaunch_stderr="$work/relaunch-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp . Main >"$relaunch_stdout" 2>"$relaunch_stderr")
relaunch_observed="$(<"$relaunch_stdout")"
relaunch_diagnostic="$(grep '^\[ravel\]' "$relaunch_stderr")"
relaunch_expected='allowed=true
filtered=a
b
simple=ab
valid=true
transformed=1 methods=4
client=false:player:false
common=verbose:player:false
saved=1:0
invalid=1:114:client.enabled78:`client.enabled` was left unchanged: the setting holds a boolean, not a string
clientAfter=false:player:false
commonAfter=verbose:player:false
mods=1:16:client1:01:11:113:configuration7:running8:required4:true5:false1:0
[ravel] rewrote 1 classes and 4 methods; 5 hook invocations reached native code'

printf '%s\n' "$relaunch_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$relaunch_stderr"

if [ "$relaunch_observed" != "$relaunch_expected" ]; then
	echo >&2
	echo "the relaunched harness did not read the persisted settings; expected:" >&2
	printf '%s\n' "$relaunch_expected" >&2
	exit 1
fi

if [ "$relaunch_diagnostic" != "$expected_diagnostic" ]; then
	echo >&2
	echo "the relaunched harness did not produce the recorded diagnostic; expected:" >&2
	printf '%s\n' "$expected_diagnostic" >&2
	exit 1
fi
cp "$screen" "$(dirname "$agent")/ravel-screen.jar"

settings_stdout="$work/settings-stdout"
settings_stderr="$work/settings-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp ".:$screen" SettingsScreenMain >"$settings_stdout" 2>"$settings_stderr")
settings_observed="$(<"$settings_stdout")"
settings_expected='owner=configuration
refused=
crossings=1:1
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

printf '%s\n' "$settings_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$settings_stderr"

if [ "$settings_observed" != "$settings_expected" ]; then
	echo >&2
	echo "the generated settings screen did not preserve state and cross exactly twice; expected:" >&2
	printf '%s\n' "$settings_expected" >&2
	exit 1
fi

refusal_work="$work/refusal"
mkdir "$refusal_work"
printf 'version:1\n' >"$refusal_work/options.txt"
refusal_stdout="$work/refusal-stdout"
refusal_stderr="$work/refusal-stderr"
(cd "$refusal_work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp "$work:$screen" SettingsScreenMain refuse-save >"$refusal_stdout" 2>"$refusal_stderr")
refusal_observed="$(<"$refusal_stdout")"
refusal_expected='owner=configuration
refusedPresent=true
refusedPrivatePath=false
crossings=1:1
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

printf '%s\n' "$refusal_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$refusal_stderr"

if [ "$refusal_observed" != "$refusal_expected" ]; then
	echo >&2
	echo "the generated settings screen did not surface a redacted save refusal; expected:" >&2
	printf '%s\n' "$refusal_expected" >&2
	exit 1
fi

ui_relaunch_stdout="$work/ui-relaunch-stdout"
ui_relaunch_stderr="$work/ui-relaunch-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp . Main >"$ui_relaunch_stdout" 2>"$ui_relaunch_stderr")
ui_relaunch_observed="$(<"$ui_relaunch_stdout")"
ui_relaunch_expected='allowed=true
filtered=a
b
simple=ab
valid=true
transformed=1 methods=4
client=true:player:false
common=quiet:player:false
saved=1:0
invalid=1:114:client.enabled78:`client.enabled` was left unchanged: the setting holds a boolean, not a string
clientAfter=false:player:false
commonAfter=quiet:player:true
mods=1:16:client1:01:11:113:configuration7:running8:required4:true5:false1:0
[ravel] rewrote 1 classes and 4 methods; 5 hook invocations reached native code'

printf '%s\n' "$ui_relaunch_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$ui_relaunch_stderr"

if [ "$ui_relaunch_observed" != "$ui_relaunch_expected" ]; then
	echo >&2
	echo "the JVM relaunch did not read the settings-screen edit from disk; expected:" >&2
	printf '%s\n' "$ui_relaunch_expected" >&2
	exit 1
fi

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
modWidgets=5
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
