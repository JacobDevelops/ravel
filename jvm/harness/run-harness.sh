#!/usr/bin/env bash
# Exercises the agent, transformer and native library against a real JVM and stand-in game.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
jar="$root/target/ravel-jvm/ravel-agent.jar"
screen="$root/target/ravel-jvm/ravel-screen.jar"
library="$root/target/release"
# Hook loads an explicit file rather than searching the JVM's library path.
host="$library/libravel_bootstrap.so"
work="$root/target/ravel-jvm/harness"
agent="$work/agent/ravel-agent.jar"
# Stand-ins carry the selected version's linked names.
standins="$root/target/ravel-jvm/generated-harness"
# Keep generated game, driver and sibling sources in separate compile trees.
standin_game="$standins/game"
standin_drivers="$standins/driver"
standin_sibling="$standins/sibling"

if [ ! -f "$jar" ]; then
	echo "the agent jar is missing; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java && ./jvm/build/build-agent.sh" >&2
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
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi

# Guava and DataFixerUpper are unrenamed libraries shared by all generated stand-ins.
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

# Remap unnamed-package stand-ins to linked names after each tree's last javac.
aliases="$standins/aliases.txt"
if [ ! -f "$aliases" ]; then
	echo "the generated stand-in game names no aliases.txt, so nothing says which names" >&2
	echo "it was written under; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi
remapper="$work/remap"
mkdir -p "$remapper"
javac -Xlint:all -Werror -d "$remapper" "$root/jvm/harness/remap/Remap.java"
remap() {
	"${jvm[@]}" -cp "$remapper" Remap "$aliases" "$@"
}

# Side detection uses the entry point seen by the transformer, not the driver.
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

# Disable shared archives: appending the agent jar otherwise adds JVM notices to stderr.
jvm=(java -Xshare:off)

sections_record="$root/target/ravel-jvm/harness-sections.txt"
: >"$sections_record"
sections_ran=""
section_ran() {
	sections_ran="${sections_ran:+$sections_ran,}$1"
	printf '%s' "$sections_ran" >"$sections_record"
}

# Read screen names from the same generated constants the agent uses.
generated_screens="$root/target/ravel-jvm/generated/dev/ravel/jvm/ScreenNames.java"
generated_names="$root/target/ravel-jvm/generated/dev/ravel/jvm/GameNames.java"
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

# Transformer-only targets use reviewed names on both sides, independent of version data.
javac -d "$work" "$root/jvm/harness/net/minecraft/util/StringUtil.java"
entry_points "$work"

# Isolate generated UI classes from the transformer stand-in with the same reviewed name.
ui="$work/ui"
mkdir -p "$ui"
javac -Xlint:all -Werror -d "$ui" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$ui"
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
# This fixture substitutes Hook, so compile the modern adapter closure rather
# than sweeping legacy sources that require a legacy generated Hook surface.
agent_sources=()
for source in Boot Agent Transformer HookTarget Members Content Datapack Outward Persistence Screens; do
	agent_sources+=("$root/jvm/bootstrap/dev/ravel/jvm/$source.java")
done
javac -Xlint:all -Werror -d "$order/agent-classes" \
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/GameNames.java" \
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/GameMembers.java" \
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/MenuLayout.java" \
	"$root/target/ravel-jvm/generated/dev/ravel/jvm/ScreenNames.java" \
	"${agent_sources[@]}" \
	"$root/jvm/harness/initialization/dev/ravel/jvm/Hook.java"
mapfile -d '' hierarchy_sources < <(find "$root/jvm/harness/hierarchy" -name '*.java' -print0 | sort -z)
javac -d "$order/classes" "${hierarchy_sources[@]}"
javac -cp "$work:$order/classes:$order/agent-classes" -d "$order/classes" \
	"$root/jvm/harness/InitializationOrderMain.java" \
	"$root/jvm/harness/HierarchyResolutionMain.java"
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


section_ran initialization
section_ran hierarchy

# Field targets must survive rewriting; markers and counters identify the transformed class.

