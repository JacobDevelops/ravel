#!/usr/bin/env bash
# Holds every source-informed specification to the names of the tree it was
# written from.
#
# checks/verify-spec-identifiers.sh reads the M5 inventory, so the only upstream
# names it can see are mod ids, node ids, display names and artifact file stems.
# It says so itself. Every reader of the foundation wave found what that leaves
# uncovered: a class, a method, a field, a recipe type, a serialised key or a
# package segment lifted verbatim out of the upstream tree and used as a name in
# the prose. Those words are in the source and nowhere in the inventory, so no
# amount of inventory reading finds them.
#
# This check reads the source instead. For each specification whose provenance
# record names a source-code source, it extracts the names that tree declares
# and applies the same two rules the sibling check applies, on the same
# normalization:
#
#   U1  the specification carries an upstream name verbatim, word-bounded and
#       case-insensitive.
#   N1  the specification carries a name that is not verbatim but becomes one
#       under the documented normalization: fold to lower case and drop every
#       character that is not a letter or a digit.
#
# Two sides that cannot be the same thing. The upstream name set is extracted
# from the fetched tree mechanically and no rule here decides what belongs in
# it. checks/source-name-collisions.tsv is written by hand and excuses ordinary
# English words that genuinely collide; nothing derives it from either side.
#
# The fetched tree is not tracked in either repository -- it lives in the public
# checkout's ignored fixture tree, which is the whole point of a clean-room read
# -- so on a machine that never fetched it this check prints a loud skip line per
# entry rather than passing quietly or failing. A skip is not a pass and says so.
#
# What it cannot catch is printed on every run. See the line at the end.
set -euo pipefail

checks_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=checks/public-repo.sh
. "$checks_dir/public-repo.sh"
mode="${1:---fixture}"
case "$mode" in --offline|--fixture) ;; *) echo "usage: $0 [--offline|--fixture]" >&2; exit 2 ;; esac

spec_root="$registry_root/specifications/${RAVEL_WAVE:-m6-foundation}"
provenance_root="$registry_root/provenance/${RAVEL_WAVE:-m6-foundation}"
source_root="${RAVEL_CLEAN_ROOM_SRC:-$repo_root/tests/fixtures/local/m6-clean-room/src}"
manifest="${RAVEL_CLEAN_ROOM_MANIFEST:-$repo_root/tests/fixtures/local/m6-clean-room/fetch-manifest.txt}"
collisions="$checks_dir/source-name-collisions.tsv"

for path in "$spec_root" "$provenance_root"; do
  if [ ! -d "$path" ]; then
    printf 'specification source-name check failed:\n- missing input: %s\n' \
      "${path#"$registry_root/"}" >&2
    exit 1
  fi
done
if [ ! -f "$collisions" ]; then
  printf 'specification source-name check failed:\n- missing input: %s\n' \
    "${collisions#"$registry_root/"}" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

normalize_stream() {
  LC_ALL=C tr '[:upper:]' '[:lower:]' | LC_ALL=C sed 's/[^a-z0-9]//g'
}

