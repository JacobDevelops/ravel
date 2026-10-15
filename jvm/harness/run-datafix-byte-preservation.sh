#!/usr/bin/env bash
# Measures opaque saved-data byte preservation against the restricted game jar.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
screens="$root/target/ravel-jvm/generated-screen"
work="$root/target/ravel-jvm/datafix-byte-preservation"

# Generated sources may be packaged or in the unnamed package.
mapfile -d '' found < <(find "$screens" -name RavelWorldData.java -print0 2>/dev/null | sort -z)
if [[ ${#found[@]} -ne 1 ]]; then
    echo "the generated screen tree holds ${#found[@]} RavelWorldData sources, not one; run:" >&2
    echo "  cargo build -p ravel-minecraft-adapter" >&2
    exit 1
fi
generated_source="${found[0]}"

# Bind the measurement to the generated source, not the environment's version.
version="$(sed -n '1s/^.*Generated from the pinned Minecraft \(.*\) inputs\..*$/\1/p' "$generated_source")"
if [[ -z "$version" ]]; then
    echo "$generated_source carries no generated version header; run:" >&2
    echo "  cargo build -p ravel-minecraft-adapter" >&2
    exit 1
fi

# Template overrides may use a different storage contract and need a separate measurement.
if [[ -f "$root/jvm/screen/template/$version/RavelWorldData.java.in" ]]; then
    echo "Minecraft $version overrides the shared saved-data template, so it stores through" >&2
    echo "a contract this codec-era measurement does not describe." >&2
    exit 1
fi

fixture="$root/tests/fixtures/local/tooling/minecraft-$version"
client="$fixture/client.jar"
manifest="$fixture/version.json"

if [[ ! -f "$client" || ! -f "$manifest" ]]; then
    echo "the pinned $version client fixture is absent; see tests/fixtures/local/README.md" >&2
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

# Select the fixer's jar from this version's manifest, not a hard-coded coordinate.
fixer_path="$(python3 - "$manifest" <<'FIXER'
import json
import pathlib
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text())
for library in manifest["libraries"]:
    if library.get("name", "").startswith("com.mojang:datafixerupper:"):
        artifact = library.get("downloads", {}).get("artifact")
        if artifact is not None:
            print(artifact["path"])
            break
FIXER
)"
if [[ -z "$fixer_path" ]]; then
    echo "$manifest declares no com.mojang:datafixerupper artifact" >&2
    exit 1
fi

library_root=""
for candidate in "${library_roots[@]}"; do
    if [[ -f "$candidate/$fixer_path" ]]; then
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

# Compile the production source; real-jar generics differ from the erased build stubs.
javac -Xlint:all,-rawtypes,-unchecked,-cast -Werror -cp "$classpath" -d "$work" \
    "$generated_source" \
    "$root/jvm/harness/DataFixBytePreservationMain.java"
java -cp "$work:$classpath" DataFixBytePreservationMain
