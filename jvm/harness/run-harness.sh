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
# ADG-5's amendment: the generated Hook loads the library from the file this
# property names, so the harness names the file the build produced rather than a
# directory for the JVM to search.
host="$library/libravel_bootstrap.so"
work="$root/target/ravel-jvm/harness"
agent="$work/agent/ravel-agent.jar"
# The stand-in game, generated per version from `jvm/harness/template/`. Its
# classes carry the names this version links rather than the Mojang ones, which
# is what lets a section resolving a generated name run under a version that
# renames it.
standins="$root/target/ravel-jvm/generated-harness"
# The three trees the generator renders, kept apart below the generated root the
# way their templates are: the stand-in game, Ravel's own drivers, and the
# stand-in half of the sibling artifact. A sweep over one of them is a directory
# rather than a list of the files the other two happen to hold.
standin_game="$standins/game"
standin_drivers="$standins/driver"
standin_sibling="$standins/sibling"

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
if [ ! -d "$standins" ] || [ ! -d "$standin_game" ]; then
	echo "the generated stand-in game is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi

# One game and one set of libraries, swept once: the persistence, content and
# outward sections all compile them, and three sweeps would let one of them
# drift. Guava and DataFixerUpper are libraries beside the game rather than part
# of it, so no published mapping renames them and their stand-ins are written
# once rather than generated.
mapfile -d '' standin_sources < <(find "$standin_game" -name '*.java' -print0 | sort -z)
mapfile -d '' libraries < <(find "$root/jvm/harness/guava" "$root/jvm/harness/datafixers" \
	-name '*.java' -print0 | sort -z)

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

# What puts the linked names back into the stand-ins a section runs against.
#
# A version that keeps a handful of classes in a package and moves the rest into
# the unnamed package cannot have its stand-in game written as Java at all — a
# class inside a package has no syntax for naming one outside every package — so
# the generator writes the whole of that game in the unnamed package and records
# each pair it moved in `aliases.txt`, as `written=linked`. The adapters resolve
# the linked name, so every directory a section runs against is passed through
# this after the last javac that writes into it and before anything reads it
# back: the pair is a name, and a name in a class file lives only in the constant
# pool. An unsplit version leaves the file empty and this is a no-op.
aliases="$standins/aliases.txt"
if [ ! -f "$aliases" ]; then
	echo "the generated stand-in game names no aliases.txt, so nothing says which names" >&2
	echo "it was written under; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
remapper="$work/remap"
mkdir -p "$remapper"
javac -Xlint:all -Werror -d "$remapper" "$root/jvm/harness/remap/Remap.java"
remap() {
	"${jvm[@]}" -cp "$remapper" Remap "$aliases" "$@"
}

# The two distribution entry points, and the dispatcher they run a case's driver
# through.
#
# Ravel names the running side from the first of these the transformer sees, so
# a case that expects a mod set enters at one of them and names its driver as an
# argument, exactly as a launch of the game enters at the game's own entry point.
# Both are unobfuscated under every tracked version, so they are written once
# rather than generated; the `side` section reads the generated names back and
# fails if that ever stops being true.
entry_sources=(
	"$root/jvm/harness/dev/ravel/jvm/harness/Entry.java"
	"$root/jvm/harness/net/minecraft/client/main/Main.java"
	"$root/jvm/harness/net/minecraft/server/Main.java"
)
client_entry="net.minecraft.client.main.Main"
server_entry="net.minecraft.server.Main"
entry_points() {
	javac -Xlint:all -Werror -d "$1" "${entry_sources[@]}"
}

# Every section this run reached, for the record the gate reads. A section is
# added after it passes, never before, so the file says what was proved rather
# than what was attempted.
# How every case below starts a JVM.
#
# The shared class archive is turned off rather than left to the JVM. Ravel's
# agent appends its own jar to the bootstrap loader's search path, which is what
# puts one copy of its classes in front of every loader in the process; the JVM
# cannot share an archive across an appended boot class path and turns sharing
# off itself, saying so on stderr once per launch. Every case here compares a
# run's stderr exactly, so this says up front what the JVM would have done
# anyway rather than writing a notice about it into every expectation.
jvm=(java -Xshare:off)

sections_record="$root/target/ravel-jvm/harness-sections.txt"
: >"$sections_record"
sections_ran=""
section_ran() {
	sections_ran="${sections_ran:+$sections_ran,}$1"
	printf '%s' "$sections_ran" >"$sections_record"
}

# What this version calls Ravel's own screens. A screen extends a game class, so
# it is generated into the name space that class is in, and under an obfuscated
# version that is the unnamed package. The drivers below print the class they
# landed on, and this is what those lines are compared against — read from the
# class the agent jar opens them through rather than written down here.
generated_screens="$root/target/ravel-jvm/generated/dev/ravel/jvm/ScreenNames.java"
generated_names="$root/target/ravel-jvm/generated/dev/ravel/jvm/GameNames.java"
# One generated constant's value, out of whichever generated class declares it.
# Written once because two readers want it: the drivers below open a screen and
# the `side` section enters an entry point, and both are names only this version
# can say.
linked_name() {
	local found
	found="$(awk -v want="$2" '$0 ~ ("static final String " want " =") {
		match($0, /"[^"]*"/)
		print substr($0, RSTART + 1, RLENGTH - 2)
	}' "$1")"
	if [ -z "$found" ]; then
		echo "$1 names no $2, so nothing says what this version calls it" >&2
		exit 1
	fi
	printf '%s' "$found"
}
screen_name() {
	linked_name "$generated_screens" "$1"
}
mods_screen="$(screen_name RAVEL_MODS_SCREEN)"
settings_screen="$(screen_name RAVEL_SETTINGS_SCREEN)"
profiling_screen="$(screen_name RAVEL_PROFILING_SCREEN)"

# The transformer sections' own stand-in, which is Ravel's test data on both
# sides: their targets are hand-written in `harness/initialization`'s stand-in
# `Hook` under the reviewed names, so the class they are matched against is
# written under those names too and neither side is version data.
javac -d "$work" "$root/jvm/harness/net/minecraft/util/StringUtil.java"
# The entry points, which every case that expects a mod set enters at. They are
# compiled into both trees below because both run one.
entry_points "$work"

# The tree the settings and screen sections run in: the generated stand-in game,
# the drivers that exercise it, and the side marker beside them. Kept apart from
# `$work` because the two carry the same class under an unobfuscated version —
# the transformer sections' stand-in above is written under the reviewed name,
# and this one under the name this version links — and a class is one file on a
# class path or it is whichever came first.
ui="$work/ui"
mkdir -p "$ui"
javac -Xlint:all -Werror -d "$ui" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$ui"
# `Main` needs the agent jar on its compile class path only for the mid-run
# `Agent.transformedClasses()` call. A real launch never compiles against it.
javac -Xlint:all -Werror -cp "$ui:$jar" -d "$ui" "$standin_drivers/Main.java"
javac -Xlint:all -Werror -cp "$ui" -d "$ui" \
	"$standin_drivers/TitleMenuMain.java" \
	"$standin_drivers/TitleMenuMissingMain.java"
javac -Xlint:all -Werror -cp "$ui:$jar:$screen" -d "$ui" \
	"$standin_drivers/SettingsScreenMain.java" \
	"$standin_drivers/ProfilingScreenMain.java"
remap "$ui"

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
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/MenuLayout.java" \
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
printf 'Premain-Class: dev.ravel.jvm.Boot\nCan-Retransform-Classes: false\n' >"$order/agent.manifest"
mapfile -d '' order_compiled < <(cd "$order/agent-classes" && find . -name '*.class' -print0 | sort -z)
order_entries=()
for class in "${order_compiled[@]}"; do
	order_entries+=(-C "$order/agent-classes" "${class#./}")
done
jar --create --file "$order_agent" --manifest "$order/agent.manifest" \
	--date 1980-01-01T00:00:02Z "${order_entries[@]}"
order_stdout="$order/stdout"
order_stderr="$order/stderr"
(cd "$order/classes" && "${jvm[@]}" \
	-javaagent:"$order_agent" \
	-cp ".:$work" "$client_entry" InitializationOrderMain >"$order_stdout" 2>"$order_stderr")
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
(cd "$order/classes" && "${jvm[@]}" \
	-javaagent:"$order_agent" \
	-Dravel.test.transform.hierarchy=true \
	-cp ".:$work" "$client_entry" HierarchyResolutionMain >"$hierarchy_stdout" 2>"$hierarchy_stderr")
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
	(cd "$order/classes" && "${jvm[@]}" \
		-javaagent:"$order_agent" \
		-Dravel.test.transform.refusal=true \
		-Dravel.test.refusal.verdict="$verdict" \
		-cp ".:$work" "$client_entry" RefusalMain >"$refusal_stdout" 2>"$refusal_stderr")
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

