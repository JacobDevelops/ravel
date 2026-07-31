//! What holds the loader while the game is running.
//!
//! `ravel-loader` can broadcast an event and route a command, but both take the
//! mod values beside the lifecycle machine rather than inside it, so something
//! in the process has to own both. Nothing did: `broadcast` and `route` had no
//! caller outside their own tests.
//!
//! This is that owner, and it is deliberately not a new mechanism. [`dispatch`]
//! already holds `Box<dyn HookObserver>` values in a `'static` registry and
//! already shows every hook invocation to each of them, so the join is one more
//! observer rather than a second path beside the first. That also answers the
//! lifetime question by construction: the registry is `'static`, this value owns
//! its machine and its mods outright, and nothing it touches borrows from the
//! JVM. ADG-4 is satisfied for the same reason — the projection below copies
//! only owned data out of a [`HookValue`], and the slice is dropped when the
//! call returns, so nothing Ravel keeps can go stale.
//!
//! # A mod cannot re-enter this, and that is what makes the lock safe
//!
//! [`LoaderRuntime::observe`] holds a non-reentrant [`Mutex`] while mod code
//! runs, so the question that decides whether this design is sound is whether a
//! mod can cause a second hook to fire on the same thread. It cannot, and not by
//! convention: **mod code has no way to call into the JVM at all.**
//!
//! - `ravel-host-api` depends on `ravel-core` and nothing else, and the SDK
//!   reaches only configuration, data, network and save. Neither can name `jni`,
//!   which only `ravel-minecraft` and this crate depend on.
//!   `docs/architecture/dependency-rules.md` is what holds that shape, and cargo
//!   is what enforces it.
//! - A mod is never shown a [`HookValue`]. The projection below turns one into a
//!   `GameEvent` or a `CommandInvocation` — a fieldless enum and owned text —
//!   before any mod code runs, so ADG-4's borrowed handle does not reach a mod
//!   even as something to look at.
//! - `JavaHandle`'s whole surface is the constructor the generated exports call
//!   and the declared class name. Nothing on it calls anything.
//! - The one Rust-to-JVM call in the workspace is `guard::text`, and
//!   `guard::enter` runs it while collecting values, before `dispatch::deliver`
//!   is reached. Nothing calls into the JVM while this lock is held.
//!
//! A hook fires when the game runs a rewritten method. A mod that cannot reach
//! the game cannot make one run, so there is no path back into `observe` from
//! inside it. **The day something gives Rust a way to call Java, this paragraph
//! stops being true and this lock becomes a deadlock in a running game.** That
//! is the sentence to find before adding one.
//!
//! Two game threads do reach this — a client tick and an integrated server tick
//! are not the same thread — but that is contention rather than reentrancy, and
//! a mutex is what contention is for. One waits for the other.
//!
//! [`dispatch`]: crate::dispatch

use std::sync::{Arc, Mutex, PoisonError};

use ravel_core::{
    ContentId, ContentKind, DiagnosticMessage, FailureAttribution, FailureEnvironment, RegistryKey,
    StructuredFailure,
};
use ravel_host_api::{CommandInvocation, GameEvent, HostMod};
use ravel_loader::{Delivery, LifecycleMachine, ModSurface, broadcast_profiled, route_profiled};
use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};
use ravel_observability::{Profiler, ProfilingReport};

use crate::guard::HookError;

/// Which reviewed hook stands for which event.
///
/// Two hooks can mean one event and that is not a duplicate: an integrated
/// server and a dedicated one both end a server tick, and a client reloads its
/// own packs while a server reloads its resources. The mod is told what
/// happened, not which class it happened in.
const EVENTS: [(&str, GameEvent); 5] = [
    ("server.tick", GameEvent::ServerTick),
    ("server.tick-integrated", GameEvent::ServerTick),
    ("client.tick", GameEvent::ClientTick),
    (
        "resource.load-server-resources",
        GameEvent::ResourcesReloaded,
    ),
    ("resource.reload-client-packs", GameEvent::ResourcesReloaded),
];

