//! Measures disabled-profiling overhead against a predetermined normal-mode budget.

#![forbid(unsafe_code)]

mod measure;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ravel_observability::{Metric, ProfilingReport};
use ravel_spike_harness as harness;
use serde_json::{Value, json};

use measure::{ASSUMED_CROSSINGS_PER_TICK, Overhead, Timing};

/// Where the record belongs for the M3 gate to find it.
const RESULT: &str = "evidence/m3/profiling-overhead.json";

/// The hardware profile every Ravel Linux measurement is compared on.
const HARDWARE_PROFILE: &str = "tests/performance/hardware/linux-reference.toml";

/// One percent of a 20 TPS tick, in nanoseconds, fixed before measurement.
const NORMAL_MODE_BUDGET_NS: f64 = 500_000.0;

/// The metric the budget bounds.
const BUDGET_METRIC: &str = "profiling.off.perTick";

/// The system property that turns the collectors on.
const SWITCH: &str = "-Dravel.profiling=true";

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("wrote the profiling overhead record to {path}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("the profiling overhead measurement failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let root = repository_root()?;
    // Allow later runs to preserve the budget baseline.
    let result = output_path()?;
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;
    let pinned = harness::pinned_inputs(&root).map_err(|error| error.to_string())?;
    let profile =
        harness::sha256_file(&root.join(HARDWARE_PROFILE)).map_err(|error| error.to_string())?;

    let overhead = measure::run();

    let record = json!({
        "schemaVersion": 1,
        "record": "m3-w6-profiling-overhead",
        "milestone": "M3",
        "workPackage": "M3-W6",
        "summary": "What Ravel's opt-in profiling collectors cost a tick when nobody turned them \
                    on, what they can and cannot attribute to a mod, and the normal-mode budget \
                    the first of those is held to.",
        "identity": {
            "target": "x86_64-linux",
            "hardwareProfile": "linux-reference-v1",
            "hardwareProfileSha256": profile,
            "minecraft": pinned.minecraft,
            "buildProfile": "release",
            "clock": "monotonic",
        },
        "trial": {
            "startedAt": started_at,
            "revision": {
                "id": revision.id,
                "changeId": revision.change_id,
                "dirty": revision.dirty,
            },
            "commandSha256": harness::command_sha256(&[
                "cargo",
                "run",
                "--release",
                "-p",
                "ravel-spike-m3-profiling-overhead",
            ]),
        },
        "collectors": collectors_record(),
        "overhead": overhead_record(&overhead),
        "budget": budget_record(&overhead),
        "whatThisDoesNotSay": caveats(),
    });

    let mut text = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("the record does not serialize: {error}"))?;
    text.push('\n');
    fs::write(root.join(&result), text)
        .map_err(|error| format!("cannot write {result}: {error}"))?;
    Ok(result)
}

/// Where the record goes: the M3 record unless a caller names another path.
fn output_path() -> Result<String, String> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next() {
        None => Ok(RESULT.to_owned()),
        Some(argument) if argument == "--output" => arguments
            .next()
            .ok_or_else(|| "--output needs a repository-relative path".to_owned()),
        Some(other) => Err(format!("unknown argument `{other}`")),
    }
}

/// What the collectors are, read out of the crate that implements them.
///
/// Nothing here is written twice. The metric table, the accuracy statement and
/// the unsupported list all come from `ravel-observability`, so a metric that
/// becomes supported changes this record by being rerun and cannot be made to
/// say otherwise by editing it.
fn collectors_record() -> Value {
    json!({
        "crate": "ravel-observability",
        "default": "off",
        "switch": SWITCH,
        "switchDetail": "Opt-in, and opt-in at startup. The collectors are chosen when the host \
                         reads the property and never toggled afterwards, which is what lets the \
                         off path be one branch on a plain flag rather than an atomic load on \
                         every crossing. The precedent is `-Dravel.report.loader=true`, the \
                         measurement switch `evidence/m3/class-loader-identity.json` was taken \
                         under.",
        "attributionAccuracy": ProfilingReport::attribution_accuracy(),
        "metrics": Metric::ALL
            .into_iter()
            .map(|metric| {
                let support = metric.support();
                let mut entry = json!({
                    "metric": metric.name(),
                    "unit": metric.unit(),
                    "support": support.name(),
                });
                if let Some(detail) = support.detail() {
                    entry["detail"] = json!(detail);
                }
                entry
            })
            .collect::<Vec<_>>(),
        "unsupported": ProfilingReport::unsupported()
            .into_iter()
            .map(|(metric, _)| metric.name())
            .collect::<Vec<_>>(),
        "reportSurface": "A screen drawn in Java from a payload built in Rust, the way ADG-6 \
                          draws every screen Ravel owns. Rust renders nothing.",
    })
}

