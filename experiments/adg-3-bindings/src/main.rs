//! Measures the ADG-3 binding candidates and writes their evidence.
//!
//! Two extractors read the same pinned jar and generate the same binding. Two
//! further mechanisms are recorded as rejected. The generated binding and its
//! mapping lock are the deliverable; that they are reproducible, and that a
//! drifted input is refused, is the evidence.

#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use ravel_spike_adg3 as bindings;
use ravel_spike_harness as harness;

use bindings::Extractor;
use harness::result::{
    Allocation, BindingGeneration, Boundary, Calls, Candidate, Classification, Clock, Deviation,
    Disposition, Distribution, FailureBehaviour, FailureOutcome, FailureScenario, Gate,
    HashedArtifact, Identity, Measurement, Memory, Operability, Packaging, PrismLaunch, Revision,
    Safety, SpikeResult, Startup, Target, Trial, TrialKind, Unit,
};
use harness::{ChildTrial, HarnessError, PROBE_TIMEOUT, PinnedInputs, TRIAL_TIMEOUT, TrialReport};

/// Binary that performs one generation.
const GENERATOR: &str = "ravel-spike-adg-3-generate";
/// The one hardware profile M1 compares on.
const HARDWARE_PROFILE: &str = "linux-reference-v1";
/// Where results belong for the milestone gate to find them.
const RESULT_DIRECTORY: &str = "evidence/m1/spikes/adg-3";

fn main() -> ExitCode {
    match run() {
        Ok(written) => {
            println!("wrote {written} ADG-3 spike results to {RESULT_DIRECTORY}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ADG-3 measurement failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<usize, String> {
    let layout = Layout::discover()?;
    let pinned = harness::pinned_inputs(&layout.root).map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;

    let mut written = 0;
    for extractor in [Extractor::ClassFile, Extractor::Javap] {
        let evidence = measure(extractor, &layout)?;
        let result = assemble(extractor, &evidence, &layout, &pinned, &revision)?;
        write(&layout, &result.candidate.id.clone(), &result)?;
        written += 1;
    }

    // The selected extractor's generation is the one that lands in the tracked
    // version data, so it is written last and from a clean directory.
    install_mapping_lock(&layout)?;

    for rejection in REJECTIONS {
        let result = rejected(rejection, &pinned, &revision)?;
        write(&layout, rejection.id, &result)?;
        written += 1;
    }
    Ok(written)
}

/// A mechanism rejected without measurement, and why.
struct Rejection {
    id: &'static str,
    title: &'static str,
    reason: &'static str,
    diagnostics: &'static str,
}

const REJECTIONS: &[Rejection] = &[
    Rejection {
        id: "adg3.official-mappings",
        title: "Mojang's published mapping file",
        reason: "26.1.2 publishes no client mapping download, and its class files carry real \
                 names, so there is nothing for a mapping file to translate. The pinned version \
                 manifest is the evidence: it lists client and server downloads and no mapping \
                 entry. An obfuscated future version would bring this candidate back, and the \
                 adapter inputs already name the mapping source so that change would be \
                 explicit.",
        diagnostics: "a missing mapping file cannot be diagnosed at all: it simply does not \
                      exist upstream",
    },
    Rejection {
        id: "adg3.third-party-mappings",
        title: "Community mapping sets",
        reason: "A third-party mapping set would add a per-version external dependency, with \
                 its own release cadence and licence, to translate names the jar already \
                 carries. It also puts a human-curated naming layer between the pinned jar and \
                 the generated binding, which is one more thing that can drift silently.",
        diagnostics: "drift in a community mapping set is only visible after it is published",
    },
    Rejection {
        id: "adg3.runtime-reflection",
        title: "Resolve members by reflection at startup",
        reason: "Reflection needs no generation step, which is its whole appeal, and it defers \
                 every version error to the moment a player triggers the hook. Ravel has to \
                 refuse an incompatible mod before a world opens, and a resolution failure \
                 discovered mid-game is the opposite of that. It also costs a lookup per \
                 resolution where a generated constant costs nothing.",
        diagnostics: "a failure names the missing member, but only once the game reaches it",
    },
    Rejection {
        id: "adg3.hand-written-version-data",
        title: "Hand-written version data",
        reason: "Writing descriptors by hand cannot detect drift: there is nothing to compare a \
                 hand-written file against except the jar it was meant to describe, which is \
                 the generation this gate is choosing between. It also does not survive the \
                 hook inventory growing past a page.",
        diagnostics: "a wrong descriptor is found by a runtime failure, not by a build",
    },
];

/// Everything one measured extractor produced.
struct Evidence {
    trial: ChildTrial,
    started_at: String,
    command_sha256: String,
    failures: Vec<FailureOutcome>,
    partial_output: String,
}

fn measure(extractor: Extractor, layout: &Layout) -> Result<Evidence, String> {
    let out = layout.work.join(extractor.flag());
    let argv = layout.command(extractor, &out, "generate");
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let trial = harness::run_child_trial(&argv[0], &argv[1..], proc_root(), TRIAL_TIMEOUT)
        .map_err(|error| error.to_string())?;
    if trial.report.candidate != extractor.flag() {
        return Err(format!(
            "the trial reported candidate {} instead of {}",
            trial.report.candidate,
            extractor.flag()
        ));
    }

    let partial = crash_probe(extractor, layout)?;
    let mut failures = in_process_failures(&trial.report)?;
    failures.push(FailureOutcome {
        scenario: FailureScenario::Crash,
        outcome: FailureBehaviour::FailedClosed,
        detail: partial.clone(),
    });
    failures.push(timeout_probe(extractor, layout));

    Ok(Evidence {
        trial,
        started_at,
        command_sha256: harness::command_sha256(&argv),
        failures,
        partial_output: partial,
    })
}

/// Kills a generation part way and checks what it left behind.
///
/// A build that picks up a half-written binding is worse than a build that
/// fails, so the generator writes to a temporary name and renames.
fn crash_probe(extractor: Extractor, layout: &Layout) -> Result<String, String> {
    let out = layout.work.join(format!("{}-crash", extractor.flag()));
    drop(fs::remove_dir_all(&out));
    let argv = layout.command(extractor, &out, "crash");
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot run the crash probe: {error}"))?;

    let leftovers = fs::read_dir(&out)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|kind| kind == "pending")
        })
        .count();
    let signal = status.signal().map_or_else(
        || "no signal".to_owned(),
        |signal| format!("signal {signal}"),
    );
    Ok(if leftovers == 0 {
        format!("the generator died on {signal} and left no half-written output behind")
    } else {
        format!("the generator died on {signal} and left {leftovers} unfinished files behind")
    })
}