section_ran initialization
section_ran hierarchy
section_ran refusal

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
entry_points "$members/classes"
# `MembersMain` needs the agent classes only to read the rewrite marker and the
# stand-in hook's counter. A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$members/classes:$order/agent-classes" -d "$members/classes" \
	"$root/jvm/harness/MembersMain.java"
members_stdout="$members/stdout"
members_stderr="$members/stderr"
(cd "$members/classes" && "${jvm[@]}" \
	-javaagent:"$order_agent" \
	-Dravel.test.transform.members=true \
	-Dravel.report.targets=true \
	-cp . "$client_entry" MembersMain >"$members_stdout" 2>"$members_stderr")
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

section_ran members

# One entry of the generated hook table, read by Ravel's own prologue name — the
# one word in an entry that no version renames. The harness's stand-in `Hook`
# stands in for the native library and not for the inventory, so every seam it
# installs has to be this version's rather than a name written down here, and so
# has the prologue's own descriptor: the rewritten call site links on it, and the
# two tracked versions' open seam takes a different number of arguments. The
# table is written seven lines to an entry, class first and prologue fifth, so an
# entry is the four lines before the prologue and the two after it.
generated_hook="$root/target/ravel-jvm/generated/dev/ravel/jvm/Hook.java"
hook_seam() {
	awk -v prologue="$1" '
		{ line[NR] = $0 }
		$0 ~ ("^[[:space:]]*\"" prologue "\",$") { at = NR }
		END {
			if (at == 0) { exit 1 }
			for (field = at - 4; field <= at + 2; field++) {
				value = line[field]
				sub(/^[[:space:]]+/, "", value)
				sub(/\),$/, "", value)
				sub(/,$/, "", value)
				gsub(/^"|"$/, "", value)
				printf "%s%s", (field > at - 4 ? ":" : ""), value
			}
			print ""
		}
	' "$generated_hook"
}

