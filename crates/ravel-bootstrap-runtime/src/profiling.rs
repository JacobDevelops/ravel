//! Launch-time profiling opt-in and reports encoded for the Java screen.
//!
//! Integrated-server measurements remain separate labelled rows. Client header
//! fields retain their original meaning and wire version.

use std::fmt::Write;

use jni::EnvUnowned;
use jni::objects::{JClass, JString};
use ravel_observability::{Metric, ProfilingReport};

use crate::guard::hand_document;
use crate::runtime::LoaderRuntime;
use crate::wire::{VERSION, count, field, flag};

/// The system property a launch turns the collectors on with.
pub(crate) const SWITCH: &str = "-Dravel.profiling=true";

/// Encodes the report, or returns nothing before runtime installation.
fn document() -> Option<String> {
    let integrated = crate::runtime::integrated_runtime();
    document_with_integrated(
        crate::runtime::installed().map(AsRef::as_ref),
        integrated.as_deref(),
    )
}

/// The report a screen reads from one runtime, or nothing without one.
fn document_of(runtime: Option<&LoaderRuntime>) -> Option<String> {
    Some(render(&runtime?.profiling_report()))
}

/// Snapshots both runtimes only when a screen requests a report.
fn document_with_integrated(
    runtime: Option<&LoaderRuntime>,
    integrated: Option<&LoaderRuntime>,
) -> Option<String> {
    let runtime = runtime?;
    let Some(integrated) = integrated else {
        return document_of(Some(runtime));
    };
    Some(render_with_integrated(
        &runtime.profiling_report(),
        Some(&integrated.profiling_report()),
    ))
}

/// Encodes measurements alongside their attribution and support qualifications.
fn render(report: &ProfilingReport) -> String {
    render_with_integrated(report, None)
}

/// Adds server rows without merging their costs or changing client header semantics.
fn render_with_integrated(
    report: &ProfilingReport,
    integrated: Option<&ProfilingReport>,
) -> String {
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

    count(
        &mut out,
        report.mods().len() + integrated.map_or(0, |server| server.mods().len()),
    );
    let mut label = String::new();
    for (entry, server) in report.mods().iter().map(|entry| (entry, false)).chain(
        integrated
            .into_iter()
            .flat_map(|report| report.mods().iter().map(|entry| (entry, true))),
    ) {
        if server {
            label.clear();
            write!(label, "{} [integrated-server]", entry.mod_id)
                .expect("writing into a String cannot fail");
            field(&mut out, &label);
        } else {
            field(&mut out, entry.mod_id.as_str());
        }
        // Totals may exceed the Java reader's int-sized count type.
        field(&mut out, &entry.cost.inclusive_nanos.to_string());
        field(&mut out, &entry.cost.hook_calls().to_string());
        field(&mut out, &entry.cost.event_calls.to_string());
        field(&mut out, &entry.cost.command_calls.to_string());
        field(&mut out, &entry.cost.slow_callbacks.to_string());
        field(&mut out, &entry.cost.registry_claims.to_string());
    }

    let accuracy = ProfilingReport::attribution_accuracy();
    count(&mut out, accuracy.len() + usize::from(integrated.is_some()));
    for sentence in accuracy {
        field(&mut out, sentence);
    }
    if let Some(server) = integrated {
        label.clear();
        write!(
            label,
            "[integrated-server] {} ticks; profiling {}. Header tick count and slow-callback threshold describe the client; server rows retain independent costs.",
            server.ticks(),
            if server.is_on() { "on" } else { "off" },
        )
        .expect("writing into a String cannot fail");
        field(&mut out, &label);
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
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    hand_document(env, |_| Ok(document()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_host_api::{GameEvent, Mod, ModError};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
    use ravel_minecraft::{HookId, HookKind, HookTarget};
    use ravel_observability::{Metric, Profiler};

    use super::document_of;
    use crate::runtime::{LoaderRuntime, advance_through};
    use crate::wire::fields;

    fn mod_id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

    struct Quiet(ModId);

    impl Mod for Quiet {
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
        running_on(profiler, Side::Client)
    }

    fn running_on(profiler: Profiler, side: Side) -> Arc<LoaderRuntime> {
        let entry = LifecycleEntry {
            mod_id: mod_id("example_mod"),
            sides: SideSet::new([side]).expect("one side"),
            requirement: Requirement::Required,
            saved_data_versions: std::collections::BTreeSet::new(),
        };
        let mut machine =
            LifecycleMachine::new(side, vec![entry]).expect("a one-mod plan is valid");
        advance_through(&mut machine, LifecycleState::Running);
        let mods: Vec<Box<dyn Mod>> = vec![Box::new(Quiet(mod_id("example_mod")))];
        Arc::new(LoaderRuntime::with_observers(
            machine,
            mods,
            profiler,
            crate::runtime::pre_world_report(),
            crate::runtime::test_launch(),
            std::collections::BTreeMap::new(),
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

    #[test]
    fn integrated_costs_keep_side_identity_and_client_tick_header() {
        let client = running(Profiler::on(Duration::from_secs(1)));
        let server = running_on(Profiler::on(Duration::from_secs(1)), Side::IntegratedServer);
        client.observe(&target("client.tick"), &[]);
        server.observe(&target("server.tick-integrated"), &[]);
        server.observe(&target("server.tick-integrated"), &[]);
        let read = fields(
            &super::document_with_integrated(Some(&client), Some(&server))
                .expect("client installed"),
        );

        assert_eq!(read[0], "1", "the wire version is unchanged");
        assert_eq!(
            read[3],
            client.profiling_report().ticks().to_string(),
            "header retains client ticks rather than summing two runtimes",
        );
        assert_eq!(
            read[5], "2",
            "the same mod on two sides is not deduplicated"
        );
        assert_eq!(read[6], "example_mod");
        assert_eq!(read[8], "1");
        assert_eq!(read[13], "example_mod [integrated-server]");
        assert_eq!(read[15], "2", "server hooks remain independent");
        let server_summary = format!(
            "[integrated-server] {} ticks; profiling on.",
            server.profiling_report().ticks(),
        );
        assert!(read.iter().any(|field| field.starts_with(&server_summary)));
    }

    #[test]
    fn absent_integrated_runtime_preserves_the_original_report() {
        let client = running(Profiler::off());
        assert_eq!(
            super::document_with_integrated(Some(&client), None),
            document_of(Some(&client)),
        );
        assert_eq!(super::document_with_integrated(None, Some(&client)), None,);
    }
}