/// Reads the three scenarios the generator observes in its own process.
fn in_process_failures(report: &TrialReport) -> Result<Vec<FailureOutcome>, String> {
    [
        ("invalid-input", FailureScenario::InvalidInput),
        ("callback-failure", FailureScenario::CallbackFailure),
        ("unavailable-runtime", FailureScenario::UnavailableRuntime),
    ]
    .into_iter()
    .map(|(name, scenario)| {
        let reported = report
            .failures
            .iter()
            .find(|failure| failure.scenario == name)
            .ok_or_else(|| format!("the trial did not exercise {name}"))?;
        Ok(FailureOutcome {
            scenario,
            outcome: match reported.outcome.as_str() {
                "failed-closed" => FailureBehaviour::FailedClosed,
                "continued" => FailureBehaviour::Continued,
                other => return Err(format!("the trial reported the unknown outcome {other}")),
            },
            detail: reported.detail.clone(),
        })
    })
    .collect()
}

/// Observes what a generator that never finishes costs the harness.
fn timeout_probe(extractor: Extractor, layout: &Layout) -> FailureOutcome {
    let out = layout.work.join(format!("{}-hang", extractor.flag()));
    let argv = layout.command(extractor, &out, "hang");
    let (outcome, detail) =
        match harness::run_child_trial(&argv[0], &argv[1..], proc_root(), PROBE_TIMEOUT) {
            Err(HarnessError::Timeout(_)) => (
                FailureBehaviour::FailedClosed,
                format!(
                    "the harness killed a generator that never announced readiness within {} \
                     seconds",
                    PROBE_TIMEOUT.as_secs()
                ),
            ),
            Ok(_) => (
                FailureBehaviour::Continued,
                "the hang mode reported a generation".to_owned(),
            ),
            Err(HarnessError::Command(_)) => (
                FailureBehaviour::FailedClosed,
                "the hung generator died before it could report".to_owned(),
            ),
            Err(HarnessError::Io(_) | HarnessError::Report(_)) => (
                FailureBehaviour::NotEvaluated,
                "the hang probe could not be observed".to_owned(),
            ),
        };
    FailureOutcome {
        scenario: FailureScenario::Timeout,
        outcome,
        detail,
    }
}

