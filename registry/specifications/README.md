# Behaviour specifications

One prose specification per wave entry, written by a reader who saw the upstream
source, for an implementer who never will.

`ROADMAP.md:831` requires every counterpart to be implemented from an accepted
behaviour specification. These are those specifications.

## The rule the directory exists to keep

A specification states **what** an entry does — what it registers, what it
persists and when, which side it runs on, what it accepts and refuses, and what
each lifecycle event makes it do. It does not state **how** the original does it:
no class layout, no call graph, no method decomposition, no field names, no
algorithm where an ordinary one would serve. A specification that encodes the
original's structure produces a transliteration through prose, and the identifier
rules cannot catch that, because a transliteration through prose has new names.

Where a specific algorithm genuinely is the behaviour — a wire format, a hash, an
ordering others depend on — the specification says so and says why it is required
rather than incidental.

## No upstream identifier appears in a specification

Not a package, class, method, field, mod id, or file stem. Where a thing must be
referred to, it is named for what it does. `checks/verify-counterparts.sh` holds
the identifier rules; they were run over this directory and returned no finding.
That check's remaining rules are about how a Rust counterpart package names
itself, and a prose specification is not one, so it is the identifier rules
specifically that these files are held to.

## Honesty markers

Each specification marks every claim as **read** (taken from the pinned source),
**inferred** (deduced, not stated) or **unknown** (could not be determined). Each
ends with what it does not specify, including how much of the entry was read.

A reader who is also the specification's author has nothing contradicting them.
The markers, and the closing coverage statement, are what stands in for that
contradiction. They are weaker than a second reader and are not a substitute for
one.

## Provenance

`provenance/<wave>/<id>.json` records, per entry, where the source came from, the
exact revision, which licence file was read and where, and a digest over what was
fetched. The fetched source itself is never tracked: it lives only in the public
repository's ignored fixture tree.
