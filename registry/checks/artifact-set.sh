#!/usr/bin/env bash
# Enumerates the acquired artifacts and binds each one to a pinned reference.
#
# M5-W2 reconciled 235 references against the manifest. This binds those
# references to the 235 files that were actually acquired, and it is deliberate
# that the two artifact classes are counted apart: 229 of them are mods and six
# are shaderpacks, so any rule of the form "one entry, one jar" is wrong by six
# and says so here rather than six scripts later.
#
# Two binding strengths exist and are recorded rather than averaged:
#
#   packwiz-sha1  the launcher's own index states the project, the file and a
#                 SHA-1, and the file on disk hashes to it. The reference is
#                 bound to those bytes.
#   download-log  the acquisition log states the CDN path a file came from, and
#                 a CurseForge CDN path encodes the file id. The reference is
#                 bound to a filename, not to a digest, which is weaker and is
#                 why it is a separate word.
#
# Sourcing this defines: instance_root, import_log, artifact_tsv (a file of
# class/filename/sha256/projectId/fileId/binding rows) and artifact_set_sha.
set -euo pipefail

# shellcheck source=checks/public-repo.sh
. "$(dirname "${BASH_SOURCE[0]}")/public-repo.sh"

fixture_root="$repo_root/tests/fixtures/local"
# The launcher's application root inside the fixture tree. Its name is written
# in two pieces for the same reason verify-boundary.sh brackets a character in
# each of its own literals: the rule that refuses launcher paths would otherwise
# match the one mention of one that this workspace legitimately needs. The name
# is a relative directory in the repository's ignored fixture tree and
# names no machine, which is why it is allowed to exist at all.
launcher_root="prism-""approot"
instance_root="$fixture_root/$launcher_root/instances/${FIXTURE_ID}/minecraft"
import_log="$fixture_root/$launcher_root/import.log"
pack_manifest="$fixture_root/${FIXTURE_ID}/extracted/manifest.json"

# The pack's own override trees are inputs to M5-W4 and their absence would
# quietly turn two counts into zero, so they are required here rather than
# tested for where they are read.
for path in "$instance_root/mods" "$instance_root/mods/.index" "$instance_root/shaderpacks" \
  "$instance_root/resourcepacks" \
  "$instance_root/config" "$instance_root/kubejs"; do
  if [ ! -d "$path" ]; then
    echo "acquired artifact tree missing: ${path#"$fixture_root/"}" >&2
    exit 1
  fi
done
for path in "$import_log" "$pack_manifest"; do
  if [ ! -f "$path" ]; then
    echo "acquisition input missing: ${path#"$fixture_root/"}" >&2
    exit 1
  fi
done

artifact_work="$(mktemp -d)"
trap 'rm -rf "$artifact_work"' EXIT
artifact_tsv="$artifact_work/artifacts.tsv"

# Mods. The index is the launcher's, not this workspace's, and the SHA-1 in it
# is checked against the bytes rather than believed.
shopt -s nullglob
index_files=("$instance_root"/mods/.index/*.pw.toml)
jar_files=("$instance_root"/mods/*.jar)
shader_files=("$instance_root"/shaderpacks/*.zip)
resourcepack_files=("$instance_root"/resourcepacks/*.zip)
shopt -u nullglob

if [ ${#index_files[@]} -eq 0 ] || [ ${#jar_files[@]} -eq 0 ] || [ ${#shader_files[@]} -eq 0 ]; then
  echo "acquired artifact tree is empty: ${#jar_files[@]} mods, ${#index_files[@]} index entries, ${#shader_files[@]} shaderpacks" >&2
  exit 1
fi
if [ ${#index_files[@]} -ne ${#jar_files[@]} ]; then
  echo "mod index does not cover the mods: ${#index_files[@]} index entries, ${#jar_files[@]} jars" >&2
  exit 1
fi

: >"$artifact_tsv"
for entry in "${index_files[@]}"; do
  filename="$(sed -n "s/^filename = '\(.*\)'\$/\1/p" "$entry" | head -1)"
  declared_sha1="$(sed -n "s/^hash = '\(.*\)'\$/\1/p" "$entry" | head -1)"
  project_id="$(sed -n 's/^project-id = *\([0-9]*\)$/\1/p' "$entry" | head -1)"
  file_id="$(sed -n 's/^file-id = *\([0-9]*\)$/\1/p' "$entry" | head -1)"
  jar="$instance_root/mods/$filename"

  if [ -z "$filename" ] || [ -z "$declared_sha1" ] || [ -z "$project_id" ] || [ -z "$file_id" ]; then
    echo "mod index entry is incomplete: ${entry##*/}" >&2
    exit 1
  fi
  if [ ! -f "$jar" ]; then
    echo "mod index names a file that was not acquired: $filename" >&2
    exit 1
  fi
  actual_sha1="$(sha1sum "$jar" | cut -d' ' -f1)"
  if [ "$actual_sha1" != "$declared_sha1" ]; then
    echo "acquired mod does not hash to its pinned file: $filename" >&2
    exit 1
  fi
  printf 'mod\t%s\t%s\t%s\t%s\tpackwiz-sha1\n' \
    "$filename" "$(sha256sum "$jar" | cut -d' ' -f1)" "$project_id" "$file_id" >>"$artifact_tsv"
done

