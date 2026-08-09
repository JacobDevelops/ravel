//! Measures the ADG-4 state-ownership models and writes their evidence.
//!
//! One representative object flow, three owners. What the comparison shows is a
//! trade rather than a winner on one axis: bytes duplicated against bytes crossed
//! per flow.
//!
//! The runner itself needs no unsafe.

#![forbid(unsafe_code)]

use std::ffi::OsStr;
use std::fs;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use ravel_spike_harness as harness;

use harness::result::{
    Allocation, Boundary, BoundaryDependency, Calls, Candidate, Classification, Clock, Deviation,
    Disposition, Distribution, FailureBehaviour, FailureOutcome, FailureScenario, Gate,
    HashedArtifact, Identity, JavaIdentity, Measurement, Memory, Operability, OwnershipModel,
    Packaging, PrismLaunch, Revision, Safety, SpikeResult, Startup, StateOwnership, Target, Trial,
    TrialKind, Unit,
};
use harness::{
    ChildTrial, HarnessError, PINNED_JAVA_ENV, PROBE_TIMEOUT, PinnedInputs, TRIAL_TIMEOUT,
    TrialReport,
};

/// Name `System.loadLibrary` resolves.
const LIBRARY: &str = "ravel_spike_adg4";
/// File that name resolves to.
const LIBRARY_FILE: &str = "libravel_spike_adg4.so";
/// Shared trial entry point.
const TRIAL_CLASS: &str = "dev.ravel.spike4.Flow";
/// Flows discarded before the measured window.
const WARMUP: u64 = 200_000;
/// Timed batches in the measured window.
const SAMPLES: u64 = 20_000;
/// Flows per timed batch.
const BATCH: u64 = 64;
/// Flows the measured window covers.
const FLOWS: u64 = SAMPLES * BATCH;
/// The one hardware profile M1 compares on.
const HARDWARE_PROFILE: &str = "linux-reference-v1";
/// Where results belong for the milestone gate to find them.
const RESULT_DIRECTORY: &str = "evidence/m1/spikes/adg-4";