members="$work/members"
mkdir -p "$members/classes"
mapfile -d '' member_sources < <(find "$root/jvm/harness/members" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -d "$members/classes" "${member_sources[@]}"
entry_points "$members/classes"
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

# HookTarget entries span six lines: the prologue has four fields before it and one after.
generated_hook="$root/target/ravel-jvm/generated/dev/ravel/jvm/Hook.java"
hook_seam() {
	awk -v prologue="$1" '
		{ line[NR] = $0 }
		$0 ~ ("^[[:space:]]*\"" prologue "\",$") { at = NR }
		END {
			if (at == 0) { exit 1 }
			for (field = at - 4; field <= at + 1; field++) {
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

# Derive rewrite counts from this version's generated inventory.
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

# Fill by nameref so a missing seam exits this shell, not just a command-substitution subshell.
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

persist_seams=()
install_seams persist_seams \
	worldOpenPersistence worldSavePersistence worldCreatePackRepository screenCreateWorldInit

# Exercise reflective persistence against this version's generated stand-in game.
persist="$work/persistence"
persist_agent="$persist/agent/ravel-agent.jar"
persist_screen="$persist/agent/ravel-screen.jar"
mkdir -p "$persist/classes" "$persist/sibling" "$(dirname "$persist_agent")"
javac -Xlint:all -Werror -d "$persist/classes" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$persist/classes"
javac -Xlint:all -Werror -cp "$persist/classes:$order/agent-classes" -d "$persist/classes" \
	"$standin_drivers/PersistenceMain.java"
# Compile the sibling with the game's types and the generated ScreenNames package.
mapfile -d '' sibling_sources < <(find "$standin_sibling" -name '*.java' -print0 | sort -z)
javac -Xlint:all -Werror -cp "$persist/classes" -d "$persist/sibling" "${sibling_sources[@]}"
# The sibling and game share aliases, so remap both after compiling the sibling.
remap "$persist/classes" "$persist/sibling"
jar --create --file "$persist_screen" --date 1980-01-01T00:00:02Z -C "$persist/sibling" .
cp "$order_agent" "$persist_agent"

# Evidence binds to this declared count, checked against actual lifecycles below.
persistence_lifecycles_declared=9
persistence_lifecycles=0

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

# Stored bytes distinguish an actual read from a reported absence.
persistence_run admitted empty replacement 'old-aggregate' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=1 accessClosed=false stemClosed=false crossed=1 read=old-aggregate
saves=1 writes=1 written=replaced
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'

# A failed native returns null and must refuse, unlike the empty admission verdict.
persistence_run null-verdict null unchanged '' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=0 accessClosed=true stemClosed=true crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'

persistence_run refused 'this world holds data Ravel cannot read' unchanged '' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=0 accessClosed=true stemClosed=true crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'

# Non-replacement answers, including native failure, must leave vanilla saving intact.
for answer in unchanged failed mismatch null; do
	persistence_run "save-$answer" empty "$answer" '' 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
datapack=world/. loadsAtWrite=0 available=[ravel]
loads=1 accessClosed=false stemClosed=false crossed=1 read=absent
saves=1 writes=0 written=
[ravel] rewrote 4 classes and 4 methods; 10 hook invocations reached native code'
done

# Staging-only cases are not world lifecycles; compare stderr to catch swallowed failures.
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

# An unchanged pack needs no reload; retained selection distinguishes success from refusal.
screen_run staged-unchanged unchanged 'staged=screen stagedWrites=1 screenLoads=1 loaded=[vanilla, file/ravel=old]
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'

# Identical pack ids still require a reload when their on-disk contents change.
screen_run staged-replaced replaced 'staged=screen stagedWrites=1 screenLoads=2 loaded=[vanilla, file/ravel=new]
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code'

# A staged pack missing from discovery must report the lost recipes and tags.
screen_run staged-undiscovered undiscovered 'staged=screen stagedWrites=1 screenLoads=1 loaded=[vanilla]
[ravel] rewrote 2 classes and 2 methods; 4 hook invocations reached native code' \
	'[ravel] the game did not find the mods'"'"' data pack where it was staged, so this world will have none of their recipes or tags'

# No supplied data must add no reload or id crossing.
screen_run staged-nothing nothing 'staged=screen stagedWrites=1 screenLoads=1 loaded=[vanilla]
[ravel] rewrote 2 classes and 2 methods; 3 hook invocations reached native code'

# Without the sibling there is no refusal screen; release the lock and throw via the game.
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
# Shutdown still prints on stdout; driver counters alone reveal whether the body ran.
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

# A present server class with no installed save prologue must refuse before asking Rust.
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

if [ "$persistence_lifecycles" -ne "$persistence_lifecycles_declared" ]; then
	echo >&2
	echo "this harness ran $persistence_lifecycles world lifecycles and declares $persistence_lifecycles_declared; update persistence_lifecycles_declared and the evidence/m3 records that cite it" >&2
	exit 1
fi

section_ran persistence
# The dedicated admission crossing above, not observation hooks, refuses world loads.
section_ran refusal

# Stand-in registries reject duplicates, frozen writes and missing property ids.

content="$work/content"
content_agent="$content/agent/ravel-agent.jar"
content_screen="$content/agent/ravel-screen.jar"
mkdir -p "$content/classes" "$content/sibling" "$(dirname "$content_agent")"
javac -Xlint:all -Werror -d "$content/classes" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$content/classes"
javac -Xlint:all -Werror -cp "$content/classes:$order/agent-classes" -d "$content/classes" \
	"$standin_drivers/ContentMain.java"
remap "$content/classes"
# Compile the shipped sibling against stand-ins, not a copy of its version-specific logic.
content_sibling="$(find "$root/target/ravel-jvm/generated-screen" -name 'RavelContent.java')"
if [ ! -f "$content_sibling" ]; then
	echo "the generated content sibling is missing; run:" >&2
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi
javac -Xlint:all -Werror -cp "$content/classes" -d "$content/sibling" "$content_sibling"
remap "$content/sibling"
jar --create --file "$content_screen" --date 1980-01-01T00:00:02Z -C "$content/sibling" .
cp "$order_agent" "$content_agent"
content_seams=()
install_seams content_seams registryBuiltInFreeze

# Generator provenance selects keyed or unkeyed property expectations, never source wording.
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
	echo "  cargo run -p ravel-xtask -- generate java" >&2
	exit 1
fi
case "$properties_origin" in
shared)
	# Placing items prefix their properties with the block they hold.
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

# Only Bundled is on the system class path; game classes belong to its child loader.
mkdir -p "$content/launcher"
javac -Xlint:all -Werror -d "$content/launcher" "$root/jvm/harness/Bundled.java"

# A launcher runs through the bundler's isolated loader instead of the direct class path.
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

# Registration must also map both declared block states for protocol encoding.
content_run admitted \
	'block:example_mod:copper:50.0:1200.0:true:false,item:example_mod:ingot:16:true' \
	"block=example_mod:copper->$(described Block '' block example_mod:copper 50.0/1200.0/true/false)
item=example_mod:ingot->$(described Item '' item example_mod:ingot 16/true)
states=2 cached=2
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code"

# Repeat the admission through a platform-parented child loader with no game on the system path.
content_run bundled \
	'block:example_mod:copper:50.0:1200.0:true:false,item:example_mod:ingot:16:true' \
	"block=example_mod:copper->$(described Block '' block example_mod:copper 50.0/1200.0/true/false)
item=example_mod:ingot->$(described Item '' item example_mod:ingot 16/true)
states=2 cached=2
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code" \
	bundled

# Declare the placing item first to require block-before-item registration independent of payload order.
content_run placing-item \
	'item:example_mod:lantern:64:false,block:example_mod:lantern:3.0:3.0:false:false' \
	"block=example_mod:lantern->$(described Block '' block example_mod:lantern 3.0/3.0/false/false)
item=example_mod:lantern->$(described BlockItem example_mod:lantern/ item example_mod:lantern 64/false)
states=2 cached=2
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code"

# Failed state mapping must retain the registered block for its placing item and report the partial failure.
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

# Unknown row widths invalidate the whole payload, including already-read rows.
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

# Empty admission still crosses the native boundary.
content_run empty empty 'block=
item=
states=0 cached=0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# Native failure must register nothing.
content_run null-answer null 'block=
item=
states=0 cached=0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

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

# Literal outward payloads independently witness the Rust encoder's cross-language contract.

outward="$work/outward"
outward_agent="$outward/agent/ravel-agent.jar"
mkdir -p "$outward/classes" "$(dirname "$outward_agent")"
javac -Xlint:all -Werror -d "$outward/classes" "${standin_sources[@]}" "${libraries[@]}"
entry_points "$outward/classes"
javac -Xlint:all -Werror -cp "$outward/classes:$order/agent-classes" -d "$outward/classes" \
	"$standin_drivers/OutwardMain.java"
remap "$outward/classes"
cp "$order_agent" "$outward_agent"
outward_seams=()
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

# Exercise the hook crossing and the adapter's action read.
outward_run performed '1:11:110:audio.play1:35:audio10:door_opens1:1' \
	'played=audio:door_opens@1.0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

outward_run two '1:11:210:audio.play1:35:audio10:door_opens1:110:audio.play1:35:audio11:door_closes1:2' \
	'played=audio:door_opens@1.0,audio:door_closes@2.0
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

# Per-action arity lets an unknown verb preserve the known action that follows.
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

outward_run empty empty 'played=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

outward_run null-answer null 'played=
hooks=2
[ravel] rewrote 1 classes and 1 methods; 2 hook invocations reached native code'

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

# Missing or invalid explicit library paths must refuse instead of searching.
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

# Keep missing-target refusal in this shell, then derive counts from the generated table.
settings_seams=()
install_seams settings_seams chatFilterText
settings_target="${settings_seams[0]#-Dravel.test.target.chatFilterText=}"
settings_target="${settings_target%%:*}"
settings_methods="$(hook_targets_on "$settings_target")"
settings_target="${settings_target//\//.}"

# Settings round trips use the shipped agent and matching generated stand-ins.
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

# MenuLayout supplies either an inventory row or DERIVED; negative rows are valid values.
generated_layout="$root/target/ravel-jvm/generated/dev/ravel/jvm/MenuLayout.java"
menu_row="$(awk '/static final int RAVEL_ROW/ { sub(/^[^=]*= */, ""); sub(/;.*$/, ""); print }' \
	"$generated_layout")"
if [ -z "$menu_row" ]; then
	echo "$generated_layout declares no row, so nothing says where this version's" >&2
	echo "menu puts Ravel's own" >&2
	exit 1
fi
if [ "$menu_row" = DERIVED ]; then
	# The stand-in column starts at 100, spaced by 10, and ends at 120.
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

# UI stand-ins retain widget state across re-init, unlike compile-only stubs.
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

# Check readability at GUI-scaled sizes, including full text rather than scrolled tails.
profiling_stdout="$work/profiling-stdout"
profiling_stderr="$work/profiling-stderr"
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-cp "$ui:$screen" "$client_entry" ProfilingScreenMain >"$profiling_stdout" 2>"$profiling_stderr")
profiling_observed="$(<"$profiling_stdout")"
# Spare rows follow priority order; unmeasurable ticks must never appear as measurements.
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

# Active collectors must retain four deliveries per mod while still omitting unmeasurable ticks.
driven_stdout="$work/profiling-driven-stdout"
driven_stderr="$work/profiling-driven-stderr"
(cd "$work" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$host" \
	-Dravel.profiling=true \
	-cp "$ui:$screen" "$client_entry" ProfilingScreenMain driven >"$driven_stdout" 2>"$driven_stderr")
driven_observed="$(<"$driven_stdout")"
# Small windows have no mod-row budget; DRIVEN_SIZES documents the omitted middle size.
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

# Malformed resourcePacks must remain untouched and its notice fully reachable by scrolling.
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

# A taller viewport must preserve row order, reachability and on-screen navigation buttons.
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

# A one-row viewport must scroll every row and keep focus only on entries still drawn.
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

# With no list-row budget, scrolling must not move the offset; only the summary may clip.
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

# Entry points determine side; read generated names to detect any newly obfuscated entry point.
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
# Isolate resource-pack state so earlier launches cannot seed later cases.
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

# Only client-facing pack notice kinds are filtered; record-keeping faults apply to every side.
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
if grep -qF 'resource pack its mods supply' "$side/dedicated-stdout"; then
	echo >&2
	echo "a dedicated server carried a pack notice under another wording:" >&2
	grep '^mods=' "$side/dedicated-stdout" >&2
	exit 1
fi

# A named entry class without main must report its missing prologue and install no side.
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

# A non-game process must remain inert, with no guessed side or installed set.
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

# Discover installed packages, then save and reload mod state in separate JVM processes.
cluster="$work/cluster"
cluster_record="$cluster/ravel-mods-record"
mkdir -p "$cluster/classes" "$cluster/ravel/config" "$cluster/ravel/mods"
printf 'version:1\n' >"$cluster/options.txt"
cp "$work/ravel/config/configuration.json" "$cluster/ravel/config/configuration.json"

# Package overrides remain digest-verified; the expected surface count belongs to the composed host.
cluster_packages=()
if [ -n "${RAVEL_HARNESS_PACKAGE_DIRS:-}" ]; then
	IFS=: read -r -a cluster_packages <<<"$RAVEL_HARNESS_PACKAGE_DIRS"
else
	for package in client-only saved-data; do
		cluster_packages+=("$root/test-mods/$package")
	done
fi
if [ "${#cluster_packages[@]}" -eq 0 ]; then
	echo "RAVEL_HARNESS_PACKAGE_DIRS named no package directory" >&2
	exit 1
fi
for package in "${cluster_packages[@]}"; do
	case "$package" in
	/*) ;;
	*)
		echo "a package directory to install is named absolutely; this one was not: $package" >&2
		exit 1
		;;
	esac
	if [ ! -d "$package" ]; then
		echo "no package directory to install is at: $package" >&2
		exit 1
	fi
	cp -r "$package" "$cluster/ravel/mods/$(basename "$package")"
done
cluster_mods="${RAVEL_HARNESS_COMPOSED_MODS:-5}"
# A cluster-only host override leaves the other sections on the compiled host.
cluster_host="${RAVEL_HARNESS_HOST:-$host}"
if [ ! -f "$cluster_host" ]; then
	echo "no host library to launch the cluster with is at: $cluster_host" >&2
	exit 1
fi

javac -Xlint:all -Werror -d "$cluster/classes" "${entry_sources[@]}"
javac -Xlint:all -Werror -cp "$jar" -d "$cluster/classes" \
	"$root/jvm/harness/cluster/ClusterMain.java"

cluster_run() {
	local label="$1" half="$2" expected="$3" want_status="${4:-0}"
	local out="$cluster/$label-stdout"
	local err="$cluster/$label-stderr"
	local status=0
	(cd "$cluster" && "${jvm[@]}" \
		-javaagent:"$agent" \
		-Dravel.host="$cluster_host" \
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

cluster_play="[ravel] cluster fresh-play=passed mods=$cluster_mods
[ravel] cluster save=passed record=written
[ravel] cluster exit=passed record=captured
[ravel] rewrote 0 classes and 0 methods; 0 hook invocations reached native code"
cluster_run fresh-play play "$cluster_play"

# A reload must advance stored state rather than write the fresh world's bytes again.
cluster_reload="[ravel] cluster reload=passed mods=$cluster_mods record=advanced
[ravel] rewrote 0 classes and 0 methods; 0 hook invocations reached native code"
cluster_run reload reload "$cluster_reload"
cp "$cluster_record.reloaded" "$cluster/first-reloaded"

# Replaying the same record must produce identical successor bytes in a new process.
cluster_run reload-again reload "$cluster_reload"
if ! cmp -s "$cluster/first-reloaded" "$cluster_record.reloaded"; then
	echo >&2
	echo "two reloads of one record wrote different aggregates, so what a reload reads is not the record it was handed" >&2
	exit 1
fi

# Corruption must refuse reload; its diagnostic identifies the step and record, not a fixed byte.
cluster_offset=$(( $(stat -c%s "$cluster_record") / 2 ))
cluster_byte="$(od -An -tu1 -N1 -j "$cluster_offset" "$cluster_record" | tr -d ' ')"
printf '%b' "$(printf '\\x%02x' $(( cluster_byte ^ 1 )))" \
	| dd of="$cluster_record" bs=1 seek="$cluster_offset" count=1 conv=notrunc status=none
cluster_mutation_out="$cluster/mutated-stdout"
cluster_mutation_status=0
(cd "$cluster" && "${jvm[@]}" \
	-javaagent:"$agent" \
	-Dravel.host="$cluster_host" \
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