fn assemble(
    extractor: Extractor,
    evidence: &Evidence,
    layout: &Layout,
    pinned: &PinnedInputs,
    revision: &Revision,
) -> Result<SpikeResult, String> {
    let report = &evidence.trial.report;
    let generated = layout.work.join(extractor.flag());
    let artifacts = hashed(&generated)?;
    let installed = artifacts_bytes(&generated)?;

    Ok(SpikeResult {
        schema_version: 1,
        protocol_version: "spike-v1".to_owned(),
        gate: Gate::Bindings,
        candidate: Candidate {
            id: format!("adg3.{}-extraction", extractor.flag()),
            title: match extractor {
                Extractor::ClassFile => "Parse the pinned jar's class files".to_owned(),
                Extractor::Javap => "Ask the JDK's javap to describe the pinned jar".to_owned(),
            },
            disposition: Disposition::Measured,
            rejection_reason: None,
        },
        trial: Trial {
            revision: revision.clone(),
            command_sha256: evidence.command_sha256.clone(),
            started_at: evidence.started_at.clone(),
            duration_ms: evidence.trial.duration_ms,
            clock: Clock::Monotonic,
            warmup_invocations: 0,
            // One generation is the unit of work here, and the targets it
            // resolved are what it did.
            measured_invocations: whole(metric(report, "generation.targets")?),
        },
        identity: Identity {
            target: Target::X8664Linux,
            hardware_profile: HARDWARE_PROFILE.to_owned(),
            minecraft: pinned.minecraft.clone(),
            prism: pinned.prism.clone(),
            // Generation reads class files; it starts no JVM of its own. The
            // javap extractor starts one per class, recorded as a deviation.
            java: None,
            trial_kind: TrialKind::Harness,
        },
        memory: memory(&evidence.trial),
        startup: Startup {
            process_to_ready: Measurement::exact(
                "spike.harness",
                milliseconds(evidence.trial.ready_nanos),
                Unit::Milliseconds,
            ),
            candidate_delta: Measurement::unavailable(
                "spike.harness",
                "a generator has no control to subtract: its whole run is its cost",
            ),
        },
        calls: Calls {
            invocation_count: 0,
            callback_nanoseconds: Distribution::unavailable(
                "spike.harness",
                "generation crosses no call boundary; its cost is one build-time run",
            ),
        },
        boundary: Boundary {
            bytes_per_call: no_call_boundary(),
            total_bytes: Measurement::exact("spike.harness", as_float(installed), Unit::Bytes),
        },
        allocation: Allocation {
            allocations_per_call: no_call_boundary(),
            allocated_bytes_per_call: no_call_boundary(),
        },
        packaging: packaging(extractor, &artifacts, installed),
        failures: evidence.failures.clone(),
        safety: Safety {
            maintained_unsafe_blocks: 0,
            maintained_unsafe_detail: "reading a class file is reading bytes; the generator and \
                                       both extractors are safe Rust"
                .to_owned(),
            audited_boundary_dependencies: Vec::new(),
        },
        operability: Operability {
            diagnostics: note(report, "extraction.agreement")?,
            debug_workflow: "generation is an ordinary build-time program, so a failure is a \
                             build failure with a message naming the hook that did not resolve"
                .to_owned(),
            crash_attribution: evidence.partial_output.clone(),
            prism_launch: PrismLaunch::NotEvaluated,
        },
        artifacts,
        isolation: None,
        abi_evolution: None,
        mod_to_mod_calls: None,
        binding_generation: Some(BindingGeneration {
            duration_ms: whole(metric(report, "generation.duration_ms")?),
            output_sha256: note(report, "generation.sha256")?,
            output_bytes: whole(metric(report, "generation.bytes")?),
            drift_detected: note(report, "generation.drift")?.starts_with("refused"),
        }),
        state_ownership: None,
        prism_integration: None,
        deviations: deviations(extractor, report)?,
    })
}

/// What a build-time step has instead of a per-call number.
fn no_call_boundary() -> Measurement {
    Measurement::unavailable("spike.harness", "generation crosses no call boundary")
}

/// Peak and retained cost of one generation, which has no bootstrap of its own.
fn memory(trial: &ChildTrial) -> Memory {
    Memory {
        bootstrap_peak_rss: Measurement::exact(
            "spike.harness",
            as_float(trial.bootstrap_peak_rss_bytes),
            Unit::Bytes,
        ),
        retained_rss: Measurement::exact(
            "spike.harness",
            as_float(trial.retained_rss_bytes),
            Unit::Bytes,
        ),
        retained_classes: Measurement::unavailable("spike.jmx", "generation hosts no class loader"),
    }
}

