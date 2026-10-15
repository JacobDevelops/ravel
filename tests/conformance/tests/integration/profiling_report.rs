//! Profiling behavior and the accuracy, support, privacy, and budget contracts in recorded evidence.

use std::fs;
use std::path::{Path, PathBuf};

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_host_api::{GameEvent, HostMod};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, broadcast, broadcast_profiled};
use ravel_observability::{CallKind, Metric, Profiler, ProfilingReport, Support};
use serde_json::Value;

const EVIDENCE_FILE: &str = "evidence/m3/profiling-overhead.json";

// Independent limit: one percent of a tick at 20 TPS.
const NORMAL_MODE_BUDGET_NS: f64 = 500_000.0;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn record() -> Value {
    let path = repository_root().join(EVIDENCE_FILE);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("`{EVIDENCE_FILE}` should be readable: {error}"));
    serde_json::from_str(&text).expect("the record should be JSON")
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("the field should be an array")
        .iter()
        .map(|entry| entry.as_str().expect("the entry should be text").to_owned())
        .collect()
}

fn id(name: &str) -> ModId {
    name.parse().expect("a valid mod id")
}

struct Quiet(ModId);

impl HostMod for Quiet {
    fn id(&self) -> &ModId {
        &self.0
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
        Ok(())
    }

    fn observe(&mut self, _event: GameEvent) -> Result<(), ravel_host_api::ModError> {
        Ok(())
    }
}

fn running() -> LifecycleMachine {
    let entry = LifecycleEntry {
        mod_id: id("example_mod"),
        sides: SideSet::new([Side::Client]).expect("one side"),
        requirement: Requirement::Required,
        saved_data_versions: std::collections::BTreeSet::new(),
    };
    let mut host = Quiet(id("example_mod"));
    ravel_loader::drive_borrowed(
        Side::Client,
        &ravel_host_runtime::environment(),
        vec![entry],
        &mut [&mut host],
    )
    .expect("the production driver starts the quiet mod")
}

#[test]
fn the_recorded_metric_table_is_what_the_collectors_can_actually_measure() {
    let record = record();
    let rows = record["collectors"]["metrics"]
        .as_array()
        .expect("the record states a metric table");

    assert_eq!(
        rows.len(),
        Metric::ALL.len(),
        "the record describes a different number of metrics than the crate has"
    );
    for (row, metric) in rows.iter().zip(Metric::ALL) {
        let support = metric.support();
        assert_eq!(row["metric"], metric.as_str());
        assert_eq!(row["unit"], metric.unit());
        assert_eq!(
            row["support"],
            support.as_str(),
            "`{}` is recorded as something the crate no longer says",
            metric.as_str()
        );
        match support.detail() {
            None => assert!(
                row.get("detail").is_none(),
                "`{}` is exact and the record explains it anyway",
                metric.as_str()
            ),
            Some(detail) => assert_eq!(
                row["detail"],
                detail,
                "`{}`'s reason has changed since the record was written",
                metric.as_str()
            ),
        }
    }
}

#[test]
fn every_metric_m3_cannot_measure_is_named_in_the_record_rather_than_absent() {
    let record = record();
    let recorded = strings(&record["collectors"]["unsupported"]);
    let actual = ProfilingReport::unsupported()
        .into_iter()
        .map(|(metric, _)| metric.as_str().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        recorded, actual,
        "the record's unsupported list is not the crate's"
    );
    assert!(
        !actual.is_empty(),
        "a build that claims to measure everything has stopped being honest about one of these"
    );
    for metric in Metric::ALL {
        let support = metric.support();
        assert_eq!(
            actual.contains(&metric.as_str().to_owned()),
            !support.is_measured(),
            "`{}` is on one list and not the other",
            metric.as_str()
        );
    }
}

#[test]
fn the_recorded_accuracy_statement_is_the_one_the_collectors_publish() {
    let record = record();

    assert_eq!(
        strings(&record["collectors"]["attributionAccuracy"]),
        ProfilingReport::attribution_accuracy(),
        "the record's accuracy statement is not the one a player would be shown"
    );
}

// Derived figures use unrounded percentiles; the evidence must state that rounding rule.
#[test]
fn the_record_states_the_rounding_the_derived_figures_were_computed_under() {
    let record = record();
    let rounding = record["overhead"]["rounding"]
        .as_str()
        .expect("the record states the rounding its derived figures were computed under");

    for required in [
        "perTick.nanoseconds",
        "breakEven.crossingsPerTick",
        "two decimals",
    ] {
        assert!(
            rounding.contains(required),
            "`overhead.rounding` no longer says what `{required}` was rounded from"
        );
    }
}

#[test]
fn the_normal_mode_budget_holds_against_its_own_measurement() {
    let record = record();
    let limits = record["budget"]["limits"]
        .as_array()
        .expect("the record states its limits");

    assert_eq!(limits.len(), 1, "the record bounds exactly one metric");
    let limit = &limits[0];
    assert_eq!(limit["metric"], "profiling.off.perTick");
    assert_eq!(limit["unit"], "nanoseconds");
    assert_eq!(limit["operator"], "less-than-or-equal");
    assert_eq!(
        limit["limit"].as_f64(),
        Some(NORMAL_MODE_BUDGET_NS),
        "the recorded limit is not one per cent of a tick"
    );

    let measured = limit["measured"]
        .as_f64()
        .expect("the measurement is a number");
    assert!(
        measured <= NORMAL_MODE_BUDGET_NS,
        "profiling costs {measured} ns a tick when it is off, against a limit of \
         {NORMAL_MODE_BUDGET_NS}"
    );
}