# ---------------------------------------------------------------------------
# The deny-list: names that belong to the language, its library, or the game
# platform every one of these trees is written against. A word here is not an
# upstream name, so carrying it says nothing about who wrote the specification.
# Maintained by hand. Anything a tree declares that is not on it is upstream
# until checks/source-name-collisions.tsv excuses it.
# ---------------------------------------------------------------------------
cat >"$work/deny" <<'DENY'
abstract
accept
apply
array
arraylist
assert
boolean
break
builder
byte
bytebuf
case
catch
char
class
clone
codec
collection
collections
comparable
compare
compareto
concurrent
const
consumer
continue
default
double
else
entry
enum
equals
exception
extends
false
final
finally
float
function
getclass
getinstance
getkey
getname
getordinal
getvalue
goto
hashcode
hashmap
hashset
immutable
implements
import
instance
instanceof
integer
interface
iterable
iterator
lambda
lang
linkedlist
long
mojang
native
none
notify
null
number
object
optional
override
package
predicate
private
protected
public
rawtypes
record
return
runnable
runtime
serializable
short
static
strictfp
string
stringbuilder
super
supplier
suppresswarnings
switch
synchronized
this
throw
throws
tostring
transient
true
type
unchecked
unsafe
util
value
values
void
volatile
while
DENY
cat >>"$work/deny" <<'DENY'
blockentity
blockpos
blockstate
blocktags
capability
chunkpos
clientlevel
componenttype
compoundtag
creativemodetab
damagesource
datacomponent
datagen
datapack
direction
entitytype
gametest
gsonhelper
guigraphics
identifier
ingredient
interactionhand
interactionresult
inventory
itemstack
itemtags
levelaccessor
livingentity
loottable
minecraft
mixin
namespace
neoforge
neoforged
nonnullist
particle
playerentity
recipeholder
recipeserializer
recipetype
registerevent
registries
registryaccess
registryobject
resourcekey
resourcelocation
serverlevel
serverplayer
simplecontainer
soundevent
stream_codec
streamcodec
tagkey
texture
translatable
vec3
voxelshape
DENY
cat >>"$work/deny" <<'DENY'
advancement
argumenttype
biome
biomes
blockentities
blocks
bucket
carpet
chunk
crafting
creative
crop
crops
cursor
damage
dragon
drops
enchantable
enchantment
entities
entity
experience
fluid
fluids
font
harvest
inventory
item
items
keyboard
keyset
level
light
loot
material
menu
model
models
nether
network
overworld
packet
payload
payloads
player
potion
recipe
recipes
redstone
render
rendertype
resource
resourcepack
resources
screen
seeds
serializer
server
shaped
shapeless
shovel
slider
slot
slots
sound
stack
stacks
stone
structure
sword
table
tables
tag
tags
texture
textures
tooltip
tooltips
unbaked
widget
window
world
DENY
cat >>"$work/deny" <<'DENY'
cache
cached
categories
channel
component
components
condition
conditions
connect
container
content
contents
converter
counter
deferred
dimension
dimensions
entryset
error
event
expression
extract
extraction
factory
field
fieldtype
filter
generate
generated
generator
group
handler
helper
helpers
holder
index
input
inputs
key
keys
layer
layout
length
listener
logging
lookup
match
matcher
matches
metadata
method
mode
module
modules
offset
output
parameter
parameters
parse
path
pattern
plugin
predicate
predicates
property
query
queue
range
rank
reader
registrar
registration
registry
reload
request
resolve
result
scheduler
scheduling
search
section
sections
setup
size
state
storage
stream
style
support
sync
target
targets
test
toggle
typed
update
upgrade
usage
validator
validators
value
version
widths
wrapped
wrapper
writer
DENY
cat >>"$work/deny" <<'DENY'
block
button
diamond
capabilities
capability
deepslate
essences
facing
feature
flower
ingot
multiblock
nugget
pickaxe
shard
tank
template
tool
tools
translation
trigger
vanilla
DENY
LC_ALL=C sort -u "$work/deny" -o "$work/deny"

{ grep -v '^#' "$collisions" || true; } | cut -f1 | LC_ALL=C tr '[:upper:]' '[:lower:]' |
  sed 's/[[:space:]]*$//' | { grep -v '^$' || true; } | LC_ALL=C sort -u >"$work/excused"

