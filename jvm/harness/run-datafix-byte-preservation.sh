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
screens="$root/target/ravel-jvm/generated-screen"
work="$root/target/ravel-jvm/datafix-byte-preservation"

# The source is searched for rather than spelled, because where it lands is a
# per-version fact: a version with packages puts it under `dev/ravel/jvm/screen`
# and one whose game is in the unnamed package puts it at the tree root. A
# spelled path would report the source missing for the second kind, which is the
# one answer that is never true.
mapfile -d '' found < <(find "$screens" -name RavelWorldData.java -print0 2>/dev/null | sort -z)
if [[ ${#found[@]} -ne 1 ]]; then
    echo "the generated screen tree holds ${#found[@]} RavelWorldData sources, not one; run:" >&2
    echo "  cargo build -p ravel-minecraft-adapter" >&2
    exit 1
fi
generated_source="${found[0]}"

# Which version this measurement is about is read off the source it is about to
# compile, not off the environment. Both build scripts beside this one read the
# generator's own output for the same reason: the generated tree is shared, one
# version at a time, and a run that took the version from `RAVEL_MINECRAFT_VERSION`
# would link the jar the caller named against whatever the last build left behind.
# The provenance rides on the first line either way: on the package declaration
# under a version with packages, and alone under one whose game is in the unnamed
# package, so the match is anchored to the line and not to the column.
version="$(sed -n '1s/^.*Generated from the pinned Minecraft \(.*\) inputs\..*$/\1/p' "$generated_source")"
if [[ -z "$version" ]]; then
    echo "$generated_source carries no generated version header; run:" >&2
    echo "  cargo build -p ravel-minecraft-adapter" >&2
    exit 1
fi

# What this measurement encodes through is a codec carried by a `SavedDataType`,
# and that is an era rather than a spelling: an earlier era registers saved data
# through a factory over `CompoundTag` and declares no such type under any name.
# A version that overrides the shared screen template has its own storage contract,
# so the fixer question for it is a different measurement with its own answer, and
# ADG-7 records it as unowned rather than answered here. Refusing by the override
# is what makes that a stated result instead of a compiler error about a name.
if [[ -f "$root/jvm/screen/template/$version/RavelWorldData.java.in" ]]; then
    echo "Minecraft $version overrides the shared saved-data template, so it stores through" >&2
    echo "a contract this codec-era measurement does not describe; see ADG-7." >&2
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

# The library root is chosen by probing for the fixer's own jar, and which jar
# that is comes from this version's manifest rather than from a coordinate
# written here. A hard-coded one would name a single version's fixer while
# everything around it followed the generated tree, so a root holding another
# game's libraries would be selected as if it held this one's.
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

# The production saved-data class is compiled here rather than copied, because
# the shape being measured has to be production's own and not a restatement of
# it. It is the generated source rather than the template, because the template
# names the game through placeholders and the generator is what renders them into
# the names this version links. It is the only production source on this command
# line: everything around it in the generated tree is screens.
#
# Three lints are off and the rest stay fatal. The screen jar compiles this class
# against the generated stubs, which carry erased descriptors, so `BASE64_STRING`
# and `SavedDataType` are raw there and the source is written for that. Against
# the real jar the same source meets the generics the stubs dropped, and every
# suppressed warning below is that difference — not something the screen build's
# own `-Xlint:all -Werror` would let through.
javac -Xlint:all,-rawtypes,-unchecked,-cast -Werror -cp "$classpath" -d "$work" \
    "$generated_source" \
    "$root/jvm/harness/DataFixBytePreservationMain.java"
java -cp "$work:$classpath" DataFixBytePreservationMain