# How many of the generated table's targets sit on one class, by its internal
# name. The table is the version's own hook inventory rendered, so this is how
# many methods the transformer rewrites when that class goes past — which is
# what a section asserting a count has to read rather than write down.
hook_targets_on() {
	awk -v class="$1" '
		/new HookTarget\($/ {
			getline
			sub(/^[[:space:]]*"/, "")
			sub(/",$/, "")
			if ($0 == class) { count++ }
		}
		END { print count + 0 }
	' "$generated_hook"
}

# Fills the named array with the `-D` arguments the stand-in `Hook` reads one
# section's seams back through. A prologue the table does not name stops the run
# rather than being left out, because a section short of a seam transforms less
# than it says it does and passes while doing it — which is why this writes into
# an array rather than printing one, where the refusal would leave a subshell and
# nothing else.
install_seams() {
	local -n into="$1"
	shift
	into=()
	local prologue entry
	for prologue in "$@"; do
		if ! entry="$(hook_seam "$prologue")"; then
			echo "the generated hook table names no $prologue seam, so this section" >&2
			echo "would transform a class this version does not link" >&2
			exit 1
		fi
		into+=("-Dravel.test.target.$prologue=$entry")
	done
}

install_seams persist_seams \
	worldOpenPersistence worldSavePersistence worldCreatePackRepository screenCreateWorldInit

# ADG-7's world persistence seams and the two data pack seams, against the
# generated stand-in game. The adapter is reflective and reaches the sibling
# artifact by name, so a stand-in carrying the names this version links exercises
# the real control flow: what is stood in for is the game, not Ravel. It runs at
# every version because the stand-ins are generated from the same data the names
# the adapter resolves are, so the obfuscated case is the one this section reads
# rather than the one it skips.
persist="$work/persistence"
persist_agent="$persist/agent/ravel-agent.jar"
persist_screen="$persist/agent/ravel-screen.jar"
mkdir -p "$persist/classes" "$persist/sibling" "$(dirname "$persist_agent")"
javac -Xlint:all -Werror -d "$persist/classes" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$persist/classes"
# `PersistenceMain` needs the agent classes only to read the stand-in hook's
# counters. A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$persist/classes:$order/agent-classes" -d "$persist/classes" \
	"$standin_drivers/PersistenceMain.java"
# Against the stand-in game, because the sibling is the compiled half of the
# seam and its argument vectors are the game's own types. It is generated beside
# the real one and into the same package, because the agent jar opens both by the
# constant `ScreenNames` carries.
mapfile -d '' sibling_sources < <(find "$standin_sibling" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -cp "$persist/classes" -d "$persist/sibling" "${sibling_sources[@]}"
# Both directories at once and after the sibling, because the sibling half is
# rendered under the alias like the stand-ins it compiles against.
remap "$persist/classes" "$persist/sibling"
jar --create --file "$persist_screen" --date 1980-01-01T00:00:02Z -C "$persist/sibling" .
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
	(cd "$persist/classes" && "${jvm[@]}" \
		-javaagent:"$persist_agent" \
		"${persist_seams[@]}" \
		-Dravel.test.transform.persistence=true \
		-Dravel.test.persistence.saveSeamMissing="$save_seam" \
		-Dravel.test.persistence.open="$open" \
		-Dravel.test.persistence.save="$save" \
		-Dravel.test.persistence.stored="$stored" \
		-cp . "$client_entry" PersistenceMain >"$out" 2>"$err")
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
persistence_run admitted empty replacement 'old-aggregate' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=1 accessClosed=false stemClosed=false crossed=1 read=old-aggregate
saves=1 writes=1 written=replaced
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'

# Null is what the JVM receives when the native failed, and it refuses. This is
# the polarity the whole design turns on: empty admits, so a panic cannot.
persistence_run null-verdict null unchanged '' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=0 accessClosed=true stemClosed=true crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'

# A non-empty verdict is a refusal carrying the diagnostic Rust owns.
persistence_run refused 'this world holds data Ravel cannot read' unchanged '' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=0 accessClosed=true stemClosed=true crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'

# Every save answer that is not a replacement writes nothing, and vanilla saving
# runs in each of them. Null is one of the four, because a failed native must
# cost one Ravel checkpoint and nothing else.
for answer in unchanged failed mismatch null; do
	persistence_run "save-$answer" empty "$answer" '' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=1 accessClosed=false stemClosed=false crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'
done

# The create-world screen's own cases, which reach no world. What each is about
# is what the screen did with the answer the staging crossing gave it, and
# opening and saving a world afterwards would restate a lifecycle every run
# above already runs. They are not world lifecycles and are not counted as any.
# The fourth argument is everything this run may say on stderr, which is empty
# for every run that went right. Without comparing it a swallowed exception in
# the adapter's own staging path would look exactly like a case that decided
# there was nothing to do.
screen_run() {
	local label="$1" staging="$2" expected="$3" expected_diagnostic="${4:-}"
	local out="$persist/$label-stdout"
	local err="$persist/$label-stderr"
	(cd "$persist/classes" && "${jvm[@]}" \
		-javaagent:"$persist_agent" \
		"${persist_seams[@]}" \
		-Dravel.test.transform.persistence=true \
		-Dravel.test.persistence.saveSeamMissing=false \
		-Dravel.test.persistence.open=empty \
		-Dravel.test.persistence.save=unchanged \
		-Dravel.test.persistence.stored= \
		-Dravel.test.persistence.staging="$staging" \
		-Dravel.test.persistence.screenOnly=true \
		-cp . "$client_entry" PersistenceMain >"$out" 2>"$err")
	local observed
	observed="$(<"$out")"
	printf '%s\n' "$observed"
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$err"
	if [ "$observed" != "$expected" ]; then
		echo >&2
		echo "create-world screen run '$label' did not produce the recorded output; expected:" >&2
		printf '%s\n' "$expected" >&2
		exit 1
	fi
	local diagnosed
	diagnosed="$(<"$err")"
	if [ "$diagnosed" != "$expected_diagnostic" ]; then
		echo >&2
		echo "create-world screen run '$label' did not say on stderr what it records; expected:" >&2
		printf '%s\n' "$expected_diagnostic" >&2
		exit 1
	fi
}

# A pack the screen's configuration already enables, whose files are the ones
# already on disk: nothing to select and nothing to load. This is the run that
# says the load is asked for on a condition rather than every time.
#
# The verdict is in the line rather than implied by a count of zero. One load is
# the screen's own opening one and the content it holds is the pack that was
# there, so the pack was found, selected and left alone — where a refusal would
# have dropped the id from what the screen holds, and a swallowed exception
# would have said so on the stderr this now compares.
screen_run staged-unchanged unchanged 'staged=screen stagedWrites=1 screenLoads=1 loaded=[vanilla, file/ravel=old]
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'

# The same screen with the pack replaced under it, which is a world recreated
# after a mod's data changed. The ids are identical on both sides, so the game's
# own comparison finds nothing to do and the screen would hand the created world
# the data it loaded before the replacement. The load is forced through the call
# that comparison guards, and what settles it is the content the screen ends up
# holding: the ids are identical across the replacement, so a load that restated
# them would pass whether it re-read the files or not.
screen_run staged-replaced replaced 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'

# The pack is on disk and no scan of the staging directory finds it under the id
# the crossing named, which is what an unreadable pack looks like from here.
# Selecting keeps only what the scan found, so the id does not take, and the
# world would be created with none of the mods' data and nothing said about it.
screen_run staged-undiscovered undiscovered 'staged=screen stagedWrites=1 screenLoads=1 loaded=[vanilla]
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code' \
	'[ravel] the game did not find the mods'"'"' data pack where it was staged, so this world will have none of their recipes or tags'

# No mod supplies data, so nothing is staged and the screen is asked for nothing:
# the load it ran when it opened is the only one, and the id crossing is never
# reached, which is the invocation this run has one fewer of. A launch like this
# has to cost the player exactly what it did before Ravel was installed.
screen_run staged-nothing nothing 'staged=screen stagedWrites=1 screenLoads=1 loaded=[vanilla]
[ravel] rewrote 2 classes and 2 methods; 3 hook invocations reached native code'

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
(cd "$persist/classes" && "${jvm[@]}" \
	-javaagent:"$persist_agent" \
	"${persist_seams[@]}" \
	-Dravel.test.transform.persistence=true \
	-Dravel.test.persistence.saveSeamMissing=false \
	-Dravel.test.persistence.open=empty \
	-Dravel.test.persistence.save=unchanged \
	-Dravel.test.persistence.stored= \
	-cp . "$client_entry" PersistenceMain >"$absent_out" 2>"$absent_err") || absent_status=$?
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
persistence_missing_save_stdout='staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=0 accessClosed=true stemClosed=true crossed=0 read=absent
saves=1 writes=0 written=
[ravel] rewrote 3 classes and 3 methods; 7 hook invocations reached native code'
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

section_ran persistence

# The content-registration seam, against the generated stand-in game. The adapter
# is reflective and reads the admitted set across its own crossing, so a stand-in
# carrying the names this version links exercises the real control flow: what is
# stood in for is the game, not Ravel. It runs at every version because the
# stand-ins are generated from the same data the adapter's names are, so the
# obfuscated case is the one this section reads rather than the one it skips. The stand-in registries refuse a
# duplicate and refuse everything once frozen, and the properties objects refuse
# to describe a block or an item that was never given an id — so a registration
# that only looked like one fails here.

content="$work/content"
content_agent="$content/agent/ravel-agent.jar"
content_screen="$content/agent/ravel-screen.jar"
mkdir -p "$content/classes" "$content/sibling" "$(dirname "$content_agent")"
javac -Xlint:all -Werror -d "$content/classes" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$content/classes"
# `ContentMain` needs the agent classes only to read the stand-in hook's counter.
# A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$content/classes:$order/agent-classes" -d "$content/classes" \
	"$standin_drivers/ContentMain.java"
remap "$content/classes"
# The registration itself lives in the sibling artifact, because the two tracked
# versions build a block differently rather than name it differently, so this
# section needs the sibling jar the persistence section already needed. What is
# compiled here is the generated source a launch ships rather than a copy of it:
# the stand-ins carry this version's linked names, so the real sibling compiles
# against them and a rename that broke it would break here.
content_sibling="$(find "$root/target/ravel-jvm/generated-screen" -name 'RavelContent.java')"
if [ ! -f "$content_sibling" ]; then
	echo "the generated content sibling is missing; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
javac -Xlint:all -Werror -cp "$content/classes" -d "$content/sibling" "$content_sibling"
remap "$content/sibling"
jar --create --file "$content_screen" --date 1980-01-01T00:00:02Z -C "$content/sibling" .
cp "$order_agent" "$content_agent"
install_seams content_seams registryBuiltInFreeze

# What a registered block or item prints back, which is version data rather than
# one answer asserted over every version. A version whose properties builders
# take a registry key has the adapter stamp one before the value exists, and the
# value prints that key: a wrong root or a wrong identifier fails here. A version
# whose builders take none names content by the identifier it was registered
# under and has no key to print.
#
# Decided by asking the generator rather than by naming a version, the way the
# section guards above are. The generator writes which tree rendered each
# harness source beside the sources themselves, and the shared properties
# templates are the id-bearing pair, so a version is on the plain expectation
# only where it says it replaced them. Reading the generated Java for a phrase
# instead made the weaker expectation the one a moved literal silently selected.
origins="$standins/overrides.txt"
if ! properties_origin="$(awk -F= -v block='game/BlockBehaviour$Properties' \
	-v item='game/Item$Properties' '
	$1 == block { found_block = $2 }
	$1 == item { found_item = $2 }
	END {
		if (found_block == "" || found_item == "" || found_block != found_item) {
			exit 1
		}
		print found_block
	}' "$origins")"; then
	echo "the generated harness record at $origins does not say, for both properties" >&2
	echo "templates alike, which tree rendered them; run:" >&2
	echo "  cargo build -p ravel-minecraft-adapter" >&2
	exit 1
fi
case "$properties_origin" in
shared)
	# The class, what precedes its properties (a placing item leads with the block
	# it holds and nothing else leads with anything), then the key and the values.
	described() { printf '%s(%s%s/%s|%s)' "$1" "$2" "$3" "$4" "$5"; }
	;;
override)
	described() { printf '%s(%s%s)' "$1" "$2" "$5"; }
	;;
*)
	echo "the generated harness record says the properties templates were" >&2
	echo "'$properties_origin' rendered, which is neither tree" >&2
	exit 1
	;;
esac

# The bundler's own shape, compiled apart from everything it must not see.
# `Bundled` is the only class on the class path of the run below, so the game,
# the entry points and the driver are reachable only through the child loader it
# builds — which is how the published server jar loads all three.
mkdir -p "$content/launcher"
javac -Xlint:all -Werror -d "$content/launcher" "$root/jvm/harness/Bundled.java"

# Each run is one initialization: the registries freeze once, and what they hold
# afterwards is the whole answer.
#
# `launcher` says how the entry point is reached: empty puts the game on the
# class path and enters it directly, `bundled` puts nothing but the launcher
# there and lets it define the game under a child of the platform loader.
content_run() {
	local label="$1" admitted="$2" expected="$3" launcher="${4:-}"
	local out="$content/$label-stdout"
	local err="$content/$label-stderr"
	local entry=(-cp . "$client_entry" ContentMain)
	if [ -n "$launcher" ]; then
		entry=(-cp "$content/launcher" Bundled "$content/classes" "$client_entry" ContentMain)
	fi
	(cd "$content/classes" && "${jvm[@]}" \
		-javaagent:"$content_agent" \
		"${content_seams[@]}" \
		-Dravel.test.transform.content=true \
		-Dravel.test.content.admitted="$admitted" \
		"${entry[@]}" >"$out" 2>"$err")
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
# types, and the block's states are in the mapper the protocol encodes a block
# update through. Two crossings — the hook's, and the adapter's read.
#
# `states` is the count that was missing. A block reaches the registry on
# registration and reaches the mapper only if something walks its states
# afterwards, which vanilla does once for its own blocks and never again. The
# stand-in block declares two states, so a walk that adds one and stops reads
# differently from one that adds them all.
content_run admitted \
	'block:example_mod:copper:50.0:1200.0:true:false,item:example_mod:ingot:16:true' \
	"block=example_mod:copper->$(described Block '' block example_mod:copper 50.0/1200.0/true/false)
item=example_mod:ingot->$(described Item '' item example_mod:ingot 16/true)
states=2 cached=2
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code"

# The same registration, reached the way the published dedicated server reaches
# its entry point: a child `URLClassLoader` whose parent is the platform loader,
# holding the game and the entry point, with nothing but the launcher on the
# class path. Two things are provable only here. Ravel's own classes are reached
# at all, which needs the agent jar on the bootstrap loader's search path,
# because the system class path the `-javaagent` was appended to is not on this
# loader's parent chain. And every game name the adapter resolves — the block
# class, and the sibling artifact's supertypes through the screen loader's parent
# — resolves in the loader that defined the entry point, because the system
# loader holds no game class in this process at all.
#
# The expectation is the `admitted` case's, unchanged and deliberately so: what
# this case varies is the loader and nothing else, so any difference is the
# loader's.
content_run bundled \
	'block:example_mod:copper:50.0:1200.0:true:false,item:example_mod:ingot:16:true' \
	"block=example_mod:copper->$(described Block '' block example_mod:copper 50.0/1200.0/true/false)
item=example_mod:ingot->$(described Item '' item example_mod:ingot 16/true)
states=2 cached=2
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code" \
	bundled

# A block and an item under one identifier, which is how a mod declares a block
# you can hold. The item has to be the block's placing item and be built from the
# block: a plain item has no use-on, so right clicking it does nothing at all,
# which is what the eighth launch found. The stand-in prints a placing item
# differently from a plain one, so registering the wrong kind reads differently.
#
# The payload declares the item first, on purpose. Nothing about the order a mod
# writes its entries in says a block comes before the item that places it, so the
# adapter reads the payload whole and registers every block before any item.
content_run placing-item \
	'item:example_mod:lantern:64:false,block:example_mod:lantern:3.0:3.0:false:false' \
	"block=example_mod:lantern->$(described Block '' block example_mod:lantern 3.0/3.0/false/false)
item=example_mod:lantern->$(described BlockItem example_mod:lantern/ item example_mod:lantern 64/false)
states=2 cached=2
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code"

# A block whose state walk fails after it is registered. The two are separate
# steps and only the second went wrong, so the block is in the registry and in no
# id mapper: what a player hits is a placement that disconnects the session, and
# the diagnostic has to say that rather than say the block did not register.
#
# The item is the proof that the block was recorded before the walk ran. It shares
# the block's identifier, so it is that block's placing item — which it could only
# be if the walk's failure left the block in the map.
content_run unmapped-states \
	'block:example_mod:unmappable:1.0:1.0:false:false,item:example_mod:unmappable:1:false' \
	"block=example_mod:unmappable->$(described Block '' block example_mod:unmappable 1.0/1.0/false/false)
item=example_mod:unmappable->$(described BlockItem example_mod:unmappable/ item example_mod:unmappable 1/false)
states=0 cached=0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code"
content_unmapped_diagnostic="$(grep '^\[ravel\] registered example_mod' "$content/unmapped-states-stderr")"
content_unmapped_expected='[ravel] registered example_mod:unmappable but its states are not mapped, so placing it will disconnect the session (java.lang.IllegalStateException)'
if [ "$content_unmapped_diagnostic" != "$content_unmapped_expected" ]; then
	echo >&2
	echo "a block whose states could not be mapped did not say so; expected:" >&2
	printf '%s\n' "$content_unmapped_expected" >&2
	exit 1
fi

# A kind this version has no width for, which refuses the whole payload. The
# rows before it are read and then dropped: a row's width is what its kind says
# it is, so a kind with no width leaves every field after it unread, and what the
# rows already collected mean depends on a payload this adapter has turned out
# not to understand. Registering them would be acting on the half it happened to
# reach first, and guessing a width instead would read the next row's fields as
# this row's behaviour and register plausible nonsense.
content_run unknown-kind \
	'block:example_mod:copper:50.0:1200.0:true:false,gadget:example_mod:widget:1:false,item:example_mod:ingot:16:true' \
	'block=
item=
states=0 cached=0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'
content_unknown_diagnostic="$(grep '^\[ravel\] content registration was offered the kind' "$content/unknown-kind-stderr")"
content_unknown_expected='[ravel] content registration was offered the kind gadget, whose width it does not know, so it registered none of the set'
if [ "$content_unknown_diagnostic" != "$content_unknown_expected" ]; then
	echo >&2
	echo "an unreadable kind did not name itself; expected:" >&2
	printf '%s\n' "$content_unknown_expected" >&2
	exit 1
fi

# Nothing admitted is a launch with no set installed, and it registers nothing
# rather than failing. The crossing still happens, which is what says the empty
# answer came from Rust and not from a seam that never fired.
content_run empty empty 'block=
item=
states=0 cached=0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# Null is what the JVM receives when the native failed. Nothing is registered on
# it, because registering a set nothing checked is the worse half of the trade.
content_run null-answer null 'block=
item=
states=0 cached=0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# A payload built under a contract this adapter never agreed to is refused whole
# rather than read half of.
content_run malformed malformed 'block=
item=
states=0 cached=0
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

section_ran content

# The outward-action seam, against the generated stand-in game. The adapter is
# reflective and reads the queued actions across its own crossing, so a stand-in
# carrying the names this version links exercises the real control flow: what is
# stood in for is the game, not Ravel. The payloads below are written out rather than built, and each
# one also appears as a literal in `crates/ravel-bootstrap/src/outward.rs` — the
# two ends of this crossing are two languages, and a shared builder would only
# prove one side agrees with itself.

outward="$work/outward"
outward_agent="$outward/agent/ravel-agent.jar"
mkdir -p "$outward/classes" "$(dirname "$outward_agent")"
# The same stand-in game as the content section, because it is one game: the
# sound seam and the registration seam name the same identifier class, and two
# trees would let one of them drift.
javac -Xlint:all -Werror -d "$outward/classes" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$outward/classes"
# `OutwardMain` needs the agent classes only to read the stand-in hook's counter.
# A real launch never compiles against them.
javac -Xlint:all -Werror -cp "$outward/classes:$order/agent-classes" -d "$outward/classes" \
	"$standin_drivers/OutwardMain.java"
remap "$outward/classes"
cp "$order_agent" "$outward_agent"
install_seams outward_seams clientTick

outward_run() {
	local label="$1" actions="$2" expected="$3"
	local out="$outward/$label-stdout"
	local err="$outward/$label-stderr"
	(cd "$outward/classes" && "${jvm[@]}" \
		-javaagent:"$outward_agent" \
		"${outward_seams[@]}" \
		-Dravel.test.transform.outward=true \
		-Dravel.test.outward.actions="$actions" \
		-cp . "$client_entry" OutwardMain >"$out" 2>"$err")
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

section_ran outward

# The failed-transformation check and the two refusals below run at every
# version. The transformer supplies its own target here, and a refusal to load a
# library nothing names mentions no game class at all.

failure_stdout="$order/failure-stdout"
failure_stderr="$order/failure-stderr"
(cd "$order/classes" && "${jvm[@]}" \
	-javaagent:"$order_agent" \
	-Dravel.test.transform.failure=true \
	-cp ".:$work" "$client_entry" InitializationOrderMain >"$failure_stdout" 2>"$failure_stderr")
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

section_ran transformation

# ADG-5's amendment is only worth having if a launch that names no library
# refuses instead of searching for one, so both refusals are run rather than
# read. Each is the same launch as the one below with the property broken, and
# each has to end nonzero and say which property and which path.
refuse_load() {
	refusal_what="$1"
	refusal_stderr="$work/load-$2-stderr"
	shift 2
	if (cd "$work" && "${jvm[@]}" \
		-javaagent:"$agent" \
		"$@" \
		-cp "$ui" "$client_entry" Main >/dev/null 2>"$refusal_stderr"); then
		echo >&2
		echo "a launch whose $refusal_what started anyway" >&2
		exit 1
	fi
	if ! grep -qF -- "$refusal_message" "$refusal_stderr"; then
		echo >&2
		echo "a launch whose $refusal_what did not say so; expected to find:" >&2
		printf '%s\n' "$refusal_message" >&2
		cat "$refusal_stderr" >&2
		exit 1
	fi
}

refusal_message='-Dravel.host is unset, so nothing names the ravel_bootstrap library this class loads'
refuse_load 'library property is unset' unset

refusal_message="-Dravel.host names $work/no-such-library.so, which is not a file, so the ravel_bootstrap library cannot be loaded"
refuse_load 'library property names no file' missing -Dravel.host="$work/no-such-library.so"

section_ran load

# The class the settings round trip's targets sit on, and how many of them this
# version pins there. Both are read from the generated table rather than written
# down, and the count is what makes the arithmetic below version-honest: the
# stand-in for that class carries every member the table declares on it, so the
# number the transformer rewrites is the number the table names. A stand-in
# missing one rewrites fewer than the table pins and the comparison fails, which
# is the direction a missing hook target has to fail in.
#
# Routed through `install_seams` so a version whose table names no such seam gets
# that function's refusal rather than a bare nonzero exit from a subshell.
install_seams settings_seams chatFilterText
settings_target="${settings_seams[0]#-Dravel.test.target.chatFilterText=}"
settings_target="${settings_target%%:*}"
settings_methods="$(hook_targets_on "$settings_target")"
settings_target="${settings_target//\//.}"

# The settings round trip, which drives the shipped agent jar rather than one the
# harness assembles, so the targets it rewrites are the generated ones. It runs
# at every version because the stand-in they are matched against is generated
# from the same version data those targets are, so the two names are one string
# by construction rather than by luck.
stdout="$work/stdout"
stderr="$work/stderr"
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-Dravel.report.targets=true \
	-cp "$ui" "$client_entry" Main >"$stdout" 2>"$stderr")
observed="$(<"$stdout")"
diagnostic="$(grep '^\[ravel\]' "$stderr")"

expected='filtered=a
b
simple=ab
valid=true
transformed=1 methods='"$settings_methods"'
client=false:pack:false
common=quiet:default:false
saved=1:0
invalid=1:114:client.enabled78:`client.enabled` was left unchanged: the setting holds a boolean, not a string
clientAfter=false:player:false
commonAfter=quiet:default:true
mods=1:36:client1:01:51:111:client_only7:running8:required4:true5:false4:true1:01:213:configuration7:running8:required4:true5:false4:true1:01:310:registries7:running8:required4:true5:false5:false1:01:410:saved_data7:running8:required4:true5:false5:false1:01:511:server_only8:resolved8:required5:false5:false5:false1:01:0
[ravel] rewrote 1 classes and '"$settings_methods"' methods; 4 hook invocations reached native code
[ravel] rewritten targets: '"$settings_target"'='"$settings_methods"''
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

section_ran settings

# The two screen-jar routes below drive `TitleMenuMissingMain`, which is compiled
# against this version's generated screen stubs, so they belong to the screens
# section and are recorded with it rather than with the settings round trip.
# Where this version's title menu puts Ravel's own row, and whether its column
# builder hands one back at all. A column that returns its own row says what is
# free below it, and the prologue hands back the row it moved down to; one that
# returns void says nothing, and the version's own inventory declares the row
# instead. `MenuLayout` is generated from that inventory, so reading it here is
# what keeps the three numbers below from being one version's asserted over both.
# `DERIVED` is the sentinel that class declares for the first case, and it is
# read by name rather than by its value for the reason the class states: a row
# may legally be negative, so no ordinary number could stand for "there is one".
generated_layout="$root/target/ravel-jvm/generated/dev/ravel/jvm/MenuLayout.java"
menu_row="$(awk '/static final int RAVEL_ROW/ { sub(/^[^=]*= */, ""); sub(/;.*$/, ""); print }' \
	"$generated_layout")"
if [ -z "$menu_row" ]; then
	echo "$generated_layout declares no row, so nothing says where this version's" >&2
	echo "menu puts Ravel's own" >&2
	exit 1
fi
if [ "$menu_row" = DERIVED ]; then
	# The harness hands the column a top of 100 and a spacing of 10, so the column
	# ends at 120 and the prologue hands back the row below the one it added.
	menu_unhooked=120
	menu_hooked=130
	menu_button=130
else
	menu_unhooked=null
	menu_hooked=null
	menu_button=$((100 + menu_row))
fi

missing_stdout="$work/missing-stdout"
missing_stderr="$work/missing-stderr"
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui" "$client_entry" TitleMenuMissingMain >"$missing_stdout" 2>"$missing_stderr")
missing_observed="$(<"$missing_stdout")"
missing_diagnostic="$(grep '^\[ravel\]' "$missing_stderr")"
missing_expected='result='"$menu_unhooked"'
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
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui" "$client_entry" TitleMenuMissingMain >"$invalid_stdout" 2>"$invalid_stderr")
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
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui" "$client_entry" Main >"$relaunch_stdout" 2>"$relaunch_stderr")
relaunch_observed="$(<"$relaunch_stdout")"
relaunch_diagnostic="$(grep '^\[ravel\]' "$relaunch_stderr")"
relaunch_expected='filtered=a
b
simple=ab
valid=true
transformed=1 methods='"$settings_methods"'
client=false:player:false
common=verbose:player:false
saved=1:0
invalid=1:114:client.enabled78:`client.enabled` was left unchanged: the setting holds a boolean, not a string
clientAfter=false:player:false
commonAfter=verbose:player:false
mods=1:36:client1:01:51:111:client_only7:running8:required4:true5:false4:true1:01:213:configuration7:running8:required4:true5:false4:true1:01:310:registries7:running8:required4:true5:false5:false1:01:410:saved_data7:running8:required4:true5:false5:false1:01:511:server_only8:resolved8:required5:false5:false5:false1:01:0
[ravel] rewrote 1 classes and '"$settings_methods"' methods; 4 hook invocations reached native code'

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

# The screen sections, which need more than a name to coincide: they press
# buttons and read widget lists that survive a re-init, so they need stand-ins
# that behave rather than the generated stubs' empty bodies. Those stand-ins are
# generated too, from the same version data the screens they carry are, so these
# sections run wherever the screen jar itself can be built.
cp "$screen" "$(dirname "$agent")/ravel-screen.jar"

settings_stdout="$work/settings-stdout"
settings_stderr="$work/settings-stderr"
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui:$screen" "$client_entry" SettingsScreenMain >"$settings_stdout" 2>"$settings_stderr")
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
(cd "$refusal_work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui:$screen" "$client_entry" SettingsScreenMain refuse-save >"$refusal_stdout" 2>"$refusal_stderr")
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
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui:$screen" "$client_entry" ProfilingScreenMain >"$profiling_stdout" 2>"$profiling_stderr")
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
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-Dravel.profiling=true \
	-cp "$ui:$screen" "$client_entry" ProfilingScreenMain driven >"$driven_stdout" 2>"$driven_stderr")
driven_observed="$(<"$driven_stdout")"
# The two smallest windows spend every row on the summary and the statement, so
# they draw no mod row to read a count off; that is the documented row budget
# rather than a lost measurement, and only the largest window shows the whole set.
# 640x360 is absent here and not from the off report beside it, for the reason
# `ProfilingScreenMain.DRIVEN_SIZES` records.
driven_expected='driven=320x240 fits=true within=true ticks=none entered=none
driven=480x270 fits=true within=true ticks=none entered=none
driven=960x540 fits=true within=true ticks=none entered=0,4,4,4,4
[ravel] rewrote 2 classes and 4 methods; 7 hook invocations reached native code'

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
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui" "$client_entry" Main >"$ui_relaunch_stdout" 2>"$ui_relaunch_stderr")
ui_relaunch_observed="$(<"$ui_relaunch_stdout")"
ui_relaunch_expected='filtered=a
b
simple=ab
valid=true
transformed=1 methods='"$settings_methods"'
client=true:player:false
common=quiet:player:false
saved=1:0
invalid=1:114:client.enabled78:`client.enabled` was left unchanged: the setting holds a boolean, not a string
clientAfter=false:player:false
commonAfter=quiet:player:true
mods=1:36:client1:01:51:111:client_only7:running8:required4:true5:false4:true1:01:213:configuration7:running8:required4:true5:false4:true1:01:310:registries7:running8:required4:true5:false5:false1:01:410:saved_data7:running8:required4:true5:false5:false1:01:511:server_only8:resolved8:required5:false5:false5:false1:01:0
[ravel] rewrote 1 classes and '"$settings_methods"' methods; 4 hook invocations reached native code'

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
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-Dstdout.encoding=UTF-8 \
	-cp "$ui" "$client_entry" TitleMenuMain >"$menu_stdout" 2>"$menu_stderr")