/// The reviewed hook that carries a command a player typed.
const COMMAND: &str = "command.perform-prefixed";

/// The loader, and the mods it drives, for as long as the process runs.
pub struct LoaderRuntime {
    inner: Mutex<Inner>,
}

struct Inner {
    machine: LifecycleMachine,
    mods: Vec<Box<dyn HostMod>>,
    /// The collectors, off unless the launch asked for them.
    profiler: Profiler,
}

/// What one hook invocation asks the loader to do.
enum Work {
    Event(GameEvent),
    Command(CommandInvocation),
}

impl LoaderRuntime {
    /// Holds `machine` and `mods` together for the rest of the run.
    ///
    /// The machine arrives already driven. This owns the set and shows it what
    /// the game does; running the pipeline that produced it is the caller's, and
    /// a runtime that also drove the lifecycle would be two things.
    #[must_use]
    pub fn new(machine: LifecycleMachine, mods: Vec<Box<dyn HostMod>>) -> Self {
        Self::profiling(machine, mods, Profiler::off())
    }

    /// The same runtime with the collectors the launch asked for.
    ///
    /// Separate from [`Self::new`] because profiling is opt-in and the default
    /// is what almost every caller wants. `crate::profiling` reads the switch
    /// once, here, and never again: the flag is a plain field rather than an
    /// atomic precisely because nothing toggles it while the game runs, which
    /// is what keeps the off path to one branch.
    #[must_use]
    pub fn profiling(
        machine: LifecycleMachine,
        mods: Vec<Box<dyn HostMod>>,
        profiler: Profiler,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                machine,
                mods,
                profiler,
            }),
        }
    }

    /// Builds the profiling report from the collectors as they stand now.
    ///
    /// Read live and under the same short lock the mod surface takes, for the
    /// same reason: a report assembled at launch would go on saying a mod costs
    /// nothing after it started costing something.
    ///
    /// The registry footprint is read out of the machine here rather than
    /// accumulated by a collector, which is why it is exact.
    #[must_use]
    pub(crate) fn profiling_report(&self) -> ProfilingReport {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let claims = inner
            .machine
            .order()
            .map(|mod_id| (mod_id.clone(), inner.machine.registry_claims(mod_id)))
            .collect::<Vec<_>>();
        for (mod_id, count) in claims {
            inner
                .profiler
                .registry_claims(&mod_id, count.try_into().unwrap_or(u64::MAX));
        }
        inner.profiler.report()
    }

    /// Builds the mod list's model from the set as it stands right now.
    ///
    /// The lock is taken to walk the machine and released with the value, so a
    /// screen opening costs one short contention with the tick rather than
    /// holding anything while it draws. Reading live rather than from a snapshot
    /// is what stops a diagnostics screen reporting a set as healthy after
    /// something in it has failed.
    ///
    /// A poisoned lock is recovered for the same reason [`Self::observe`]
    /// recovers one: the panic that poisoned it happened in a mod's delivery,
    /// before anything wrote the machine, and refusing here would leave a player
    /// with no way to see why their mods are broken at exactly the moment they
    /// need it.
    #[must_use]
    pub(crate) fn surface(&self) -> ModSurface {
        ModSurface::of(
            &self
                .inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .machine,
        )
    }
}

#[cfg(test)]
impl LoaderRuntime {
    /// Fails one mod on the held machine, as a delivery would.
    ///
    /// Exists so a test can change the set *after* something has already read
    /// it, which is the only way to tell a live read from a snapshot.
    pub(crate) fn fail_while_running(
        &self,
        mod_id: &ravel_core::ModId,
        attribution: FailureAttribution,
    ) {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .machine
            .fail(mod_id, attribution)
            .expect("a mod the machine holds can fail");
    }
}

