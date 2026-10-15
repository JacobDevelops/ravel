//! Launch-time profiling opt-in and reports encoded for the Java screen.

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};
use ravel_observability::{Metric, ProfilingReport};

use crate::guard::HookError;
use crate::runtime::LoaderRuntime;
use crate::wire::{VERSION, count, field, flag};

/// The system property a launch turns the collectors on with.
pub(crate) const SWITCH: &str = "-Dravel.profiling=true";

/// Encodes the report, or returns nothing before runtime installation.
fn document() -> Option<String> {
    document_of(crate::runtime::installed().map(AsRef::as_ref))
}

/// The report a screen reads from one runtime, or nothing without one.
fn document_of(runtime: Option<&LoaderRuntime>) -> Option<String> {
    Some(render(&runtime?.profiling_report()))
}

/// Encodes measurements alongside their attribution and support qualifications.
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
        // Totals may exceed the Java reader's int-sized count type.
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

    count(&mut out, Metric::ALL.len());
    for metric in Metric::ALL {
        let support = metric.support();
        field(&mut out, metric.as_str());
        field(&mut out, metric.unit());
        field(&mut out, support.as_str());
        field(&mut out, support.detail().unwrap_or(""));
    }

    out
}

/// Hands the screen the whole profiling report.
///
/// # Panics
///
/// Panics become Java `RuntimeException`s rather than unwinding into the JVM.
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
    use ravel_host_api::{GameEvent, HostMod, ModError};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
    use ravel_minecraft::{HookId, HookKind, HookObserver, HookTarget};
    use ravel_observability::{Metric, Profiler};

    use super::document_of;
    use crate::runtime::LoaderRuntime;
    use crate::wire::fields;

    fn mod_id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

    struct Quiet(ModId);

    impl HostMod for Quiet {
        fn id(&self) -> &ModId {
            &self.0
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn observe(&mut self, _event: GameEvent) -> Result<(), ModError> {
            Ok(())
        }
    }

    fn target(hook: &'static str) -> HookTarget {
        HookTarget::new(
            HookId::new(hook),
            "net/minecraft/Ignored",
            "ignored",
            "()V",
            HookKind::InstanceMethod,
        )
    }

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
        Arc::new(LoaderRuntime::with_observers(
            machine,
            mods,
            profiler,
            crate::runtime::pre_world_report(),
        ))
    }

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
            assert_eq!(table[at], metric.as_str());
            assert_eq!(table[at + 1], metric.unit());
            assert_eq!(table[at + 2], metric.support().as_str());
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
