#!/usr/bin/env bash
# Recomputes every counterpart package's declared digests against its own files.
#
# This reads bytes: it checks each package's tracked files against their digests. It answers
# different questions and neither stands in for the other: a counterpart can
# carry no upstream identifier and still declare a digest for a file it no longer
# has, which is what this check was written after finding — an edited source file
# passed the whole gate because nothing recomputed the manifest that binds it.
#
# The manifest is the public package format, so this holds it to the rule the
# public repository states for one: every path under source.files is hashed and
# compared, and the provenance record is hashed against the digest the manifest
# records for it.
#
# It adds one rule the public loader has no reason to: every file in the package
# has to be accounted for. The loader verifies what a manifest lists because an
# author's unlisted file is the author's business. Here an unlisted file is a
# file nothing checks inside a workspace whose whole claim is that everything is
# checked, so a file that is neither listed nor one of the two records the
# manifest points at is a finding.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
registry_root="$(cd "$checks_dir/.." && pwd)"
counterparts="${1:-$registry_root/mods}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

: >"$work/out"

packages=0
digests=0

for package in "$counterparts"/*/; do
  [ -d "$package" ] || continue
  name="$(basename "$package")"
  manifest="$package/package.json"

  if [ ! -f "$manifest" ]; then
    printf '%s has no package.json, so nothing declares what it is\n' "$name" >>"$work/out"
    continue
  fi
  packages=$((packages + 1))

  if ! jq -e . "$manifest" >/dev/null 2>&1; then
    printf '%s: package.json is not readable as JSON\n' "$name" >>"$work/out"
    continue
  fi

  # Declared source files, each hashed and compared.
  jq -r '.source.files[] | .path + "\t" + .sha256' "$manifest" >"$work/declared"
  : >"$work/listed"
  while IFS="$(printf '\t')" read -r path declared; do
    [ -n "$path" ] || continue
    printf '%s\n' "$path" >>"$work/listed"
    if [ ! -f "$package/$path" ]; then
      printf '%s: the manifest declares `%s`, which is not there\n' "$name" "$path" >>"$work/out"
      continue
    fi
    digests=$((digests + 1))
    actual="$(sha256sum "$package/$path" | cut -d' ' -f1)"
    if [ "$actual" != "$declared" ]; then
      printf '%s: `%s` hashes %s but the manifest records %s\n' \
        "$name" "$path" "$actual" "$declared" >>"$work/out"
    fi
  done <"$work/declared"

  # The provenance record the manifest points at, hashed the same way.
  record="$(jq -r '.provenance.record // empty' "$manifest")"
  declared_record="$(jq -r '.provenance.sha256 // empty' "$manifest")"
  if [ -z "$record" ] || [ -z "$declared_record" ]; then
    printf '%s: the manifest names no provenance record and digest\n' "$name" >>"$work/out"
  elif [ ! -f "$package/$record" ]; then
    printf '%s: the manifest names the provenance record `%s`, which is not there\n' \
      "$name" "$record" >>"$work/out"
  else
    digests=$((digests + 1))
    actual="$(sha256sum "$package/$record" | cut -d' ' -f1)"
    if [ "$actual" != "$declared_record" ]; then
      printf '%s: `%s` hashes %s but the manifest records %s\n' \
        "$name" "$record" "$actual" "$declared_record" >>"$work/out"
    fi
  fi

  # Everything in the package is either listed, the manifest, or the record.
  {
    printf 'package.json\n'
    [ -n "$record" ] && printf '%s\n' "$record"
    cat "$work/listed"
  } | LC_ALL=C sort -u >"$work/accounted"

  find -L "$package" -type f -not -path '*/target/*' -printf '%P\n' |
    LC_ALL=C sort -u >"$work/present"

  LC_ALL=C comm -23 "$work/present" "$work/accounted" | while IFS= read -r unlisted; do
    [ -n "$unlisted" ] || continue
    printf '%s: `%s` is in the package and nothing declares a digest for it\n' \
      "$name" "$unlisted" >>"$work/out"
  done
done

if [ "$packages" -eq 0 ]; then
  echo "no counterpart package was found under $counterparts" >&2
  exit 1
fi

if [ -s "$work/out" ]; then
  printf 'counterpart package check failed:\n' >&2
  sed 's/^/- /' "$work/out" >&2
  exit 1
fi

printf 'counterpart packages: %s packages, %s digests recomputed, every file accounted\n' \
  "$packages" "$digests"
printf 'counterpart packages: what this does not catch: a hand that edits a file and refreshes its digest in the same change, which is the ordinary way to make a legitimate edit and is indistinguishable from any other; anything about what the bytes mean, which is the package'"'"'s own tests; and a package whose directory is absent altogether, since this walks what is there rather than a list of what should be.\n'
