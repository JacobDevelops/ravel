#!/usr/bin/env bash
set -euo pipefail
umask 077
export LC_ALL=C

usage() {
  printf 'usage: summarize-prism-launch.sh official-atm11 INSTANCE_DIR OUTPUT_JSON\n' >&2
  exit 2
}

fail() {
  printf 'launch summary rejected: %s\n' "$1" >&2
  exit 1
}

test "$#" -eq 3 || usage
workload="$1"
instance_input="$2"
output_input="$3"
test "$workload" = "official-atm11" || usage

for command in date find jq ln realpath rg sha256sum sort stat xargs; do
  command -v "$command" >/dev/null || fail "required command is unavailable"
done

repository_input="${RAVEL_EVIDENCE_ROOT:-}"
if test -z "$repository_input"; then
  repository_input="$(jj root 2>/dev/null)" || fail "Ravel repository root is unavailable"
fi
repository="$(realpath -e -- "$repository_input")"
instance="$(realpath -e -- "$instance_input")"
private_root="$(realpath -e -- "$repository/evidence/private")"
output_parent="$(realpath -e -- "$(dirname -- "$output_input")")"
output="$output_parent/$(basename -- "$output_input")"

case "$output_parent/" in
  "$private_root/"*) ;;
  *) fail "output must stay under the private evidence boundary" ;;
esac
test ! -e "$output" && test ! -L "$output" || fail "output already exists"

current_parent="$output_parent"
while true; do
  parent_mode="$(stat -c '%a' "$current_parent")"
  case "$parent_mode" in
    *[1-7][0-7] | *[0-7][1-7]) fail "output directory is not owner-only" ;;
  esac
  test "$current_parent" != "$private_root" || break
  current_parent="$(dirname -- "$current_parent")"
done

component_file="$instance/mmc-pack.json"
config_file="$instance/instance.cfg"
log_file="$instance/minecraft/logs/latest.log"
mods_dir="$instance/minecraft/mods"
saves_dir="$instance/minecraft/saves"
for required in "$component_file" "$config_file" "$log_file"; do
  test -f "$required" && test ! -L "$required" || fail "required Prism launch input is missing"
done
test -d "$mods_dir" && test -d "$saves_dir" || fail "required Minecraft instance directory is missing"

descriptor="$repository/tests/fixtures/descriptors/atm11-0.2.1.json"
manifest="$repository/tests/fixtures/local/atm11-0.2.1/extracted/manifest.json"
archive="$repository/tests/fixtures/local/atm11-0.2.1/source/atm11-0.2.1.zip"
for required in "$descriptor" "$manifest" "$archive"; do
  test -f "$required" && test ! -L "$required" || fail "pinned ATM11 fixture input is missing"
done

jq --exit-status '
  (.components | length) == 3
  and any(.components[]; .uid == "org.lwjgl3" and .version == "3.4.1")
  and any(.components[]; .uid == "net.minecraft" and .version == "26.1.2")
  and any(.components[]; .uid == "net.neoforged" and .version == "26.1.2.78")
  and ([.components[].uid]
    | all(. == "org.lwjgl3" or . == "net.minecraft" or . == "net.neoforged"))
' "$component_file" >/dev/null || fail "instance components do not match pinned ATM11"

descriptor_manifest_sha="$(jq -er '.manifest.sha256' "$descriptor")"
descriptor_archive_sha="$(jq -er '.source.sha256' "$descriptor")"
expected_references="$(jq -er '.manifest.expectedReferenceCount' "$descriptor")"
test "$(sha256sum "$manifest" | cut -d' ' -f1)" = "$descriptor_manifest_sha" \
  || fail "fixture manifest hash does not match its descriptor"
test "$(sha256sum "$archive" | cut -d' ' -f1)" = "$descriptor_archive_sha" \
  || fail "fixture archive hash does not match its descriptor"
jq --exit-status \
  --argjson expected "$expected_references" \
  '.name == "All the Mods 11"
    and .version == "0.2.1"
    and .minecraft.version == "26.1.2"
    and any(.minecraft.modLoaders[];
      .id == "neoforge-26.1.2.78" and .primary == true)
    and (.files | length) == $expected
    and all(.files[]; .required == true)' \
  "$manifest" >/dev/null || fail "fixture manifest does not match pinned ATM11 metadata"