/// What the collectors cost.
fn overhead_record(overhead: &Overhead) -> Value {
    json!({
        "unit": "nanoseconds",
        "method": "paired batches. One sample times a thousand instrumented calls and a thousand \
                   bare calls and records the gap between them, alternating which of the two runs \
                   first. A gap is therefore the distribution of the overhead itself rather than \
                   a difference between two runs' percentiles, which would not be a percentile of \
                   anything.",
        "clamping": "A sample whose instrumented batch finished ahead of its control is recorded \
                     as zero rather than as a negative time, so the figure is biased upward. That \
                     is the safe direction for a limit.",
        "rounding": "Every figure in this record is rounded to two decimals, which is as fine as \
                     the wall clock behind it can honestly be read. perTick.nanoseconds and \
                     breakEven.crossingsPerTick are computed from the unrounded percentiles and \
                     rounded afterwards, so applying perTick.formula to the rounded percentiles \
                     printed here does not reproduce them: the gap is the digits the rounding \
                     dropped, and it is stated here rather than left for a reader to hit.",
        "off": {
            "perCrossing": timing(overhead.off_per_crossing),
            "perTickBoundary": timing(overhead.off_per_tick_boundary),
            "detail": "The whole normal-mode cost. With the collectors off, a crossing tests one \
                       flag and calls the mod, and a tick boundary tests the same flag; no clock \
                       is read, no map is touched, and nothing is allocated.",
        },
        "on": {
            "perCrossing": timing(overhead.on_per_crossing),
            "detail": "Recorded and not bounded. What profiling costs while it is running depends \
                       on how many mods and collectors are attached, and none of that exists yet; \
                       M9 is where the number is set against a real pack.",
        },
        "perTick": {
            "assumedCrossingsPerTick": ASSUMED_CROSSINGS_PER_TICK,
            "assumptionDetail": "An assumption, not a measurement. The real per-tick crossing \
                                 count is its own open work — the M2 record puts it at zero for \
                                 M2 and says M3 is where the first non-zero one is taken — and \
                                 this budget is deliberately written per tick rather than per \
                                 crossing so that it does not wait on it. Ten thousand is a \
                                 ceiling rather than a prediction: a 256-mod set would have to be \
                                 entered thirty-nine times each in one tick to reach it.",
            "nanoseconds": round(overhead.off_per_tick()),
            "formula": "one tick boundary plus the assumed crossings, each at its 99th percentile",
        },
        "breakEven": {
            "crossingsPerTick": round(
                overhead.break_even_crossings_per_tick(NORMAL_MODE_BUDGET_NS),
            ),
            "detail": "How many crossings one tick would need before profiling being off costs a \
                       whole per cent of it. This is the figure that rests on no assumption, and \
                       it is what answers the budget outright once the real crossing count is \
                       recorded: compare the two numbers.",
        },
    })
}

