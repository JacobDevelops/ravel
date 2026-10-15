#!/usr/bin/env bash
# Triages each acquired artifact's rights from what it declares about itself.
#
# The mechanical half of M5-W5, shared by the producer and the consumer. Two
# facts feed it and both come from outside this workspace: the licence string a
# mod states in its own metadata, and what the acquisition log records about how
# the artifact was obtained.
#
# The statuses are the public provenance schema's vocabulary — cleared,
# permission-required, unresolved, prohibited, not-applicable — so a private
# per-artifact verdict and a public provenance record say the same words.
#
# Two rules are inherited from M5-W1 rather than invented here. Names stay
# unresolved because whether an original mod id may be advertised is a question
# SPEC.md 7.3 defers, and branding is prohibited outright wherever a mod ships
# one, because a mod's logo is its author's and this workspace never reproduces
# it. That is the same reasoning that made the pack's branding, quests and
# scripts prohibited rather than unresolved: they name material nothing here
# extracts, so no later review clears them.
#
# Behaviour is cleared, and that is the judgement the milestone turns on. Ravel
# re-specifies observed behaviour instead of copying code, so a licence that
# forbids source use does not block the work — it blocks one method of doing it.
# An unresolved or prohibited code status stops publication and stops reading the
# source; it does not stop the black-box inventory.
#
# It is a policy of this workspace applied unconditionally, not something read
# off an artifact: no input changes it and no check can fail on it. It is written
# as a constant below for that reason, and the count of artifacts carrying it is
# never reported as if it were a measurement, because it is only the number of
# artifacts. The same is true of names, which M5-W1 fixed at unresolved.
set -euo pipefail

