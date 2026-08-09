//! The opt-in profiling switch and the report crossing.
//!
//! `ROADMAP.md`'s M3-W6 row asks for collectors and a profiling UI or report,
//! for per-mod attribution whose accuracy is explicit, and for unsupported
//! metrics to be explicit too. The collectors and both statements live in
//! `ravel-observability`; this module is the two places they touch the JVM.
//!
//! # The switch crosses once, at premain
//!
//! Profiling is off unless a launch asks for it, and the asking is
//! `-Dravel.profiling=true` — the shape `-Dravel.report.loader=true` already
//! set for the measurement switch `evidence/m3/class-loader-identity.json` was
//! taken under. Java reads the property because Java is where a system property
//! is, and hands the answer across once before any mod exists. ADG-1 gives Java
//! a way to call Rust and nothing the other way, so this is the direction the
//! switch can travel in.
//!
//! Reading it once is what makes the off path cheap. The collectors hold a
//! plain flag rather than an atomic, because nothing toggles it while the game
//! runs; `evidence/m3/profiling-overhead.json` measures what that costs a tick
//! and is the budget it is held to.
//!
//! # The report crosses like every other screen
//!
//! ADG-6 draws every screen Ravel owns in Java and Ravel renders nothing from
//! Rust, and M3-W5's mod list is named there as the same mechanism rather than
//! a second one. That holds a third time: one batched read on the framing in
//! [`crate::wire`], carrying numbers and the sentences that qualify them, and
//! `RavelProfilingScreen` decides what a player sees.
//!
//! The unsupported metrics cross with the rest. A screen that simply omitted
//! them would show a player a shorter list and no reason, which is the silence
//! the row exists to end — an absent metric and one that measured zero look
//! identical.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};
use ravel_observability::{Metric, ProfilingReport};

use crate::guard::{HookError, text};
use crate::runtime::LoaderRuntime;
use crate::wire::{VERSION, count, field, flag};

/// The system property a launch turns the collectors on with.
pub(crate) const SWITCH: &str = "-Dravel.profiling=true";

/// Whether this launch asked for profiling.
///
/// A static rather than a field, because the answer arrives at premain and the
/// runtime it configures is installed much later. Relaxed ordering is enough:
/// the write happens on the premain thread before any game thread exists, and
/// the JVM's own thread start is the barrier between them.
static REQUESTED: AtomicBool = AtomicBool::new(false);

/// The installed runtime, or nothing before one is installed.
static RUNTIME: Mutex<Option<Arc<LoaderRuntime>>> = Mutex::new(None);

/// Whether this launch asked for the collectors.
pub(crate) fn requested() -> bool {
    REQUESTED.load(Ordering::Relaxed)
}

/// Makes the collectors readable by the profiling screen.
pub(crate) fn install(runtime: Arc<LoaderRuntime>) {
    *lock() = Some(runtime);
}

/// Takes the store, recovering it from a previous panic.
///
/// Recovered rather than refused for the reason `crate::mods` recovers one:
/// nothing here mutates the runtime, so a poisoned lock cannot mean a
/// half-built one, and refusing would take the report away at the moment
/// something has gone wrong enough to want it.
fn lock() -> std::sync::MutexGuard<'static, Option<Arc<LoaderRuntime>>> {
    RUNTIME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The whole report, as one payload.
///
/// Returns nothing before a set is installed, which is a launch that has not
/// got far enough to have mods to profile rather than a failure.
fn document() -> Option<String> {
    document_of(lock().clone().as_deref())
}

/// The report a screen reads from one runtime, or nothing without one.
///
/// Separate from [`document`] so the absent case is a value a caller passes
/// rather than the state of the process's own store. A test that had to empty
/// that store to see the case would be asserting against every other test in
/// the process, and the plain runner shares one.
fn document_of(runtime: Option<&LoaderRuntime>) -> Option<String> {
    Some(render(&runtime?.profiling_report()))
}