/// A runtime that both the hook registry and the mod list screen hold.
///
/// The registry takes ownership of a `Box<dyn HookObserver>` and the screen
/// needs the same object rather than a second one, so the shared handle is what
/// is installed. The newtype exists only because neither the trait nor `Arc` is
/// this crate's to implement the other for.
struct Shared(Arc<LoaderRuntime>);

impl HookObserver for Shared {
    fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome {
        self.0.observe(target, values)
    }
}

/// Installs a driven mod set so every hook invocation reaches it.
///
/// # Errors
///
/// Returns [`HookError::Unavailable`] when the observer registry cannot be
/// written, which happens only after a previous holder panicked inside it.
pub fn install(machine: LifecycleMachine, mods: Vec<Box<dyn HostMod>>) -> Result<(), HookError> {
    let runtime = Arc::new(LoaderRuntime::profiling(
        machine,
        mods,
        Profiler::from_switch(crate::profiling::requested()),
    ));
    // One installation point for every reader, so there is no launch in which
    // hooks reach the mod set but a screen finds nothing.
    crate::mods::install(Arc::clone(&runtime));
    crate::profiling::install(Arc::clone(&runtime));
    crate::dispatch::register(Box::new(Shared(runtime)))
}

impl HookObserver for LoaderRuntime {
    /// Shows one hook invocation to the mod set.
    ///
    /// This never vetoes. A veto throws inside the game, and every hook this
    /// projects is one the game calls on its own schedule — a tick that threw
    /// because a mod declined would take the game down for a fault the loader
    /// already has a state for. A mod that fails is failed on the machine
    /// instead, which stops it receiving anything further and leaves the reason
    /// where a report reads it.
    fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome {
        // The lock is poisoned exactly when a previous delivery unwound, which
        // means a mod panicked and the guard caught it at the JNI boundary.
        // Recovering rather than propagating is deliberate and is not a shrug: a
        // panic can only originate inside `broadcast` or `route`, which is
        // before anything below mutates the machine, so the machine is never
        // left half-written by one. Refusing the lock instead would turn one
        // mod's bug into every later hook failing, silently, for the rest of the
        // run.
        //
        // What this does not do is attribute the panic. The mod that panicked is
        // still `Running` and will be shown the next tick as well. Catching a
        // panic at the mod boundary rather than at the JNI one is ADG-2's
        // execution-model question and is not settled here.
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(work) = inner.work(target, values) {
            inner.deliver(&work);
        }
        HookOutcome::Proceed
    }
}