menu_observed="$(<"$menu_stdout")"
menu_diagnostic="$(grep '^\[ravel\]' "$menu_stdout")"

menu_expected='result='"$menu_hooked"'
buttons=1
label=Ravel Mods
buttonY='"$menu_button"'
screen='"$mods_screen"'
modWidgets=10
notices=0
within=true
aligned=true
noticeRows=0
summary=true 5 mods on client, none needing attention
row=true 1. client_only — running
row=true 2. configuration — running
row=true 3. registries — running
row=true 4. saved_data — running
row=true 5. server_only — resolved (not on this side)
sideways=false held=true layouts=0
scrollable=false rows=7 shown=7 held=true hooks=0
keyDown=false at=0
keyPageDown=false at=0
keyPageUp=false at=0
keyUp=false at=0
keyOther=false at=0
keyEscape=true at=0
focusDone=false kept=true
focusMoved=false drawn=true kept=true
entry=Settings: client_only
settings='"$settings_screen"'
settingsOwner=client_only
settingsRows=1
settingsDone='"$mods_screen"' crossed=1 saves=1:1
entry=Settings: configuration
settings='"$settings_screen"'
settingsOwner=configuration
settingsRows=2
settingsDone='"$mods_screen"' crossed=1 saves=2:2
entry=Profiling
profiling='"$profiling_screen"'
profilingDone='"$mods_screen"' crossed=1
entry=gui.done
hostile=negativeNotices:false
hostile=shortNoticeBlock:false
hostile=wellFormed:true
emptyRead=true
emptySummary=No mods are installed, 1 needs attention
failedRead=true
failedSummary=true 1 mod on client, 1 needs attention
failedRow=true 1. brokenmod 2.1.0 — failed  [needs attention]
failedRow=true a required registry claim was refused at registry
failedRow=true worldgen/biome via brokenmod -> otherpack: the claim
failedRow=true was already held by another mod and this one asked
failedRow=true for it again, so neither could be registered
[ravel] rewrote 3 classes and 5 methods; 19 hook invocations reached native code'
menu_expected_diagnostic='[ravel] rewrote 3 classes and 5 methods; 19 hook invocations reached native code'

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

