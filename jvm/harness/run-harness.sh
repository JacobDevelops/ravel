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
javac -cp "$work:$jar:$screen" -d "$work" \
	"$root/jvm/harness/SettingsScreenMain.java" \
	"$root/jvm/harness/ProfilingScreenMain.java"

order="$work/initialization-order"
order_agent="$order/agent/ravel-agent.jar"
mkdir -p "$order/classes" "$(dirname "$order_agent")" "$order/agent-classes"
javac -Xlint:all -Werror -d "$order/agent-classes" \
	"$root/jvm/bootstrap/dev/ravel/jvm/Agent.java" \
	"$root/jvm/bootstrap/dev/ravel/jvm/HookTarget.java" \
	"$root/jvm/bootstrap/dev/ravel/jvm/Persistence.java" \
	"$root/jvm/bootstrap/dev/ravel/jvm/Screens.java" \
	"$root/jvm/bootstrap/dev/ravel/jvm/Transformer.java" \
	"$root/jvm/harness/initialization/dev/ravel/jvm/Hook.java"
mapfile -d '' hierarchy_sources < <(find "$root/jvm/harness/hierarchy" -name '*.java' -print0 | sort -z)
javac -d "$order/classes" "${hierarchy_sources[@]}"
mapfile -d '' refusal_sources < <(find "$root/jvm/harness/refusal" -name '*.java' -print0 | sort -z)
javac -d "$order/classes" "${refusal_sources[@]}"
javac -cp "$work:$order/classes:$order/agent-classes" -d "$order/classes" \
	"$root/jvm/harness/InitializationOrderMain.java" \
	"$root/jvm/harness/HierarchyResolutionMain.java" \
	"$root/jvm/harness/RefusalMain.java"
printf 'Premain-Class: dev.ravel.jvm.Agent\nCan-Retransform-Classes: false\n' >"$order/agent.manifest"
mapfile -d '' order_compiled < <(cd "$order/agent-classes" && find . -name '*.class' -print0 | sort -z)
order_entries=()
for class in "${order_compiled[@]}"; do
	order_entries+=(-C "$order/agent-classes" "${class#./}")
done
jar --create --file "$order_agent" --manifest "$order/agent.manifest" \
	--date 1980-01-01T00:00:02Z "${order_entries[@]}"
order_stdout="$order/stdout"
order_stderr="$order/stderr"
(cd "$order/classes" && java \
	-javaagent:"$order_agent" \
	-cp ".:$work" InitializationOrderMain >"$order_stdout" 2>"$order_stderr")
order_observed="$(<"$order_stdout")"
order_diagnostic="$(grep '^\[ravel\]' "$order_stderr")"
order_expected='allowed=true
filtered=a
b
simple=ab
valid=true
transformed=1 methods=4
installed=true unknown=false
[ravel] rewrote 1 classes and 4 methods; 5 hook invocations reached native code'
order_expected_diagnostic='[ravel] ravel-screen.jar is missing or not a regular file beside the agent jar'

printf '%s\n' "$order_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$order_stderr"

if [ "$order_observed" != "$order_expected" ]; then
	echo >&2
	echo "a target loaded during production initialization escaped the transformer; expected:" >&2
	printf '%s\n' "$order_expected" >&2
	exit 1
fi

if [ "$order_diagnostic" != "$order_expected_diagnostic" ]; then
	echo >&2
	echo "the initialization-order harness did not produce the recorded diagnostic; expected:" >&2
	printf '%s\n' "$order_expected_diagnostic" >&2
	exit 1
fi

hierarchy_stdout="$order/hierarchy-stdout"
hierarchy_stderr="$order/hierarchy-stderr"
(cd "$order/classes" && java \
	-javaagent:"$order_agent" \
	-Dravel.test.transform.hierarchy=true \
	-cp ".:$work" HierarchyResolutionMain >"$hierarchy_stdout" 2>"$hierarchy_stderr")
hierarchy_observed="$(<"$hierarchy_stdout")"
hierarchy_diagnostic="$(grep '^\[ravel\]' "$hierarchy_stderr")"
hierarchy_expected='before=1 methods=1
value=target
after=2 methods=2
[ravel] rewrote 2 classes and 2 methods; 2 hook invocations reached native code'

printf '%s\n' "$hierarchy_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$hierarchy_stderr"