impl Inner {
    /// Reads what the game just did out of one hook invocation.
    fn work(&self, target: &HookTarget, values: &[HookValue<'_>]) -> Option<Work> {
        let id = target.id().as_str();
        if let Some((_, event)) = EVENTS.iter().find(|(hook, _)| *hook == id) {
            return Some(Work::Event(*event));
        }
        if id != COMMAND {
            return None;
        }
        // The typed line is found by kind rather than by position. A descriptor
        // gains and loses parameters between versions, and the one thing that
        // does not change is that exactly one of them is the text.
        let typed = values.iter().find_map(|value| match value {
            HookValue::Text(text) => Some(text.as_str()),
            _ => None,
        })?;
        self.command(typed)
    }

    /// Turns a typed command line into an invocation, when it names one.
    ///
    /// The root literal has to be a whole content id, because that is what a mod
    /// claimed: `Registrar::claim` keys the table on a [`RegistryKey`], whose id
    /// carries the owning mod's namespace. A bare literal names no claim that
    /// could exist, so `/give` and every other vanilla command falls through
    /// here untouched rather than being guessed at.
    ///
    /// Whether a player should be able to type an unqualified root literal is a
    /// question about command naming that nothing has answered. This does not
    /// answer it: it declines the ones it cannot resolve rather than inventing a
    /// lookup the claim table does not support.
    fn command(&self, typed: &str) -> Option<Work> {
        let (literal, rest) = typed.split_once(char::is_whitespace).unwrap_or((typed, ""));
        let id = literal.parse::<ContentId>().ok()?;
        Some(Work::Command(CommandInvocation::new(
            RegistryKey::new(ContentKind::Command, id),
            self.machine.side(),
            rest.split_whitespace().map(str::to_owned).collect(),
        )))
    }

    /// Delivers one piece of work and records whatever failed doing it.
    fn deliver(&mut self, work: &Work) {
        // The borrows end with this block, so recording a failure can take the
        // machine mutably afterwards. `broadcast` wants a slice of references
        // and the runtime owns boxes, so the slice is built per delivery; that
        // is one allocation per tick, and removing it means changing what
        // `ravel-loader` accepts rather than what this holds.
        let delivered = {
            let mut borrowed: Vec<&mut dyn HostMod> =
                self.mods.iter_mut().map(|host| &mut **host).collect();
            match work {
                Work::Event(event) => {
                    broadcast_profiled(&self.machine, &mut borrowed, *event, &mut self.profiler)
                }
                Work::Command(invocation) => {
                    route_profiled(&self.machine, &mut borrowed, invocation, &mut self.profiler)
                }
            }
        };

        // A refusal means the delivery reached nobody and changed nothing: the
        // set is not running yet, the command belongs to no mod here, or the
        // owner does not load on this side. None of those is a mod's failure and
        // none of them is news — a client tick before the set starts is the
        // ordinary case, not an incident.
        if let Ok(delivery) = delivered {
            self.record(&delivery);
        }
    }

    /// Fails the mod that ended a delivery, so it stops receiving.
    fn record(&mut self, delivery: &Delivery) {
        let Some(failure) = delivery.failure() else {
            return;
        };
        let attribution = FailureAttribution {
            mod_id: Some(failure.mod_id().clone()),
            version: None,
            dependency_chain: Vec::new(),
            state: failure.lifecycle_state(),
            kind: failure.failure_kind(),
            surface: None,
            environment: environment(),
            // Raw, and redacted by the report surface on the way in. A mod's own
            // words are the one thing in this path nobody in this workspace
            // wrote, and `HostError` already says which failures quote them.
            message: DiagnosticMessage::raw(failure.to_string()),
        };
        // A mod that has already ended cannot fail again, and the machine says
        // so rather than being asked twice.
        let _ = self.machine.fail(failure.mod_id(), attribution);
    }
}

/// Where this process is running, for a failure that has to say so.
fn environment() -> FailureEnvironment {
    FailureEnvironment {
        ravel: env!("CARGO_PKG_VERSION")
            .parse()
            .expect("every crate in this workspace carries a semver version"),
        minecraft: ravel_minecraft_adapter::pinned::MINECRAFT_VERSION.to_owned(),
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex};

    use ravel_core::{
        ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet,
        StructuredFailure,
    };
    use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod, Registrar};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
    use ravel_minecraft::{HookId, HookKind, HookObserver, HookOutcome, HookTarget, HookValue};

    use super::{COMMAND, EVENTS, LoaderRuntime, environment};

