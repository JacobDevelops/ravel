# Counterpart identity and aliases

Status: accepted before M6-W3, for every counterpart Ravel ships and for the
private compatibility workspace that produces them. It is not an architecture
decision gate and carries no ADG number: `SPEC.md` §15 owns those, and its unused
numbers are reserved for subjects this record is not about. It does not decide
anything new about identity — `SPEC.md` §M11 already requires that "Ravel's
canonical IDs and internal APIs remain independent of NeoForge" and already lists
identity aliases among the mechanisms M11 implements. What it decides is where an
upstream name is allowed to appear, which M11 left open because M11 is about
joining a server and this is about writing a counterpart.

## The problem this record exists for

M6 enforces rewrite integrity mechanically. `checks/verify-counterparts.sh` in
the private workspace reads every file of every counterpart package and fails on
any upstream package, class, mod, node, or display name, verbatim or under the
repository's documented normalization. A transliteration carries the original's
names and a rewrite does not, so the check is what separates them without anyone
having to judge.

Counterparts also have to be *compatible*. A world save that already refers to a
mod's namespaced ids keeps referring to them after the mod is replaced, and
`plans.json` lists `registry-parity` among the test kinds an entry must pass.
Read one way, that requires a counterpart to claim the same namespace and the
same ids as the thing it replaces.

Those two read as a direct contradiction: a mod id is exactly the kind of
identifier the check bans, and drop-in replacement appears to require carrying
it. The first agent to reach the conflict stopped and escalated rather than
resolving it, which is the correct instinct, because both available local repairs
are damaging. Adding an exception to the check turns an enforcing mechanism into
a decorative one. Publishing upstream ids into the lexicon does the same thing
while looking tidier — the lexicon exists to hold names *to* a published list,
and a list that publishes whatever a counterpart happens to carry cannot refuse
anything.

The rights record shows M5 saw this and did not settle it: across the whole
inventory the `names` surface is `unresolved`, while every other surface —
`behaviour`, `code`, `textures`, `models`, `branding` — carries a decided
verdict.

## Decision

**A counterpart's own identity is Ravel-native, everywhere, without exception.**
Every id a counterpart registers, every namespace it claims, every symbol in its
source, and every name in its package tree is its own. No counterpart source file
carries an upstream identifier, and the check stands unmodified.

**Identity mapping is a host concern, not a per-counterpart one.** Ravel maps a
save's existing ids to the counterpart that now owns them at load time, the same
way `SPEC.md` §M11 maps identity when joining a server that speaks the original
namespaces. A counterpart does not adopt a foreign namespace to be compatible any
more than Ravel adopts NeoForge's internals to talk to a NeoForge server.

**Where a save-compatibility requirement genuinely exists, the upstream id is
declared in exactly one file per package**: `counterparts/<package>/aliases.json`.
One row per mapping — the upstream namespaced id, the counterpart's Ravel-native
id, and the canonical id of the entry being replaced — and nothing else in the
file. It is excluded from the identifier scan **by kind, with a named boundary**,
not by exception: the exclusion names one filename whose declared and only
purpose is to carry upstream names, and the check enforces that the file contains
nothing besides alias rows.

A row exists only where a real requirement is identified. A manifest listing
every id a mod ever registered is padding, and padding is how a record stops
being contradictable.

### Why one declared file rather than a carve-out

A carve-out would let upstream names appear anywhere a counterpart author decided
they were needed, and the check would then be answering a question nobody could
audit: it would report the names it was told to ignore. The declared-file shape
answers a better question. Every upstream name a counterpart carries is in one
file, that file may contain nothing else, and so "did this counterpart carry
upstream names, and which" is settled by reading one small file per package
rather than by trusting a scan's exclusion list.

This inverts the usual direction of an exclusion. An exclusion normally *widens*
what may pass; this one narrows where a thing may appear, and the check gets
stricter as a result: an upstream id in a source file fails exactly as before,
and an upstream id in the alias file is now accompanied by a machine-readable
statement of what it maps to and why it is there.

### Proved by mutation, not asserted

The exclusion is a boundary only if it refuses the case that would make it a
hole. Three mutations are required before any counterpart relying on it may be
admitted, and their exit codes recorded:

| mutation | required |
| --- | --- |
| an upstream id in a counterpart source file | nonzero |
| an upstream id in `aliases.json` as a declared alias row | zero |
| a non-alias key or free text added to `aliases.json` | nonzero |

The third is the one that matters. If it passes, the exclusion is not a boundary
at all — it is a file anyone can hide an upstream name in by giving it a
different key, and the check has been quietly converted into the decorative
version this record refused.

## What this decides against

**Against counterparts claiming upstream namespaces.** It is the shortest path to
`registry-parity` and it would make every counterpart a partial transliteration
by construction, in the one dimension a player and a save can both see. It also
contradicts `SPEC.md` §M11's exit criterion directly.

**Against relaxing the check for compatibility's sake.** The check is M6's
obligation to *enforce* rather than assert, and `ROADMAP.md` requires it to be
enforcing rather than advisory before M6 may close. A check relaxed to admit the
first hard case admits every later one.

**Against leaving it to each counterpart's author.** Two agents writing two
counterparts on the same day already produced two different specification shapes
when nothing decided the shape. Identity is not a place to discover that again.

## Consequences

- Ravel needs a load-time identity mapping capability. It does not exist today;
  M11 was going to build one for server interop and now has a second consumer, so
  its design must serve both. That is a dependency this record creates and does
  not discharge.
- `registry-parity` needs restating as a test of *observable* parity rather than
  of literal id equality, which this record makes deliberately false. It must not
  be restated as "a save referring to the old ids loads and plays": `SPEC.md` §17
  makes importing official ATM11 worlds an explicit non-goal, so no such save is
  in scope and that test could never be written. Observable parity is a claim
  about the counterpart, not about an upstream artefact — that a world the Ravel
  pack itself created round-trips over Ravel-native ids, and that the
  counterpart's registered content behaves as the specification says the upstream
  content behaves. Both are checkable inside the initial scope; neither needs an
  upstream save to exist.
- The private workspace's identifier check gains one filename it treats
  specially, plus the enforcement that the file is only alias rows.
- M5's `names` rights surface has an answer for the counterpart side: a
  counterpart carries no upstream name outside a declared mapping. This record
  does not resolve the surface for publication, because a mapping file naming a
  mod is still naming it, and whether that is publishable is a rights question
  this record does not reach.

## Limits of this record

It does not say how the host resolves an alias, in what order, or what happens
when two counterparts claim the same upstream id. Those belong to the M11
mechanism.

That last case is **not hypothetical**. This record first called it "a real
possibility", which was a guess; a constant-pool read of the private inventory
has since found a referenced target with *three* distinct canonical providers and
no rule anywhere that chooses between them. The private workspace records which
target and which providers, because upstream identifiers do not cross into this
repository. What belongs here is the shape: collision is present in the pack that
must ship, it arrives with more than two claimants, and M11's mechanism therefore
has to decide precedence rather than assume uniqueness and check for it.

It does not decide anything about assets, branding, or display names, which are
governed per surface by the rights record and by the private workspace's
source-access policy.

It assumes a save's ids are the only compatibility surface that forces an
upstream name into a counterpart. That assumption is load-bearing and untested:
recipes referring to foreign ids, cross-mod integration hooks keyed by name, and
pack-level configuration all plausibly do the same, and each would need a row in
the same file or a reason it does not. The first counterpart to hit one of those
should report it as a finding against this record rather than widening the file
quietly.