# The same screen on an instance whose `resourcePacks` line is not a list. Ravel
# writes the pack, leaves that line exactly as it is, and cannot switch the pack
# on — so every model the mods registered draws as a placeholder and this notice
# is the only thing that says why. It reaches the summary and drawn rows here,
# which is the surface launch 14 found it never reached, and all seven of its
# lines are on the screen where launch 20 saw one: the window opens on the first
# eight rows of fourteen and `walk` below scrolls to the rest.
notice_work="$root/target/ravel-jvm/harness-notice"
rm -rf "$notice_work"
cp -r "$work" "$notice_work"
# Without this the seeder answers from its own record before it reads settings.
rm -f "$notice_work/ravel/seeded-packs"
printf 'version:1\nresourcePacks:oops\n' >"$notice_work/options.txt"
notice_stdout="$notice_work/notice-stdout"
notice_stderr="$notice_work/notice-stderr"
(cd "$notice_work" && "${jvm[@]}" \
	-javaagent:"$notice_work/agent/ravel-agent.jar" \
	-Dravel.host="$host" \
	-Dstdout.encoding=UTF-8 \
	-cp "$notice_work/ui" "$client_entry" TitleMenuMain 400x300 >"$notice_stdout" 2>"$notice_stderr")
notice_observed="$(grep -E '^(modWidgets|notices|within|aligned|summary|noticeRows?|row|entry|scrollable|sideways|key|focus|failed|walk)' "$notice_stdout")"
notice_expected='modWidgets=11
notices=1
within=true
aligned=true
noticeRows=7
summary=true 5 mods on client, 1 needs attention, rows 1-8 of 14
noticeRow=true [needs attention] Ravel wrote the resource pack its
noticeRow=true mods supply and could not switch it on, because the
noticeRow=true `resourcePacks` list in `options.txt` is not a list
noticeRow=true of pack names and was left exactly as it is. It
noticeRow=true starts `oops`. Repairing or deleting that line lets
noticeRow=true the next launch switch the pack on. Turn `ravel` on
noticeRow=true under Options, Resource Packs to use them now.
row=true 1. client_only — running
sideways=false held=true layouts=0
walkSummary=true 5 mods on client, 1 needs attention, rows 1-8 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 2-9 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 3-10 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 4-11 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 5-12 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 6-13 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 7-14 of 14
walkRows=14 shown=8 positions=7
walkOrder=true slices=true held=true stable=true
walkTrailing=true within=true fits=true aligned=true
walkLayouts=14 perLayout=true
keyDown=true at=1
keyPageDown=true at=6
keyPageUp=true at=0
keyUp=false at=0
keyOther=false at=0
keyEscape=true at=0
focusDone=true kept=true
focusMoved=true drawn=true kept=true
entry=Profiling
entry=gui.done
failedRead=true
failedSummary=true 1 mod on client, 1 needs attention
failedRow=true 1. brokenmod 2.1.0 — failed  [needs attention]
failedRow=true a required registry claim was refused at registry
failedRow=true worldgen/biome via brokenmod -> otherpack: the claim
failedRow=true was already held by another mod and this one asked
failedRow=true for it again, so neither could be registered'

