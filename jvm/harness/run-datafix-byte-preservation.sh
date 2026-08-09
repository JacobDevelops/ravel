#!/usr/bin/env bash
# Measures whether the pinned game's selected vanilla saved-data fixer leaves
# Ravel's opaque byte payload unchanged, over the shape `RavelWorldData` stores.
#
# This is a measurement and not a gate. It needs the restricted pinned client jar
# and a real Minecraft library root, neither of which `ravel:verify` can assume,
# so nothing in the gate graph runs it and no devenv task wraps it. ADG-7 records
# what it measured, at which revision, and what would have to change for this to
# become a gate.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$root/tests/fixtures/local/tooling/minecraft-26.1.2"
client="$fixture/client.jar"
manifest="$fixture/version.json"
work="$root/target/ravel-jvm/datafix-byte-preservation"

if [[ ! -f "$client" || ! -f "$manifest" ]]; then
    echo "the pinned 26.1.2 client fixture is absent; see tests/fixtures/local/README.md" >&2
    exit 1
fi

library_roots=()
if [[ -n "${RAVEL_MINECRAFT_LIBRARY_ROOT:-}" ]]; then
    library_roots+=("$RAVEL_MINECRAFT_LIBRARY_ROOT")
fi
library_roots+=(
    "$HOME/.local/share/ravel-m0-prism-11.0.3/libraries"
    "$HOME/.local/share/PrismLauncher/libraries"
    "$HOME/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/libraries"
)

library_root=""
for candidate in "${library_roots[@]}"; do
    if [[ -f "$candidate/com/mojang/datafixerupper/9.0.19/datafixerupper-9.0.19.jar" ]]; then
        library_root="$candidate"
        break
    fi
done
if [[ -z "$library_root" ]]; then
    echo "the pinned Minecraft libraries are absent; set RAVEL_MINECRAFT_LIBRARY_ROOT" >&2
    exit 1
fi

mapfile -t libraries < <(python3 - "$manifest" "$library_root" <<'PY'
import json
import pathlib
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text())
root = pathlib.Path(sys.argv[2])
for library in manifest["libraries"]:
    artifact = library.get("downloads", {}).get("artifact")
    if artifact is None:
        continue
    path = root / artifact["path"]
    if path.is_file():
        print(path)
PY
)
if [[ ${#libraries[@]} -eq 0 ]]; then
    echo "the selected Minecraft library root contains no pinned artifacts" >&2
    exit 1
fi

classpath="$client"
for library in "${libraries[@]}"; do
    classpath+=":$library"
done

rm -rf "$work"
mkdir -p "$work"

# The production saved-data class is compiled here rather than copied, because
# the shape being measured has to be production's own and not a restatement of
# it. It is the only production source on this command line: everything around it
# in `jvm/screen/src` is screens.
#
# Three lints are off and the rest stay fatal. The screen jar compiles this class
# against the generated stubs, which carry erased descriptors, so `BASE64_STRING`
# and `SavedDataType` are raw there and the source is written for that. Against
# the real jar the same source meets the generics the stubs dropped, and every
# suppressed warning below is that difference — not something the screen build's
# own `-Xlint:all -Werror` would let through.
javac -Xlint:all,-rawtypes,-unchecked,-cast -Werror -cp "$classpath" -d "$work" \
    "$root/jvm/screen/src/dev/ravel/jvm/screen/RavelWorldData.java" \
    "$root/jvm/harness/DataFixBytePreservationMain.java"
java -cp "$work:$classpath" DataFixBytePreservationMain
