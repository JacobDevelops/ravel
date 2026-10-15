#!/usr/bin/env bash
# Holds the declared-metadata reader to every row it parsed.
#
# Round 1's review found a reader that recognised one spelling of a dependency
# block and dropped the others. The answer was a real parser, and a second reader
# to catch a defect in the first. Round 2's review found what that answer does
# not cover: `checks/modmeta.py` parsed every dependency a jar-in-jar library
# declares and emitted only the library's identity, `checks/recheck-metadata.sh`
# did the identical thing, the two agreed record for record, and 58 declarations
# left no node, no edge and no diagnostic. A comparison is evidence about
# disagreement. Two readers implementing the same rule agree, and agreement is
# what the comparison reports.
#
# So this check does not compare readers. It compares one reader to itself, in
# the one direction a shared rule cannot hide in: rows parsed against rows
# emitted. The reader writes a ledger at the point its parser returns — before
# any decision about what to keep — and this holds the emitted stream to it:
#
#   emitted(artifact) == parsed(artifact) + synthesised(artifact) - refused(artifact)
#
# A reader that parses N declarations and emits N-58 of them fails here whatever
# the second reader does, because the ledger counted the 58 where they were
# produced. The only way out of the equation is a refusal, and a refusal is a
# named row against an enumerated table: only a nested library's loader, licence
# and mixin lines have a reason available to them. A dependency row has no reason
# it could be refused under, so it can only be emitted.
#
# Round 3's review found the same class one level above where that equation can
# reach. Both readers discovered nested libraries from the jar-in-jar manifest
# and returned nothing when there was no manifest, so an artifact shipping two
# libraries and listing neither balanced its books perfectly: parsed, emitted and
# refused all agreed, and the rows had simply never existed. The equation cannot
# see that, because it asks what a reading said and not which readings there were
# to make. The damning part was that the reader wrote a table of contents naming
# both libraries on the same pass, and nothing compared the two.
#
# So this check now also decides which sites had to exist, and it decides it from
# the tables of contents rather than from either reader's discovery rule:
#
#   every entry in a jar-in-jar directory, at any depth, is a site that must have
#   been read, and every entry named like an archive anywhere is a site that must
#   at least have been found and declined by name
#
# The tables of contents are produced twice, by two readers, and compared byte
# for byte in recheck-metadata.sh, so this file is holding the readers to a
# listing neither of them derived from the rule under test. A decline is the one
# way out, and like a row refusal it is a named kind against a table this file
# owns and the reader does not — and the site is checked against the reason, so a
# reader cannot call a library cargo: the decline is refused if the archive sits
# in a jar-in-jar directory.
#
# Sourcing this defines reader_accounting; mod-metadata.sh calls it, so no
# producer and no consumer in this workspace can act on an unaccounted reading.
set -euo pipefail