printf '%s\n' "$notice_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$notice_stderr"

if [ "$notice_observed" != "$notice_expected" ]; then
	echo >&2
	echo "the pack notice did not reach the mod screen; expected:" >&2
	printf '%s\n' "$notice_expected" >&2
	exit 1
fi

# The same instance in the window a 1920x1080 display gives this screen at the
# GUI scale the game picks for it. Three more rows than the run above, and the
# list is the same fourteen rows seen through a taller window: the same rows in
# the same order, three fewer stops to reach the end of them. What must not
# change between the two sizes is that every row is still reachable, the summary
# still says which of them is on screen, and Profiling and Done are still inside
# the window.
sized_stdout="$notice_work/sized-stdout"
sized_stderr="$notice_work/sized-stderr"
(cd "$notice_work" && "${jvm[@]}" \
	-javaagent:"$notice_work/agent/ravel-agent.jar" \
	-Dravel.host="$host" \
	-Dstdout.encoding=UTF-8 \
	-cp "$notice_work/ui" "$client_entry" TitleMenuMain 640x360 >"$sized_stdout" 2>"$sized_stderr")
sized_observed="$(grep -E '^(modWidgets|within|aligned|summary|noticeRows?|row|entry|scrollable|sideways|key|focus|failed|walk)' "$sized_stdout")"
sized_expected='modWidgets=14
within=true
aligned=true
noticeRows=7
summary=true 5 mods on client, 1 needs attention, rows 1-11 of 14
noticeRow=true [needs attention] Ravel wrote the resource pack its
noticeRow=true mods supply and could not switch it on, because the
noticeRow=true `resourcePacks` list in `options.txt` is not a list
noticeRow=true of pack names and was left exactly as it is. It
noticeRow=true starts `oops`. Repairing or deleting that line lets
noticeRow=true the next launch switch the pack on. Turn `ravel` on
noticeRow=true under Options, Resource Packs to use them now.
row=true 1. client_only — running
row=true 2. configuration — running
row=true 3. registries — running
row=true 4. saved_data — running
sideways=false held=true layouts=0
walkSummary=true 5 mods on client, 1 needs attention, rows 1-11 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 2-12 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 3-13 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 4-14 of 14
walkRows=14 shown=11 positions=4
walkOrder=true slices=true held=true stable=true
walkTrailing=true within=true fits=true aligned=true
walkLayouts=8 perLayout=true
keyDown=true at=1
keyPageDown=true at=3
keyPageUp=true at=0
keyUp=false at=0
keyOther=false at=0
keyEscape=true at=0
focusDone=true kept=true
focusMoved=true drawn=true kept=true
entry=Profiling
entry=gui.done
failedRead=true
failedSummary=true 1 mod on client, 1 needs attention
failedRow=true 1. brokenmod 2.1.0 — failed  [needs attention]
failedRow=true a required registry claim was refused at registry
failedRow=true worldgen/biome via brokenmod -> otherpack: the claim
failedRow=true was already held by another mod and this one asked
failedRow=true for it again, so neither could be registered'

printf '%s\n' "$sized_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$sized_stderr"

if [ "$sized_observed" != "$sized_expected" ]; then
	echo >&2
	echo "the notice did not fit the window it was budgeted for; expected:" >&2
	printf '%s\n' "$sized_expected" >&2
	exit 1
fi

# The window with one row below the summary, which is where a list this size is
# either scrollable or useless. Nothing is dropped at this size and nothing is
# counted as dropped: all fourteen rows are reached one at a time, in the same
# order the taller windows draw them, and the summary says which one is on
# screen. This is the size the old row budget could say least about — it had two
# groups to report as cut and no room to report them, so the summary itself was
# drawn clipped — and the position clause is short enough to fit where those
# clauses were not.
#
# It is also what a page means when a window has one row: `keyPageDown` moves by
# one, and the focus a settings entry held does not survive a notch, because the
# window it lands on draws a different row. That is `drawn=false kept=false`, and
# the two agreeing is the assertion — the focus comes back where the entry
# survived and nowhere else.
tiny_stdout="$notice_work/tiny-stdout"
tiny_stderr="$notice_work/tiny-stderr"
(cd "$notice_work" && "${jvm[@]}" \
	-javaagent:"$notice_work/agent/ravel-agent.jar" \
	-Dravel.host="$host" \
	-Dstdout.encoding=UTF-8 \
	-cp "$notice_work/ui" "$client_entry" TitleMenuMain 400x140 >"$tiny_stdout" 2>"$tiny_stderr")
tiny_observed="$(grep -E '^(modWidgets|within|aligned|summary|noticeRows?|row|entry|scrollable|sideways|key|focus|failed|walk)' "$tiny_stdout")"
tiny_expected='modWidgets=4
within=true
aligned=true
noticeRows=7
summary=true 5 mods on client, 1 needs attention, rows 1-1 of 14
noticeRow=true [needs attention] Ravel wrote the resource pack its
sideways=false held=true layouts=0
walkSummary=true 5 mods on client, 1 needs attention, rows 1-1 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 2-2 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 3-3 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 4-4 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 5-5 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 6-6 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 7-7 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 8-8 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 9-9 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 10-10 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 11-11 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 12-12 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 13-13 of 14
walkSummary=true 5 mods on client, 1 needs attention, rows 14-14 of 14
walkRows=14 shown=1 positions=14
walkOrder=true slices=true held=true stable=true
walkTrailing=true within=true fits=true aligned=true
walkLayouts=28 perLayout=true
keyDown=true at=1
keyPageDown=true at=2
keyPageUp=true at=1
keyUp=true at=0
keyOther=false at=0
keyEscape=true at=0
focusDone=true kept=true
focusMoved=true drawn=false kept=false
entry=Profiling
entry=gui.done
failedRead=true
failedSummary=true 1 mod on client, 1 needs attention, rows 1-1 of 5
failedRow=true 1. brokenmod 2.1.0 — failed  [needs attention]'