/// The budget, and the rule for reading it.
fn budget_record(overhead: &Overhead) -> Value {
    json!({
        "protocol": "m2-loader-budget-v1",
        "hardwareProfile": "linux-reference-v1",
        "rule": "A measurement above its limit on this hardware profile is a regression and stops \
                 the milestone that produced it. This limit is one per cent of a tick: at 20 TPS \
                 a tick is 50 ms, so 500 µs of added time per tick, summed across every crossing \
                 in that tick, measured with profiling not enabled.",
        "decidedNotDerived": "The M2 limits are each their own measurement doubled. This one is \
                              not: it was fixed before the measurement, because what a player is \
                              promised is that profiling costs them nothing when it is off, and \
                              that promise is a fraction of a tick rather than whatever today's \
                              code happens to do. A measurement far under its limit is the \
                              expected result and not a sign the limit is wrong.",
        "anchoredToTheTick": "Per tick rather than per loader pipeline, because profiling is \
                              sampled per tick; and a per-tick total rather than a per-crossing \
                              limit, so that it does not depend on the crossings-per-tick count, \
                              which is not recorded yet.",
        "profilingOnIsNotBounded": "Only the off figure is limited. What profiling costs while it \
                                    runs is recorded above and gated at M9, against a real pack, \
                                    because its cost depends on how many mods and collectors are \
                                    attached and none of that exists at M3.",
        "restatement": "This limit holds on linux-reference-v1 only. A different machine \
                        remeasures rather than reinterprets it, because a budget without the \
                        machine it was measured on is not a budget.",
        "enforcement": "`cargo xtask verify budget` compares a run against this and runs inside \
                        `ravel:verify`. Re-running this measurement writes a new record, so pass \
                        an output path rather than overwriting the one the limit is read from.",
        "limits": [
            {
                "metric": BUDGET_METRIC,
                "unit": "nanoseconds",
                "measured": round(overhead.off_per_tick()),
                "limit": NORMAL_MODE_BUDGET_NS,
                "operator": "less-than-or-equal",
            },
        ],
    })
}

/// What this measurement does not say, said before someone reads it as saying
/// it.
fn caveats() -> Value {
    json!([
        "The crossings measured here are safe Rust calls through a trait object in one process. A \
         hook crossing from the game costs what ADG-1 measured, and this figure is what profiling \
         adds to a crossing rather than what a crossing costs.",
        "The per-tick figure multiplies a per-crossing cost by an assumed crossing count. The \
         count is not measured anywhere yet, so the per-tick number inherits whatever that \
         assumption is wrong by; the break-even count beside it does not.",
        "Nothing here runs a JVM, opens a world, or ticks a server. The tick is arithmetic over \
         measured per-call costs, not an observed tick.",
        "The mod body is one wrapping increment. A real callback is slower, which makes the \
         *relative* overhead smaller than it looks here and the absolute overhead the same.",
        "The profiling-on figure is one mod, one collector, and a map with one entry in it. It is \
         a floor, and it is recorded rather than bounded for exactly that reason.",
        "Nothing here measures the accuracy of an attribution. The metric table states what each \
         number can and cannot tell apart, and this run times the collectors rather than \
         checking them.",
        "The off path is a flag on a struct rather than an atomic, which is only sound because \
         the switch is read once at startup. A future that lets a player turn profiling on \
         mid-session changes the thing measured here and has to remeasure it.",
    ])
}

fn timing(timing: Timing) -> Value {
    json!({
        "samples": timing.samples,
        "median": round(timing.percentiles.median),
        "p95": round(timing.percentiles.p95),
        "p99": round(timing.percentiles.p99),
    })
}

/// Rounds to two decimals, which is as precise as a wall clock gets here.
fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("cannot resolve the repository root: {error}"))
}

#[cfg(test)]
mod tests {
    use ravel_observability::{Metric, ProfilingReport};
    use ravel_spike_harness::Percentiles;

    use super::{
        BUDGET_METRIC, NORMAL_MODE_BUDGET_NS, SWITCH, budget_record, caveats, collectors_record,
        overhead_record, repository_root, round, timing,
    };
    use crate::measure::{ASSUMED_CROSSINGS_PER_TICK, Overhead, Timing};

    fn flat(value: f64) -> Timing {
        Timing {
            samples: 5,
            percentiles: Percentiles {
                median: value,
                p95: value,
                p99: value,
            },
        }
    }

    fn overhead() -> Overhead {
        Overhead {
            off_per_crossing: flat(0.05),
            off_per_tick_boundary: flat(0.05),
            on_per_crossing: flat(45.0),
        }
    }

