//! Measures the ADG-1 JVM entry candidates and writes their evidence.
//!
//! Four candidates share one workload, one hook target, and one report shape, so
//! what differs between them is the entry mechanism: a control with no hook, an
//! instrumentation agent whose injected call stays in Java, the same agent
//! calling an exported JNI symbol, and a Rust-hosted VM binding the same
//! callback with `RegisterNatives`. Two further mechanisms are recorded as
//! rejected rather than measured.
//!
//! The runner itself needs no unsafe: only the two boundary bindings do.

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
    HashedArtifact, Identity, JavaIdentity, Measurement, Memory, Operability, Packaging,
    PrismLaunch, Revision, Safety, SpikeResult, Startup, Target, Trial, TrialKind, Unit,
};
use harness::{
    ChildTrial, HarnessError, JavaRuntime, PINNED_JAVA_ENV, PROBE_TIMEOUT, PinnedInputs,
    TRIAL_TIMEOUT, TrialReport,
};

/// Pinned game jar the hook target is read from; it is never tracked.
const CLIENT_JAR: &str = "tests/fixtures/local/tooling/minecraft-26.1.2/client.jar";
/// Class holding the pinned hook target.
const HOOK_CLASS: &str = "net.minecraft.util.StringUtil";
/// Hook target: a small, side-effect-free method that initializes standalone.
const HOOK_METHOD: &str = "isAllowedChatCharacter";
/// Name `System.loadLibrary` resolves for the JNI-export candidate.
const LIBRARY: &str = "ravel_spike_adg1";
/// File that name resolves to.
const LIBRARY_FILE: &str = "libravel_spike_adg1.so";
/// Binary that lets Rust own process start.
const HOSTED_BINARY: &str = "ravel-spike-adg-1-hosted";
/// Shared trial entry point.
const TRIAL_CLASS: &str = "dev.ravel.spike.Trial";
/// Invocations discarded before the measured window.
const WARMUP: u64 = 200_000;
/// Timed batches in the measured window.
const SAMPLES: u64 = 20_000;
/// Invocations per timed batch.
const BATCH: u64 = 64;
/// Invocations the measured window covers.
const INVOCATIONS: u64 = SAMPLES * BATCH;
/// The one hardware profile M1 compares on.
const HARDWARE_PROFILE: &str = "linux-reference-v1";
/// Where results belong for the milestone gate to find them.
const RESULT_DIRECTORY: &str = "evidence/m1/spikes/adg-1";