mod_file_count="$(find "$mods_dir" -maxdepth 1 -type f -name '*.jar' -printf '.' | wc -c)"
test "$mod_file_count" -gt 0 || fail "official ATM11 instance has no mod JARs"

lifecycle="$(
  awk '
    /Sound engine started/ {
      sound_count++
      sound_line = NR
    }
    /Starting integrated minecraft server/ {
      server_count++
      server_line = NR
    }
    /Saving and pausing game/ {
      save_count++
      save_line = NR
    }
    /Stopping server/ {
      stop_count++
      stop_line = NR
    }
    /ThreadedAnvilChunkStorage: All dimensions are saved/ {
      dimensions_count++
      dimensions_line = NR
    }
    END {
      printf "%d %d %d %d %d %d %d %d %d %d\n",
        sound_count, sound_line,
        server_count, server_line,
        save_count, save_line,
        stop_count, stop_line,
        dimensions_count, dimensions_line
    }
  ' "$log_file"
)"
read -r \
  sound_count sound_line \
  server_count server_line \
  save_count save_line \
  stop_count stop_line \
  dimensions_count dimensions_line <<<"$lifecycle"
test "$sound_count" -eq 1 \
  && test "$server_count" -eq 1 \
  && test "$save_count" -eq 1 \
  && test "$stop_count" -eq 1 \
  && test "$dimensions_count" -ge 1 \
  || fail "launch log does not contain one complete client/server lifecycle"
test "$sound_line" -lt "$server_line" \
  && test "$server_line" -lt "$save_line" \
  && test "$save_line" -lt "$stop_line" \
  && test "$stop_line" -lt "$dimensions_line" \
  || fail "launch lifecycle events are not in the required order"

mapfile -d '' level_files < <(find "$saves_dir" -mindepth 2 -maxdepth 2 -type f -name level.dat -print0)
test "${#level_files[@]}" -eq 1 || fail "expected exactly one saved launch-proof world"
level_file="${level_files[0]}"
world_dir="$(dirname -- "$level_file")"
world_file_count="$(find "$world_dir" -type f -printf '.' | wc -c)"
test "$world_file_count" -gt 0 || fail "saved launch-proof world is empty"
level_dat_sha="$(sha256sum "$level_file" | cut -d' ' -f1)"
world_tree_sha="$(
  cd -- "$world_dir"
  find . -type f -print0 \
    | sort -z \
    | xargs -0 sha256sum \
    | sha256sum \
    | cut -d' ' -f1
)"

config_value() {
  local key="$1"
  local count
  count="$(rg --count-matches "^${key}=" "$config_file" 2>/dev/null || true)"
  test "${count:-0}" -eq 1 || fail "instance configuration key is absent or duplicated"
  awk -v key="$key" 'index($0, key "=") == 1 { print substr($0, length(key) + 2) }' "$config_file"
}

max_memory_mib="$(config_value MaxMemAlloc)"
min_memory_mib="$(config_value MinMemAlloc)"
window_width="$(config_value MinecraftWinWidth)"
window_height="$(config_value MinecraftWinHeight)"
override_commands="$(config_value OverrideCommands)"
pre_launch_command="$(config_value PreLaunchCommand)"
post_exit_command="$(config_value PostExitCommand)"
wrapper_command="$(config_value WrapperCommand)"
for integer in "$max_memory_mib" "$min_memory_mib" "$window_width" "$window_height"; do
  [[ "$integer" =~ ^[0-9]+$ ]] || fail "instance numeric setting is invalid"
done

test -z "$pre_launch_command" && test -z "$post_exit_command" \
  || fail "pre-launch and post-exit commands are not permitted"
if test -n "$wrapper_command" && test "$wrapper_command" != "false"; then
  fail "wrapper commands are not permitted"
fi

command_is_configured() {
  if test -n "$1" && test "$1" != "false"; then
    printf true
  else
    printf false
  fi
}