if [ "$hierarchy_observed" != "$hierarchy_expected" ]; then
	echo >&2
	echo "hierarchy resolution loaded a later target before its own transformation; expected:" >&2
	printf '%s\n' "$hierarchy_expected" >&2
	exit 1
fi

if [ "$hierarchy_diagnostic" != "$order_expected_diagnostic" ]; then
	echo >&2
	echo "the hierarchy-resolution harness did not produce the recorded diagnostic; expected:" >&2
	printf '%s\n' "$order_expected_diagnostic" >&2
	exit 1
fi

# The opted-in prologue, both ways round. A refused call must skip the body it
# precedes and nothing else, and the same rewritten method must fall through
# untouched when the verdict is false — which is what says the early return is a
# branch rather than a removal.
for verdict in true false; do
	refusal_stdout="$order/refusal-$verdict-stdout"
	refusal_stderr="$order/refusal-$verdict-stderr"
	(cd "$order/classes" && java \
		-javaagent:"$order_agent" \
		-Dravel.test.transform.refusal=true \
		-Dravel.test.refusal.verdict="$verdict" \
		-cp ".:$work" RefusalMain >"$refusal_stdout" 2>"$refusal_stderr")
	refusal_observed="$(<"$refusal_stdout")"
	if [ "$verdict" = true ]; then
		refusal_bodies=0
	else
		refusal_bodies=1
	fi
	refusal_expected="bodies=$refusal_bodies methods=1
[ravel] rewrote 1 classes and 1 methods; 1 hook invocations reached native code"

	printf '%s\n' "$refusal_observed"
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$refusal_stderr"

	if [ "$refusal_observed" != "$refusal_expected" ]; then
		echo >&2
		echo "a prologue verdict of $verdict did not decide whether the body ran; expected:" >&2
		printf '%s\n' "$refusal_expected" >&2
		exit 1
	fi
done

