#!/usr/bin/env bash
# Proves that the pinned game's selected vanilla saved-data fixer leaves Ravel's
# opaque byte payload unchanged before the persistence adapter relies on it.
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
javac -Xlint:all -Werror -cp "$classpath" -d "$work" \
    "$root/jvm/harness/DataFixBytePreservationMain.java"
java -cp "$work:$classpath" DataFixBytePreservationMain
