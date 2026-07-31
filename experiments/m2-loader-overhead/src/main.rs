//! Measures what Ravel's loader costs, and turns the measurement into the
//! numeric budget M3 regresses against.
//!
//! `ROADMAP.md`'s M2-W8 row asks for a Linux no-op loader benchmark whose
//! measured cost establishes the regression budget used from M3 onward. Two
//! things are measured here: the pre-world pipeline itself, against the tracked
//! no-op package, and the build-side numbers ADG-2 deferred to M2 — link time,
//! binary growth, and compile time for a pack-sized set.
//!
//! Measuring is all this does. ADG-2 is deferred to M3 and
//! `docs/architecture/dependency-rules.md` forbids selecting a mod execution
//! model before then, so nothing here loads a dynamic library, starts a guest
//! runtime, spawns a mod process, or exports a foreign symbol, and nothing it
//! writes prefers one candidate over another.

#![forbid(unsafe_code)]

mod loader;
mod pack;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ravel_spike_harness as harness;
use serde_json::{Value, json};

use loader::{LoaderOverhead, Timing, as_f64};
use pack::BuildCost;

/// Where the record belongs for the M2 gate to find it.
const RESULT: &str = "evidence/m2/loader-overhead.json";
/// The hardware profile every Ravel Linux measurement is compared on.
const HARDWARE_PROFILE: &str = "tests/performance/hardware/linux-reference.toml";
/// Headroom between a measured value and the budget set from it.
///
/// Two, because a budget with no headroom fails on scheduler noise and a budget
/// with a lot of it stops catching anything. A regression that doubles the
/// loader's cost is worth stopping a milestone for.
const HEADROOM: f64 = 2.0;

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("wrote the loader overhead record");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("the loader overhead measurement failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = repository_root()?;
    // A later milestone compares against this record, so it must be able to
    // write its own rather than overwrite the baseline.
    let result = output_path()?;
    let work = loader::work_directory(&root);
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;
    let pinned = harness::pinned_inputs(&root).map_err(|error| error.to_string())?;
    let profile =
        harness::sha256_file(&root.join(HARDWARE_PROFILE)).map_err(|error| error.to_string())?;

    let overhead = loader::measure(&root, &work)?;
    let build = pack::measure(&root, &work)?;

    let record = json!({
        "schemaVersion": 1,
        "record": "m2-w8-loader-overhead",
        "milestone": "M2",
        "workPackage": "M2-W8",
        "summary": "What Ravel's pre-world loader costs on Linux for the no-op package, what a \
                    linked pack-sized set costs to build, and the numeric budget those \
                    measurements set for M3.",
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
                "ravel-spike-m2-loader-overhead",
            ]),
        },
        "loader": loader_record(&overhead),
        "modExecution": mod_execution_record(&overhead, &build),
        "budget": budget_record(&overhead, &build),
        "whatThisDoesNotSay": caveats(),
    });

    let mut text = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("the record does not serialize: {error}"))?;
    text.push('\n');
    fs::write(root.join(&result), text).map_err(|error| format!("cannot write {result}: {error}"))
}

/// Where the record goes: the M2 record unless a caller names another path.
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

