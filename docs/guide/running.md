# The lifecycle, events, sides and packets

## The lifecycle

`LifecycleState` is the vocabulary, in order: `Discovered`, `Validated`,
`Resolved`, `Loaded`, `Constructed`, `Registered`, `Running`, `Stopping`,
`Stopped`, and `Failed`, which is reachable from every non-terminal state. The
successful path advances one state at a time. Your `enter` is called for each
step; do work in the phase it belongs to, and refuse in the phase where the
problem is real.

Two of them are where most mods do something: `Constructed` is where a mod exists
and its configuration and services have been settled, and `Registered` is after
the registration barrier closed and static content is committed. Nothing is
delivered to a mod before the set reaches `Running`.

## Events

`GameEvent` is a closed set — `ServerTick`, `ClientTick`, `ResourcesReloaded` —
and closed deliberately: an unknown event would name something no version adapter
has resolved a hook for. Implement `observe` and `match`; ignoring an event costs
one arm.

The loader broadcasts every event to the mods that load on the running side, in
resolved load order, so a mod that cares about ordering sees the same order every
run. A mod that fails on an event ends that broadcast, and the mods after it
never see it.

Host integrations use one profiler-taking loader entry point per operation:
`broadcast`, `deliver_command`, and `deliver_packet`. A halted set has a distinct
`DispatchRefused::Halted` refusal rather than an inactive-recipient refusal.

An event that cannot happen on the running side is refused with
`DispatchRefused::EventWrongSide` before any mod receives it. A client tick on a
dedicated server is an invalid broadcast, not an empty audience. `ClientTick`
arms in a server-side mod are therefore dead code rather than rare code, and a
mod counting ticks has to say which side it counts on.

**There is no hooks module and you never name a hook.** A hook is a host-side
target in a Minecraft version's data, resolved against that version's mappings.
`GameEvent` is what a hook has become by the time it reaches a mod, which is why
a mod is not affected by one moving between game versions.

## Commands

Claim a `ContentKind::Command` entry in the registration window and implement
`run_command`. An invocation reaches only the mod that claimed the root literal,
so you are never asked to run a command you do not own and never have to check.
A command's arguments are text a player typed and nothing has interpreted them
for you.
An invocation's `side` is `Side::IntegratedServer` or `Side::DedicatedServer`, the
world side that executes it. The loader rejects any other value.

Two mods cannot claim the same command root, and a command owned by a mod that
does not load on the executing side is refused by name rather than swallowed.
On a client distribution, integrated-server commands require the owner's
`sides` to include `IntegratedServer`; `Client` alone does not authorize them.

## Sides

Your manifest's `sides` covers a mod that is entirely client-side or entirely
server-side: the loader parks it before any of its code runs elsewhere. It does
not cover the ordinary case, which is a mod that loads everywhere and has
client-only *parts*.

`ClientOnly` is for those parts. Both halves of it — building and reading — take
the side, so **your mod has to carry its own side in its struct.** A mod is told
which side it is on by the host that constructed it and cannot work it out for
itself, and `ClientOnly::get` asks again rather than trusting what the
constructor was told, so that a constructor which was given the wrong side fails
at the accessor instead of silently holding client state on a server. That means
a field, not a local:

```rust
pub struct CopperLanterns {
    side: Side,
    report: ClientOnly<LatestReport>,
}

impl CopperLanterns {
    fn new(side: Side) -> Self {
        Self {
            side,
            // On a server side the closure is never called, so the value is not
            // built rather than being built and hidden.
            report: ClientOnly::built_on(side, LatestReport::default),
        }
    }

    fn note(&mut self, line: &str) {
        if let Ok(report) = self.report.get_mut(self.side) {
            report.lines.push(line.to_owned());
        }
    }
}
```

Reaching for it on a server side returns `ClientOnlyError::WrongSide`, which is
yours to convert: a mod that treats it as fatal returns `ModError::Refused`, and
one with nothing to report — the `if let Ok` above — carries on. On the client,
a value that was never built returns `ClientOnlyError::NotBuilt` instead.
`is_present()` answers whether the value was built at all, which is what a test asserts on.

Client-only means state a mod keeps *because* it is on the client — what a
`to-client` packet last carried, what a client tick last counted. **Drawing is
not one of them.** Ravel gives a mod no rendering path; every
screen Ravel owns is declared in data and drawn in Java, and nothing extends that
to a mod's own.

## Packets

**There is no outbound send.** You can declare, encode, decode and test a packet,
and `encode_from` sends nothing on any version. On Minecraft 1.12.2, an inbound
vanilla custom payload reaches the loader's `PacketDelivery` path on the owning
thread. The loader refuses a wrong side, a direction that may not be received
here, or an absent or non-reacting owner before `receive_packet` runs. Other
adapters carry no inbound packets yet. See [What does not work yet](limits.md).

Declare a `PacketContract` — an id in your namespace, a `Direction`, a version —
and implement `Packet` for the value that will travel under it. `encode` and
`decode` own the framing, so you write and read your own fields and can never
write one framing and read another.

A `PacketContract` is a triple rather than a name, so unlike a `RegistryKey`, a
`TagId` or a `ServiceId` it has no `to_string()`. Read the parts: `id()`, which
is a `PacketId` and does print, `direction()` and `version()`.

A payload is refused rather than misread in every way it can be wrong, and
`WireError` is the whole of those refusals. Its variants are
`WireError::Truncated`, the buffer ending inside a field;
`WireError::TooLarge`, a field exceeding its contract's size ceiling;
`WireError::InvalidValue`, a readable field whose value its domain refuses;
`WireError::TrailingBytes`, the body ending with bytes left over;
`WireError::NotUtf8`, a length-prefixed text field that is not UTF-8;
`WireError::NotABool`, a boolean byte that is neither zero nor one;
`WireError::ContentId`, a text field that is not a legal namespaced content identifier;
`WireError::Identity`, a payload naming one contract and read as another; and
`WireError::Version`, a payload written under another version of the contract.
Every one is named here rather than summarised, because an author writing an
exhaustive `match` needs the set and not a description of it; the `network`
module's rustdoc is where what each variant carries is written down.

Arriving on a side the direction does not permit is a different refusal in a
different type. `DeliveryRefused::WrongSide` is what `Delivery::decode` returns,
and it returns it before any decoding runs, so none of your reading code has
executed when it does.

Raise the contract version whenever you change what you send. The other end will
then refuse instead of reading new fields out of an old buffer.
