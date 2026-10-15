#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
subject="$script_dir/summarize-prism-launch.sh"
test_root="$(mktemp -d)"
trap 'rm -rf -- "$test_root"' EXIT

repository="$test_root/repository"
instance="$test_root/instance"
private_output="$repository/evidence/private/m0"
mkdir -p \
  "$private_output" \
  "$repository/tests/fixtures/descriptors" \
  "$repository/tests/fixtures/local/atm11-0.2.1/extracted" \
  "$repository/tests/fixtures/local/atm11-0.2.1/source" \
  "$instance/minecraft/logs" \
  "$instance/minecraft/mods" \
  "$instance/minecraft/saves/private-world"
chmod 700 "$repository/evidence/private" "$private_output"

printf 'fixture archive\n' \
  >"$repository/tests/fixtures/local/atm11-0.2.1/source/atm11-0.2.1.zip"
printf '{"name":"All the Mods 11","version":"0.2.1","minecraft":{"version":"26.1.2","modLoaders":[{"id":"neoforge-26.1.2.78","primary":true}]},"files":[{"projectID":1,"fileID":2,"required":true}]}\n' \
  >"$repository/tests/fixtures/local/atm11-0.2.1/extracted/manifest.json"
archive_sha="$(sha256sum "$repository/tests/fixtures/local/atm11-0.2.1/source/atm11-0.2.1.zip" | cut -d' ' -f1)"
manifest_sha="$(sha256sum "$repository/tests/fixtures/local/atm11-0.2.1/extracted/manifest.json" | cut -d' ' -f1)"
jq -n \
  --arg archive_sha "$archive_sha" \
  --arg manifest_sha "$manifest_sha" \
  '{
    schemaVersion: 1,
    id: "atm11-0.2.1",
    source: {sha256: $archive_sha},
    manifest: {
      sha256: $manifest_sha,
      name: "All the Mods 11",
      version: "0.2.1",
      minecraft: "26.1.2",
      modLoader: "neoforge-26.1.2.78",
      expectedReferenceCount: 1
    }
  }' >"$repository/tests/fixtures/descriptors/atm11-0.2.1.json"

printf '%s\n' \
  '{"components":[' \
  '{"uid":"org.lwjgl3","version":"3.4.1"},' \
  '{"uid":"net.minecraft","version":"26.1.2"},' \
  '{"uid":"net.neoforged","version":"26.1.2.78"}' \
  ']}' >"$instance/mmc-pack.json"
printf '%s\n' \
  'MaxMemAlloc=4096' \
  'MinMemAlloc=512' \
  'MinecraftWinHeight=480' \
  'MinecraftWinWidth=854' \
  'OverrideCommands=true' \
  'PreLaunchCommand=' \
  'PostExitCommand=' \
  'WrapperCommand=' \
  >"$instance/instance.cfg"
printf '%s\n' \
  'Sound engine started' \
  'Starting integrated minecraft server' \
  'ThreadedAnvilChunkStorage: All dimensions are saved' \
  'Saving and pausing game' \
  'Stopping server' \
  'ThreadedAnvilChunkStorage: All dimensions are saved' >"$instance/minecraft/logs/latest.log"
printf 'mod\n' >"$instance/minecraft/mods/example.jar"
printf 'world\n' >"$instance/minecraft/saves/private-world/level.dat"

summary="$private_output/official-atm11-launch.json"
RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  official-atm11 "$instance" "$summary"

jq --exit-status '
  .schemaVersion == 1
  and .evidenceKind == "native"
  and .workPackage == "M0-W7"
  and .workload == "official-atm11"
  and .instance.id == "atm11-0.2.1"
  and .instance.modFileCount == 1
  and .instance.expectedLoader == "neoforge"
  and .instance.unexpectedLoaderPresent == false
  and .instance.wrapperKind == "none"
  and .instance.wrapperCommandSha256 == null
  and .launch.clientReadyEventObserved == true
  and .launch.integratedServerStarted == true
  and .launch.saveObserved == true
  and .launch.serverStopped == true
  and .launch.allDimensionsSavedAfterStop == true
  and .launch.lifecycleOrderValid == true
  and .freshWorld.worldFileCount == 1
  and ([.. | strings | contains("/private/")] | any | not)
' "$summary" >/dev/null
test "$(stat -c '%a' "$summary")" = 600

incomplete="$private_output/incomplete.json"
printf '%s\n' \
  'Starting integrated minecraft server' \
  'Sound engine started' \
  'Saving and pausing game' \
  'Stopping server' \
  'ThreadedAnvilChunkStorage: All dimensions are saved' \
  >"$instance/minecraft/logs/latest.log"
if RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  official-atm11 "$instance" "$incomplete" 2>/dev/null; then
  printf 'expected unordered launch log to be rejected\n' >&2
  exit 1
fi
test ! -e "$incomplete"

printf '%s\n' \
  'Sound engine started' \
  'Starting integrated minecraft server' \
  'Saving and pausing game' \
  'Stopping server' \
  'ThreadedAnvilChunkStorage: All dimensions are saved' >"$instance/minecraft/logs/latest.log"
sed -i 's#WrapperCommand=.*#WrapperCommand=/private/untrusted-wrapper --#' \
  "$instance/instance.cfg"
if RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  official-atm11 "$instance" "$private_output/untrusted-wrapper.json" 2>/dev/null; then
  printf 'expected untrusted wrapper to be rejected\n' >&2
  exit 1
fi
test ! -e "$private_output/untrusted-wrapper.json"
sed -i \
  's#WrapperCommand=.*#WrapperCommand=/private/ravel-cli performance supervise-initial-linux /private/atm11-markers.sock --#' \
  "$instance/instance.cfg"
sed -i 's/OverrideCommands=true/OverrideCommands=false/' "$instance/instance.cfg"
if RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  official-atm11 "$instance" "$private_output/inert-wrapper.json" 2>/dev/null; then
  printf 'expected retired supervisor wrapper to be rejected\n' >&2
  exit 1
fi
test ! -e "$private_output/inert-wrapper.json"
sed -i 's/OverrideCommands=false/OverrideCommands=true/; s#WrapperCommand=.*#WrapperCommand=#' \
  "$instance/instance.cfg"

chmod 755 "$private_output"
unsafe="$private_output/unsafe-directory.json"
if RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  official-atm11 "$instance" "$unsafe" 2>/dev/null; then
  printf 'expected a non-owner-only output directory to be rejected\n' >&2
  exit 1
fi
test ! -e "$unsafe"
chmod 700 "$private_output"

if RAVEL_EVIDENCE_ROOT="$repository" "$subject" \
  official-atm11 "$instance" "$repository/outside.json" 2>/dev/null; then
  printf 'expected output outside the private evidence boundary to be rejected\n' >&2
  exit 1
fi
test ! -e "$repository/outside.json"

printf 'summarize-prism-launch tests passed\n'