rights_model() {
  local out="${meta_work:?rights_model requires mod-metadata.sh}"

  # How each artifact was obtained, from the acquisition log alone.
  local acquisition="$out/acquisition.tsv"
  {
    grep -oE 'edge\.forgecdn\.net/files/[0-9]+/[0-9]+/[^"]+' "$import_log" |
      sed -E 's#.*/([^/]+)$#\1\tautomated-curseforge#'
    grep -oE 'Found alternative on modrinth "[^"]+"' "$import_log" |
      sed -E 's#.*"(.*)"#\1\talternate-distributor#' || true
    grep -oE '\[Blocked Mods Dialog\] Name match found: "[^"]+"' "$import_log" |
      sed -E 's#.*"(.*)"#\1\tdistribution-restricted#' || true
  } | awk -F'\t' 'BEGIN { OFS = "\t" }
    {
      name = $1
      # CDN names are percent-encoded; the files on disk are not.
      while (match(name, /%[0-9A-Fa-f][0-9A-Fa-f]/)) {
        hex = toupper(substr(name, RSTART + 1, 2))
        code = (index("0123456789ABCDEF", substr(hex, 1, 1)) - 1) * 16
        code += index("0123456789ABCDEF", substr(hex, 2, 1)) - 1
        name = substr(name, 1, RSTART - 1) sprintf("%c", code) substr(name, RSTART + 3)
      }
      print name, $2
    }' | sort -u >"$acquisition"

  if [ ! -s "$acquisition" ]; then
    echo "acquisition log records no artifact provenance" >&2
    return 1
  fi

  awk -F'\t' -v rights_out="$out/rights.tsv" '
    function normalise(raw,   s) {
      s = tolower(raw)
      gsub(/[ \t]+/, " ", s)
      return s
    }
    # A declaration that reserves the assets apart from the code says so in its
    # own words, and both halves have to survive. Reading the reservation as the
    # whole licence would forbid source use the author allowed; reading the code
    # licence as the whole licence would clear assets the author reserved. So the
    # split is detected first, and each half is answered from the words that
    # qualify it rather than from which half happens to be tested first.
    function reserves_assets(s) {
      return (s ~ /all rights reserved/ && s ~ /assets?|textures?|models?|sounds?|artwork|resources?/)
    }
    function bucket(raw,   s) {
      s = normalise(raw)
      if (s == "") return "absent"
      if (s ~ /^https?:/ || s ~ /^see /) return "reference-only"
      # The restrictive readings are tested before the permissive ones, so a
      # licence that reserves everything and then mentions a permissive
      # component cannot bucket as that component.
      if (reserves_assets(s)) sub(/all rights reserved/, "", s)
      else if (s ~ /all rights reserved|^arr$|arr /) return "all-rights-reserved"
      # Noncommercial and no-derivatives are restrictions a licence states, not
      # properties of the Creative Commons family. Requiring a cc token first
      # meant a licence that says "non-commercial" in plain words bucketed as
      # custom, so the test is for the restriction, and the CC abbreviations are
      # one spelling of it rather than the precondition for reading it.
      if (s ~ /non-? ?commercial/ || s ~ /no[ -]?derivativ|noderiv/) return "noncommercial-or-noderivatives"
      if (s ~ /(^|[^a-z0-9])(nc|nd)([^a-z0-9]|$)/) return "noncommercial-or-noderivatives"
      if (s ~ /agpl/) return "strong-copyleft"
      if (s ~ /lgpl|lesser general public/) return "weak-copyleft"
      if (s ~ /gpl|general public license/) return "strong-copyleft"
      if (s ~ /mpl|mozilla public|epl|eclipse public/) return "weak-copyleft"
      # A licence whose whole subject is forbidding one use of the work. Every
      # PolyForm variant permits the work generally and then names a use it does
      # not permit, and the use Shield names is competing with the licensor,
      # which is what a native reimplementation of that mod is. The round-4
      # review was right that custom understated it: "we do not know" is the
      # wrong answer about a licence that says so plainly.
      if (s ~ /polyform/) return "source-restricted"
      # An attribution obligation is a condition on use rather than a grant of
      # it, so it belongs with the copyleft families and not with the permissive
      # ones. CC-BY used to bucket permissive here, which cleared the assets and
      # recorded the obligation nowhere.
      if (s ~ /(^|[^a-z0-9])cc.?by([^a-z0-9]|$)|creative commons attribution/) return "attribution-required"
      # Every permissive token is matched at a word boundary. The round-4 review
      # found this test was the loosest in the function and the only one that
      # unlocks source reading: a custom licence whose prose contained
      # "permitted", "limited" or "submit" bucketed permissive on the substring
      # "mit". The restrictive tests above stay loose on purpose, because a
      # restrictive test firing where it need not is the safe direction.
      if (s ~ /(^|[^a-z0-9])(mit|apache|bsd|isc|unlicense|wtfpl|cc0|zlib|public domain)([^a-z0-9]|$)/)
        return "permissive"
      return "custom"
    }
    function code_status(b) {
      if (b == "permissive") return "cleared"
      if (b == "weak-copyleft" || b == "strong-copyleft" || b == "attribution-required")
        return "permission-required"
      if (b == "all-rights-reserved" || b == "noncommercial-or-noderivatives" ||
          b == "source-restricted") return "prohibited"
      return "unresolved"
    }
    function asset_status(b, reserved, present) {
      if (present == 0) return "not-applicable"
      if (reserved == "true") return "prohibited"
      return code_status(b)
    }
    function worst(a, b) { return (rank[a] > rank[b]) ? a : b }

    BEGIN {
      rank["not-applicable"] = 0; rank["cleared"] = 1
      rank["permission-required"] = 2; rank["unresolved"] = 3; rank["prohibited"] = 4
    }

    FILENAME == acquisition_file { how[$1] = $2; next }

    FILENAME == meta_file {
      if ($2 == "LICENSE") declared[$1] = $3
      else if ($2 == "MOD") { if ($6 != "") logo[$1] = 1
                              mods[$1] = ($1 in mods) ? mods[$1] "," $3 : $3 }
      else if ($2 == "NESTED" && $4 != "") mods[$1] = ($1 in mods) ? mods[$1] "," $4 : $4
      next
    }

    FILENAME == surfaces_file {
      klass[$1] = $2; project[$1] = $3; file[$1] = $4
      textures[$1] = $8; models[$1] = $9; sounds[$1] = $10
      next
    }

    END {
      for (f in klass) {
        raw = (f in declared) ? declared[f] : ""
        b = bucket(raw)
        # The reservation is read from the declaration, not from which family
        # the code half landed in. Asking the bucket would have dropped the
        # reservation for every author who reserved their assets and licensed
        # their code under anything but a permissive licence.
        reserved = reserves_assets(normalise(raw)) ? "true" : "false"

        code = code_status(b)
        tex = asset_status(b, reserved, textures[f] + 0)
        mod = asset_status(b, reserved, models[f] + 0)
        snd = asset_status(b, reserved, sounds[f] + 0)
        # Two of the seven surfaces are constants rather than readings. No input
        # can change either, so the number of records carrying them is the number
        # of records and says nothing; only branding is decided per artifact,
        # from whether the artifact declares a logo.
        names = "unresolved"
        behaviour = "cleared"
        branding = (f in logo) ? "prohibited" : "not-applicable"

        acquired = (f in how) ? how[f] : "unrecorded"

        if (code == "cleared") source_use = "permitted"
        else if (code == "permission-required") source_use = "conditional"
        else if (code == "prohibited") source_use = "prohibited"
        else source_use = "unresolved"

        publication = "cleared"
        publication = worst(publication, code)
        publication = worst(publication, tex)
        publication = worst(publication, mod)
        publication = worst(publication, snd)
        publication = worst(publication, names)
        publication = worst(publication, branding)
        publication = (publication == "cleared") ? "cleared" : "blocked:" publication

        printf "%s\t%s\t%s\t%s\t%s", f, klass[f], project[f], file[f],
          ((f in mods) ? mods[f] : "-") >rights_out
        printf "\t%s\t%s\t%s\t%s", raw, b, reserved, acquired >rights_out
        printf "\t%s\t%s\t%s\t%s\t%s\t%s\t%s", code, tex, mod, snd, names, branding, behaviour >rights_out
        printf "\t%s\t%s\n", source_use, publication >rights_out
      }
    }
  ' acquisition_file="$acquisition" meta_file="$meta_tsv" surfaces_file="$surfaces_tsv" \
    "$acquisition" "$meta_tsv" "$surfaces_tsv"

  if [ ! -s "$out/rights.tsv" ]; then
    echo "rights model produced no records" >&2
    return 1
  fi
  sort -o "$out/rights.tsv" "$out/rights.tsv"
  printf 'rights_tsv=%q\n' "$out/rights.tsv"
}