fn main() -> ExitCode {
    match run() {
        Ok(written) => {
            println!("wrote {written} ADG-1 spike results to {RESULT_DIRECTORY}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ADG-1 measurement failed: {error}");
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

    let mut control_ready = None;
    let mut written = 0;
    for entry in [
        Entry::Baseline,
        Entry::AgentJava,
        Entry::AgentNative,
        Entry::Hosted,
    ] {
        let evidence = measure(entry, &layout)?;
        if entry.is_control() {
            control_ready = Some(evidence.trial.ready_nanos);
        }
        let result = assemble(entry, &evidence, &layout, &pinned, &revision, control_ready)?;
        written += usize::from(write(&layout, entry.id(), &result)?);
    }

    for rejection in REJECTIONS {
        let result = rejected(rejection, &pinned, &java, &revision)?;
        written += usize::from(write(&layout, rejection.id, &result)?);
    }

    Ok(written)
}

/// One measured entry mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    /// No hook at all: the cost of the pinned call site on its own.
    Baseline,
    /// Instrumentation agent whose injected call stays inside the JVM.
    AgentJava,
    /// Instrumentation agent calling an exported, JNI-mangled symbol.
    AgentNative,
    /// Rust owns process start and binds the same callback with `RegisterNatives`.
    Hosted,
}

impl Entry {
    fn id(self) -> &'static str {
        match self {
            Self::Baseline => "adg1.control-no-hook",
            Self::AgentJava => "adg1.agent-java-callback",
            Self::AgentNative => "adg1.agent-jni-export",
            Self::Hosted => "adg1.hosted-register-natives",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Baseline => "Control: pinned call site with no hook",
            Self::AgentJava => "java.lang.instrument agent, callback stays in Java",
            Self::AgentNative => "java.lang.instrument agent, exported JNI symbol",
            Self::Hosted => "Rust-hosted JVM, callback bound with RegisterNatives",
        }
    }

    fn boundary(self) -> &'static str {
        match self {
            Self::Baseline => "none",
            Self::AgentJava => "java",
            Self::AgentNative => "native",
            Self::Hosted => "hosted",
        }
    }

    fn is_control(self) -> bool {
        self == Self::Baseline
    }

    /// Whether this candidate is expected to rewrite the pinned target.
    fn transforms(self) -> bool {
        !self.is_control()
    }

    fn callback(self) -> &'static str {
        if self == Self::AgentJava {
            "onJavaCallback"
        } else {
            "onNativeCallback"
        }
    }

    /// Command that runs this candidate in the requested trial mode.
    fn argv(self, layout: &Layout, mode: &str) -> Vec<String> {
        let agent = format!(
            "{}={HOOK_CLASS}:{HOOK_METHOD}:{}",
            layout.agent_jar.display(),
            self.callback()
        );
        let mut argv = if self == Self::Hosted {
            vec![
                layout.hosted.display().to_string(),
                "--classpath".to_owned(),
                layout.classpath.clone(),
                "--agent".to_owned(),
                agent,
                "--".to_owned(),
            ]
        } else {
            let mut java = vec![
                layout.java.clone(),
                "-cp".to_owned(),
                layout.classpath.clone(),
                // Every candidate loads a library at least once, in the
                // unavailable-runtime scenario, so this stays uniform rather
                // than becoming a difference between candidates.
                "--enable-native-access=ALL-UNNAMED".to_owned(),
            ];
            if self == Self::AgentNative {
                java.push(format!(
                    "-Djava.library.path={}",
                    layout.library_dir.display()
                ));
            }
            if self.transforms() {
                java.push(format!("-javaagent:{agent}"));
            }
            java.push(TRIAL_CLASS.to_owned());
            java
        };

        argv.extend(
            [
                "--candidate",
                self.id(),
                "--boundary",
                self.boundary(),
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

    /// Files a deployment of this candidate would have to install.
    fn artifacts(self, layout: &Layout) -> Vec<PathBuf> {
        let mut paths = vec![layout.trial_jar.clone()];
        if self.transforms() {
            paths.push(layout.agent_jar.clone());
        }
        match self {
            Self::AgentNative => paths.push(layout.library.clone()),
            Self::Hosted => paths.push(layout.hosted.clone()),
            Self::Baseline | Self::AgentJava => {}
        }
        paths
    }

    /// Unsafe surface a production implementation would have to maintain.
    fn safety(self) -> Safety {
        let (blocks, detail) = match self {
            Self::Baseline => (0, "the control crosses no boundary"),
            Self::AgentJava => (
                0,
                "the injected call stays inside the JVM, so no Rust boundary exists",
            ),
            Self::AgentNative => (
                1,
                "one exported `extern \"system\"` symbol carrying the JNI-mangled name",
            ),
            Self::Hosted => (
                2,
                "one raw native-method descriptor and one RegisterNatives call, both \
                 encapsulated by jni",
            ),
        };
        Safety {
            maintained_unsafe_blocks: blocks,
            maintained_unsafe_detail: detail.to_owned(),
            audited_boundary_dependencies: if blocks == 0 {
                Vec::new()
            } else {
                vec![BoundaryDependency {
                    name: "jni".to_owned(),
                    version: "0.22.4".to_owned(),
                    licence: "MIT OR Apache-2.0".to_owned(),
                }]
            },
        }
    }

    fn operability(self) -> Operability {
        match self {
            Self::Baseline => Operability {
                diagnostics: "the control has nothing of its own to diagnose".to_owned(),
                debug_workflow: "a plain Java debugger attaches to the trial".to_owned(),
                crash_attribution: "any crash belongs to the runtime or the game".to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
            Self::AgentJava => Operability {
                diagnostics: "agent failures surface as ordinary Java exceptions with a \
                              complete stack trace"
                    .to_owned(),
                debug_workflow: "one Java debugger covers both the agent and the injected call"
                    .to_owned(),
                crash_attribution: "a failure names the agent class in the Java stack trace"
                    .to_owned(),
                prism_launch: PrismLaunch::StockJvmArguments,
            },
            Self::AgentNative => Operability {
                diagnostics: "Java-side failures keep their stack trace; native failures \
                              surface as a JNI exception or a process abort"
                    .to_owned(),
                debug_workflow: "two debuggers: a Java debugger for the agent and a native \
                                 debugger for the callback"
                    .to_owned(),
                crash_attribution: "a native abort ends the game process and names the \
                                    shared object rather than a Java frame"
                    .to_owned(),
                prism_launch: PrismLaunch::StockJvmArguments,
            },
            Self::Hosted => Operability {
                diagnostics: "Rust owns process start, so bootstrap failures are reported \
                              before a VM exists"
                    .to_owned(),
                debug_workflow: "a native debugger owns the process from the first \
                                 instruction; a Java debugger attaches afterwards"
                    .to_owned(),
                crash_attribution: "the process is the Ravel binary, so a crash is \
                                    attributed to Ravel by default"
                    .to_owned(),
                prism_launch: PrismLaunch::StockWrapperCommand,
            },
        }
    }
}

/// A mechanism rejected without measurement, and why.
struct Rejection {
    id: &'static str,
    title: &'static str,
    reason: &'static str,
    unsafe_blocks: u32,
    unsafe_detail: &'static str,
    operability: &'static str,
}

const REJECTIONS: &[Rejection] = &[
    Rejection {
        id: "adg1.jvmti-native-agent",
        title: "JVMTI native agent",
        reason: "JVMTI reaches the same hook through a raw C environment table: an exported \
                 Agent_OnLoad, a jvmtiEnv function-table dereference per capability, event \
                 callback registration, and class retransformation. It buys nothing the \
                 java.lang.instrument agent does not already provide for this hook shape, \
                 while adding unsafe surface that no audited crate encapsulates.",
        unsafe_blocks: 4,
        unsafe_detail: "enumerated from the JVMTI contract rather than measured: Agent_OnLoad \
                        export, jvmtiEnv table dereference, SetEventCallbacks, and \
                        RetransformClasses",
        operability: "a JVMTI failure is reported by the VM as an agent load error with no \
                      Java stack trace",
    },
    Rejection {
        id: "adg1.external-bytecode-library",
        title: "External Java bytecode library inside the agent",
        reason: "The pinned runtime ships java.lang.classfile, so an external rewriting \
                 library would add a shaded Java dependency to every launch for an API the \
                 runtime already provides. Ravel also ships no Java artifacts it does not \
                 have to, and a shaded library is exactly such an artifact.",
        unsafe_blocks: 0,
        unsafe_detail: "a Java rewriting library needs no unsafe Rust; the rejection is about \
                        shipped dependencies, not safety",
        operability: "a shaded rewriting library has to be version-matched to the runtime by \
                      hand on every game update",
    },
];

/// Everything one measured candidate produced.
struct Evidence {
    trial: ChildTrial,
    started_at: String,
    command_sha256: String,
    failures: Vec<FailureOutcome>,
}

fn measure(entry: Entry, layout: &Layout) -> Result<Evidence, String> {
    let argv = entry.argv(layout, "measure");
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let trial = harness::run_child_trial(&argv[0], &argv[1..], proc_root(), TRIAL_TIMEOUT)
        .map_err(|error| error.to_string())?;

    if trial.report.candidate != entry.id() {
        return Err(format!(
            "the trial reported candidate {} instead of {}",
            trial.report.candidate,
            entry.id()
        ));
    }
    if trial.report.transformed != entry.transforms() {
        return Err(format!(
            "{} reported transformed={} so its measurement describes a different mechanism",
            entry.id(),
            trial.report.transformed
        ));
    }

    let mut failures = in_process_failures(&trial.report)?;
    failures.push(crash_probe(entry, layout)?);
    failures.push(timeout_probe(entry, layout));

    Ok(Evidence {
        trial,
        started_at,
        command_sha256: harness::command_sha256(&argv),
        failures,
    })
}

/// Reads the three scenarios a trial can exercise inside its own process.
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
            outcome: behaviour(&reported.outcome)?,
            detail: reported.detail.clone(),
        })
    })
    .collect()
}