/// What the generated binding weighs, and what the build machine needs to make it.
fn packaging(extractor: Extractor, artifacts: &[HashedArtifact], installed: u64) -> Packaging {
    Packaging {
        installed_bytes: installed,
        artifact_count: u64::try_from(artifacts.len()).unwrap_or(u64::MAX),
        required_components: artifacts
            .iter()
            .map(|artifact| artifact.name.clone())
            .chain(Some(match extractor {
                Extractor::ClassFile => "a zip reader in the build tooling".to_owned(),
                Extractor::Javap => "a JDK on the build machine".to_owned(),
            }))
            .collect(),
    }
}

fn deviations(extractor: Extractor, report: &TrialReport) -> Result<Vec<Deviation>, String> {
    let mut deviations = vec![Deviation {
        summary: format!(
            "a drifted input was refused, observed rather than assumed: {}",
            note(report, "generation.drift")?
        ),
        classification: Classification::NonBlocking,
    }];
    if extractor == Extractor::Javap {
        deviations.push(Deviation {
            summary: "the javap extractor starts a JVM per class, and that JVM's memory is not \
                      included in this trial's memory, which only covers the generator process"
                .to_owned(),
            classification: Classification::NonBlocking,
        });
    }
    if extractor == Extractor::ClassFile {
        deviations.push(Deviation {
            summary: "the class-file extractor reads jar entries through the shell's unzip \
                      rather than a Rust zip reader, so its measured duration includes one \
                      process per class; production would read the archive in process"
                .to_owned(),
            classification: Classification::NonBlocking,
        });
    }
    Ok(deviations)
}

fn rejected(
    rejection: &Rejection,
    pinned: &PinnedInputs,
    revision: &Revision,
) -> Result<SpikeResult, String> {
    let unmeasured = || {
        Measurement::unavailable(
            "spike.harness",
            "the candidate was rejected before measurement",
        )
    };
    Ok(SpikeResult {
        schema_version: 1,
        protocol_version: "spike-v1".to_owned(),
        gate: Gate::Bindings,
        candidate: Candidate {
            id: rejection.id.to_owned(),
            title: rejection.title.to_owned(),
            disposition: Disposition::Rejected,
            rejection_reason: Some(rejection.reason.to_owned()),
        },
        trial: Trial {
            revision: revision.clone(),
            command_sha256: harness::sha256_bytes(&[]),
            started_at: harness::utc_timestamp().map_err(|error| error.to_string())?,
            duration_ms: 0,
            clock: Clock::Monotonic,
            warmup_invocations: 0,
            measured_invocations: 0,
        },
        identity: Identity {
            target: Target::X8664Linux,
            hardware_profile: HARDWARE_PROFILE.to_owned(),
            minecraft: pinned.minecraft.clone(),
            prism: pinned.prism.clone(),
            java: None,
            trial_kind: TrialKind::Harness,
        },
        memory: Memory {
            bootstrap_peak_rss: unmeasured(),
            retained_rss: unmeasured(),
            retained_classes: unmeasured(),
        },
        startup: Startup {
            process_to_ready: unmeasured(),
            candidate_delta: unmeasured(),
        },
        calls: Calls {
            invocation_count: 0,
            callback_nanoseconds: Distribution::unavailable(
                "spike.harness",
                "the candidate was rejected before measurement",
            ),
        },
        boundary: Boundary {
            bytes_per_call: unmeasured(),
            total_bytes: unmeasured(),
        },
        allocation: Allocation {
            allocations_per_call: unmeasured(),
            allocated_bytes_per_call: unmeasured(),
        },
        packaging: Packaging {
            installed_bytes: 0,
            artifact_count: 0,
            required_components: Vec::new(),
        },
        failures: vec![FailureOutcome {
            scenario: FailureScenario::InvalidInput,
            outcome: FailureBehaviour::NotEvaluated,
            detail: "the candidate was rejected before its failure matrix was exercised".to_owned(),
        }],
        safety: Safety {
            maintained_unsafe_blocks: 0,
            maintained_unsafe_detail: "no extraction mechanism needs unsafe Rust; this rejection \
                                       is not about safety"
                .to_owned(),
            audited_boundary_dependencies: Vec::new(),
        },
        operability: Operability {
            diagnostics: rejection.diagnostics.to_owned(),
            debug_workflow: "not evaluated".to_owned(),
            crash_attribution: "not evaluated".to_owned(),
            prism_launch: PrismLaunch::NotEvaluated,
        },
        artifacts: Vec::new(),
        isolation: None,
        abi_evolution: None,
        mod_to_mod_calls: None,
        // A rejected candidate generated nothing, so there is nothing to record.
        binding_generation: None,
        state_ownership: None,
        prism_integration: None,
        deviations: Vec::new(),
    })
}