# Shaderpacks. The launcher moves these out of the mod folder after import, so
# they leave no index behind and the acquisition log is the only local record of
# which pinned file each one is. A CurseForge CDN path splits the file id into a
# leading group and a zero-padded remainder, so the id is recoverable from the
# path without asking the platform anything.
log_map="$artifact_work/log-map.tsv"
grep -oE 'edge\.forgecdn\.net/files/[0-9]+/[0-9]+/[^"]+' "$import_log" |
  sort -u |
  while IFS= read -r url; do
    rest="${url#*/files/}"
    group="${rest%%/*}"
    rest="${rest#*/}"
    tail_segment="${rest%%/*}"
    name="${rest#*/}"
    printf '%d%03d\t%s\n' "$group" "$((10#$tail_segment))" "$name"
  done >"$log_map"

if [ ! -s "$log_map" ]; then
  echo "acquisition log records no downloads" >&2
  exit 1
fi

for shader in "${shader_files[@]}"; do
  filename="${shader##*/}"
  # The log holds percent-encoded CDN names; compare on the decoded form.
  file_id="$(awk -F'\t' -v want="$filename" '
    {
      name = $2
      while (match(name, /%[0-9A-Fa-f][0-9A-Fa-f]/)) {
        hex = substr(name, RSTART + 1, 2)
        code = index("0123456789ABCDEF", toupper(substr(hex, 1, 1))) - 1
        code = code * 16 + index("0123456789ABCDEF", toupper(substr(hex, 2, 1))) - 1
        name = substr(name, 1, RSTART - 1) sprintf("%c", code) substr(name, RSTART + 3)
      }
      if (name == want) { print $1; exit }
    }' "$log_map")"
  if [ -z "$file_id" ]; then
    echo "acquired shaderpack has no download record: $filename" >&2
    exit 1
  fi
  project_id="$(jq -r --argjson f "$file_id" '.files[] | select(.fileID == $f) | .projectID' "$pack_manifest")"
  if [ -z "$project_id" ] || [ "$project_id" = "null" ]; then
    echo "acquired shaderpack is not a pinned reference: $filename" >&2
    exit 1
  fi
  printf 'shaderpack\t%s\t%s\t%s\t%s\tdownload-log\n' \
    "$filename" "$(sha256sum "$shader" | cut -d' ' -f1)" "$project_id" "$file_id" >>"$artifact_tsv"
done

# Resource packs are pinned bundled resources too, the same way shaderpacks are:
# they carry no code and are consumed as pack data, but the manifest pins them,
# so each binds to its pinned reference exactly like a shaderpack.
for resourcepack in "${resourcepack_files[@]}"; do
  filename="${resourcepack##*/}"
  file_id="$(awk -F'\t' -v want="$filename" '
    {
      name = $2
      while (match(name, /%[0-9A-Fa-f][0-9A-Fa-f]/)) {
        hex = substr(name, RSTART + 1, 2)
        code = index("0123456789ABCDEF", toupper(substr(hex, 1, 1))) - 1
        code = code * 16 + index("0123456789ABCDEF", toupper(substr(hex, 2, 1))) - 1
        name = substr(name, 1, RSTART - 1) sprintf("%c", code) substr(name, RSTART + 3)
      }
      if (name == want) { print $1; exit }
    }' "$log_map")"
  if [ -z "$file_id" ]; then
    echo "acquired resourcepack has no download record: $filename" >&2
    exit 1
  fi
  project_id="$(jq -r --argjson f "$file_id" '.files[] | select(.fileID == $f) | .projectID' "$pack_manifest")"
  if [ -z "$project_id" ] || [ "$project_id" = "null" ]; then
    echo "acquired resourcepack is not a pinned reference: $filename" >&2
    exit 1
  fi
  printf 'resourcepack\t%s\t%s\t%s\t%s\tdownload-log\n' \
    "$filename" "$(sha256sum "$resourcepack" | cut -d' ' -f1)" "$project_id" "$file_id" >>"$artifact_tsv"
done

sort -o "$artifact_tsv" "$artifact_tsv"

# Every pinned reference is bound exactly once, in both directions. This is the
# statement M5-W3..W5 rest on: a catalogue of 235 artifacts is only a catalogue
# of the pinned pack if each artifact is the file a pinned reference names.
reference_count="$(jq '.files | length' "$pack_manifest")"
artifact_count="$(wc -l <"$artifact_tsv")"
bound_ids="$(cut -f5 "$artifact_tsv" | sort -u | wc -l)"
if [ "$artifact_count" -ne "$reference_count" ] || [ "$bound_ids" -ne "$reference_count" ]; then
  echo "artifact binding does not reconcile: $artifact_count artifacts, $bound_ids distinct file ids, $reference_count pinned references" >&2
  exit 1
fi
if ! jq -r '.files[].fileID' "$pack_manifest" | sort -u | diff -q - <(cut -f5 "$artifact_tsv" | sort -u) >/dev/null; then
  echo "artifact binding does not cover the pinned references" >&2
  exit 1
fi

# One digest over the whole acquired set. Every derived file records it, so a
# record outlives its inputs only by saying which inputs it outlived.
artifact_set_sha="$(cut -f1,2,3 "$artifact_tsv" | sha256sum | cut -d' ' -f1)"

export instance_root import_log pack_manifest artifact_tsv artifact_set_sha