/// Writes one report onto the wire.
///
/// The numbers and the sentences that qualify them travel together. Sending the
/// numbers alone would hand a screen a column of nanoseconds with nothing
/// saying that a mod is charged for host work it called into, which is the
/// difference between a profiler and a scoreboard.
fn render(report: &ProfilingReport) -> String {
    let mut out = String::new();
    field(&mut out, VERSION);
    flag(&mut out, report.is_on());
    field(&mut out, SWITCH);
    count(
        &mut out,
        usize::try_from(report.ticks()).unwrap_or(usize::MAX),
    );
    count(
        &mut out,
        usize::try_from(report.slow_callback().as_micros()).unwrap_or(usize::MAX),
    );

    count(&mut out, report.mods().len());
    for entry in report.mods() {
        field(&mut out, entry.mod_id.as_str());
        // Nanoseconds as a field rather than a count: a long session's total
        // outgrows what a screen reads back as an `int`, and a field lets the
        // screen decide what to do about that rather than losing the number
        // here.
        field(&mut out, &entry.cost.inclusive_nanos.to_string());
        field(&mut out, &entry.cost.hook_calls().to_string());
        field(&mut out, &entry.cost.event_calls.to_string());
        field(&mut out, &entry.cost.command_calls.to_string());
        field(&mut out, &entry.cost.slow_callbacks.to_string());
        field(&mut out, &entry.cost.registry_claims.to_string());
    }

    let accuracy = ProfilingReport::attribution_accuracy();
    count(&mut out, accuracy.len());
    for sentence in accuracy {
        field(&mut out, sentence);
    }

    // Every metric, not only the unsupported ones. A screen that listed what is
    // missing without what is present cannot tell a player which of the two a
    // number they are looking at came from.
    count(&mut out, Metric::ALL.len());
    for metric in Metric::ALL {
        let support = metric.support();
        field(&mut out, metric.name());
        field(&mut out, metric.unit());
        field(&mut out, support.name());
        field(&mut out, support.detail().unwrap_or(""));
    }

    out
}

/// Whether a launch asked for the collectors, given the property it set.
///
/// [`None`] is a launch that set no property, which is almost every launch, and
/// it is a distinct value rather than an empty string on purpose: the JVM hands
/// a missing property across as `null`, and a `null` read as text is an error
/// rather than a word this could compare.
fn requested_by(property: Option<&str>) -> bool {
    property == Some("true")
}