printf '%s\n' "$tiny_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$tiny_stderr"

if [ "$tiny_observed" != "$tiny_expected" ]; then
	echo >&2
	echo "a one-row window did not reach every row of the list; expected:" >&2
	printf '%s\n' "$tiny_expected" >&2
	exit 1
fi

# The window with no room for a row at all. `Layout.rows` bottoms out at one row
# and the summary takes it, so the list has none on screen — and this is the size
# where a screen that counted notches anyway does its damage: the offset walks to
# the end of a list nobody scrolled, and the next window tall enough to draw a row
# opens part of the way down it. So nothing moves here. The wheel and all four
# keys are refused, the offset is where it started, and the summary says there is
# no room rather than claiming a slice.
#
# `aligned=false` here is the summary and not a row. It is the one line this
# screen never splits, it is wider than the column at this width, and a line
# wider than the box it is drawn in is drawn from a different offset by the
# version that centres. `summary=false` on the line above says the same thing:
# what is misaligned is exactly what is clipped, and every row this screen draws
# at all is both whole and aligned.
noroom_stdout="$notice_work/noroom-stdout"
noroom_stderr="$notice_work/noroom-stderr"
(cd "$notice_work" && "${jvm[@]}" \
	-javaagent:"$notice_work/agent/ravel-agent.jar" \
	-Dravel.host="$host" \
	-Dstdout.encoding=UTF-8 \
	-cp "$notice_work/ui" "$client_entry" TitleMenuMain 400x118 >"$noroom_stdout" 2>"$noroom_stderr")
noroom_observed="$(grep -E '^(modWidgets|within|aligned|summary|noticeRows?|row|entry|scrollable|sideways|key|focus|failed|walk)' "$noroom_stdout")"
noroom_expected='modWidgets=3
within=true
aligned=false
noticeRows=7
summary=false 5 mods on client, 1 needs attention, no room for 14 rows
sideways=false held=true layouts=0
scrollable=false rows=14 shown=0 held=true hooks=0
keyDown=false at=0
keyPageDown=false at=0
keyPageUp=false at=0
keyUp=false at=0
keyOther=false at=0
keyEscape=true at=0
focusDone=false kept=true
focusHeld=none
entry=Profiling
entry=gui.done
failedRead=true
failedSummary=false 1 mod on client, 1 needs attention, no room for 5 rows'

printf '%s\n' "$noroom_observed"
while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$noroom_stderr"

if [ "$noroom_observed" != "$noroom_expected" ]; then
	echo >&2
	echo "a window with room for no row did not refuse to move; expected:" >&2
	printf '%s\n' "$noroom_expected" >&2
	exit 1
fi

section_ran screens

# The running side, named from the entry point the process was started at.
#
# Nothing at premain says which distribution a process is. The command line
# carries no program arguments, the only property the game sets is
# `java.awt.headless`, and asking what the JVM can load answers wrongly on the
# published server jar, which is a bundler: at premain its real jars are on no
# loader yet. What does say it is the class the process entered at, and the
# transformer sees every class before it initialises.
#
# So the three cases below vary the entry point and nothing else. The first two
# run against one tree holding both entry points, so what decides the answer is
# which one the JVM was pointed at rather than which one was reachable. The third
# holds neither.
#
# Two of them also carry the composed set's side rule end to end: `client_only`
# runs on the client and is parked on the dedicated server, `server_only` does the
# opposite, and both are decided by the `sides` list each mod ships.
#
# The two names are read back out of the generated `GameNames` rather than
# trusted: they are unobfuscated under every tracked version, and a version that
# renamed one would leave the stand-ins standing in for nothing.
for pair in "CLIENT_ENTRY_POINT:$client_entry" "SERVER_ENTRY_POINT:$server_entry"; do
	generated="$(linked_name "$generated_names" "${pair%%:*}")"
	if [ "$generated" != "${pair#*:}" ]; then
		echo >&2
		echo "this version links ${pair%%:*} as $generated, and the stand-in the cases" >&2
		echo "below enter at is written as ${pair#*:}, so no case enters an entry point" >&2
		exit 1
	fi
done

side="$work/side"
# A directory per case rather than one shared between them. Ravel writes a
# resource pack into the working directory it is run in, so two cases sharing one
# would have the second read what the first left, and the notice the first case
# asserts is the notice an instance with no `options.txt` yet produces.
mkdir -p "$side/client" "$side/dedicated" "$side/none" "$side/classes" "$side/no-entry-method"
entry_points "$side/client"
entry_points "$side/dedicated"
javac -Xlint:all -Werror -cp "$jar" -d "$side/classes" "$root/jvm/harness/SideMain.java" \
	"$root/jvm/harness/LoadEntryMain.java"
javac -Xlint:all -Werror -d "$side/no-entry-method" \
	"$root/jvm/harness/no-entry-method/net/minecraft/server/Main.java"

side_expected='client=mods=1:36:client1:01:51:111:client_only7:running8:required4:true5:false4:true1:01:213:configuration7:running8:required4:true5:false4:true1:01:310:registries7:running8:required4:true5:false5:false1:01:410:saved_data7:running8:required4:true5:false5:false1:01:511:server_only8:resolved8:required5:false5:false5:false1:01:1238:Ravel wrote the resource pack its mods supply and could not switch it on, because this instance has no `options.txt` yet. It will be switched on the next time the game starts. Turn `ravel` on under Options, Resource Packs to use them now.
dedicated=mods=1:316:dedicated-server1:01:51:113:configuration7:running8:required4:true5:false4:true1:01:210:registries7:running8:required4:true5:false5:false1:01:310:saved_data7:running8:required4:true5:false5:false1:01:411:server_only7:running8:required4:true5:false5:false1:01:511:client_only8:resolved8:required5:false5:false5:false1:01:0'
side_observed=""
for case in client dedicated; do
	case_stdout="$side/$case-stdout"
	case_stderr="$side/$case-stderr"
	if [ "$case" = client ]; then
		case_entry="$client_entry"
	else
		case_entry="$server_entry"
	fi
	(cd "$side/$case" && "${jvm[@]}" \
		-javaagent:"$agent" \
		-Dravel.host="$host" \
		-cp ".:$side/classes" "$case_entry" SideMain >"$case_stdout" 2>"$case_stderr")
	side_observed="$side_observed$case=$(head -n 1 "$case_stdout")"$'\n'
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$case_stderr"
done
side_observed="${side_observed%$'\n'}"

printf '%s\n' "$side_observed"

if [ "$side_observed" != "$side_expected" ]; then
	echo >&2
	echo "the entry point the process was started at did not decide what ran; expected:" >&2
	printf '%s\n' "$side_expected" >&2
	exit 1
fi

# And the advice is a client's — the advice, not every notice. The side filter
# names kinds rather than a shared ending: the two that send a player to Options,
# Resource Packs are the two a dedicated server has no screen for, and the record
# Ravel could not keep is a fault on every side and rides everywhere. These
# instances have no `options.txt`, so the kind this launch seeds is the first of
# those two, and it is matched by the sentence only that kind writes rather than
# by the ending all three of the loud ones share. The unfiltered kind is driven
# in `ravel-host-runtime`'s own unit test, which is where it can be produced.
no_settings='could not switch it on, because this instance has no `options.txt` yet'
if grep -qF "$no_settings" "$side/dedicated-stdout"; then
	echo >&2
	echo "a dedicated server was given advice about a screen it does not have:" >&2
	grep '^mods=' "$side/dedicated-stdout" >&2
	exit 1
fi
if ! grep -qF "$no_settings" "$side/client-stdout"; then
	echo >&2
	echo "no side carried that notice kind at all, so the case above proves nothing" >&2
	exit 1
fi
# And nothing else reached it either, so the filter is a filter and not a launch
# that happened to word its notice differently.
if grep -qF 'resource pack its mods supply' "$side/dedicated-stdout"; then
	echo >&2
	echo "a dedicated server carried a pack notice under another wording:" >&2
	grep '^mods=' "$side/dedicated-stdout" >&2
	exit 1
fi

# An entry point that carries the name and not the method. The transformer sees
# the class it names the side from, finds no `main([Ljava/lang/String;)V` to
# prepend to, and records the miss — and the prologue therefore never runs, so no
# side is named and nothing is installed. Ravel is present and inert, which is
# indistinguishable from a working launch in everything except the two lines
# below: one says which entry point loaded without entering, and one says why.
no_method_stdout="$side/no-entry-method-stdout"
no_method_stderr="$side/no-entry-method-stderr"
(cd "$side/no-entry-method" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp ".:$side/classes" LoadEntryMain "$server_entry" \
	>"$no_method_stdout" 2>"$no_method_stderr")