reader_accounting() {
  local out="${meta_work:?reader_accounting requires mod-metadata.sh}"
  local ledger="${ledger_tsv:?reader_accounting requires the ledger mod-metadata.sh names}"
  local nested="${nested_entries_tsv:?reader_accounting requires the nested tables of contents}"

  if [ ! -s "$ledger" ]; then
    echo "the declared-metadata reader wrote no ledger, so nothing accounts for what it read" >&2
    return 1
  fi
  if [ ! -s "$entries_tsv" ] || [ ! -s "$nested" ]; then
    echo "the declared-metadata reader wrote no table of contents, so nothing says which nested libraries there were to read" >&2
    return 1
  fi

  local failures="$out/accounting-failures"
  : >"$failures"

  # The refusal table. It lives here rather than in the reader, so a reader that
  # invents a reason to drop a row is refused by a file it does not write. Each
  # refusal carries the refused row beside the kind it claims, and the two have
  # to agree: mislabelling a dependency as a licence line is how a reader would
  # otherwise balance its own books.
  local refusals="$out/admissible-refusals.tsv"
  {
    printf 'LOADER\ta nested library states its own loader, which is not the containing artifact'"'"'s\n'
    printf 'LICENSE\ta nested library states its own licence, which is not the containing artifact'"'"'s\n'
    printf 'MIXIN\ta nested library configures its own mixins, which are not the containing artifact'"'"'s\n'
  } >"$refusals"

  # The decline table, for archives rather than rows. It is the same mechanism
  # one level up: a reader may leave an archive unread only for a reason named
  # here, and the site is checked against the reason so that the label cannot do
  # the work. There is exactly one such reason, and a loader agrees with it.
  # There are two, and only one of them is ever allowed to stand where a loader
  # looks. UNREADABLE exists so that an archive the reader could not open says
  # so instead of being recorded as read: round 4's review found the reader
  # writing READ before it opened anything, which meant a malformed library
  # deleted its own table of contents and the induction below terminated on the
  # reader's word for it. In a jar-in-jar directory UNREADABLE fails here, which
  # is the point — a library a loader would load and this workspace cannot open
  # is a fact about the fixture that has to be looked at.
  local declines="$out/admissible-declines.tsv"
  {
    printf 'CARGO\tthe archive is not in a jar-in-jar directory, so the artifact ships it rather than loading it\n'
    printf 'UNREADABLE\tthe reader found the archive and could not open it, so nothing below it was read\n'
  } >"$declines"

  awk -F'\t' -v admissible="$refusals" -v declines="$declines" \
    -v ledger_file="$ledger" -v entries_file="$entries_tsv" -v nested_file="$nested" '
    # Where a loader loads a shipped library from, and what an archive is
    # conventionally called. Both live here rather than in either reader: the
    # question this file answers is which sites had to exist, so a reader that
    # narrowed its own idea of where to look would otherwise be the only witness
    # against itself.
    function jarjar(name) {
      return (name ~ /^META-INF\/jarjar\//) || (name ~ /^META-INF\/jars\//)
    }
    function archive_named(name) {
      return tolower(name) ~ /\.(jar|zip|jmod|war|ear|aar|apk|zipx)$/
    }
    function require_site(f, entry, key,   leaf) {
      if (entry ~ /\/$/) return
      if (jarjar(entry)) {
        leaf = entry
        sub(/.*\//, "", leaf)
        # The manifest in those directories names libraries rather than being
        # one. It is the only entry there that is not a site, and nothing else
        # about it is believed: it is not read.
        if (leaf != "metadata.json") must_read[key] = f
        return
      }
      if (archive_named(entry)) must_find[key] = f
    }

    FILENAME == admissible { reason[$1] = $2; next }
    FILENAME == declines { decline_reason[$1] = $2; next }

    FILENAME == entries_file { require_site($1, $2, $1 "!" $2); next }
    FILENAME == nested_file  { require_site($1, $3, $1 "!" $2 "!" $3); next }

    FILENAME == ledger_file {
      if ($1 == "SITE") {
        disposition[$3] = $4
        if ($4 != "READ") {
          leaf = $3
          sub(/.*!/, "", leaf)
          if (!($4 in decline_reason))
            printf "%s: an archive was left unread as %s, and no decline of that kind is admissible\n", $3, $4 >>failures
          else if ($5 != decline_reason[$4])
            printf "%s: an archive was left unread for a reason the accounting table does not give: %s\n", $3, $5 >>failures
          else if (jarjar(leaf))
            printf "%s: an archive in a jar-in-jar directory was left unread as %s, and that is where a loader looks for a library\n", $3, $4 >>failures
        }
        next
      }

      if ($1 != "PARSED" && $1 != "SYNTH" && $1 != "REFUSED") {
        printf "the reader ledger holds an unknown entry kind: %s\n", $1 >>failures
        known[$2] = 1
        next
      }
      # Rows read at a nested site are rows read somewhere the reader says it
      # went, so the site has to be one it recorded going to.
      if (index($3, "!") > 0) claimed[$3] = 1

      if ($1 == "PARSED")       { parsed[$2] += $4 }
      else if ($1 == "SYNTH")   { synth[$2] += $4 }
      else if ($1 == "REFUSED") {
        refused[$2]++

        # A refusal is admissible only for a kind the table names, only with the
        # reason the table gives it, and only at a nested site — the containing
        # artifact never declines its own declaration.
        split($6, row, "|")
        if (!($4 in reason))
          printf "%s: a %s row was refused, and no refusal of that kind is admissible\n", $3, $4 >>failures
        else if ($5 != reason[$4])
          printf "%s: a %s row was refused for a reason the accounting table does not give: %s\n", $3, $4, $5 >>failures
        else if (row[1] != $4)
          printf "%s: a row refused as %s is a %s row\n", $3, $4, row[1] >>failures
        else if (index($3, "!") == 0)
          printf "%s: a %s row was refused outside a nested declaration\n", $3, $4 >>failures
      }
      known[$2] = 1
      next
    }

    # The emitted stream.
    { emitted[$1]++; seen[$1] = 1 }

    END {
      # Which nested libraries there were to read, decided from the tables of
      # contents rather than from the rule under test.
      for (k in must_read) {
        if (!(k in disposition))
          printf "%s: a jar-in-jar directory holds a library this reader never found\n", k >>failures
        else if (disposition[k] != "READ")
          printf "%s: a jar-in-jar library was left unread as %s\n", k, disposition[k] >>failures
      }
      for (k in must_find)
        if (!(k in disposition))
          printf "%s: this artifact ships an archive the reader never found\n", k >>failures
      for (k in claimed)
        if (disposition[k] != "READ")
          printf "%s: rows were read at a nested site the reader never recorded finding\n", k >>failures

      for (f in seen)
        if (!(f in known))
          printf "%s: rows were emitted for an artifact the ledger does not account for\n", f >>failures
      for (f in known) {
        if (!(f in seen))
          printf "%s: the ledger accounts for an artifact that emitted no rows\n", f >>failures
        expect = parsed[f] + synth[f] - refused[f]
        if (emitted[f] != expect)
          printf "%s: the reader parsed %d rows and synthesised %d and refused %d, which is %d to emit, and emitted %d\n",
            f, parsed[f], synth[f], refused[f], expect, emitted[f] >>failures
      }
    }
  ' failures="$failures" "$refusals" "$declines" "$entries_tsv" "$nested" "$ledger" "$meta_tsv"

  if [ -s "$failures" ]; then
    echo "the declared-metadata reader did not account for every row it parsed:" >&2
    sort "$failures" | sed -n '1,40p' >&2
    return 1
  fi

  ledger_parsed="$(awk -F'\t' '$1 == "PARSED" { n += $4 } END { print n + 0 }' "$ledger")"
  ledger_refused="$(awk -F'\t' '$1 == "REFUSED" { n++ } END { print n + 0 }' "$ledger")"
  ledger_sites="$(awk -F'\t' '$1 == "PARSED" { n++ } END { print n + 0 }' "$ledger")"
  ledger_read="$(awk -F'\t' '$1 == "SITE" && $4 == "READ" { n++ } END { print n + 0 }' "$ledger")"
  ledger_declined="$(awk -F'\t' '$1 == "SITE" && $4 != "READ" { n++ } END { print n + 0 }' "$ledger")"
  export ledger_parsed ledger_refused ledger_sites ledger_read ledger_declined
}