fn main() -> ExitCode {
    match run() {
        Ok(written) => {
            println!("wrote {written} ADG-4 spike results to {RESULT_DIRECTORY}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ADG-4 measurement failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<usize, String> {
    let root = repository_root()?;
    let pinned = harness::pinned_inputs(&root).map_err(|error| error.to_string())?;
    let java = harness::java_runtime(&pinned.java).map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;

    let layout = Layout::discover(root, &java.program)?;
    layout.build()?;

    let mut written = 0;
    for owner in [Owner::Java, Owner::Rust, Owner::Mirrored] {
        let evidence = measure(owner, &layout)?;
        let result = assemble(owner, &evidence, &layout, &pinned, &revision)?;
        let file = format!("adg4-{}.json", owner.flag());
        written += usize::from(
            harness::write_result(&layout.root.join(RESULT_DIRECTORY).join(file), &result)
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(written)
}

/// Who holds the state between flows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    /// The JVM object holds it; every flow ships what it needs.
    Java,
    /// The native side holds it; Java ships a handle.
    Rust,
    /// Both hold a copy; Java ships only what moved.
    Mirrored,
}

impl Owner {
    fn flag(self) -> &'static str {
        match self {
            Self::Java => "java-authoritative",
            Self::Rust => "rust-authoritative",
            Self::Mirrored => "mirrored",
        }
    }

    fn id(self) -> String {
        format!("adg4.{}", self.flag())
    }

    fn title(self) -> &'static str {
        match self {
            Self::Java => "Java owns the state; every flow ships what it needs",
            Self::Rust => "The loader owns the state; Java holds a handle",
            Self::Mirrored => "Both sides hold a copy; only changes ship",
        }
    }

    fn model(self) -> OwnershipModel {
        match self {
            Self::Java => OwnershipModel::JavaAuthoritative,
            Self::Rust => OwnershipModel::RustAuthoritative,
            Self::Mirrored => OwnershipModel::Mirrored,
        }
    }

    fn operability(self) -> Operability {
        match self {
            Self::Java => Operability {
                diagnostics: "the game's own object is the only copy, so a debugger or a crash \
                              report shows the truth"
                    .to_owned(),
                debug_workflow: "one Java debugger sees all state; the native side holds nothing \
                                 to inspect"
                    .to_owned(),
                crash_attribution: "a wrong value is attributable to the flow that computed it, \
                                    because nothing else could have changed it"
                    .to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
            Self::Rust => Operability {
                diagnostics: "the loader holds the truth, so the game's own tools cannot show \
                              it and Ravel owes a way to read it"
                    .to_owned(),
                debug_workflow: "a native debugger for the state, a Java debugger for the game \
                                 that no longer holds it"
                    .to_owned(),
                crash_attribution: "state surviving in the loader outlives the Java frame that \
                                    set it, which makes attribution harder, not easier"
                    .to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
            Self::Mirrored => Operability {
                diagnostics: "two copies means a diagnostic has to say which one it read, and a \
                              divergence between them is a class of bug the other models cannot \
                              have"
                    .to_owned(),
                debug_workflow: "both debuggers, and a question about which copy is right"
                    .to_owned(),
                crash_attribution: "a stale mirror produces a wrong value with no failing call \
                                    to attribute it to"
                    .to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
        }
    }

    /// What this model costs in correctness risk rather than in bytes.
    fn risk(self) -> &'static str {
        match self {
            Self::Java => "no copy can go stale, because there is one copy",
            Self::Rust => {
                "the game cannot see the state it is supposed to own, so every Java \
                           read becomes a boundary crossing that this flow does not count"
            }
            Self::Mirrored => "the mirror can go stale, and nothing in the mechanism notices",
        }
    }
}

/// Everything one measured model produced.
struct Evidence {
    trial: ChildTrial,
    started_at: String,
    command_sha256: String,
    failures: Vec<FailureOutcome>,
}

fn measure(owner: Owner, layout: &Layout) -> Result<Evidence, String> {
    let argv = layout.command(owner, "measure");
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let trial = harness::run_child_trial(&argv[0], &argv[1..], proc_root(), TRIAL_TIMEOUT)
        .map_err(|error| error.to_string())?;
    if trial.report.candidate != owner.flag() {
        return Err(format!(
            "the trial reported candidate {} instead of {}",
            trial.report.candidate,
            owner.flag()
        ));
    }

    let mut failures = in_process_failures(&trial.report)?;
    failures.push(crash_probe(owner, layout)?);
    failures.push(timeout_probe(owner, layout));

    Ok(Evidence {
        trial,
        started_at,
        command_sha256: harness::command_sha256(&argv),
        failures,
    })
}

/// Reads the three scenarios a trial exercises inside its own process.
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

/// Observes what a flow that ends the process does to the trial.
fn crash_probe(owner: Owner, layout: &Layout) -> Result<FailureOutcome, String> {
    let argv = layout.command(owner, "crash");
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(&layout.work)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot run the crash probe: {error}"))?;

    let (outcome, detail) = match (status.code(), status.signal()) {
        (Some(0), _) => (
            FailureBehaviour::Continued,
            "the crash mode exited successfully".to_owned(),
        ),
        (Some(code), _) => (
            FailureBehaviour::FailedClosed,
            format!("the process exited with status {code} and reported nothing"),
        ),
        (None, Some(signal)) => (
            FailureBehaviour::FailedClosed,
            format!("the process died on signal {signal} and reported nothing"),
        ),
        (None, None) => (
            FailureBehaviour::NotEvaluated,
            "the crash mode produced no readable exit status".to_owned(),
        ),
    };
    Ok(FailureOutcome {
        scenario: FailureScenario::Crash,
        outcome,
        detail,
    })
}

/// Observes what a trial that never reaches readiness costs the harness.
fn timeout_probe(owner: Owner, layout: &Layout) -> FailureOutcome {
    let argv = layout.command(owner, "hang");
    let (outcome, detail) =
        match harness::run_child_trial(&argv[0], &argv[1..], proc_root(), PROBE_TIMEOUT) {
            Err(HarnessError::Timeout(_)) => (
                FailureBehaviour::FailedClosed,
                format!(
                    "the harness killed a trial that never announced readiness within {} seconds",
                    PROBE_TIMEOUT.as_secs()
                ),
            ),
            Ok(_) => (
                FailureBehaviour::Continued,
                "the hang mode reported a measurement".to_owned(),
            ),
            Err(HarnessError::Command(_)) => (
                FailureBehaviour::FailedClosed,
                "the hung trial died before it could report".to_owned(),
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
    owner: Owner,
    evidence: &Evidence,
    layout: &Layout,
    pinned: &PinnedInputs,
    revision: &Revision,
) -> Result<SpikeResult, String> {
    let report = &evidence.trial.report;
    let java = jvm_identity(report, pinned)?;
    let duplicated = metric(report, "state.duplicated_bytes")?;
    let per_flow = metric(report, "state.boundary_bytes_per_flow")?;
    let artifacts = hashed(layout)?;

    Ok(SpikeResult {
        schema_version: 1,
        protocol_version: "spike-v1".to_owned(),
        gate: Gate::StateOwnership,
        candidate: Candidate {
            id: owner.id(),
            title: owner.title().to_owned(),
            disposition: Disposition::Measured,
            rejection_reason: None,
        },
        trial: Trial {
            revision: revision.clone(),
            command_sha256: evidence.command_sha256.clone(),
            started_at: evidence.started_at.clone(),
            duration_ms: evidence.trial.duration_ms,
            clock: Clock::Monotonic,
            warmup_invocations: WARMUP,
            measured_invocations: FLOWS,
        },
        identity: Identity {
            target: Target::X8664Linux,
            hardware_profile: HARDWARE_PROFILE.to_owned(),
            minecraft: pinned.minecraft.clone(),
            prism: pinned.prism.clone(),
            java: Some(java.clone()),
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
                "every model starts the same runtime and library, so there is no control to \
                 subtract that differs from the models",
            ),
        },
        calls: Calls {
            invocation_count: FLOWS,
            callback_nanoseconds: Distribution::from_batches(
                "spike.trial",
                &report.batch_nanos,
                BATCH,
            ),
        },
        boundary: Boundary {
            bytes_per_call: Measurement::exact("spike.trial", per_flow, Unit::BytesPerCall),
            total_bytes: Measurement::exact("spike.trial", per_flow * as_float(FLOWS), Unit::Bytes),
        },
        allocation: no_allocation_counter(),
        packaging: packaging(&artifacts, installed_bytes(layout)?),
        failures: evidence.failures.clone(),
        safety: Safety {
            maintained_unsafe_blocks: 1,
            maintained_unsafe_detail: "every model reaches the loader through the one exported \
                                       symbol ADG-1 selected; ownership adds none of its own"
                .to_owned(),
            audited_boundary_dependencies: vec![BoundaryDependency {
                name: "jni".to_owned(),
                version: "0.22.4".to_owned(),
                licence: "MIT OR Apache-2.0".to_owned(),
            }],
        },
        operability: owner.operability(),
        artifacts,
        isolation: None,
        abi_evolution: None,
        mod_to_mod_calls: None,
        binding_generation: None,
        state_ownership: Some(StateOwnership {
            model: owner.model(),
            duplicated_state_bytes: Measurement::exact("spike.trial", duplicated, Unit::Bytes),
            boundary_bytes_per_flow: Measurement::exact("spike.trial", per_flow, Unit::Bytes),
        }),
        prism_integration: None,
        deviations: deviations(owner, &java, pinned),
    })
}

/// What a flow of primitives has instead of an allocation count.
fn no_allocation_counter() -> Allocation {
    let reason = "a flow of primitives allocates nothing to count; the trial reports no counter";
    Allocation {
        allocations_per_call: Measurement::unavailable("spike.jmx", reason),
        allocated_bytes_per_call: Measurement::unavailable("spike.jmx", reason),
    }
}

/// What a deployment of any model installs, which is the same two files.
fn packaging(artifacts: &[HashedArtifact], installed_bytes: u64) -> Packaging {
    Packaging {
        installed_bytes,
        artifact_count: u64::try_from(artifacts.len()).unwrap_or(u64::MAX),
        required_components: artifacts
            .iter()
            .map(|artifact| artifact.name.clone())
            .collect(),
    }
}

/// Bootstrap and retained cost observed at the trial's ready boundary.
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
        retained_classes: match trial.report.loaded_classes {
            Some(classes) => Measurement::exact("spike.jmx", as_float(classes), Unit::Classes),
            None => Measurement::unavailable("spike.jmx", "the trial reported no class count"),
        },
    }
}

fn deviations(owner: Owner, java: &JavaIdentity, pinned: &PinnedInputs) -> Vec<Deviation> {
    let mut deviations = vec![
        Deviation {
            summary: format!("what this model costs beyond bytes: {}", owner.risk()),
            classification: Classification::NonBlocking,
        },
        Deviation {
            summary: "the flow carries primitives, so nothing here measures what an object \
                      graph would cost to move or mirror; that is M3's question once real \
                      counterparts exist"
                .to_owned(),
            classification: Classification::NonBlocking,
        },
    ];
    if owner == Owner::Java {
        deviations.push(Deviation {
            summary: "Java-authoritative is measured shipping the state by value. Implementing \
                      it by reading the object's fields from native code instead would cost one \
                      extra boundary crossing per field, which ADG-1 prices at about ten \
                      nanoseconds each"
                .to_owned(),
            classification: Classification::NonBlocking,
        });
    }
    if !java.pinned {
        deviations.push(Deviation {
            summary: format!(
                "measured on runtime {} rather than the pinned game runtime {}; set \
                 {PINNED_JAVA_ENV} to the pinned runtime to remove this difference",
                java.version, pinned.java
            ),
            classification: Classification::NonBlocking,
        });
    }
    deviations
}

/// The JVM an ADG-4 trial ran on, which it must always report.
fn jvm_identity(report: &TrialReport, pinned: &PinnedInputs) -> Result<JavaIdentity, String> {
    let (version, runtime) = report
        .java_version
        .as_ref()
        .zip(report.java_runtime.as_ref())
        .ok_or_else(|| "an ADG-4 trial must report the JVM it ran on".to_owned())?;
    Ok(JavaIdentity {
        pinned: *version == pinned.java,
        version: version.clone(),
        runtime: runtime.clone(),
    })
}

/// A number the trial reported under a documented key.
fn metric(report: &TrialReport, key: &str) -> Result<f64, String> {
    report
        .gate_metrics
        .get(key)
        .copied()
        .ok_or_else(|| format!("the trial reported no {key}"))
}

fn hashed(layout: &Layout) -> Result<Vec<HashedArtifact>, String> {
    [&layout.trial_jar, &layout.library]
        .into_iter()
        .map(|path| {
            Ok(HashedArtifact {
                // Base names only: an artifact path would identify the machine.
                name: base_name(path)?,
                sha256: harness::sha256_file(path).map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

fn installed_bytes(layout: &Layout) -> Result<u64, String> {
    let mut total = 0;
    for path in [&layout.trial_jar, &layout.library] {
        let name = base_name(path)?;
        total += fs::metadata(path)
            .map_err(|error| format!("cannot size {name}: {error}"))?
            .len();
    }
    Ok(total)
}

/// Where the trial inputs live and how they are built.
struct Layout {
    root: PathBuf,
    work: PathBuf,
    classes: PathBuf,
    sources: PathBuf,
    trial_jar: PathBuf,
    library: PathBuf,
    library_dir: PathBuf,
    java: String,
}

impl Layout {
    fn discover(root: PathBuf, java: &str) -> Result<Self, String> {
        let library_dir = std::env::current_exe()
            .map_err(|error| format!("cannot resolve this executable: {error}"))?
            .parent()
            .ok_or_else(|| "this executable has no directory".to_owned())?
            .to_path_buf();
        let work = root.join("target/spike/adg-4");
        Ok(Self {
            sources: root.join("experiments/adg-4-state/java"),
            classes: work.join("classes"),
            trial_jar: work.join("flow.jar"),
            library: library_dir.join(LIBRARY_FILE),
            library_dir,
            java: java.to_owned(),
            root,
            work,
        })
    }

    /// Compiles the shared trial class and packages it.
    fn build(&self) -> Result<(), String> {
        fs::create_dir_all(&self.classes)
            .map_err(|error| format!("cannot create the work directory: {error}"))?;

        let mut sources = fs::read_dir(self.sources.join("dev/ravel/spike4"))
            .map_err(|error| format!("cannot list the trial sources: {error}"))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension() == Some(OsStr::new("java")))
            .collect::<Vec<_>>();
        sources.sort();

        let mut javac = vec!["-d".to_owned(), self.classes.display().to_string()];
        javac.extend(sources.iter().map(|path| path.display().to_string()));
        // javac and jar come from the development JDK: the pinned game runtime
        // ships no compiler.
        run_tool("javac", &javac)?;
        run_tool(
            "jar",
            &[
                "--create".to_owned(),
                "--file".to_owned(),
                self.trial_jar.display().to_string(),
                "-C".to_owned(),
                self.classes.display().to_string(),
                ".".to_owned(),
            ],
        )?;

        if !self.library.is_file() {
            return Err(format!(
                "{LIBRARY_FILE} is missing; build the crate before measuring it"
            ));
        }
        Ok(())
    }

    fn command(&self, owner: Owner, mode: &str) -> Vec<String> {
        let mut argv = vec![
            self.java.clone(),
            "-cp".to_owned(),
            self.trial_jar.display().to_string(),
            format!("-Djava.library.path={}", self.library_dir.display()),
            "--enable-native-access=ALL-UNNAMED".to_owned(),
            TRIAL_CLASS.to_owned(),
        ];
        argv.extend(
            [
                "--model",
                owner.flag(),
                "--mode",
                mode,
                "--library",
                LIBRARY,
            ]
            .map(str::to_owned),
        );
        for (name, value) in [
            ("--warmup", WARMUP),
            ("--samples", SAMPLES),
            ("--batch", BATCH),
        ] {
            argv.push(name.to_owned());
            argv.push(value.to_string());
        }
        argv
    }
}

fn run_tool(program: &str, arguments: &[String]) -> Result<(), String> {
    let status = Command::new(program)
        .args(arguments)
        .status()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !status.success() {
        return Err(format!("{program} failed"));
    }
    Ok(())
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("cannot resolve the repository root: {error}"))
}

fn base_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| "an artifact path has no file name".to_owned())
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