# The four inventory entries that pin a field, against a stand-in game. A field
# is not a call target: the generator emits no export and no prologue for one, so
# there is nothing to rewrite and nothing to count. What each entry claims is
# that the class the transformer rewrote still declares that member under the
# pinned descriptor, so each field is driven beside a method target in its own
# class — the rewrite marker and the crossing count are what say the class going
# past was the transformed one.
members="$work/members"
mkdir -p "$members/classes"
mapfile -d '' member_sources < <(find "$root/jvm/harness/members" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -d "$members/classes" "${member_sources[@]}"
# `MembersMain` needs the agent classes only to read the rewrite marker and the
# stand-in hook's counter. A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$members/classes:$order/agent-classes" -d "$members/classes" \
	"$root/jvm/harness/MembersMain.java"
members_stdout="$members/stdout"
members_stderr="$members/stderr"
(cd "$members/classes" && java \
	-javaagent:"$order_agent" \
	-Dravel.test.transform.members=true \
	-Dravel.report.targets=true \
	-cp . MembersMain >"$members_stdout" 2>"$members_stderr")
members_observed="$(<"$members_stdout")"
members_diagnostic="$(grep '^\[ravel\]' "$members_stderr")"
members_expected='input.smooth-last-amount net.minecraft.util.SmoothDouble.lastAmount:D value=0.5 rewrote=true
registry.closed net.minecraft.core.MappedRegistry.frozen:Z value=true rewrote=true
command.table net.minecraft.commands.Commands.dispatcher:Lcom/mojang/brigadier/CommandDispatcher; value=1 rewrote=true
server.tick-count net.minecraft.server.MinecraftServer.tickCount:I value=1 rewrote=true
bodies=1 hooks=4 methods=4
[ravel] rewrote 4 classes and 4 methods; 4 hook invocations reached native code
[ravel] rewritten targets: net.minecraft.commands.Commands=1,net.minecraft.core.MappedRegistry=1,net.minecraft.server.MinecraftServer=1,net.minecraft.util.SmoothDouble=1'

printf '%s\n' "$members_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$members_stderr"

if [ "$members_observed" != "$members_expected" ]; then
	echo >&2
	echo "a pinned field did not survive its own class being rewritten; expected:" >&2
	printf '%s\n' "$members_expected" >&2
	exit 1
fi

if [ "$members_diagnostic" != "$order_expected_diagnostic" ]; then
	echo >&2
	echo "the pinned-member harness did not produce the recorded diagnostic; expected:" >&2
	printf '%s\n' "$order_expected_diagnostic" >&2
	exit 1
fi

# ADG-7's two world persistence seams, against a stand-in game. The adapter is
# reflective and reaches the sibling artifact by name, so a stand-in with the
# pinned names and descriptors exercises the real control flow: what is stood in
# for is the game, not Ravel.
persist="$work/persistence"
persist_agent="$persist/agent/ravel-agent.jar"
persist_screen="$persist/agent/ravel-screen.jar"
mkdir -p "$persist/classes" "$persist/sibling" "$(dirname "$persist_agent")"
mapfile -d '' persist_sources < <(find "$root/jvm/harness/persistence" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -d "$persist/classes" "${persist_sources[@]}"
# `PersistenceMain` needs the agent classes only to read the stand-in hook's
# counters. A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$persist/classes:$order/agent-classes" -d "$persist/classes" \
	"$root/jvm/harness/PersistenceMain.java"
javac -Xlint:all -Werror -d "$persist/sibling" \
	"$root/jvm/harness/sibling/dev/ravel/jvm/screen/RavelWorldData.java"
jar --create --file "$persist_screen" --date 1980-01-01T00:00:02Z \
	-C "$persist/sibling" dev/ravel/jvm/screen/RavelWorldData.class
cp "$order_agent" "$persist_agent"

# Each run is one world lifecycle: one open and one save. `label` is what fails
# if the run does not produce `expected`, and the properties are the two answers
# Rust would have given.
persistence_run() {
	local label="$1" open="$2" save="$3" stored="$4" expected="$5"
	local out="$persist/$label-stdout"
	local err="$persist/$label-stderr"
	(cd "$persist/classes" && java \
		-javaagent:"$persist_agent" \
		-Dravel.test.transform.persistence=true \
		-Dravel.test.persistence.open="$open" \
		-Dravel.test.persistence.save="$save" \
		-Dravel.test.persistence.stored="$stored" \
		-cp . PersistenceMain >"$out" 2>"$err")
	local observed
	observed="$(<"$out")"
	printf '%s\n' "$observed"
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$err"
	if [ "$observed" != "$expected" ]; then
		echo >&2
		echo "world persistence run '$label' did not produce the recorded output; expected:" >&2
		printf '%s\n' "$expected" >&2
		exit 1
	fi
}

# An admitted world runs its own body, keeps its lock and its stem, and the one
# replacement answer reaches the storage exactly once. The stored bytes reaching
# the crossing are what says the open path read them rather than reporting
# absence for a world that holds data.
persistence_run admitted empty replacement 'old-aggregate' 'loads=1 accessClosed=false stemClosed=false crossed=1 read=old-aggregate
saves=1 writes=1 written=replaced
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'

# Null is what the JVM receives when the native failed, and it refuses. This is
# the polarity the whole design turns on: empty admits, so a panic cannot.
persistence_run null-verdict null unchanged '' 'loads=0 accessClosed=true stemClosed=true crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'

# A non-empty verdict is a refusal carrying the diagnostic Rust owns.
persistence_run refused 'this world holds data Ravel cannot read' unchanged '' 'loads=0 accessClosed=true stemClosed=true crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'

# Every save answer that is not a replacement writes nothing, and vanilla saving
# runs in each of them. Null is one of the four, because a failed native must
# cost one Ravel checkpoint and nothing else.
for answer in unchanged failed mismatch null; do
	persistence_run "save-$answer" empty "$answer" '' 'loads=1 accessClosed=false stemClosed=false crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'
done

# The sibling artifact is absent, which is deliberately the opposite of the
# screen jar next door: an absent screen loses the settings UI and leaves a
# working game, and an absent persistence component refuses the world load. The
# crossing count says the refusal landed before the world was ever offered to
# Rust.
mv "$persist_screen" "$persist/sibling.jar"
persistence_absent_stdout='loads=0 accessClosed=true stemClosed=true crossed=0 read=absent
saves=1 writes=0 written=
[ravel] rewrote 2 classes and 2 methods; 3 hook invocations reached native code'
persistence_run absent-sibling empty unchanged '' "$persistence_absent_stdout"
absent_diagnostic="$(grep '^\[ravel\]' "$persist/absent-sibling-stderr")"
absent_expected_diagnostic='[ravel] ravel-screen.jar is missing or not a regular file beside the agent jar
[ravel] refused a world load: Ravel'"'"'s world persistence component is missing from this installation.'
if [ "$absent_diagnostic" != "$absent_expected_diagnostic" ]; then
	echo >&2
	echo "an absent sibling artifact did not refuse with the recorded diagnostic; expected:" >&2
	printf '%s\n' "$absent_expected_diagnostic" >&2
	exit 1
fi
mv "$persist/sibling.jar" "$persist_screen"

failure_stdout="$order/failure-stdout"
failure_stderr="$order/failure-stderr"
(cd "$order/classes" && java \
	-javaagent:"$order_agent" \
	-Dravel.test.transform.failure=true \
	-cp ".:$work" InitializationOrderMain >"$failure_stdout" 2>"$failure_stderr")
failure_observed="$(<"$failure_stdout")"
failure_diagnostic="$(grep '^\[ravel\]' "$failure_stderr")"
failure_expected='allowed=true
filtered=a
b
simple=ab
valid=true
transformed=0 methods=0
installed=false unknown=false
[ravel] rewrote 0 classes and 0 methods; 0 hook invocations reached native code
[ravel] failed to transform 1 classes
[ravel] failed target: net.minecraft.util.StringUtil=java.lang.IllegalArgumentException'
failure_expected_diagnostic='[ravel] failed to transform net.minecraft.util.StringUtil (java.lang.IllegalArgumentException)
[ravel] ravel-screen.jar is missing or not a regular file beside the agent jar'

printf '%s\n' "$failure_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$failure_stderr"

if [ "$failure_observed" != "$failure_expected" ]; then
	echo >&2
	echo "a failed transformation did not preserve the original class and report its type; expected:" >&2
	printf '%s\n' "$failure_expected" >&2
	exit 1
fi

if [ "$failure_diagnostic" != "$failure_expected_diagnostic" ]; then
	echo >&2
	echo "a failed transformation did not produce the redacted diagnostic; expected:" >&2
	printf '%s\n' "$failure_expected_diagnostic" >&2
	exit 1
fi

# `java.library.path` is the directory holding the library, not the file.
stdout="$work/stdout"
stderr="$work/stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-Dravel.report.targets=true \
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
mods=1:16:client1:01:21:113:configuration7:running8:required4:true5:false1:01:210:saved_data7:running8:required4:true5:false1:0
[ravel] rewrote 1 classes and 4 methods; 5 hook invocations reached native code
[ravel] rewritten targets: net.minecraft.util.StringUtil=4'
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
mods=1:16:client1:01:21:113:configuration7:running8:required4:true5:false1:01:210:saved_data7:running8:required4:true5:false1:0
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

# The profiling report at the window sizes a GUI scale produces. What is checked
# is readability rather than construction: the screen always built, and the two
# longest lines it built still rendered only their tails, because a value wider
# than its box scrolls to the cursor `setValue` parks at the end. `fits` is that
# defect, `within` is the same defect downwards once splitting spends rows, and
# `caveat` is the sentence the row exists for read back out of the rows it was
# split across.
profiling_stdout="$work/profiling-stdout"
profiling_stderr="$work/profiling-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp ".:$screen" ProfilingScreenMain >"$profiling_stdout" 2>"$profiling_stderr")
profiling_observed="$(<"$profiling_stdout")"
# `rows` climbs with the window because the report gives its spare rows away in
# a fixed order, so the number is what says the priority ladder still holds: the
# two smallest sizes have nothing left over and draw the summary and the
# statement alone.
profiling_expected='size=320x240 rows=7 fits=true within=true caveat=whole
size=480x270 rows=8 fits=true within=true caveat=whole
size=640x360 rows=10 fits=true within=true caveat=whole
size=960x540 rows=10 fits=true within=true caveat=whole
[ravel] rewrote 1 classes and 1 methods; 4 hook invocations reached native code'

printf '%s\n' "$profiling_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$profiling_stderr"

if [ "$profiling_observed" != "$profiling_expected" ]; then
	echo >&2
	echo "the profiling screen did not draw every line readably; expected:" >&2
	printf '%s\n' "$profiling_expected" >&2
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
mods=1:16:client1:01:21:113:configuration7:running8:required4:true5:false1:01:210:saved_data7:running8:required4:true5:false1:0
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
modWidgets=6
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