#[test]
fn profiling_on_is_recorded_and_deliberately_not_bounded() {
    let record = record();

    assert!(
        record["overhead"]["on"]["perCrossing"]["p99"].is_number(),
        "what profiling costs while it runs is not recorded"
    );
    let bounded = record["budget"]["limits"]
        .as_array()
        .expect("the record states its limits")
        .iter()
        .any(|limit| limit["metric"].as_str() == Some("profiling.on.perCrossing"));
    assert!(
        !bounded,
        "profiling-on is bounded at M3, and its cost depends on a pack that does not exist yet"
    );
}

#[test]
fn the_break_even_crossing_count_is_recorded_so_the_assumed_one_can_be_replaced() {
    let record = record();
    let assumed = record["overhead"]["perTick"]["assumedCrossingsPerTick"]
        .as_f64()
        .expect("the record states the crossing count it assumed");
    let break_even = record["overhead"]["breakEven"]["crossingsPerTick"]
        .as_f64()
        .expect("the record states the count at which the budget is reached");

    assert!(
        break_even > assumed,
        "the assumed crossing count is already at or past the budget, so the figure beside it is \
         not headroom"
    );
}

#[test]
fn the_recorded_evidence_names_no_path_and_no_secret() {
    let stored = fs::read_to_string(repository_root().join(EVIDENCE_FILE))
        .expect("the record should be readable");
    let lowered = stored.to_lowercase();

    for forbidden in [
        "/home/",
        "/users/",
        "c:\\users\\",
        "/.minecraft",
        "tests/fixtures/local/",
        "evidence/private/",
        "token",
        "password",
        "secret",
        "credential",
        "authorization",
    ] {
        assert!(
            !lowered.contains(forbidden),
            "the record contains `{forbidden}`"
        );
    }
    assert!(
        !stored.contains(&repository_root().display().to_string()),
        "the record names the directory it was generated in"
    );
}

#[test]
fn a_dispatch_with_the_collectors_off_delivers_exactly_what_one_without_them_delivers() {
    let machine = running();
    let mut plain = Quiet(id("example_mod"));
    let mut profiled = Quiet(id("example_mod"));

    let unprofiled =
        broadcast(&machine, &mut [&mut plain], GameEvent::ClientTick).expect("the set is running");
    let mut off = Profiler::off();
    let observed = broadcast_profiled(
        &machine,
        &mut [&mut profiled],
        GameEvent::ClientTick,
        &mut off,
    )
    .expect("the set is running");

    assert_eq!(unprofiled, observed);
    assert!(
        !off.report().is_on(),
        "an off profiler reported itself as running"
    );
    assert_eq!(off.report().mods(), []);
}

#[test]
fn a_profiled_dispatch_attributes_the_delivery_to_the_mod_that_received_it() {
    let machine = running();
    let mut host = Quiet(id("example_mod"));
    let mut profiler = Profiler::on(std::time::Duration::from_secs(1));

    for _ in 0..3 {
        broadcast_profiled(
            &machine,
            &mut [&mut host],
            GameEvent::ClientTick,
            &mut profiler,
        )
        .expect("the set is running");
        profiler.tick();
    }

    let report = profiler.report();
    assert!(report.is_on());
    assert_eq!(report.ticks(), 3);
    assert_eq!(report.mods().len(), 1);
    assert_eq!(report.mods()[0].mod_id, id("example_mod"));
    assert_eq!(report.mods()[0].cost.event_calls, 3);
    assert_eq!(report.mods()[0].cost.hook_calls(), 3);
    assert_eq!(
        report.mods()[0].cost.boundary_calls(),
        3,
        "one crossing per delivery, charged to the mod it reached"
    );
}

#[test]
fn a_metric_that_is_measured_at_all_is_labelled_exact_or_approximate_and_never_both() {
    for metric in Metric::ALL {
        let support = metric.support();
        match support {
            Support::Exact => {
                assert!(support.is_measured());
                assert_eq!(support.as_str(), "exact");
                assert!(support.detail().is_none());
            }
            Support::Approximate(_) => {
                assert!(support.is_measured());
                assert_eq!(support.as_str(), "approximate");
                assert!(support.detail().is_some());
            }
            Support::Unsupported(_) => {
                assert!(!support.is_measured());
                assert_eq!(support.as_str(), "unsupported");
                assert!(support.detail().is_some());
            }
        }
    }
}

#[test]
fn the_collectors_the_record_describes_are_the_ones_the_workspace_ships() {
    let record = record();

    assert_eq!(record["collectors"]["crate"], "ravel-observability");
    assert_eq!(
        record["collectors"]["default"], "off",
        "profiling is opt-in, and a record saying otherwise describes a different build"
    );
    let switch = record["collectors"]["switch"]
        .as_str()
        .expect("the record names the switch");
    assert!(
        switch.starts_with("-Dravel."),
        "the switch is a JVM system property, the way every other Ravel switch is: {switch}"
    );

    let off = Profiler::off().report();
    assert!(!off.is_on());
    assert_eq!(off.lines().len(), 1);
    assert!(off.lines()[0].contains("profiling is off"));
}

#[test]
fn a_mod_that_was_never_entered_is_absent_rather_than_a_row_of_zeroes() {
    let mut profiler = Profiler::on(std::time::Duration::from_secs(1));
    profiler.measure(&id("busy_mod"), CallKind::Event, || ());

    let report = profiler.report();
    assert_eq!(report.mods().len(), 1);
    assert_eq!(report.mods()[0].mod_id, id("busy_mod"));
}