/// What the pre-world pipeline cost.
fn loader_record(overhead: &LoaderOverhead) -> Value {
    json!({
        "package": "noop",
        "packagesInSet": 1,
        "samplesPerMetric": overhead.whole.samples,
        "unit": "nanoseconds",
        "wholePipeline": timing(overhead.whole),
        "stages": {
            "parseManifest": timing(overhead.parse),
            "verifyIntegrity": timing(overhead.integrity),
            "resolve": timing(overhead.resolve),
            "lifecycleAndChecksResidual": {
                "nanoseconds": overhead.lifecycle_residual(),
                "method": "the whole pipeline's 99th percentile less the sum of three stage \
                           99th percentiles, each from its own run, which leaves the lifecycle \
                           machine plus the compatibility and target checks. It is a difference \
                           of percentiles and therefore not a percentile: the four tails do not \
                           coincide, so it understates the true remainder, and a negative result \
                           is clamped to zero rather than reported. Read it as an order of \
                           magnitude, not a measurement.",
            },
        },
        "scaling": {
            "method": "the same pipeline over copies of the no-op package under distinct \
                       identities, so the curve is the loader's cost per package rather than \
                       any package's own",
            "points": overhead
                .scaling
                .iter()
                .map(|point| json!({
                    "packages": point.packages,
                    "p99": point.whole.percentiles.p99,
                    "median": point.whole.percentiles.median,
                }))
                .collect::<Vec<_>>(),
            "marginalPerPackageP99": overhead.marginal_per_package(),
            "marginalMethod": "a secant between the one-package and 256-package 99th \
                               percentiles. The curve is not linear — per-package cost is flat \
                               near the middle sizes and higher at 256 — so this takes the \
                               steeper end and the budget it sets is the conservative one.",
        },
    })
}

/// The numbers ADG-2 deferred to M2, and nothing that reads as a choice.
fn mod_execution_record(overhead: &LoaderOverhead, build: &BuildCost) -> Value {
    json!({
        "gate": "ADG-2",
        "status": "measured only; ADG-2 selects the mod execution and distribution model in M3 \
                   and nothing in this record prefers a candidate",
        "modCalls": {
            "perModPerProcessRun": overhead.mod_calls_per_run,
            "perTick": 0,
            "perTickDetail": "The loader makes no mod call per tick in M2. The pipeline is \
                              pre-world and the lifecycle is the whole surface, so a mod is \
                              entered once per state and never again. ADG-2 reports the \
                              difference between its two leading candidates as 0.625 ns per \
                              call, which reaches 0.1 ms at 160000 calls in a tick; M3 is where \
                              the first non-zero per-tick count is measured and this budget is \
                              where it lands.",
            "callNanoseconds": timing(overhead.mod_call),
            "callDetail": "One HostMod::enter through the trait object the loader drives, which \
                           is the in-process safe-Rust call M2 has. It is not a measurement of \
                           any ADG-2 candidate.",
        },
        "linkedPack": {
            "packSize": build.pack_size,
            "packCompileMilliseconds": build.pack_compile_ms(),
            "compileMillisecondsPerMod": build.compile_ms_per_mod(),
            "baselineBuildMilliseconds": build.baseline_build_ms,
            "packBuildMilliseconds": build.pack_build_ms,
            "relinkMilliseconds": build.relink_ms,
            "relinkDetail": "One mod's source changed and the set was rebuilt, which is the \
                             whole cost of iterating on a mod when the set is linked. ADG-2 put \
                             this requirement on M2's tooling rather than on the mod format.",
            "baselineBinaryBytes": build.baseline_bytes,
            "packBinaryBytes": build.pack_bytes,
            "binaryGrowthBytes": build.pack_bytes.saturating_sub(build.baseline_bytes),
            "binaryGrowthBytesPerMod": build.bytes_per_mod(),
            "method": "two cold release builds of a generated workspace — one host with no mods \
                       linked, one with the whole pack — plus one incremental rebuild after a \
                       single mod changed. Per-mod figures divide the difference by the pack \
                       size and assume the cost is linear across it.",
        },
    })
}