fn behaviour(reported: &str) -> Result<FailureBehaviour, String> {
    match reported {
        "failed-closed" => Ok(FailureBehaviour::FailedClosed),
        "continued" => Ok(FailureBehaviour::Continued),
        "not-evaluated" => Ok(FailureBehaviour::NotEvaluated),
        other => Err(format!("the trial reported the unknown outcome {other}")),
    }
}

/// Observes what a deliberate crash inside the candidate does to the process.
fn crash_probe(entry: Entry, layout: &Layout) -> Result<FailureOutcome, String> {
    let argv = entry.argv(layout, "crash");
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        // Any runtime error file the crash leaves behind belongs in the work
        // directory, not in the repository root.
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

/// Observes what a candidate that never reaches readiness costs the harness.
fn timeout_probe(entry: Entry, layout: &Layout) -> FailureOutcome {
    let argv = entry.argv(layout, "hang");
    let (outcome, detail) = match harness::run_child_trial(
        &argv[0],
        &argv[1..],
        proc_root(),
        PROBE_TIMEOUT,
    ) {
        Err(HarnessError::Timeout(_)) => (
            FailureBehaviour::FailedClosed,
            format!(
                "the harness killed a candidate that never announced readiness within {} seconds",
                PROBE_TIMEOUT.as_secs()
            ),
        ),
        Ok(_) => (
            FailureBehaviour::Continued,
            "the hang mode reported a measurement".to_owned(),
        ),
        Err(HarnessError::Command(_)) => (
            FailureBehaviour::FailedClosed,
            "the hung candidate died before it could report".to_owned(),
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
    entry: Entry,
    evidence: &Evidence,
    layout: &Layout,
    pinned: &PinnedInputs,
    revision: &Revision,
    control_ready: Option<u64>,
) -> Result<SpikeResult, String> {
    let report = &evidence.trial.report;
    let java = jvm_identity(report, pinned)?;
    let artifacts = hashed(entry, layout)?;
    let boundary_bytes = as_float(report.boundary_bytes_per_call);

    Ok(SpikeResult {
        schema_version: 1,
        protocol_version: "spike-v1".to_owned(),
        gate: Gate::JvmEntry,
        candidate: Candidate {
            id: entry.id().to_owned(),
            title: entry.title().to_owned(),
            disposition: Disposition::Measured,
            rejection_reason: None,
        },
        trial: Trial {
            revision: revision.clone(),
            command_sha256: evidence.command_sha256.clone(),
            started_at: evidence.started_at.clone(),
            duration_ms: evidence.trial.duration_ms,
            clock: Clock::Monotonic,
            warmup_invocations: report.warmup,
            measured_invocations: INVOCATIONS,
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
            candidate_delta: control_delta(evidence.trial.ready_nanos, control_ready),
        },
        calls: Calls {
            invocation_count: INVOCATIONS,
            callback_nanoseconds: Distribution::from_batches(
                "spike.trial",
                &report.batch_nanos,
                BATCH,
            ),
        },
        boundary: Boundary {
            bytes_per_call: Measurement::exact("spike.trial", boundary_bytes, Unit::BytesPerCall),
            total_bytes: Measurement::exact(
                "spike.trial",
                boundary_bytes * as_float(INVOCATIONS),
                Unit::Bytes,
            ),
        },
        allocation: Allocation {
            allocations_per_call: Measurement::unavailable(
                "spike.jmx",
                "the runtime exposes allocated bytes per thread but not allocation counts",
            ),
            allocated_bytes_per_call: match report.allocated_bytes_per_call {
                // A difference of two counter reads over a batch, so sampled
                // rather than exact.
                Some(bytes) => Measurement::estimated("spike.jmx", bytes, Unit::BytesPerCall),
                None => Measurement::unavailable(
                    "spike.jmx",
                    "the runtime did not expose per-thread allocation",
                ),
            },
        },
        packaging: packaging(&artifacts, installed_bytes(entry, layout)?),
        failures: evidence.failures.clone(),
        safety: entry.safety(),
        operability: entry.operability(),
        artifacts,
        isolation: None,
        abi_evolution: None,
        mod_to_mod_calls: None,
        binding_generation: None,
        state_ownership: None,
        prism_integration: None,
        deviations: deviations(entry, &java, pinned),
    })
}

/// The JVM an ADG-1 trial ran on, which it must always report.
fn jvm_identity(report: &TrialReport, pinned: &PinnedInputs) -> Result<JavaIdentity, String> {
    let (version, runtime) = report
        .java_version
        .as_ref()
        .zip(report.java_runtime.as_ref())
        .ok_or_else(|| "an ADG-1 trial must report the JVM it ran on".to_owned())?;
    Ok(JavaIdentity {
        pinned: *version == pinned.java,
        version: version.clone(),
        runtime: runtime.clone(),
    })
}

/// Differences between the planned and the executed trial.
fn deviations(entry: Entry, java: &JavaIdentity, pinned: &PinnedInputs) -> Vec<Deviation> {
    let mut deviations = Vec::new();
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
    if entry.is_control() {
        deviations.push(Deviation {
            summary: "the control installs no callback, so its callback distribution records \
                      the unhooked cost of the same call site; the other candidates are \
                      comparable against it rather than against each other only"
                .to_owned(),
            classification: Classification::NonBlocking,
        });
    }
    deviations
}

/// Bootstrap and retained cost observed at the candidate's ready boundary.
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

/// Startup cost this candidate adds over the control.
fn control_delta(ready_nanos: u64, control_ready: Option<u64>) -> Measurement {
    match control_ready {
        Some(control) if ready_nanos >= control => Measurement::estimated(
            "spike.harness",
            milliseconds(ready_nanos - control),
            Unit::Milliseconds,
        ),
        Some(_) => Measurement::unavailable(
            "spike.harness",
            "the candidate reached readiness no later than the control, so the difference is \
             inside startup noise",
        ),
        None => Measurement::unavailable("spike.harness", "the control was not measured"),
    }
}

fn rejected(
    rejection: &Rejection,
    pinned: &PinnedInputs,
    java: &JavaRuntime,
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
        gate: Gate::JvmEntry,
        candidate: Candidate {
            id: rejection.id.to_owned(),
            title: rejection.title.to_owned(),
            disposition: Disposition::Rejected,
            rejection_reason: Some(rejection.reason.to_owned()),
        },
        trial: Trial {
            revision: revision.clone(),
            // No command ran, so the command hash is the hash of no command.
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
            java: Some(JavaIdentity {
                version: java.version.clone(),
                runtime: java.runtime.clone(),
                pinned: java.pinned,
            }),
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
            maintained_unsafe_blocks: rejection.unsafe_blocks,
            maintained_unsafe_detail: rejection.unsafe_detail.to_owned(),
            audited_boundary_dependencies: Vec::new(),
        },
        operability: Operability {
            diagnostics: rejection.operability.to_owned(),
            debug_workflow: "not evaluated".to_owned(),
            crash_attribution: "not evaluated".to_owned(),
            prism_launch: PrismLaunch::NotEvaluated,
        },
        artifacts: Vec::new(),
        isolation: None,
        abi_evolution: None,
        mod_to_mod_calls: None,
        binding_generation: None,
        state_ownership: None,
        prism_integration: None,
        deviations: Vec::new(),
    })
}

fn hashed(entry: Entry, layout: &Layout) -> Result<Vec<HashedArtifact>, String> {
    entry
        .artifacts(layout)
        .iter()
        .map(|path| {
            Ok(HashedArtifact {
                // Base names only: an artifact path would identify the machine.
                name: base_name(path)?,
                sha256: harness::sha256_file(path).map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

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

fn installed_bytes(entry: Entry, layout: &Layout) -> Result<u64, String> {
    let mut total = 0;
    for path in entry.artifacts(layout) {
        let name = base_name(&path)?;
        total += fs::metadata(&path)
            .map_err(|error| format!("cannot size {name}: {error}"))?
            .len();
    }
    Ok(total)
}

fn write(layout: &Layout, id: &str, result: &SpikeResult) -> Result<bool, String> {
    let file = format!("{}.json", id.replace('.', "-"));
    harness::write_result(&layout.root.join(RESULT_DIRECTORY).join(file), result)
        .map_err(|error| error.to_string())
}

/// Where the trial inputs live and how they are built.
struct Layout {
    root: PathBuf,
    work: PathBuf,
    classes: PathBuf,
    sources: PathBuf,
    client_jar: PathBuf,
    trial_jar: PathBuf,
    agent_jar: PathBuf,
    classpath: String,
    library_dir: PathBuf,
    library: PathBuf,
    hosted: PathBuf,
    java: String,
}

impl Layout {
    fn discover(root: PathBuf, java: &str) -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("cannot resolve this executable: {error}"))?;
        let library_dir = executable
            .parent()
            .ok_or_else(|| "this executable has no directory".to_owned())?
            .to_path_buf();

        let client_jar = root.join(CLIENT_JAR);
        if !client_jar.is_file() {
            return Err(format!(
                "the pinned client jar is absent; place it at {CLIENT_JAR} as \
                 tests/fixtures/local/README.md describes"
            ));
        }

        let work = root.join("target/spike/adg-1");
        let trial_jar = work.join("trial.jar");
        let classpath = format!("{}:{}", trial_jar.display(), client_jar.display());
        Ok(Self {
            sources: root.join("experiments/adg-1-jvm-entry/java"),
            classes: work.join("classes"),
            agent_jar: work.join("agent.jar"),
            library: library_dir.join(LIBRARY_FILE),
            hosted: library_dir.join(HOSTED_BINARY),
            java: java.to_owned(),
            root,
            work,
            client_jar,
            trial_jar,
            classpath,
            library_dir,
        })
    }

    /// Compiles the shared trial classes and packages the agent.
    fn build(&self) -> Result<(), String> {
        fs::create_dir_all(&self.classes)
            .map_err(|error| format!("cannot create the work directory: {error}"))?;

        let mut sources = Vec::new();
        for entry in fs::read_dir(self.sources.join("dev/ravel/spike"))
            .map_err(|error| format!("cannot list the trial sources: {error}"))?
        {
            let path = entry
                .map_err(|error| format!("cannot list the trial sources: {error}"))?
                .path();
            if path.extension() == Some(OsStr::new("java")) {
                sources.push(path);
            }
        }
        sources.sort();

        let mut javac = vec![
            "-cp".to_owned(),
            self.client_jar.display().to_string(),
            "-d".to_owned(),
            self.classes.display().to_string(),
        ];
        javac.extend(sources.iter().map(|path| path.display().to_string()));
        // javac and jar always come from the development JDK: the pinned game
        // runtime ships no compiler.
        run_tool("javac", &javac)?;

        let manifest = self.work.join("agent.manifest");
        fs::write(&manifest, "Premain-Class: dev.ravel.spike.Agent\n")
            .map_err(|error| format!("cannot write the agent manifest: {error}"))?;
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
        run_tool(
            "jar",
            &[
                "--create".to_owned(),
                "--file".to_owned(),
                self.agent_jar.display().to_string(),
                "--manifest".to_owned(),
                manifest.display().to_string(),
                "-C".to_owned(),
                self.classes.display().to_string(),
                ".".to_owned(),
            ],
        )?;

        for (path, name) in [(&self.library, LIBRARY_FILE), (&self.hosted, HOSTED_BINARY)] {
            if !path.is_file() {
                return Err(format!(
                    "{name} is missing; build the crate before measuring it"
                ));
            }
        }
        Ok(())
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