/// Copies the selected extractor's mapping lock into the tracked version data.
///
/// The lock is generated, so it is written by the generator rather than by hand,
/// and a change to the jar or the inventory changes it. That is how a build
/// notices drift before it trusts a binding.
fn install_mapping_lock(layout: &Layout) -> Result<(), String> {
    let generated = layout
        .work
        .join(Extractor::ClassFile.flag())
        .join("mappings.lock");
    let tracked = layout.root.join(bindings::MAPPINGS_LOCK);
    fs::copy(&generated, &tracked)
        .map(|_| ())
        .map_err(|error| format!("cannot install the mapping lock: {error}"))
}

fn hashed(directory: &Path) -> Result<Vec<HashedArtifact>, String> {
    let mut artifacts = Vec::new();
    for path in generated_files(directory)? {
        artifacts.push(HashedArtifact {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .ok_or_else(|| "a generated file has no name".to_owned())?,
            sha256: harness::sha256_file(&path).map_err(|error| error.to_string())?,
        });
    }
    Ok(artifacts)
}

fn artifacts_bytes(directory: &Path) -> Result<u64, String> {
    let mut total = 0;
    for path in generated_files(directory)? {
        total += fs::metadata(&path)
            .map_err(|error| format!("cannot size a generated file: {error}"))?
            .len();
    }
    Ok(total)
}

fn generated_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = fs::read_dir(directory)
        .map_err(|error| format!("cannot list the generated output: {error}"))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        return Err("the generator wrote nothing".to_owned());
    }
    Ok(files)
}

fn write(layout: &Layout, id: &str, result: &SpikeResult) -> Result<(), String> {
    let file = format!("{}.json", id.replace('.', "-"));
    harness::write_result(&layout.root.join(RESULT_DIRECTORY).join(file), result)
        .map_err(|error| error.to_string())
}

/// A number the generator reported under a documented key.
fn metric(report: &TrialReport, key: &str) -> Result<f64, String> {
    report
        .gate_metrics
        .get(key)
        .copied()
        .ok_or_else(|| format!("the trial reported no {key}"))
}

/// An observation the generator reported under a documented key.
fn note(report: &TrialReport, key: &str) -> Result<String, String> {
    report
        .gate_notes
        .get(key)
        .cloned()
        .ok_or_else(|| format!("the trial reported no {key}"))
}

/// Where the generator and its output live.
struct Layout {
    root: PathBuf,
    work: PathBuf,
    generator: PathBuf,
}

impl Layout {
    fn discover() -> Result<Self, String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .map_err(|error| format!("cannot resolve the repository root: {error}"))?;
        let generator = std::env::current_exe()
            .map_err(|error| format!("cannot resolve this executable: {error}"))?
            .with_file_name(GENERATOR);
        if !generator.is_file() {
            return Err(format!(
                "{GENERATOR} is missing; build it before measuring it"
            ));
        }
        let work = root.join("target/spike/adg-3");
        fs::create_dir_all(&work)
            .map_err(|error| format!("cannot create a work directory: {error}"))?;
        Ok(Self {
            root,
            work,
            generator,
        })
    }

    fn command(&self, extractor: Extractor, out: &Path, mode: &str) -> Vec<String> {
        vec![
            self.generator.display().to_string(),
            "--root".to_owned(),
            self.root.display().to_string(),
            "--out".to_owned(),
            out.display().to_string(),
            "--extractor".to_owned(),
            extractor.flag().to_owned(),
            "--mode".to_owned(),
            mode.to_owned(),
        ]
    }
}

fn proc_root() -> &'static Path {
    Path::new("/proc")
}

#[expect(
    clippy::cast_precision_loss,
    reason = "measured counts stay far below f64 integer precision limits"
)]
fn as_float(value: u64) -> f64 {
    value as f64
}

/// Nanoseconds as milliseconds, so a sub-millisecond interval stays a number.
fn milliseconds(nanos: u64) -> f64 {
    as_float(nanos) / 1_000_000.0
}

/// A reported metric back as the whole number it was.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "these metrics are whole counts the generator reported as numbers"
)]
fn whole(value: f64) -> u64 {
    value.max(0.0) as u64
}