/// The budget itself: one number per metric, and the rule for reading it.
fn budget_record(overhead: &LoaderOverhead, build: &BuildCost) -> Value {
    json!({
        "protocol": "m2-loader-budget-v1",
        "hardwareProfile": "linux-reference-v1",
        "rule": "A measurement above its limit on this hardware profile is a regression and \
                 stops the milestone that produced it. Every limit is the M2 measurement times \
                 two, so a change that doubles a loader cost is what fails.",
        "headroomFactor": HEADROOM,
        "restatement": "These limits hold on linux-reference-v1 only. A different machine \
                        remeasures rather than reinterprets them, because a budget without the \
                        machine it was measured on is not a budget.",
        "expiry": "The three build limits describe a linked pack, which is the only shape \
                   available while ADG-2 is open. M3 both consumes this budget and closes \
                   ADG-2, so if it selects anything other than linking those three limits \
                   describe a shape that no longer exists and are remeasured rather than \
                   carried. The three loader limits do not depend on that choice.",
        "enforcement": "Nothing checks these limits automatically yet. The milestone gate asks \
                        only that each one be a number above its measurement; comparing a later \
                        run against them is M3's to build, and until it does the rule above is a \
                        sentence rather than a mechanism. Re-running this measurement writes a \
                        new record, so keep the M2 record if you want the baseline: pass an \
                        output path rather than overwriting it.",
        "limits": [
            limit(
                "loader.noopPackage.pipeline.p99",
                "nanoseconds",
                overhead.whole.budget_basis(),
            ),
            limit(
                "loader.marginalCostPerPackage.p99",
                "nanoseconds",
                overhead.marginal_per_package(),
            ),
            limit(
                "loader.modCall.p99",
                "nanoseconds",
                overhead.mod_call.budget_basis(),
            ),
            limit(
                "build.packCompile",
                "milliseconds",
                as_f64(usize::try_from(build.pack_compile_ms()).unwrap_or(usize::MAX)),
            ),
            limit(
                "build.relink",
                "milliseconds",
                as_f64(usize::try_from(build.relink_ms).unwrap_or(usize::MAX)),
            ),
            limit(
                "build.binaryGrowthPerMod",
                "bytes",
                as_f64(usize::try_from(build.bytes_per_mod()).unwrap_or(usize::MAX)),
            ),
        ],
    })
}

/// One budget line: what was measured, and what M3 may not exceed.
fn limit(metric: &str, unit: &str, measured: f64) -> Value {
    json!({
        "metric": metric,
        "unit": unit,
        "measured": round(measured),
        "limit": round((measured * HEADROOM).ceil()),
        "operator": "less-than-or-equal",
    })
}

/// What this measurement does not say, said before someone reads it as saying it.
///
/// Named apart from `budget.limits`, which are numbers rather than prose.
fn caveats() -> Value {
    json!([
        "The pipeline runs no JVM. Reaching the loader from the game costs what ADG-1 measured \
         and the two add.",
        "The no-op mod body does nothing, so every loader figure is a floor rather than a \
         prediction of what a real mod set costs.",
        "The scaling curve uses copies of one package, so it measures the loader's cost per \
         package and not the cost of a dependency graph with real edges.",
        "Integrity verification reads the package's artifacts, so the pipeline figures include \
         filesystem work against a warm page cache. A cold cache is slower and is not measured.",
        "The generated pack's mods are one struct each. A pack of real mods compiles slower and \
         links larger, so the per-mod build figures are also floors.",
        "Every build figure is one unrepeated sample: one cold baseline build, one cold pack \
         build, one rebuild, differenced. There is no variance and no repetition behind them.",
        "Cargo compiles the generated pack across every core this machine has, so the per-mod \
         compile figure is amortized throughput on a sixteen-thread host rather than what one \
         mod costs to compile.",
        "Nothing here measures an ADG-2 candidate. The mod call is the in-process safe-Rust call \
         M2 has, and the pack figures describe a linked set because that is the only shape \
         available while ADG-2 is deferred — not because it was chosen.",
        "The mod call hides its receiver and its result from the optimizer, without which a \
         thousand devirtualized counter increments fold into one add and the measurement reads \
         as 0.02 ns. That barrier costs something itself, so this figure is not comparable with \
         ADG-2's 2.50 ns linked call, which was measured a different way."
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

/// Rounds to two decimals.
///
/// Enough to keep a single-digit-nanosecond call distinguishable from its
/// neighbour, and not so much that a measurement reads as more precise than a
/// wall clock can be.
fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("cannot resolve the repository root: {error}"))
}