while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$no_method_stderr"

no_method_expected="loaded=$server_entry
[ravel] rewrote 0 classes and 0 methods; 0 hook invocations reached native code
[ravel] the distribution entry point $server_entry loaded and Ravel's prologue did not run, so no side was named and no mod set was installed
[ravel] failed to transform 1 classes
[ravel] failed target: $server_entry=missing entry method"
no_method_observed="$(<"$no_method_stdout")"
printf '%s\n' "$no_method_observed"
if [ "$no_method_observed" != "$no_method_expected" ]; then
	echo >&2
	echo "an entry point Ravel could not enter was not reported as one; expected:" >&2
	printf '%s\n' "$no_method_expected" >&2
	exit 1
fi

# The process that is not the game. A JVM that enters neither distribution's
# entry point names no side, and nothing has gone wrong in it: a launcher, a
# bundler that has not extracted its server yet, this harness itself. So it
# installs nothing, exits as it would have without Ravel, and says at shutdown
# that it did — and the crossing a driver makes before any side is named answers
# nothing rather than a set composed for a guess.
none_stdout="$side/none-stdout"
none_stderr="$side/none-stderr"
none_status=0
(cd "$side/none" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp ".:$side/classes" SideMain >"$none_stdout" 2>"$none_stderr") || none_status=$?

while IFS= read -r line; do
	printf '%s\n' "$line" >&2
done <"$none_stderr"

if [ "$none_status" -ne 0 ]; then
	echo >&2
	echo "a JVM that is not the game was refused rather than left alone" >&2
	cat "$none_stdout" >&2
	exit 1
fi

if [ "$(head -n 1 "$none_stdout")" != uninstalled ]; then
	echo >&2
	echo "a crossing made before any side was named did not answer nothing:" >&2
	head -n 1 "$none_stdout" >&2
	exit 1
fi

if grep -q '^mods=' "$none_stdout"; then
	echo >&2
	echo "a launch that named no side still produced a mod surface:" >&2
	grep '^mods=' "$none_stdout" >&2
	exit 1
fi

none_reported='[ravel] no distribution entry point loaded, so no side was named and no mod set was installed'
if ! grep -qF "$none_reported" "$none_stdout"; then
	echo >&2
	echo "the shutdown report did not say why this launch installed nothing; expected:" >&2
	printf '%s\n' "$none_reported" >&2
	exit 1
fi

section_ran side

# One installed set played, saved, exited and reloaded — M6-W4's cluster
# scenario, against the shipped agent jar and the real native library.
#
# What makes it a launch rather than a fixture is the packages tree: the set is
# copied into the game directory the way an install writes it, and the launch
# discovers it there, parses each manifest against the package schema and
# verifies each declared digest before a mod is admitted. Nothing here names a
# mod to the runtime, so a package the host was not composed from refuses the
# launch rather than being quietly dropped, which is the case below that says the
# directory was read at all.
#
# The world round trip crosses at the two persistence exports the game itself
# calls, so the driver names no game class and is written once rather than
# generated. It runs as two processes because a world reopened inside the process
# that wrote it says nothing about what reached the disk: the first plays, saves
# and exits, and the second is handed the file the first left behind.
cluster="$work/cluster"
cluster_record="$cluster/ravel-mods-record"
mkdir -p "$cluster/classes" "$cluster/ravel/config" "$cluster/ravel/mods"
printf 'version:1\n' >"$cluster/options.txt"
cp "$work/ravel/config/configuration.json" "$cluster/ravel/config/configuration.json"

# The installed set, copied whole and unrewritten, which is exactly what
# installing one is: `ravel-build` copies the directory the mod ships and the
# digests it declares travel with it. The world-data owner is one of them, so the
# round trip below is a mod's own state rather than an empty aggregate.
#
# Two of the five packages this build is composed from rather than all five,
# because `test-mods/configuration`, `test-mods/registries` and
# `test-mods/server-only` each declare a `src/lib.rs` digest their own file no
# longer hashes to. An installed package whose declared digest does not match is
# refused, so those three cannot be installed into any instance today; that is a
# defect in those manifests and not something this section works around by
# skipping the verification.
cluster_packages=(client-only saved-data)
for package in "${cluster_packages[@]}"; do
	cp -r "$root/test-mods/$package" "$cluster/ravel/mods/$package"
done

javac -Xlint:all -Werror -d "$cluster/classes" "${entry_sources[@]}"
# `ClusterMain` needs the agent jar on its compile class path for the two
# persistence crossings and the mod surface. A real launch never compiles
# against it; the game declares those calls itself.
javac -Xlint:all -Werror -cp "$jar" -d "$cluster/classes" \
	"$root/jvm/harness/cluster/ClusterMain.java"

# Each half is one process. `label` is what fails if it does not produce
# `expected`, and `status` is the exit it has to end on — nonzero for the halves
# whose subject is a refusal.
cluster_run() {
	local label="$1" half="$2" expected="$3" want_status="${4:-0}"
	local out="$cluster/$label-stdout"
	local err="$cluster/$label-stderr"
	local status=0
	(cd "$cluster" && "${jvm[@]}" \
		-javaagent:"$agent" \
		-Dravel.host="$host" \
		-cp classes "$client_entry" ClusterMain "$half" "$cluster_record" \
		>"$out" 2>"$err") || status=$?
	local observed
	observed="$(<"$out")"
	printf '%s\n' "$observed"
	while IFS= read -r line; do
		printf '%s\n' "$line" >&2
	done <"$err"
	if [ "$status" -ne "$want_status" ]; then
		echo >&2
		echo "cluster run '$label' exited $status and the scenario needs $want_status" >&2
		exit 1
	fi
	if [ "$observed" != "$expected" ]; then
		echo >&2
		echo "cluster run '$label' did not produce the recorded output; expected:" >&2
		printf '%s\n' "$expected" >&2
		exit 1
	fi
}

# A first visit: a world nothing has written into, opened with the set installed,
# saved, and left. The mod count is the launch's own surface, so a package the
# discovery refused would have failed the launch before this line rather than
# reduced it.
cluster_play='[ravel] cluster fresh-play=passed mods=5
[ravel] cluster save=passed record=written
[ravel] cluster exit=passed record=captured
[ravel] rewrote 0 classes and 0 methods; 0 hook invocations reached native code'
cluster_run fresh-play play "$cluster_play"

# The second visit, in a second process holding nothing but the file the first
# one left. What it writes has to be a successor of what it read: a visit that
# ignored the record and initialized fresh state would write the first visit's
# bytes back unchanged.
cluster_reload='[ravel] cluster reload=passed mods=5 record=advanced
[ravel] rewrote 0 classes and 0 methods; 0 hook invocations reached native code'
cluster_run reload reload "$cluster_reload"
cp "$cluster_record.reloaded" "$cluster/first-reloaded"

# The same record, read again by a third process. The successor is a function of
# the record and of nothing else, so the two answers are the same bytes — which
# is what says the reload read the stored record itself rather than something
# that happened to be admissible.
cluster_run reload-again reload "$cluster_reload"
if ! cmp -s "$cluster/first-reloaded" "$cluster_record.reloaded"; then
	echo >&2
	echo "two reloads of one record wrote different aggregates, so what a reload reads is not the record it was handed" >&2
	exit 1
fi

# The mutation case, which is the whole reason the three runs above mean
# anything: one byte of the stored record is flipped and the reload has to refuse
# the world rather than open it on data nobody can vouch for. What the refusal
# says depends on which byte moved, so the diagnostic is required to name the
# step and the record rather than matched word for word.
cluster_offset=$(( $(stat -c%s "$cluster_record") / 2 ))
cluster_byte="$(od -An -tu1 -N1 -j "$cluster_offset" "$cluster_record" | tr -d ' ')"
printf "$(printf '\\x%02x' $(( cluster_byte ^ 1 )))" \
	| dd of="$cluster_record" bs=1 seek="$cluster_offset" count=1 conv=notrunc status=none
cluster_mutation_out="$cluster/mutated-stdout"
cluster_mutation_status=0
(cd "$cluster" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp classes "$client_entry" ClusterMain reload "$cluster_record" \
	>"$cluster_mutation_out" 2>"$cluster/mutated-stderr") || cluster_mutation_status=$?
cat "$cluster_mutation_out"
if [ "$cluster_mutation_status" -eq 0 ]; then
	echo >&2
	echo "a world whose stored Ravel record had a byte flipped reloaded anyway, so the reload step passes on data nothing checked" >&2
	exit 1
fi
if ! grep -q '^\[ravel\] cluster reload=failed refusal=' "$cluster_mutation_out"; then
	echo >&2
	echo "a corrupted record did not fail the reload step by name; it printed:" >&2
	cat "$cluster_mutation_out" >&2
	exit 1
fi

section_ran cluster