components="$(jq -c '[.components[] | {uid, version}] | sort_by(.uid)' "$component_file")"
component_sha="$(sha256sum "$component_file" | cut -d' ' -f1)"
descriptor_sha="$(sha256sum "$descriptor" | cut -d' ' -f1)"
log_size="$(stat -c '%s' "$log_file")"
observed_at="$(date --utc --date="@$(stat -c '%Y' "$log_file")" '+%Y-%m-%dT%H:%M:%SZ')"
log_summary_sha="$(
  printf '%s\n' \
    'client-ready-event-observed=true' \
    'integrated-server-started=true' \
    'save-observed=true' \
    'server-stop-observed=true' \
    'all-dimensions-saved-after-stop=true' \
    "observed-at=$observed_at" \
    "sanitized-source-size=$log_size" \
    | sha256sum \
    | cut -d' ' -f1
)"

temporary="$(mktemp "$output_parent/.launch-summary.XXXXXX")"
trap 'if test -n "${temporary:-}" && test -e "$temporary"; then rm -f -- "$temporary"; fi' EXIT

jq -n \
  --arg workload "$workload" \
  --arg observed_at "$observed_at" \
  --arg descriptor_sha "$descriptor_sha" \
  --arg archive_sha "$descriptor_archive_sha" \
  --arg manifest_sha "$descriptor_manifest_sha" \
  --argjson expected_references "$expected_references" \
  --arg component_sha "$component_sha" \
  --argjson components "$components" \
  --argjson mod_file_count "$mod_file_count" \
  --argjson max_memory_mib "$max_memory_mib" \
  --argjson min_memory_mib "$min_memory_mib" \
  --argjson window_width "$window_width" \
  --argjson window_height "$window_height" \
  --argjson override_commands "$(command_is_configured "$override_commands")" \
  --argjson pre_launch "$(command_is_configured "$pre_launch_command")" \
  --argjson post_exit "$(command_is_configured "$post_exit_command")" \
  --arg log_summary_sha "$log_summary_sha" \
  --arg level_dat_sha "$level_dat_sha" \
  --arg world_tree_sha "$world_tree_sha" \
  --argjson world_file_count "$world_file_count" \
  '{
    schemaVersion: 1,
    evidenceKind: "native",
    workPackage: "M0-W7",
    platform: "linux",
    architecture: "x86_64",
    hardwareProfile: "linux-reference-v1",
    workload: $workload,
    observedAt: $observed_at,
    fixture: {
      id: "atm11-0.2.1",
      descriptorSha256: $descriptor_sha,
      archiveSha256: $archive_sha,
      manifestSha256: $manifest_sha,
      expectedReferenceCount: $expected_references
    },
    instance: {
      id: "atm11-0.2.1",
      componentSha256: $component_sha,
      components: $components,
      modFileCount: $mod_file_count,
      expectedLoader: "neoforge",
      expectedLoaderVersion: "26.1.2.78",
      unexpectedLoaderPresent: false,
      maximumMemoryMiB: $max_memory_mib,
      minimumMemoryMiB: $min_memory_mib,
      windowWidth: $window_width,
      windowHeight: $window_height,
      commandOverridesConfigured: $override_commands,
      preLaunchCommandConfigured: $pre_launch,
      postExitCommandConfigured: $post_exit,
      wrapperKind: "none",
      wrapperCommandSha256: null
    },
    launch: {
      clientReadyEventObserved: true,
      integratedServerStarted: true,
      saveObserved: true,
      serverStopped: true,
      allDimensionsSavedAfterStop: true,
      lifecycleOrderValid: true,
      redactedLifecycleSummarySha256: $log_summary_sha
    },
    freshWorld: {
      levelDatSha256: $level_dat_sha,
      worldTreeSha256: $world_tree_sha,
      worldFileCount: $world_file_count
    }
  }' >"$temporary"

jq --exit-status \
  '[.. | strings | ascii_downcase
    | contains("/home/")
      or contains("/users/")
      or contains("evidence/private/")
      or contains("tests/fixtures/local/")
      or contains("token")
      or contains("authorization")] | any | not' \
  "$temporary" >/dev/null || fail "sanitized summary contains forbidden local or credential data"

chmod 600 "$temporary"
ln -- "$temporary" "$output" || fail "summary output appeared during capture"
rm -f -- "$temporary"
temporary=""
printf 'Wrote sanitized %s launch summary\n' "$workload"