    const MOD: &str = "example_mod";

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    fn key(literal: &str) -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            format!("{MOD}:{literal}")
                .parse::<ContentId>()
                .expect("a legal content id"),
        )
    }

    /// A hook target standing in for a reviewed one.
    ///
    /// Built here rather than taken from the generated table because the
    /// projection is keyed on the hook id alone. A test that read the real table
    /// would be asserting the inventory, and the test below that *should* assert
    /// the inventory does exactly that and nothing else.
    fn target(hook: &'static str) -> HookTarget {
        HookTarget::new(
            HookId::new(hook),
            "net/minecraft/Ignored",
            "ignored",
            "()V",
            HookKind::InstanceMethod,
        )
    }

    /// What the held mod was shown, readable from outside the runtime.
    ///
    /// Shared through an `Arc` rather than recovered from the trait object,
    /// because the runtime owns its mods and `HostMod` is not downcastable. This
    /// keeps the runtime the only owner of the mod while still letting a test
    /// see what reached it.
    #[derive(Debug, Default)]
    struct Log {
        seen: Vec<GameEvent>,
        ran: Vec<CommandInvocation>,
    }

    /// What the held mod does when it is shown an event.
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Behaviour {
        Records,
        Fails,
        Panics,
    }

    struct Recording {
        id: ModId,
        log: Arc<Mutex<Log>>,
        behaviour: Behaviour,
        claims: Vec<RegistryKey>,
    }

    impl HostMod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
            for claim in &self.claims {
                registrar
                    .claim(claim.clone())
                    .map_err(|refused| HostError::Refused {
                        mod_id: self.id.clone(),
                        state: LifecycleState::Constructed,
                        detail: refused.to_string(),
                    })?;
            }
            Ok(())
        }

        fn observe(&mut self, event: GameEvent) -> Result<(), HostError> {
            match self.behaviour {
                Behaviour::Records => {
                    self.log.lock().expect("nothing panicked").seen.push(event);
                    Ok(())
                }
                Behaviour::Fails => Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Running,
                    detail: "this mod declines every event".to_owned(),
                }),
                Behaviour::Panics => panic!("this mod is broken"),
            }
        }

        fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), HostError> {
            self.log
                .lock()
                .expect("nothing panicked")
                .ran
                .push(invocation.clone());
            Ok(())
        }
    }

    /// A runtime holding one running mod, with a view of what that mod is shown.
    fn running(
        side: Side,
        claims: Vec<RegistryKey>,
        behaviour: Behaviour,
    ) -> (LoaderRuntime, Arc<Mutex<Log>>) {
        let (machine, mods, log) = parts(side, claims, behaviour);
        (LoaderRuntime::new(machine, mods), log)
    }

    /// A driven set, before anything owns it.
    fn parts(
        side: Side,
        claims: Vec<RegistryKey>,
        behaviour: Behaviour,
    ) -> (LifecycleMachine, Vec<Box<dyn HostMod>>, Arc<Mutex<Log>>) {
        let log = Arc::new(Mutex::new(Log::default()));
        let mut host = Recording {
            id: id(MOD),
            log: Arc::clone(&log),
            behaviour,
            claims,
        };

        let entry = LifecycleEntry {
            mod_id: id(MOD),
            sides: SideSet::new([side]).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        };
        let mut machine = LifecycleMachine::new(side, vec![entry]).expect("a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        {
            let mut window = machine
                .registration_window(&id(MOD))
                .expect("the mod is in the plan");
            host.register(&mut window).expect("nothing conflicts");
        }
        for state in [LifecycleState::Registered, LifecycleState::Running] {
            machine.advance_all(state).expect("a legal step");
        }

        (machine, vec![Box::new(host)], log)
    }

    #[test]
    fn a_mod_that_panicked_does_not_silence_every_later_hook() {
        let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Panics);

        // The panic is caught here the way the guard catches it at the JNI
        // boundary in a real process. Either way it poisons the lock, and the
        // second delivery is the one that shows what that costs.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let deliveries = [(); 2].map(|()| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                runtime.observe(&target("server.tick"), &[]);
            }))
        });
        std::panic::set_hook(previous);

        for (which, caught) in ["first", "second"].into_iter().zip(deliveries) {
            let raised = caught.expect_err("the mod under test panics on every event");
            let message = raised
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| raised.downcast_ref::<String>().map(String::as_str))
                .unwrap_or_default();
            assert_eq!(
                message, "this mod is broken",
                "the {which} delivery reached the mod; refusing a poisoned lock would \
                 have raised about the lock instead, for the rest of the run"
            );
        }

        // The machine is intact, because a panic can only originate before
        // anything in `deliver` mutates it. The mod is still `Running`, which is
        // the gap the module doc names: a panic is not attributed the way a
        // refusal is.
        let inner = runtime
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(
            inner.machine.state(&id(MOD)).expect("known mod"),
            LifecycleState::Running
        );
    }

    #[test]
    fn an_installed_set_receives_what_the_observer_registry_delivers() {
        let (machine, mods, log) = parts(Side::DedicatedServer, Vec::new(), Behaviour::Records);

        super::install(machine, mods).expect("the registry accepts an observer");
        crate::dispatch::deliver(&target("server.tick"), &[]).expect("nothing here vetoes");

        assert_eq!(
            log.lock().expect("nothing panicked").seen,
            [GameEvent::ServerTick],
            "the join is the registry the guard already delivers through, not a second path"
        );
    }

    #[test]
    fn every_tick_hook_reaches_the_mod_as_the_event_it_stands_for() {
        for (hook, expected) in EVENTS {
            // A client tick has no meaning on a dedicated server, so each hook
            // is driven on a side its event can happen on.
            let side = if expected == GameEvent::ClientTick {
                Side::Client
            } else {
                Side::DedicatedServer
            };
            let (runtime, log) = running(side, Vec::new(), Behaviour::Records);

            let outcome = runtime.observe(&target(hook), &[]);

            assert_eq!(outcome, HookOutcome::Proceed, "{hook}");
            assert_eq!(
                log.lock().expect("nothing panicked").seen,
                [expected],
                "{hook} should reach the mod as {expected:?}"
            );
        }
    }

    #[test]
    fn two_hooks_can_mean_one_event_and_the_mod_is_told_the_event() {
        let (runtime, log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Records);

        runtime.observe(&target("server.tick"), &[]);
        runtime.observe(&target("server.tick-integrated"), &[]);

        assert_eq!(
            log.lock().expect("nothing panicked").seen,
            [GameEvent::ServerTick, GameEvent::ServerTick],
            "a mod is told a server ticked, not which class ticked it"
        );
    }

    #[test]
    fn a_hook_the_loader_has_no_meaning_for_delivers_nothing_and_proceeds() {
        let (runtime, log) = running(Side::Client, Vec::new(), Behaviour::Records);

        let outcome = runtime.observe(&target("chat.filter-text"), &[HookValue::Int(65)]);

        assert_eq!(outcome, HookOutcome::Proceed);
        let seen = log.lock().expect("nothing panicked");
        assert!(seen.seen.is_empty() && seen.ran.is_empty());
    }

    #[test]
    fn a_typed_command_reaches_the_mod_that_claimed_its_key_with_its_arguments() {
        let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

        let outcome = runtime.observe(
            &target(COMMAND),
            &[HookValue::Text(format!("{MOD}:home set  spawn"))],
        );

        assert_eq!(outcome, HookOutcome::Proceed);
        let ran = &log.lock().expect("nothing panicked").ran;
        assert_eq!(ran.len(), 1);
        assert_eq!(ran[0].key, key("home"));
        assert_eq!(ran[0].side, Side::Client);
        assert_eq!(
            ran[0].arguments,
            ["set", "spawn"],
            "arguments are split on whitespace and not otherwise interpreted"
        );
    }

    #[test]
    fn a_command_with_no_arguments_reaches_its_owner_carrying_none() {
        let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

        runtime.observe(&target(COMMAND), &[HookValue::Text(format!("{MOD}:home"))]);

        let ran = &log.lock().expect("nothing panicked").ran;
        assert_eq!(ran.len(), 1);
        assert!(ran[0].arguments.is_empty());
    }

    #[test]
    fn a_command_no_mod_claimed_is_left_alone_rather_than_guessed_at() {
        let (runtime, log) = running(Side::Client, Vec::new(), Behaviour::Records);

        for typed in ["give @s stone", "minecraft:give @s stone", ""] {
            assert_eq!(
                runtime.observe(&target(COMMAND), &[HookValue::Text(typed.to_owned())]),
                HookOutcome::Proceed,
                "`{typed}` is not Ravel's to run"
            );
        }

        assert!(
            log.lock().expect("nothing panicked").ran.is_empty(),
            "an unqualified literal names no claim that could exist, and an \
             unclaimed one names no mod here"
        );
    }

    #[test]
    fn the_typed_line_is_found_by_kind_rather_than_by_position() {
        let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

        // The command hook's own descriptor puts a reference before the text.
        runtime.observe(
            &target(COMMAND),
            &[HookValue::Int(1), HookValue::Text(format!("{MOD}:home"))],
        );

        assert_eq!(log.lock().expect("nothing panicked").ran.len(), 1);
    }

    #[test]
    fn a_command_hook_carrying_no_text_reaches_nobody() {
        let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

        let outcome = runtime.observe(&target(COMMAND), &[HookValue::Int(1)]);

        assert_eq!(outcome, HookOutcome::Proceed);
        assert!(log.lock().expect("nothing panicked").ran.is_empty());
    }

    #[test]
    fn a_mod_that_fails_while_reacting_is_failed_rather_than_thrown_at_the_game() {
        let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Fails);

        let outcome = runtime.observe(&target("server.tick"), &[]);

        assert_eq!(
            outcome,
            HookOutcome::Proceed,
            "a mod declining an event must not take the game down"
        );

        let inner = runtime.inner.lock().expect("nothing panicked");
        assert_eq!(
            inner.machine.state(&id(MOD)).expect("known mod"),
            LifecycleState::Failed
        );
        let attributed = inner
            .machine
            .failure(&id(MOD))
            .expect("known mod")
            .expect("the failure was recorded");
        assert_eq!(attributed.mod_id.as_ref(), Some(&id(MOD)));
        assert_eq!(attributed.state, LifecycleState::Running);
        assert_eq!(attributed.kind, None);
        assert!(attributed.message.text().contains("declines every event"));
    }

    #[test]
    fn a_failed_mod_is_failed_once_however_many_ticks_follow() {
        let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Fails);

        for _ in 0..3 {
            assert_eq!(
                runtime.observe(&target("server.tick"), &[]),
                HookOutcome::Proceed
            );
        }

        let inner = runtime.inner.lock().expect("nothing panicked");
        let failures = inner
            .machine
            .transcript()
            .iter()
            .filter(|event| event.state == LifecycleState::Failed)
            .count();
        assert_eq!(
            failures, 1,
            "a mod that has already ended cannot fail again, and the machine says so"
        );
    }

    #[test]
    fn every_hook_the_runtime_projects_is_one_the_reviewed_inventory_carries() {
        use ravel_minecraft::VersionAdapter;
        use ravel_minecraft_adapter::PinnedAdapter;

        let reviewed: Vec<&str> = PinnedAdapter
            .targets()
            .iter()
            .map(|target| target.id().as_str())
            .collect();

        for (hook, _) in EVENTS {
            assert!(reviewed.contains(&hook), "{hook} is not a reviewed hook");
        }
        assert!(
            reviewed.contains(&COMMAND),
            "{COMMAND} is not a reviewed hook"
        );
    }

    #[test]
    fn the_environment_a_failure_reports_is_the_process_it_happened_in() {
        let environment = environment();

        assert_eq!(environment.platform, std::env::consts::OS);
        assert_eq!(environment.architecture, std::env::consts::ARCH);
        assert_eq!(
            environment.minecraft,
            ravel_minecraft_adapter::pinned::MINECRAFT_VERSION
        );
    }

    #[test]
    fn a_host_failure_carries_the_state_the_mod_named_and_no_dependency_outcome() {
        let failure = HostError::Refused {
            mod_id: id(MOD),
            state: LifecycleState::Running,
            detail: "no".to_owned(),
        };

        assert_eq!(failure.lifecycle_state(), LifecycleState::Running);
        assert_eq!(failure.failure_kind(), None);
        assert!(
            failure.quotes_untrusted_text(),
            "the attribution carries a mod's own words and the report surface redacts them"
        );
    }
}