    #[test]
    fn the_collectors_section_is_read_out_of_the_crate_rather_than_written_here() {
        let record = collectors_record();

        assert_eq!(record["crate"], "ravel-observability");
        assert_eq!(record["default"], "off");
        assert_eq!(record["switch"], SWITCH);
        assert_eq!(
            record["metrics"].as_array().expect("a metric table").len(),
            Metric::ALL.len()
        );
        assert_eq!(
            record["attributionAccuracy"]
                .as_array()
                .expect("an accuracy statement")
                .len(),
            ProfilingReport::attribution_accuracy().len()
        );
        assert_eq!(
            record["unsupported"]
                .as_array()
                .expect("an unsupported list")
                .len(),
            ProfilingReport::unsupported().len()
        );
    }

    #[test]
    fn an_exact_metric_carries_no_detail_and_every_other_one_does() {
        let record = collectors_record();
        let rows = record["metrics"].as_array().expect("a metric table");

        for (row, metric) in rows.iter().zip(Metric::ALL) {
            assert_eq!(row["metric"], metric.name());
            assert_eq!(row["support"], metric.support().name());
            assert_eq!(
                row.get("detail").is_some(),
                metric.support().detail().is_some()
            );
        }
    }

    #[test]
    fn the_overhead_section_states_its_assumption_beside_the_figure_it_produced() {
        let record = overhead_record(&overhead());

        assert_eq!(record["unit"], "nanoseconds");
        assert_eq!(
            record["perTick"]["assumedCrossingsPerTick"],
            ASSUMED_CROSSINGS_PER_TICK
        );
        assert!(
            record["perTick"]["assumptionDetail"]
                .as_str()
                .expect("the assumption says it is one")
                .contains("assumption"),
        );
        assert!(
            record["breakEven"]["crossingsPerTick"]
                .as_f64()
                .expect("a number")
                > 0.0
        );
        assert!(record["off"]["perCrossing"]["p99"].is_number());
        assert!(record["on"]["perCrossing"]["p99"].is_number());
    }

    #[test]
    fn the_budget_states_one_limit_and_the_limit_is_one_per_cent_of_a_tick() {
        let record = budget_record(&overhead());
        let limits = record["limits"].as_array().expect("the limits");

        assert_eq!(record["protocol"], "m2-loader-budget-v1");
        assert_eq!(record["hardwareProfile"], "linux-reference-v1");
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0]["metric"], BUDGET_METRIC);
        assert_eq!(limits[0]["unit"], "nanoseconds");
        assert_eq!(limits[0]["operator"], "less-than-or-equal");
        assert_eq!(limits[0]["limit"].as_f64(), Some(NORMAL_MODE_BUDGET_NS));
        assert!(
            limits[0]["measured"].as_f64().expect("a number") <= NORMAL_MODE_BUDGET_NS,
            "this overhead should be nowhere near the limit"
        );
    }

    #[test]
    fn the_budget_says_it_was_decided_rather_than_derived_and_that_on_is_not_bounded() {
        let record = budget_record(&overhead());

        assert!(
            record["decidedNotDerived"]
                .as_str()
                .expect("the record says so")
                .contains("fixed before the measurement"),
        );
        assert!(record["profilingOnIsNotBounded"].is_string());
        assert!(record["anchoredToTheTick"].is_string());
    }

    #[test]
    fn every_caveat_is_a_sentence_rather_than_a_label() {
        let listed = caveats();
        let sentences = listed.as_array().expect("a list of caveats");

        assert!(sentences.len() >= 5);
        for sentence in sentences {
            assert!(sentence.as_str().expect("text").len() > 60);
        }
    }

    #[test]
    fn a_timing_crosses_as_its_three_percentiles_and_its_sample_count() {
        let record = timing(flat(1.234));

        assert_eq!(record["samples"], 5);
        assert_eq!(record["median"].as_f64(), Some(1.23));
        assert_eq!(record["p95"].as_f64(), Some(1.23));
        assert_eq!(record["p99"].as_f64(), Some(1.23));
    }

    #[test]
    fn a_figure_is_rounded_to_what_a_wall_clock_can_actually_say() {
        assert!((round(1.234_5) - 1.23).abs() < f64::EPSILON);
        assert!((round(1.235_5) - 1.24).abs() < f64::EPSILON);
        assert!(round(0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_repository_root_is_the_one_this_experiment_sits_two_directories_below() {
        let root = repository_root().expect("the root resolves");

        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("evidence/m3").is_dir());
    }
}
