# Driving acceptance through the loader's own crossings

Status: accepted while scoping the agentic acceptance harness, ahead of M8-W4 and
M10. An acceptance run perceives the game through the events the loader already
reports and acts on it through the instruction the Java adapter already performs,
and it adds no new crossing in either direction. The model this decides is built
and tested in `crates/ravel-acceptance`; the driver that runs it against a
launched game is owned by M8-W0 and is not built here.

This record takes no ADG number, for the reason
[`README.md`](README.md) gives about the records that already do not: `SPEC.md`
§15 owns those numbers and reserves its unused ones for other subjects. Like
[Carrying an action outward](carrying-an-action-outward.md), this decides how an
existing capability is used rather than opening a gate, and it is one layer above
that record — it is that record's mechanism put to a test's use.

## Why this record exists

`ROADMAP.md`'s M10 asks a player to walk a progression through a running game,
and M8-W4 asks for fresh-world and integration scenarios with complete coverage
of the intended progression. Neither can be a person replaying by hand on every
change, and M8-W0's note names the failure the lazy alternative falls into: "a
green that measures the tests rather than the game." So a mechanism has to exist
that drives a real modded game and decides a verdict from what the game did.

The design question was not *whether* to build it but *on what*. Three mechanisms
were available and they are not interchangeable, which is what makes this a
decision rather than a plan. The full comparison is in
[`docs/architecture/agentic-acceptance-harness.md`](../architecture/agentic-acceptance-harness.md);
the decision is here.

## Decision

**An acceptance run consumes the two crossings the loader already has, and opens
neither a third nor a private one of its own.**

- It **perceives** through `ravel_host_api::GameEvent` and a structured
  `ravel_acceptance::Observation` projected from a hook's value. It reads modded
  content by the declared key a pack registered it under — a `RegistryKey`'s kind
  and id — never by recognising a rendered pixel.
- It **acts** by handing a `ravel_acceptance::Directive` to a `GameDriver`, which
  carries it outward as [Carrying an action outward](carrying-an-action-outward.md)'s
  instruction for the Java adapter to perform. A directive is data. No Rust code
  calls a game method, and `JavaHandle` gains nothing, so ADG-1's "Nothing here
  admits a call from Rust into the game" survives this record as it survived that
  one.
- It **decides** a verdict from a `Transcript` it recorded, in one place with one
  arithmetic, so a report and a gate read the same result.

The model — scenarios, steps, assertions, the progression-checkpoint spine, the
transcript, and the `GameView`/`GameDriver` traits a run is written against — is
built in `ravel-acceptance` and is decidable without a game, which is why it is
built now. The two boundaries a live run crosses are named as traits and left to
their owners.

## Rejected

- **A vanilla-protocol bot (Mineflayer and the like).** Headless and cheap, but
  blind to modded content: ATM10:TTS's machines, GUIs and quest state are not in
  the vanilla protocol, so a protocol bot cannot see the progression it is meant
  to walk. Usable for a vanilla smoke test and nothing M10 asks for. Rejected as
  the acceptance mechanism.
- **A screen-scraping vision agent.** Understands modded content because it sees
  the rendered GUI, and is the first thing that works — it is roughly what a
  desktop-driving agent already did by hand. Rejected as the *acceptance*
  mechanism because a gate that re-parses pixels flakes on a re-render and a gate
  that flakes is not a gate. It is not forbidden as an exploratory tool; it is
  declined as the thing a verdict rests on.
- **A second generated boundary running outward for tests.** The same shape ADG-6
  refused and [Carrying an action outward](carrying-an-action-outward.md)
  declined to build: a second hook inventory and a second exception protocol for
  a direction the instruction crossing already serves. A test is not a reason to
  own a boundary a mod does not.

## Consequences

- **A run adds no attack surface and no safety exception.** Every crate keeps
  `#![forbid(unsafe_code)]`, `ravel-acceptance` included, and the directive
  crossing is the audited one the adapter already performs.
- **An effect lands after the answer.** Because the outward crossing has that
  property, a scenario that depends on an effect having happened inserts a
  `Directive::AdvanceTicks` step and asserts after it, rather than asserting in
  the step that issued the action. A surface that needs the game's answer *within*
  the same call is `RVL-GAP-10`'s and is not settled here.
- **A dropped directive is a silent failure**, exactly as
  [Carrying an action outward](carrying-an-action-outward.md) records: the adapter
  performing nothing and performing everything look identical from Rust. An
  acceptance run defends against it the way a walk already must — the following
  assertion is what catches a directive that did nothing, and a step whose
  assertion cannot be evaluated at all is a `Verdict::Errored`, distinct from a
  decided `Verdict::Failed`, so a broken driver is never counted as a passing
  game.
- **Authentication stays outside Ravel.** ADG-5 owns the signed-in session and
  keeps it Prism-owned and human-initiated; a run renews a refresh token
  non-interactively after one human bootstrap, or runs offline for single-player
  functional gates. Ravel reads and stores no credential.

## What is not decided here, and who owns it

- **The projection from a hook value to an `Observation`** is `ravel-host-api`'s
  recorded unstarted work, landing with the hook-to-loader wiring.
- **A host that performs an outward instruction against a running game** is
  `ROADMAP.md`'s M8-W0, which that row states is impossible today; the
  native-load half is ADG-5's amendment, which "no work package owns."
- **The one-time human authentication bootstrap** needs a licensed account and is
  ADG-5's, not this record's.
- **A launched-game run of any kind** needs a runtime and a display, and M9's
  performance rig additionally needs a real GPU — a different rig from the
  functional one, as the architecture document records.

Until those are met by packages that own them, no milestone may claim the game
was played on the strength of this crate's green tests. This record decides the
mechanism; it does not manufacture the run.

## Limits

- **Nothing here has driven a game.** The comparison that selected the mechanism
  rests on the two crossings' existing measurements — `GameEvent`'s side model
  and [Carrying an action outward](carrying-an-action-outward.md)'s outward-cost
  table — not on an acceptance run, because none has happened. This is a design
  decision taken ahead of its milestone, and it is disclosed as such rather than
  dressed as evidence.
- **The checkpoint spine is coarse on purpose.** `ProgressionCheckpoint` names the
  shape every pack's progression shares, not a pack's specific milestones, which
  live in the pack's own declaration. A pack that needs a finer spine adds its
  milestones there; this record does not turn the spine into an inventory.
