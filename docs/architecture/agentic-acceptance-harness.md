# Agentic acceptance harness

How an acceptance run drives a modded pack through a running game without a
person at the keyboard, and which parts of that are decided, built, or owned but
unstarted. This is the design `ROADMAP.md`'s M8-W4 fresh-world scenarios and M10
progression acceptance are met by, written down before either milestone opens so
that the crates it names exist as boundaries the way `ravel-host-api` already
does.

It is a design record, not a claim of completion. The `ravel-acceptance` crate
is real and its model is tested; the driver that runs the model against a
launched game is not, and this document says exactly where the line is so a later
milestone cannot mistake the model for the run.

## The problem M8-W4 and M10 actually pose

M10 asks a player to "walk a progression through a running game", and M8-W4 asks
for "fresh-world and integration scenarios" with "complete scenario coverage" of
the intended progression. Both are worthless if a human has to sit and play them
every time the pack or the loader changes, and both are worse than worthless if
they are met by a suite that never launches anything — `ROADMAP.md`'s M8-W0 note
names that failure directly: "a green that measures the tests rather than the
game."

So the capability required is a piece of software that (1) reaches a running,
authenticated, modded ATM10:TTS instance, (2) perceives the modded world well
enough to decide whether a progression step happened, (3) acts on the world well
enough to take the next step, and (4) records what it did into evidence a gate
reads. Each of those four is a separate problem with a separate answer, and one
of the four is where the real design choice lives.

## The fork: how a run perceives and acts

There are three ways to build (2) and (3), and the choice determines everything.

- **A vanilla-protocol bot.** A headless client speaking the network protocol,
  no rendering. Truly headless and cheap, but it understands only what the
  vanilla protocol carries. ATM10:TTS is a modded tech progression whose blocks,
  machines, GUIs and quest state are not in the vanilla protocol, so such a bot
  cannot see or operate the very things a progression walk is about. Rejected for
  the pack this repository targets: it can smoke-test a vanilla launch and
  nothing M10 asks for.

- **A screen-scraping agent.** Render to a display, screenshot, parse the pixels
  with a vision model, drive mouse and keyboard. It understands modded content
  because it sees the rendered modded GUI, which is why it is the first thing
  that works and the thing a person reaches for. But it is slow, brittle, depends
  on a display and a window manager, and turns "did the machine produce the
  ingot" into an image-recognition problem with its own false-positive rate.
  Rejected as the *acceptance* mechanism: an acceptance gate that flakes on a
  re-render is not a gate. It remains a legitimate *exploratory* tool, and this
  design does not forbid one; it declines to build the acceptance contract on it.

- **A Ravel-native control and telemetry surface.** The loader already hosts and
  intercepts the game in-process. It already reports occurrences to a mod as
  `ravel_host_api::GameEvent`, and it already carries a mod's requested action
  outward as an instruction the Java adapter performs
  (`docs/decisions/carrying-an-action-outward.md`). An acceptance run is a
  privileged consumer of exactly those two crossings: it observes structured
  world state projected from a hook's value, and it acts by handing the adapter
  the same kind of instruction a mod's action becomes. It understands modded
  content natively because it reads the pack's declared keys rather than pixels,
  it is deterministic, and it opens no crossing the loader does not already have.

**This design selects the third.** It is the only one that makes an acceptance
verdict a decidable fact about the game rather than a guess about a screenshot,
and it reuses two mechanisms the repository already decided and measured rather
than inventing a boundary.

## What each of the four parts is, and its status

### Perception — decided surface, unstarted projection

A run reads the world through `ravel_acceptance::Observation`, a structured
snapshot of declared keys and counts. A live `GameView` produces one by
projecting a hook's value onto it. That projection is the work
`crates/ravel-host-api/src/lib.rs` already records as unstarted: "What is missing
is a projection from `HookValue` to this type, and a per-target decision about
which of a hook's parameters are worth carrying." The acceptance harness does not
change that boundary — it names the trait a projection satisfies and stops there.
The `Observation` type, the assertions over it, and the verdict arithmetic are
built and tested now, because all three are decidable without a game.

### Action — decided mechanism, unstarted outbound leg