# ---------------------------------------------------------------------------
# The extractor. One candidate upstream name per line, folded to lower case:
# the file stem, every declared class, interface, enum and record name, every
# declared method name, every declared field and constant name, every segment
# of the package declaration, and every string literal shaped like a serialised
# key or a registry id together with its separated segments. Nothing under four
# characters, because a three-letter token is noise rather than a name.
# ---------------------------------------------------------------------------
cat >"$work/extract.awk" <<'AWK'
# A name is distinctive or it is not a name. A file stem, a declared type, a
# package segment and a whole string literal are kept as written. A method or a
# field name is kept only when it is a compound -- a case transition or an
# underscore -- because a one-word member name is the vocabulary every mod in
# this game shares and carrying it says nothing about who wrote the prose. A
# separated literal also yields its segments, from five characters up.
function distinctive(t) { return (t ~ /[a-z0-9][A-Z]/ || t ~ /_/) }
function emit(t) { if (t != "" && length(t) >= 4) print tolower(t) }
function emit_member(t) { if (distinctive(t)) emit(t) }
FNR == 1 { stem = FILENAME; sub(/.*\//, "", stem); sub(/\.java$/, "", stem); emit(stem); inblock = 0 }
{
  line = $0
  # Comments are prose, not names. A javadoc sentence saying "a class that
  # interns strings" declares nothing, and reading it as a declaration is how a
  # name check fills up with English.
  if (inblock) { if (line ~ /\*\//) { sub(/^.*\*\//, "", line); inblock = 0 } else next }
  while (match(line, /\/\*.*\*\//)) { line = substr(line, 1, RSTART - 1) substr(line, RSTART + RLENGTH) }
  if (match(line, /\/\*/)) { line = substr(line, 1, RSTART - 1); inblock = 1 }
  sub(/\/\/.*$/, "", line)
  if (line ~ /^[[:space:]]*$/) next
  if (line ~ /^[[:space:]]*package[[:space:]]/) {
    p = line; sub(/^[[:space:]]*package[[:space:]]+/, "", p); sub(/;.*$/, "", p)
    n = split(p, seg, /\./)
    # The package root names the project; everything below it names structure.
    # com.blakebr0.cucumber.item says who wrote it and what it is called in its
    # first three segments and says "item" in its fourth, so only the root is
    # read as a name and the domain segment in front of it is dropped.
    for (i = 1; i <= n && i <= 3; i++) {
      if (i == 1 && seg[i] ~ /^(com|net|org|io|dev|me|xyz|eu|uk)$/) continue
      emit(seg[i])
    }
    next
  }
  if (line ~ /^[[:space:]]*import[[:space:]]/) next
  t = line
  while (match(t, /(^|[^A-Za-z0-9_$@])(class|interface|enum|record)[[:space:]]+[A-Za-z_$][A-Za-z0-9_$]*/)) {
    d = substr(t, RSTART, RLENGTH); t = substr(t, RSTART + RLENGTH)
    sub(/^[^A-Za-z0-9_$]*(class|interface|enum|record)[[:space:]]+/, "", d); emit(d)
  }
  if (match(line, /^[[:space:]]*(((public|protected|private|static|final|abstract|synchronized|native|default|strictfp)[[:space:]]+)+|@[A-Za-z]+[[:space:]]+)[A-Za-z_$@<][A-Za-z0-9_$<>\[\],.?[:space:]]*[[:space:]]([A-Za-z_$][A-Za-z0-9_$]*)[[:space:]]*\(/) &&
      line ~ /\)[^)]*(\{|;|throws)/) {
    d = substr(line, RSTART, RLENGTH); sub(/[[:space:]]*\($/, "", d)
    sub(/^.*[^A-Za-z0-9_$]/, "", d); emit_member(d)
  }
  if (match(line, /^[[:space:]]*(public|protected|private|static|final|transient|volatile)[^=;(){}]*[[:space:]]([A-Za-z_$][A-Za-z0-9_$]*)[[:space:]]*[=;]/)) {
    d = substr(line, RSTART, RLENGTH); sub(/[[:space:]]*[=;][[:space:]]*$/, "", d)
    sub(/^.*[^A-Za-z0-9_$]/, "", d); emit_member(d)
  }
  t = line
  while (match(t, /"[^"]*"/)) {
    d = substr(t, RSTART + 1, RLENGTH - 2); t = substr(t, RSTART + RLENGTH)
    # A serialised key or a registry id is separated. A bare lowercase word in
    # a string is the game's own vocabulary far more often than it is a name,
    # and the mod id case is already the sibling check's, so only separated
    # literals are read here.
    if (d ~ /^[a-z][a-z0-9]*([_.:\/-][a-z0-9]+)+$/) {
      emit(d)
      n = split(d, seg, /[_.:\/-]+/)
      for (i = 1; i <= n; i++) if (length(seg[i]) >= 5) emit(seg[i])
    }
  }
}
AWK

# ---------------------------------------------------------------------------
# Resolving a specification to its fetched tree. The provenance record names the
# upstream repository and the pinned version; the tree is one directory per
# source-informed entry, named <Owner>_<Repo>@<version>. Both halves have to
# agree, so a record whose version no directory carries is a miss and not a
# silent pass. fetch-manifest.txt is read as the second witness that the tree
# was fetched for this entry rather than found lying about.
# ---------------------------------------------------------------------------
resolve_tree() {
  local owner_repo="$1" version="$2" candidates=() d
  local prefix="${owner_repo/\//_}@"
  for d in "$source_root/$prefix"*; do
    [ -d "$d" ] || continue
    case "${d##*@}" in *"$version"*) candidates+=("$d") ;; esac
  done
  if [ "${#candidates[@]}" -eq 1 ]; then
    printf '%s\n' "${candidates[0]}"
    return 0
  fi
  return 1
}

: >"$work/out"
: >"$work/skips"
checked=0
skipped=0
scanned_specs=0
token_total=0

if [ "$mode" = "--fixture" ] && { [ ! -d "$source_root" ] || [ ! -f "$manifest" ]; }; then
  printf 'required clean-room source tree or fetch manifest is missing\n' >>"$work/out"
fi
for record in "$provenance_root"/RVL-*.json; do
  [ -f "$record" ] || continue
  canonical="${record##*/}"
  canonical="${canonical%.json}"
  specification="$spec_root/$canonical.md"
  [ -f "$specification" ] || continue

  locator="$(jq -r '
    [.sources[]? | select(.category == "source-code")
     | .locator | select(test("^https://github\\.com/[^/]+/[^/]+/?$"))] | first // empty
  ' "$record")"
  [ -n "$locator" ] || continue

  scanned_specs=$((scanned_specs + 1))
  owner_repo="${locator#https://github.com/}"
  owner_repo="${owner_repo%/}"
  version="$(jq -r '.subject.version // empty' "$record")"
  if [ -z "$version" ]; then
    printf '%s: the provenance record names %s but declares no pinned version, so no fetched tree can be bound to it\n' \
      "$canonical" "$owner_repo" >>"$work/out"
    continue
  fi

  if ! tree="$(resolve_tree "$owner_repo" "$version")"; then
    skipped=$((skipped + 1))
    printf '%s: skipped: source tree absent: expected %s@*%s* under %s -- the clean-room tree is ignored and may not exist on this machine, so this entry was not checked and its result is not a pass\n' \
      "$canonical" "${owner_repo/\//_}" "$version" "$source_root" >>"$work/skips"
    continue
  fi
  if [ -f "$manifest" ] && ! LC_ALL=C grep -Fq "$owner_repo|" "$manifest"; then
    printf '%s: the fetched tree for %s is present but no row in fetch-manifest.txt records fetching it\n' \
      "$canonical" "$owner_repo" >>"$work/out"
  fi

  mapfile -d '' -t sources < <(find -L "$tree" -type f -name '*.java' -print0 | LC_ALL=C sort -z)
  if [ "${#sources[@]}" -eq 0 ]; then
    printf '%s: the fetched tree at %s carries no Java source, so no upstream name was read from it\n' \
      "$canonical" "${tree##*/}" >>"$work/out"
    continue
  fi

  LC_ALL=C awk -f "$work/extract.awk" "${sources[@]}" |
    { grep -v '^$' || true; } | LC_ALL=C sort -u >"$work/names-raw"
  # The deny-list bites on the normal form as well as the spelling, so
  # block_entity is refused for the same reason blockentity is.
  normalize_stream <"$work/deny" | { grep -v '^$' || true; } |
    LC_ALL=C sort -u >"$work/deny-normal"
  LC_ALL=C awk -v DENY="$work/deny" -v DENYN="$work/deny-normal" '
    function norm(s) { gsub(/[^A-Za-z0-9]/, "", s); return tolower(s) }
    BEGIN {
      while ((getline l < DENY) > 0) if (l != "") d[l] = 1
      while ((getline l < DENYN) > 0) if (l != "") dn[l] = 1
    }
    !($0 in d) && !(norm($0) in dn)
  ' "$work/names-raw" >"$work/names-nodeny"
  LC_ALL=C comm -23 "$work/names-nodeny" "$work/excused" >"$work/names"
  if [ ! -s "$work/names" ]; then
    printf '%s: no upstream name survived the deny-list and the collisions list, so every rule would pass by having nothing to compare against\n' \
      "$canonical" >>"$work/out"
    continue
  fi
  normalize_stream <"$work/names" | { grep -v '^$' || true; } |
    LC_ALL=C sort -u >"$work/names-normal"

  checked=$((checked + 1))
  token_total=$((token_total + $(wc -l <"$work/names" | tr -d ' ')))

  # Rule U1.
  LC_ALL=C grep -oinwFf "$work/names" -- "$specification" 2>/dev/null |
    LC_ALL=C sort -t: -k1,1n -u | while IFS=: read -r lineno token; do
    [ -n "${token:-}" ] || continue
    printf '%s:%s: %s (U1)\n' "$canonical" "$lineno" "$token"
  done >>"$work/out" || true

  # Rule N1.
  LC_ALL=C awk -v LITF="$work/names" -v NORMF="$work/names-normal" -v ID="$canonical" '
    function norm(s) { gsub(/[^A-Za-z0-9]/, "", s); return tolower(s) }
    BEGIN {
      while ((getline l < LITF) > 0) { if (l != "") lit[l] = 1 }
      while ((getline l < NORMF) > 0) { if (l != "") nrm[l] = 1 }
    }
    function examine(c,   n) {
      gsub(/^[^A-Za-z0-9]+/, "", c); gsub(/[^A-Za-z0-9]+$/, "", c)
      if (c == "" || c ~ /^[0-9]+$/) return
      n = norm(c)
      if (length(n) < 4) return
      if (!(n in nrm)) return
      if (tolower(c) in lit) return
      if (seen[n]++) return
      printf "%s:%s: %s (N1)\n", ID, FNR, c
    }
    {
      line = $0
      while (match(line, /[A-Za-z0-9_][A-Za-z0-9_.:\/-]*/)) {
        t = substr(line, RSTART, RLENGTH); line = substr(line, RSTART + RLENGTH)
        examine(t)
        m = split(t, part, /[^A-Za-z0-9]+/)
        if (m > 1) for (k = 1; k <= m; k++) examine(part[k])
      }
    }
  ' "$specification" >>"$work/out"
done

if [ "$scanned_specs" -eq 0 ]; then
  printf 'specification source-name check failed:\n- no specification under %s names a source-code source, so this check compared nothing\n' \
    "${spec_root#"$registry_root/"}" >&2
  exit 1
fi

if [ -s "$work/skips" ]; then
  printf 'specification source names: NOT CHECKED, %s of %s source-informed specifications:\n' \
    "$skipped" "$scanned_specs" >&2
  sed 's/^/- /' "$work/skips" >&2
  if [ "$mode" = "--fixture" ]; then
    printf 'required clean-room source inputs are absent for %s specifications\n' "$skipped" >>"$work/out"
  fi
fi

if [ -s "$work/out" ]; then
  printf 'specification source-name check failed:\n' >&2
  sed 's/^/- /' "$work/out" >&2
  exit 1
fi

printf 'specification source names: %s specifications read against %s upstream names from their fetched trees, no hit under rules U1 or N1\n' \
  "$checked" "$token_total"
printf 'specification source names: what this does not catch: a name the extractor does not reach, being anything outside a Java declaration, a package segment or a lowercase string literal, so no name from a resource document, a model or a language file; a name shorter than four characters; a name on the hand-maintained platform deny-list, which holds language, library and game-platform vocabulary and would hide an upstream name that happens to spell one; the %s ordinary words checks/source-name-collisions.tsv excuses by hand; a specification whose tree is absent, which is reported above and is not a pass; and a rewrite that renamed everything, which no name check can see.\n' \
  "$(wc -l <"$work/excused" | tr -d ' ')"