/// Records whether this launch asked for the collectors.
///
/// Takes the property's value rather than a boolean, because that is what
/// `System.getProperty` returns and turning it into a decision on one side or
/// the other is the same work. An absent property is an absent switch.
///
/// The absent case is the one that matters, and it is why the null is tested
/// before the text is read rather than after. `premain` passes
/// `System.getProperty("ravel.profiling")` straight across, so an ordinary
/// launch — one that never asked for profiling — hands this `null`. Reading
/// that as a string throws, the throw leaves `premain`, and the JVM treats a
/// premain that threw as fatal: the client never starts. Getting this wrong
/// does not switch profiling on by accident, it stops the game booting.
///
/// # Panics
///
/// Does not panic. The guard around the body turns a panic into a thrown
/// `RuntimeException` rather than an unwind into the JVM.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelEnableProfiling")]
pub fn ravel_enable_profiling<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    value: JString<'local>,
) {
    env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let property = if value.as_raw().is_null() {
            None
        } else {
            Some(text(env, &value)?)
        };
        REQUESTED.store(requested_by(property.as_deref()), Ordering::Relaxed);
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Hands the screen the whole profiling report.
///
/// # Panics
///
/// Does not panic, for the same reason as [`ravel_enable_profiling`].
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelProfilingReport")]
pub fn ravel_profiling_report<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            match document() {
                Some(document) => Ok(env.new_string(document)?),
                None => Ok(JString::default()),
            }
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_host_api::{GameEvent, HostError, HostMod};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
    use ravel_minecraft::{HookId, HookKind, HookObserver, HookTarget};
    use ravel_observability::{Metric, Profiler};

    use super::{document_of, requested_by};
    use crate::runtime::LoaderRuntime;

    #[test]
    fn a_launch_that_set_no_property_asked_for_nothing() {
        // The case that aborts a launch when it is got wrong. `premain` hands
        // `System.getProperty` across untouched, so an ordinary launch reaches
        // the switch with nothing, and the switch has to answer rather than
        // fail: a failure there leaves `premain` and the JVM never starts the
        // client.
        assert!(!requested_by(None));
    }

    #[test]
    fn only_the_word_the_switch_names_turns_the_collectors_on() {
        assert!(requested_by(Some("true")));

        for otherwise in ["false", "", "TRUE", "1", "yes", "true "] {
            assert!(
                !requested_by(Some(otherwise)),
                "`{otherwise}` is not the switch"
            );
        }
    }

    /// Reads a payload back into its fields, the way the screen does.
    fn fields(payload: &str) -> Vec<String> {
        let units: Vec<u16> = payload.encode_utf16().collect();
        let mut read = Vec::new();
        let mut at = 0;
        while at < units.len() {
            let separator = at
                + units[at..]
                    .iter()
                    .position(|unit| *unit == u16::from(b':'))
                    .expect("every field states a length");
            let length: usize = String::from_utf16(&units[at..separator])
                .expect("a length is text")
                .parse()
                .expect("a length is a number");
            let start = separator + 1;
            read.push(String::from_utf16(&units[start..start + length]).expect("a field is text"));
            at = start + length;
        }
        read
    }

    fn mod_id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

    /// A mod that reacts to everything and costs nothing doing it.
    struct Quiet(ModId);

    impl HostMod for Quiet {
        fn id(&self) -> &ModId {
            &self.0
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn observe(&mut self, _event: GameEvent) -> Result<(), HostError> {
            Ok(())
        }
    }

    /// A hook target standing in for a reviewed one, keyed on the id alone.
    fn target(hook: &'static str) -> HookTarget {
        HookTarget::new(
            HookId::new(hook),
            "net/minecraft/Ignored",
            "ignored",
            "()V",
            HookKind::InstanceMethod,
        )
    }

    /// A driven runtime holding one mod, profiling as asked.
    fn running(profiler: Profiler) -> Arc<LoaderRuntime> {
        let entry = LifecycleEntry {
            mod_id: mod_id("example_mod"),
            sides: SideSet::new([Side::Client]).expect("one side"),
            requirement: Requirement::Required,
            saved_data_versions: std::collections::BTreeSet::new(),
        };
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry]).expect("a one-mod plan is valid");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        let mods: Vec<Box<dyn HostMod>> = vec![Box::new(Quiet(mod_id("example_mod")))];
        Arc::new(LoaderRuntime::profiling(
            machine,
            mods,
            profiler,
            crate::runtime::pre_world_report(),
        ))
    }

    /// The payload a screen reads from `runtime`, as [`document`] builds it.
    ///
    /// The runtime is the test's own rather than the process's, so no test here
    /// installs anything and none of them can read another's report.
    fn report(runtime: &LoaderRuntime) -> String {
        document_of(Some(runtime)).expect("a runtime was handed over")
    }

    #[test]
    fn a_launch_with_no_set_installed_has_no_report_rather_than_an_empty_one() {
        assert_eq!(document_of(None), None);
    }

    #[test]
    fn a_report_from_a_normal_launch_says_the_collectors_were_off() {
        let payload = report(&running(Profiler::off()));
        let read = fields(&payload);

        assert_eq!(read[0], "1", "the payload opens with the wire version");
        assert_eq!(read[1], "false", "profiling was never asked for");
        assert_eq!(read[2], "-Dravel.profiling=true");
        assert_eq!(
            read[5], "0",
            "an off run reports no mods rather than zeroes"
        );
    }

    #[test]
    fn a_profiled_run_reports_each_mod_and_what_it_cost() {
        let runtime = running(Profiler::on(Duration::from_secs(1)));
        let tick = target("client.tick");
        runtime.observe(&tick, &[]);
        runtime.observe(&tick, &[]);

        let read = fields(&report(&runtime));

        assert_eq!(read[1], "true");
        assert_eq!(read[5], "1", "one mod was entered");
        assert_eq!(read[6], "example_mod");
        assert_eq!(read[8], "2", "two hook invocations reached it");
        assert_eq!(read[9], "2", "both were events");
        assert_eq!(read[10], "0", "neither was a command");
    }

    #[test]
    fn every_metric_crosses_with_what_can_be_said_about_it() {
        let payload = report(&running(Profiler::off()));
        let read = fields(&payload);

        // The metric table is the last block, four fields per metric.
        let table = &read[read.len() - Metric::ALL.len() * 4..];
        for (index, metric) in Metric::ALL.into_iter().enumerate() {
            let at = index * 4;
            assert_eq!(table[at], metric.name());
            assert_eq!(table[at + 1], metric.unit());
            assert_eq!(table[at + 2], metric.support().name());
            assert_eq!(table[at + 3], metric.support().detail().unwrap_or(""));
        }
        assert!(
            table.iter().any(|entry| entry == "unsupported"),
            "an unsupported metric crosses rather than being left out"
        );
    }

    #[test]
    fn the_accuracy_statement_crosses_so_a_screen_can_show_it() {
        let payload = report(&running(Profiler::off()));
        let read = fields(&payload);

        let sentences = ravel_observability::ProfilingReport::attribution_accuracy();
        for sentence in sentences {
            assert!(
                read.iter().any(|entry| entry == sentence),
                "the accuracy statement did not cross whole"
            );
        }
    }
}