A run acts by handing a `ravel_acceptance::Directive` to a `GameDriver`, which
carries it outward as `docs/decisions/carrying-an-action-outward.md`'s
instruction. That decision is accepted and measured; its outbound leg — a host
that actually performs the instruction against a running game — is `ROADMAP.md`'s
M8-W0, which that row states "is impossible rather than merely hard" today and
which "no work package owns" for its native-load half either (the ADG-5
amendment). The harness therefore names `GameDriver` as a trait with no live
implementation, and inherits M8-W0 as its blocker rather than restating it. A
directive is data, never a Rust call into the game, so the safety boundary and
ADG-1's "nothing here admits a call from Rust into the game" both hold unchanged.

Because "an action happens after the answer, not during it" is a property of that
crossing, a scenario that needs an effect to have landed inserts a following
`Directive::AdvanceTicks` step and asserts after it, rather than asserting in the
same step that issued the action.

### Authentication — decided elsewhere, needs a human once

Reaching a real Minecraft session is the Microsoft/Xbox/Minecraft-services device
flow, and `ADG-5` puts authentication, entitlement and account data outside
Ravel and Prism-owned: "The launch needs a signed-in session, so it is run by
hand rather than by the gate." The harness does not weaken that. The one-time
human step — approve a device code in a browser — mints a refresh token that a
subsequent run renews non-interactively, so "headless after the first launch" is
achievable without Ravel storing a credential. Two properties follow and are
recorded rather than assumed:

- **Functional and progression testing may not need online auth at all.** M8-W4
  and M10 are single-player with an integrated server, which a launcher can start
  in offline mode with a chosen profile name. Whether the acceptance gate runs
  offline is a launcher-configuration question the harness leaves to the run, and
  choosing offline sidesteps the credential entirely for those milestones.
- **Ravel never reads or stores the credential.** As under ADG-5, "Anything Ravel
  needs to know about an instance comes from what the operator configured on it."

### Display — a functional rig and a performance rig, and they differ

The client renders through OpenGL and needs somewhere to draw.

- **For functional and progression acceptance (M8-W4, M10):** a virtual display
  (`Xvfb` plus a minimal window manager) or software GL is sufficient, because
  these gates decide *whether* a progression state was reached, not how fast.
- **For performance acceptance (M9):** software GL is disqualifying. M9's fixed
  FPS, frame-time and pressure figures are meaningless on `llvmpipe`, so M9's rig
  is a real GPU rendering offscreen or a physical host, and it is explicitly a
  different rig from the functional one. This document does not decide M9's rig;
  it records that the two are not the same so a functional harness is never
  mistaken for a performance one.

### Orchestration — built as a pure model, run later

The loop that waits for an occasion, carries a directive, captures an
observation, evaluates an assertion and records a verdict is the
`ravel_acceptance` model: `Scenario`, `Step`, `Assertion`, `Directive`,
`ProgressionCheckpoint`, `Transcript`, and the two driver traits. All of it is
decidable without a game and is tested now. A `GameDriver`/`GameView` pair backed
by a launched game turns the same model into a run, and that pairing is the piece
that waits on M8-W0 and a runtime.

## Where the line is

Built and tested now, decidable without a game:

- the whole `ravel-acceptance` model — scenarios, steps, assertions, directives,
  the checkpoint spine, verdict arithmetic, and the `GameView`/`GameDriver`
  traits a run is written against.

Decided, but owned-and-unstarted, and named as blockers rather than reimplemented
here:

- the projection from a hook value to an `Observation` (`ravel-host-api`'s
  recorded gap);
- a host that performs an outward instruction against a running game
  (`ROADMAP.md` M8-W0, and ADG-5's amendment for the native-load half);
- the one-time human authentication bootstrap (ADG-5; needs a licensed account);
- a launched-game run of any kind, functional or performance (needs a runtime and
  a display, and for M9 a GPU).

No milestone may read a green from this crate's tests as evidence that the game
was played. That is the same discipline `ravel-host-api`'s "Nothing calls out at
any of these yet" keeps, applied one layer up.

## Relationship to the milestone tree

This harness is the mechanism M8-W4 and M10 are verified *by*, so it is built
before them the way `ravel-host-api` was built before the hooks that fill it. It
does not close either milestone and does not take an ADG number of its own beyond
the acceptance-gate record in `docs/decisions/`; `SPEC.md` §15 owns those numbers.
The open dependency it sits behind — a running game to drive — is `ROADMAP.md`'s
own open-questions surface, and this document is the design that will be ready the
moment that dependency is met.
