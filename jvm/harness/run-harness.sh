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
mkdir -p "$work/ravel/config"
cat >"$work/ravel/config/configuration.json" <<'EOF'
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
# The generated game and screen names come from the same tree the agent jar is
# built from.
# Only `Hook` is stood in for here, because it is the one class that would load
# the native library; the names the adapters resolve are the real ones.
# Swept rather than listed, because a list of the bootstrap's classes is a
# second inventory of them: a source added there compiled into the agent jar and
# not into this one, and the harness went on proving a jar the launch no longer
# ships. The generated names are named one by one because only `Hook` is stood
# in for, and its stand-in has to win over the generated one.
mapfile -d '' agent_sources < <(find "$root/jvm/bootstrap" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -d "$order/agent-classes" \
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/GameNames.java" \
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/GameMembers.java" \
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/ScreenNames.java" \
	"${agent_sources[@]}" \
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
# Against the stand-in game, because the sibling is the compiled half of the
# seam and its argument vectors are the game's own types.
javac -Xlint:all -Werror -cp "$persist/classes" -d "$persist/sibling" \
	"$root/jvm/harness/sibling/dev/ravel/jvm/screen/RavelWorldData.java" \
	"$root/jvm/harness/sibling/dev/ravel/jvm/screen/RavelWorldRefusedScreen.java"
jar --create --file "$persist_screen" --date 1980-01-01T00:00:02Z \
	-C "$persist/sibling" dev/ravel/jvm/screen/RavelWorldData.class \
	-C "$persist/sibling" dev/ravel/jvm/screen/RavelWorldRefusedScreen.class
cp "$order_agent" "$persist_agent"

# How many world lifecycles this section runs, declared in one place so
# `evidence/m3` prose can be bound to a figure rather than restating an
# uncounted one. It is asserted below against the runs that actually happened,
# so adding or removing a case fails here instead of quietly falsifying the
# records that cite it.
persistence_lifecycles_declared=9
persistence_lifecycles=0

# Each run is one world lifecycle: one open and one save. `label` is what fails
# if the run does not produce `expected`, and the properties are the two answers
# Rust would have given.
persistence_run() {
	local label="$1" open="$2" save="$3" stored="$4" expected="$5" save_seam="${6:-false}"
	persistence_lifecycles=$((persistence_lifecycles + 1))
	local out="$persist/$label-stdout"
	local err="$persist/$label-stderr"
	(cd "$persist/classes" && java \
		-javaagent:"$persist_agent" \
		-Dravel.test.transform.persistence=true \
		-Dravel.test.persistence.saveSeamMissing="$save_seam" \
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
# working game, and an absent persistence component refuses the world load.
#
# It is also the one refusal with nothing to draw, so it is the one that throws.
# Every other refusal above puts the player on the refusal screen and returns,
# and the body never running is what says the world was refused. Here there is no
# screen jar and so no screen, and returning would leave the client on the
# loading screen it was already on, holding the session lock, with nothing
# coming — which is what the first real launch against an installation missing
# this jar sat on until it was killed. So the refusal leaves through the game's
# own call and the game's own crash path reports it. The access is closed before
# the throw, so the world's lock is already released.
mv "$persist_screen" "$persist/sibling.jar"
persistence_lifecycles=$((persistence_lifecycles + 1))
absent_out="$persist/absent-sibling-stdout"
absent_err="$persist/absent-sibling-stderr"
absent_status=0
(cd "$persist/classes" && java \
	-javaagent:"$persist_agent" \
	-Dravel.test.transform.persistence=true \
	-Dravel.test.persistence.saveSeamMissing=false \
	-Dravel.test.persistence.open=empty \
	-Dravel.test.persistence.save=unchanged \
	-Dravel.test.persistence.stored= \
	-cp . PersistenceMain >"$absent_out" 2>"$absent_err") || absent_status=$?
if [ "$absent_status" -eq 0 ]; then
	echo >&2
	echo "a refusal with no screen to show returned instead of throwing, which is the launch that hangs on the loading screen holding the world lock" >&2
	exit 1
fi
# The agent's own shutdown line lands on stdout, so what says the body never ran
# is the absence of the counters `PersistenceMain` prints after it.
if grep -q '^loads=' "$absent_out"; then
	echo >&2
	echo "a refused world load ran the body it precedes; it printed:" >&2
	cat "$absent_out" >&2
	exit 1
fi
absent_diagnostic="$(grep '^\[ravel\]' "$absent_err")"
absent_expected_diagnostic='[ravel] ravel-screen.jar is missing or not a regular file beside the agent jar
[ravel] refused a world load: Ravel'"'"'s world persistence component is missing from this installation.'
if [ "$absent_diagnostic" != "$absent_expected_diagnostic" ]; then
	echo >&2
	echo "an absent sibling artifact did not refuse with the recorded diagnostic; expected:" >&2
	printf '%s\n' "$absent_expected_diagnostic" >&2
	exit 1
fi
absent_thrown="$(grep -c 'IllegalStateException: Ravel refused this world load and could not show the reason on screen' "$absent_err")"
if [ "$absent_thrown" -ne 1 ]; then
	echo >&2
	echo "a refusal with no screen did not name itself in what it threw" >&2
	exit 1
fi
mv "$persist/sibling.jar" "$persist_screen"

# The save seam is not installed, which is the other refusal ADG-7 specifies and
# the only one that needs the marker rather than a crossing to detect it. The
# server class is present and loads — `Persistence.saveSeamInstalled` loads it on
# purpose — but its transformation matched no method, so the installed-prologue set
# is empty and the world is refused before Rust is ever asked. Without this case
# the false branch of that method never executes anywhere in the gate.
persistence_missing_save_stdout='loads=0 accessClosed=true stemClosed=true crossed=0 read=absent
saves=1 writes=0 written=
[ravel] rewrote 1 classes and 1 methods; 1 hook invocations reached native code'
persistence_run missing-save-seam empty unchanged '' "$persistence_missing_save_stdout" true
missing_save_diagnostic="$(grep '^\[ravel\] refused' "$persist/missing-save-seam-stderr")"
missing_save_expected_diagnostic='[ravel] refused a world load: Ravel'"'"'s world-save hook was not installed, so this world'"'"'s mod data could not be saved again.'
if [ "$missing_save_diagnostic" != "$missing_save_expected_diagnostic" ]; then
	echo >&2
	echo "an uninstalled save seam did not refuse with the recorded diagnostic; expected:" >&2
	printf '%s\n' "$missing_save_expected_diagnostic" >&2
	exit 1
fi

# The declared figure is what `evidence/m3` cites, so it has to be the figure
# this run produced.
if [ "$persistence_lifecycles" -ne "$persistence_lifecycles_declared" ]; then
	echo >&2
	echo "this harness ran $persistence_lifecycles world lifecycles and declares $persistence_lifecycles_declared; update persistence_lifecycles_declared and the evidence/m3 records that cite it" >&2
	exit 1
fi

# The content-registration seam, against a stand-in game. The adapter is
# reflective and reads the admitted set across its own crossing, so a stand-in
# carrying the pinned names and descriptors exercises the real control flow:
# what is stood in for is the game, not Ravel. The stand-in registries refuse a
# duplicate and refuse everything once frozen, and the properties objects refuse
# to describe a block or an item that was never given an id — so a registration
# that only looked like one fails here.
content="$work/content"
content_agent="$content/agent/ravel-agent.jar"
content_screen="$content/agent/ravel-screen.jar"
mkdir -p "$content/classes" "$content/sibling" "$(dirname "$content_agent")"
mapfile -d '' content_sources < <(find "$root/jvm/harness/content" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -d "$content/classes" "${content_sources[@]}"
# `ContentMain` needs the agent classes only to read the stand-in hook's counter.
# A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$content/classes:$order/agent-classes" -d "$content/classes" \
	"$root/jvm/harness/ContentMain.java"
# The registration itself lives in the sibling artifact now, because the two
# tracked versions build a block differently rather than name it differently, so
# this section needs the sibling jar the persistence section already needed.
javac -Xlint:all -Werror -cp "$content/classes" -d "$content/sibling" \
	"$root/jvm/harness/sibling/dev/ravel/jvm/screen/RavelContent.java"
jar --create --file "$content_screen" --date 1980-01-01T00:00:02Z \
	-C "$content/sibling" dev/ravel/jvm/screen/RavelContent.class
cp "$order_agent" "$content_agent"

# Each run is one initialization: the registries freeze once, and what they hold
# afterwards is the whole answer.
content_run() {
	local label="$1" admitted="$2" expected="$3"
	local out="$content/$label-stdout"
	local err="$content/$label-stderr"
	(cd "$content/classes" && java \
		-javaagent:"$content_agent" \
		-Dravel.test.transform.content=true \
		-Dravel.test.content.admitted="$admitted" \
		-cp . ContentMain >"$out" 2>"$err")
	local observed
	observed="$(<"$out")"
	printf '%s\n' "$observed"
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$err"
	if [ "$observed" != "$expected" ]; then
		echo >&2
		echo "content registration run '$label' did not produce the recorded output; expected:" >&2
		printf '%s\n' "$expected" >&2
		exit 1
	fi
}

# The whole point of the surface: a block and an item a mod declared are in the
# game's registries, under the mod's own namespace, as values of the game's own
# types. Two crossings — the hook's, and the adapter's read.
content_run admitted \
	'block:example_mod:copper:50.0:1200.0:true:false,item:example_mod:ingot:16:true' \
	'block=example_mod:copper->Block(50.0/1200.0/true/false)
item=example_mod:ingot->Item(16/true)
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# Nothing admitted is a launch with no set installed, and it registers nothing
# rather than failing. The crossing still happens, which is what says the empty
# answer came from Rust and not from a seam that never fired.
content_run empty empty 'block=
item=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# Null is what the JVM receives when the native failed. Nothing is registered on
# it, because registering a set nothing checked is the worse half of the trade.
content_run null-answer null 'block=
item=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# A payload built under a contract this adapter never agreed to is refused whole
# rather than read half of.
content_run malformed malformed 'block=
item=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'
content_malformed_diagnostic="$(grep '^\[ravel\] content' "$content/malformed-stderr")"
content_malformed_expected='[ravel] content registration was offered a payload it cannot read'
if [ "$content_malformed_diagnostic" != "$content_malformed_expected" ]; then
	echo >&2
	echo "a malformed content payload did not refuse with the recorded diagnostic; expected:" >&2
	printf '%s\n' "$content_malformed_expected" >&2
	exit 1
fi

# The outward-action seam, against a stand-in game. The adapter is reflective
# and reads the queued actions across its own crossing, so a stand-in carrying
# the pinned names exercises the real control flow: what is stood in for is the
# game, not Ravel. The payloads below are written out rather than built, and each
# one also appears as a literal in `crates/ravel-bootstrap/src/outward.rs` — the
# two ends of this crossing are two languages, and a shared builder would only
# prove one side agrees with itself.
outward="$work/outward"
outward_agent="$outward/agent/ravel-agent.jar"
mkdir -p "$outward/classes" "$(dirname "$outward_agent")"
mapfile -d '' outward_sources < <(find "$root/jvm/harness/outward" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -d "$outward/classes" "${outward_sources[@]}" \
	"$root/jvm/harness/content/net/minecraft/resources/Identifier.java"
# `OutwardMain` needs the agent classes only to read the stand-in hook's counter.
# A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$outward/classes:$order/agent-classes" -d "$outward/classes" \
	"$root/jvm/harness/OutwardMain.java"
cp "$order_agent" "$outward_agent"

outward_run() {
	local label="$1" actions="$2" expected="$3"
	local out="$outward/$label-stdout"
	local err="$outward/$label-stderr"
	(cd "$outward/classes" && java \
		-javaagent:"$outward_agent" \
		-Dravel.test.transform.outward=true \
		-Dravel.test.outward.actions="$actions" \
		-cp . OutwardMain >"$out" 2>"$err")
	local observed
	observed="$(<"$out")"
	printf '%s\n' "$observed"
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$err"
	if [ "$observed" != "$expected" ]; then
		echo >&2
		echo "outward action run '$label' did not produce the recorded output; expected:" >&2
		printf '%s\n' "$expected" >&2
		exit 1
	fi
}

# The whole point of the surface: an action Rust encoded is performed against the
# game, with the arguments it named, through the members the pinned jar declares.
# Two crossings — the hook's, and the adapter's read.
outward_run performed '1:11:110:audio.play1:35:audio10:door_opens1:1' \
	'played=audio:door_opens@1.0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# Two actions in one payload, which is the case the decision measured: the array
# crosses once whatever it holds.
outward_run two '1:11:210:audio.play1:35:audio10:door_opens1:110:audio.play1:35:audio11:door_closes1:2' \
	'played=audio:door_opens@1.0,audio:door_closes@2.0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# A verb this version does not know is stepped over rather than losing the
# payload behind it, which is what the per-action arity is for. The known action
# after it is still performed.
outward_run unknown-verb '1:11:211:packet.send1:23:one3:two10:audio.play1:35:audio10:door_opens1:1' \
	'played=audio:door_opens@1.0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'
outward_unknown_diagnostic="$(grep '^\[ravel\] an outward action was not performed' "$outward/unknown-verb-stderr")"
outward_unknown_expected='[ravel] an outward action was not performed: packet.send/2'
if [ "$outward_unknown_diagnostic" != "$outward_unknown_expected" ]; then
	echo >&2
	echo "an unknown verb did not report the recorded diagnostic; expected:" >&2
	printf '%s\n' "$outward_unknown_expected" >&2
	exit 1
fi

# Nothing queued is the common tick, and it performs nothing rather than failing.
# The crossing still happens, which is what says the empty answer came from Rust
# and not from a seam that never fired.
outward_run empty empty 'played=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# Null is what the JVM receives when the native failed. Nothing is performed on
# it, because performing a payload nothing checked is the worse half of the trade.
outward_run null-answer null 'played=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# A payload built under a contract this adapter never agreed to is refused whole
# rather than read half of.
outward_run malformed '1:21:110:audio.play1:35:audio10:door_opens1:1' 'played=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'
outward_malformed_diagnostic="$(grep '^\[ravel\] an outward payload' "$outward/malformed-stderr")"
outward_malformed_expected='[ravel] an outward payload was offered that cannot be read'
if [ "$outward_malformed_diagnostic" != "$outward_malformed_expected" ]; then
	echo >&2
	echo "a malformed outward payload did not refuse with the recorded diagnostic; expected:" >&2
	printf '%s\n' "$outward_malformed_expected" >&2
	exit 1
fi

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
mods=1:16:client1:01:51:111:client_only7:running8:required4:true5:false1:01:213:configuration7:running8:required4:true5:false1:01:310:registries7:running8:required4:true5:false1:01:410:saved_data7:running8:required4:true5:false1:01:511:server_only8:resolved8:required5:false5:false1:0
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
mods=1:16:client1:01:51:111:client_only7:running8:required4:true5:false1:01:213:configuration7:running8:required4:true5:false1:01:310:registries7:running8:required4:true5:false1:01:410:saved_data7:running8:required4:true5:false1:01:511:server_only8:resolved8:required5:false5:false1:0
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
# statement alone. `ticks` is the number this build cannot produce and must
# therefore never state, and `named` is where it went instead — onto the
# unmeasurable list, which the two smallest windows shorten to a count and so do
# not name.
profiling_expected='size=320x240 rows=7 fits=true within=true caveat=whole ticks=none named=false
size=480x270 rows=8 fits=true within=true caveat=whole ticks=none named=false
size=640x360 rows=10 fits=true within=true caveat=whole ticks=none named=true
size=960x540 rows=10 fits=true within=true caveat=whole ticks=none named=true
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

# The same screen with the collectors on and mods really entered. The off report
# above cannot tell a summary that dropped an unproducible number from one that
# dropped everything, because an off report draws no numbers at all. Here each
# rewritten target is called once before the screen is built, so each mod has
# four deliveries charged to it: `entered` is those counts read back off the
# rows, and `ticks` is the count that must still be absent beside them.
driven_stdout="$work/profiling-driven-stdout"
driven_stderr="$work/profiling-driven-stderr"
(cd "$work" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-Dravel.profiling=true \
	-cp ".:$screen" ProfilingScreenMain driven >"$driven_stdout" 2>"$driven_stderr")
driven_observed="$(<"$driven_stdout")"
# The two smallest windows spend every row on the summary and the statement, so
# they draw no mod row to read a count off; that is the documented row budget
# rather than a lost measurement, and only the largest window shows the whole set.
# 640x360 is absent here and not from the off report beside it, for the reason
# `ProfilingScreenMain.DRIVEN_SIZES` records.
driven_expected='driven=320x240 fits=true within=true ticks=none entered=none
driven=480x270 fits=true within=true ticks=none entered=none
driven=960x540 fits=true within=true ticks=none entered=0,4,4,4,4
[ravel] rewrote 2 classes and 3 methods; 7 hook invocations reached native code'

printf '%s\n' "$driven_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$driven_stderr"

if [ "$driven_observed" != "$driven_expected" ]; then
	echo >&2
	echo "the profiled screen did not report the work the run actually did; expected:" >&2
	printf '%s\n' "$driven_expected" >&2
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
mods=1:16:client1:01:51:111:client_only7:running8:required4:true5:false1:01:213:configuration7:running8:required4:true5:false1:01:310:registries7:running8:required4:true5:false1:01:410:saved_data7:running8:required4:true5:false1:01:511:server_only8:resolved8:required5:false5:false1:0
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
modWidgets=9
entry=Configuration Settings
settings=dev.ravel.jvm.screen.RavelSettingsScreen
entry=Profiling
profiling=dev.ravel.jvm.screen.RavelProfilingScreen
entry=gui.done
[ravel] rewrote 3 classes and 4 methods; 7 hook invocations reached native code'
menu_expected_diagnostic='[ravel] rewrote 3 classes and 4 methods; 7 hook invocations reached native code'

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

# The running side, read from the JVM rather than assumed.
#
# Ravel asks premain's own JVM which Minecraft classes it can load:
# `net/minecraft/client/main/Main` is in the client distribution and not the
# dedicated server one, and `net/minecraft/SharedConstants` is in both. Every run
# above put the first on its class path, which is why they are clients. These
# three vary exactly that and nothing else, so what changes in the answer is the
# class path and not the code.
#
# Two of them also carry the composed set's side rule end to end: `client_only`
# runs on the client and is parked on the dedicated server, `server_only` does the
# opposite, and both are decided by the `sides` list each mod ships.
side="$work/side"
mkdir -p "$side/client" "$side/dedicated" "$side/undeterminable"
javac -Xlint:all -Werror -d "$side/client" \
	"$root/jvm/harness/net/minecraft/client/main/Main.java"
javac -Xlint:all -Werror -d "$side/dedicated" \
	"$root/jvm/harness/net/minecraft/SharedConstants.java"
javac -Xlint:all -Werror -cp "$jar" -d "$side/classes" "$root/jvm/harness/SideMain.java"

side_expected='client=mods=1:16:client1:01:51:111:client_only7:running8:required4:true5:false1:01:213:configuration7:running8:required4:true5:false1:01:310:registries7:running8:required4:true5:false1:01:410:saved_data7:running8:required4:true5:false1:01:511:server_only8:resolved8:required5:false5:false1:0
dedicated=mods=1:116:dedicated-server1:01:51:113:configuration7:running8:required4:true5:false1:01:210:registries7:running8:required4:true5:false1:01:310:saved_data7:running8:required4:true5:false1:01:411:server_only7:running8:required4:true5:false1:01:511:client_only8:resolved8:required5:false5:false1:0'
side_observed=""
for case in client dedicated; do
	case_stdout="$side/$case-stdout"
	case_stderr="$side/$case-stderr"
	(cd "$side/$case" && java \
		-javaagent:"$agent" \
		-Djava.library.path="$library" \
		-cp ".:$side/classes" SideMain >"$case_stdout" 2>"$case_stderr")
	side_observed="$side_observed$case=$(head -n 1 "$case_stdout")"$'\n'
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$case_stderr"
done
side_observed="${side_observed%$'\n'}"

printf '%s\n' "$side_observed"

if [ "$side_observed" != "$side_expected" ]; then
	echo >&2
	echo "the side the JVM was asked for did not decide what ran; expected:" >&2
	printf '%s\n' "$side_expected" >&2
	exit 1
fi

# The missing input. A JVM that can load neither class is one Ravel cannot place,
# and the launch fails there rather than composing a set for a guessed side.
undeterminable_stdout="$side/undeterminable-stdout"
undeterminable_stderr="$side/undeterminable-stderr"
set +e
(cd "$side/undeterminable" && java \
	-javaagent:"$agent" \
	-Djava.library.path="$library" \
	-cp ".:$side/classes" SideMain >"$undeterminable_stdout" 2>"$undeterminable_stderr")
undeterminable_status=$?
set -e

while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$undeterminable_stderr"

if [ "$undeterminable_status" -eq 0 ]; then
	echo >&2
	echo "a JVM holding no Minecraft at all still started a mod set" >&2
	exit 1
fi

for named in "net/minecraft/client/main/Main" "net/minecraft/SharedConstants"; do
	if ! grep -q "$named" "$undeterminable_stderr"; then
		echo >&2
		echo "the refusal did not name \`$named\` as a class it could not load" >&2
		exit 1
	fi
done

# The JVM prints its own abort on standard output, so what is checked is that
# no mod surface is among it: `SideMain` never ran.
if grep -q '^mods=' "$undeterminable_stdout"; then
	echo >&2
	echo "a launch that cannot name its side still produced a mod surface:" >&2
	grep '^mods=' "$undeterminable_stdout" >&2
	exit 1
fi
