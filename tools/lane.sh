#!/usr/bin/env bash
set -euo pipefail

script=$(realpath "${BASH_SOURCE[0]}")
root=$(dirname "$(dirname "$script")")

# jj invokes this mode only for package manifests; refuse non-digest differences.
if [[ ${1-} == merge-digests ]]; then
    exec python3 - "$2" "$3" "$4" "$5" <<'PY'
import copy
import json
import pathlib
import sys

base, left, right, output = map(pathlib.Path, sys.argv[1:])
try:
    documents = [json.loads(path.read_text()) for path in (base, left, right)]
    stripped = copy.deepcopy(documents)
    for document in stripped:
        for entry in document.get("source", {}).get("files", []):
            entry.pop("sha256", None)
        document.get("provenance", {}).pop("sha256", None)
    normalized = [json.dumps(document, sort_keys=True) for document in stripped]
    if not normalized[0] == normalized[1] == normalized[2]:
        sys.exit(1)
    output.write_bytes(left.read_bytes())
except (OSError, ValueError, AttributeError, TypeError):
    sys.exit(1)
PY
fi

usage() {
    printf 'Usage: tools/lane.sh up <name> <base> [--prewarm] | land <name> [onto] | down <name> [--force]\n' >&2
    exit 2
}

[[ $# -ge 2 ]] || usage
command=$1
name=$2
[[ $name =~ ^[a-zA-Z0-9][a-zA-Z0-9_-]*$ && $name != default ]] || usage
lane="$(dirname "$root")/ravel-$name"
primary="$(dirname "$root")/ravel"
[[ $lane != "$root" && $lane != "$primary" ]] || {
    printf 'Run lane.sh from a different workspace than the target lane.\n' >&2
    exit 1
}

lane_jj() {
    (cd "$lane" && jj "$@")
}

require_lane() {
    [[ -d $lane/.jj ]] && [[ $(lane_jj --ignore-working-copy workspace root) == "$lane" ]] &&
        [[ $(lane_jj --ignore-working-copy git root) == "$(jj -R "$root" git root)" ]] &&
        [[ $(lane_jj --ignore-working-copy workspace root --name "$name") == "$lane" ]] || {
        printf 'Not the expected jj workspace: %s\n' "$lane" >&2
        exit 1
    }
    lane_jj workspace update-stale
}

empty_child() {
    [[ $(lane_jj log -r @ --no-graph -T 'empty && description == ""') == true ]]
}

rebase_stack() {
    local onto=$1 tip=$2
    local roots="roots($onto..$tip)" revisions
    revisions=$(lane_jj log -r "$roots" --no-graph -T 'change_id')
    [[ -n $revisions ]] || return 0
    lane_jj rebase -s "$roots" -o "$onto"
}

up() {
    [[ $# -ge 1 && $# -le 2 ]] || usage
    local base=$1
    [[ $# == 1 || $2 == --prewarm ]] || usage
    if [[ -e $lane ]]; then
        require_lane
        rebase_stack "$base" @
    else
        jj -R "$root" workspace add --name "$name" -r "$base" "$lane"
    fi
    mkdir -p "$lane/tests/fixtures/local"
    while IFS= read -r -d '' directory; do
        [[ -e $lane/tests/fixtures/local/$(basename "$directory") ]] ||
            ln -s "$(realpath "$directory")" "$lane/tests/fixtures/local/"
    done < <(find -L "$primary/tests/fixtures/local" -mindepth 1 -maxdepth 1 -type d -print0)
    if [[ $# == 2 ]]; then
        (cd "$lane" && numen run -- mise exec -- cargo build --workspace --all-targets --locked)
    fi
    printf 'Lane ready: %s\n' "$lane"
}

resolve_digests() {
    local revision=$1
    local path
    resolved_packages=()
    [[ -n $(lane_jj log -r "$revision & conflicts()" --no-graph -T 'change_id') ]] || return 0
    while IFS= read -r path; do
        [[ $path == */package.json ]] || continue
        if lane_jj --config "merge-tools.ravel-digests.program=\"$script\"" \
            --config "merge-tools.ravel-digests.merge-args=[\"merge-digests\", \"\$base\", \"\$left\", \"\$right\", \"\$output\"]" \
            resolve -r "$revision" --tool ravel-digests "$path"; then
            resolved_packages+=("${path%/package.json}")
        else
            printf 'Keeping non-digest conflict: %s\n' "$path" >&2
        fi
    done < <(lane_jj resolve -r "$revision" --list | cut -d ' ' -f 1)
}

land() (
    [[ $# -le 1 ]] || usage
    require_lane
    local onto=${1:-main}
    local tip revision undescribed
    tip=$(lane_jj log -r @ --no-graph -T 'change_id')
    if empty_child; then
        tip=$(lane_jj log -r @- --no-graph -T 'change_id')
    fi
    undescribed=$(lane_jj log -r "($onto..$tip) & ~empty() & description(exact:\"\")" --no-graph -T 'change_id')
    if [[ -n $undescribed ]]; then
        printf 'Describe or finish every non-empty change in the lane stack before landing it.\n' >&2
        return 1
    fi
    if empty_child; then
        lane_jj abandon @
    fi
    rebase_stack "$onto" "$tip"
    local temporary='' directory
    trap 'status=$?; if ((status)); then
        if [[ -n $temporary ]]; then lane_jj abandon "$temporary"; fi
        lane_jj new "$tip"
    fi' EXIT
    local -a resolved_packages
    while IFS= read -r revision; do
        resolve_digests "$revision"
        if [[ -n $(lane_jj log -r "$revision & conflicts()" --no-graph -T 'change_id') ]]; then
            continue
        fi
        temporary=''
        lane_jj new "$revision"
        temporary=$(lane_jj log -r @ --no-graph -T 'change_id')
        (
            cd "$lane"
            mise exec -- cargo run -q -p ravel-xtask -- package digests --all
            for directory in "${resolved_packages[@]}"; do
                mise exec -- cargo run -q -p ravel-xtask -- package digests "$directory"
            done
        )
        lane_jj squash --into "$revision" --keep-emptied 'root-glob:**/package.json'
    done < <(lane_jj log -r "$onto..$tip" --reversed --no-graph -T 'change_id ++ "\n"')
    lane_jj new "$tip"
    trap - EXIT
    local conflicts
    conflicts=$(lane_jj log -r "($onto..$tip) & conflicts()" --no-graph)
    if [[ -n $conflicts ]]; then
        printf 'Remaining conflicts:\n%s\n' "$conflicts" >&2
        return 1
    fi
    printf 'Lane landed on %s: %s\n' "$onto" "$tip"
)

case "$command" in
up) up "${@:3}" ;;
land) land "${@:3}" ;;
down)
    [[ $# == 2 || ($# == 3 && $3 == --force) ]] || usage
    require_lane
    lane_jj status
    if [[ $# == 2 ]]; then
        empty=$(lane_jj log -r @ --no-graph -T 'empty')
        unlanded=$(lane_jj log -r '@- ~ ::main' --no-graph -T 'change_id')
        if [[ $empty != true || -n $unlanded ]]; then
            printf 'Refusing to remove an unfinished or unlanded lane; use --force to discard it.\n' >&2
            exit 1
        fi
    fi
    jj -R "$root" workspace forget "$name"
    rm -rf -- "$lane"
    ;;
*) usage ;;
esac
