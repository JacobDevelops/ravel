#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 FILE_ID FILENAME EXPECTED_SHA1 OUTPUT_DIRECTORY" >&2
  exit 2
}

[[ $# -eq 4 ]] || usage

file_id="$1"
filename="$2"
expected_sha1="${3,,}"
output_directory="$4"

[[ "$file_id" =~ ^[0-9]+$ ]] || {
  echo "FILE_ID must contain only decimal digits" >&2
  exit 2
}
[[ "$filename" =~ ^[A-Za-z0-9._+%-]+$ ]] || {
  echo "FILENAME contains unsupported characters" >&2
  exit 2
}
[[ "$expected_sha1" =~ ^[0-9a-f]{40}$ ]] || {
  echo "EXPECTED_SHA1 must be 40 hexadecimal characters" >&2
  exit 2
}
[[ -d "$output_directory" ]] || {
  echo "OUTPUT_DIRECTORY must already exist" >&2
  exit 2
}

output_directory="$(realpath -- "$output_directory")"
target="$output_directory/$filename"
file_group="$((10#$file_id / 1000))"
file_suffix="$((10#$file_id % 1000))"
printf -v file_suffix '%03d' "$file_suffix"
url="https://mediafilez.forgecdn.net/files/$file_group/$file_suffix/$filename"

if [[ -e "$target" ]]; then
  [[ -f "$target" && ! -L "$target" ]] || {
    echo "refusing non-regular existing target: $target" >&2
    exit 1
  }
  actual_sha1="$(sha1sum -- "$target" | cut -d ' ' -f 1)"
  [[ "$actual_sha1" == "$expected_sha1" ]] || {
    echo "existing target has unexpected SHA-1: $target" >&2
    exit 1
  }
  chmod 600 -- "$target"
  printf 'verified existing file: %s\n' "$target"
  exit 0
fi

temporary_directory="$(mktemp -d --tmpdir="$output_directory" .curseforge-recovery.XXXXXXXX)"
trap 'rm -rf -- "$temporary_directory"' EXIT
temporary_file="$temporary_directory/$filename"

curl --fail --location --silent --show-error \
  --proto '=https' --tlsv1.2 \
  --output "$temporary_file" \
  "$url"

actual_sha1="$(sha1sum -- "$temporary_file" | cut -d ' ' -f 1)"
[[ "$actual_sha1" == "$expected_sha1" ]] || {
  echo "downloaded file has unexpected SHA-1" >&2
  exit 1
}

install -m 600 -- "$temporary_file" "$target"
printf 'recovered and verified: %s\n' "$target"
