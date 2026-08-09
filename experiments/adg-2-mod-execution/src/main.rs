//! Measures the ADG-2 mod-execution candidates and writes their evidence.
//!
//! Four bindings of one mod body: a native dynamic library, a WebAssembly
//! module, a helper process, and the mod linked into the host. Each one is
//! measured by the same host binary in a child process, so the harness can watch
//! its memory and kill it if it hangs.
//!
//! The runner itself needs no unsafe.

#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use ravel_spike_harness as harness;

use harness::result::{
    Allocation, Boundary, BoundaryDependency, Calls, Candidate, Classification, Clock, Deviation,
    Disposition, Distribution, FailureBehaviour, FailureOutcome, FailureScenario, Gate,
    HashedArtifact, Identity, Measurement, Memory, Note, NoteStatus, Operability, Packaging,
    PrismLaunch, Revision, Safety, SpikeResult, Startup, Target, Trial, TrialKind, Unit,
};
use harness::{ChildTrial, HarnessError, PROBE_TIMEOUT, PinnedInputs, TRIAL_TIMEOUT, TrialReport};

/// Guest artifact for the dynamic-library candidate.
const GUEST_LIBRARY: &str = "libravel_spike_adg2_guest.so";
/// Guest artifact for the WebAssembly candidate.
const GUEST_WASM: &str = "wasm32-unknown-unknown/release/ravel_spike_adg2_guest.wasm";
/// Helper process for the process candidate.
const GUEST_WORKER: &str = "ravel-spike-adg-2-worker";
/// Host binary that runs one candidate.
const HOST_BINARY: &str = "ravel-spike-adg-2-host";
/// The one hardware profile M1 compares on.
const HARDWARE_PROFILE: &str = "linux-reference-v1";
/// Where results belong for the milestone gate to find them.
const RESULT_DIRECTORY: &str = "evidence/m1/spikes/adg-2";

/// In-process bindings measure a long window cheaply.
const IN_PROCESS_PLAN: Plan = Plan {
    warmup: 200_000,
    samples: 20_000,
    batch: 64,
};
/// A pipe round trip is thousands of times slower, so its window is shorter.
/// The percentiles stay comparable because they are per call.
const PROCESS_PLAN: Plan = Plan {
    warmup: 2_000,
    samples: 2_000,
    batch: 8,
};

fn main() -> ExitCode {
    match run() {
        Ok(written) => {
            println!("wrote {written} ADG-2 spike results to {RESULT_DIRECTORY}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ADG-2 measurement failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<usize, String> {
    let layout = Layout::discover()?;
    let pinned = harness::pinned_inputs(&layout.root).map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;

    let mut written = 0;
    for binding in [
        Binding::Static,
        Binding::Dylib,
        Binding::Wasm,
        Binding::Process,
    ] {
        let evidence = measure(binding, &layout)?;
        let result = assemble(binding, &evidence, &layout, &pinned, &revision)?;
        let file = format!("{}.json", binding.id().replace('.', "-"));
        written += usize::from(
            harness::write_result(&layout.root.join(RESULT_DIRECTORY).join(file), &result)
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(written)
}

/// How the host reaches the mod body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Binding {
    /// The mod is linked into the host and called directly.
    Static,
    /// The mod is a native dynamic library resolved by symbol name.
    Dylib,
    /// The mod is a WebAssembly module in a sandbox the host owns.
    Wasm,
    /// The mod is a helper process reached one frame at a time.
    Process,
}

impl Binding {
    fn id(self) -> &'static str {
        match self {
            Self::Static => "adg2.static-composition",
            Self::Dylib => "adg2.native-dylib",
            Self::Wasm => "adg2.wasm-sandbox",
            Self::Process => "adg2.helper-process",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Static => "Mod linked into the loader binary",
            Self::Dylib => "Mod as a native dynamic library over the C ABI",
            Self::Wasm => "Mod as a WebAssembly module in a host-owned sandbox",
            Self::Process => "Mod in a helper process over a synchronous pipe",
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dylib => "dylib",
            Self::Wasm => "wasm",
            Self::Process => "process",
        }
    }

    fn plan(self) -> Plan {
        if self == Self::Process {
            PROCESS_PLAN
        } else {
            IN_PROCESS_PLAN
        }
    }

    /// Artifact the host needs to reach this binding's mod, if any.
    fn artifact(self, layout: &Layout) -> Option<PathBuf> {
        match self {
            Self::Static => None,
            Self::Dylib => Some(layout.library.clone()),
            Self::Wasm => Some(layout.wasm.clone()),
            Self::Process => Some(layout.worker.clone()),
        }
    }

    /// The artifact a mod ships under this binding.
    ///
    /// Deliberately not the host binary: one host binary serves all four
    /// candidates here, so its size says nothing about any of them. What a
    /// binding costs to distribute is what the mod itself weighs, plus any
    /// runtime the host has to carry, which `required_components` names.
    fn artifacts(self, layout: &Layout) -> Vec<PathBuf> {
        match self.artifact(layout) {
            Some(artifact) => vec![artifact],
            // A linked mod is not separately installable: the loader binary is
            // the mod, and shipping a new mod means shipping a new loader.
            None => vec![layout.host.clone()],
        }
    }

    /// Anything beyond the mod artifact that has to be installed with it.
    fn required_components(self, artifacts: &[HashedArtifact]) -> Vec<String> {
        let mut components = artifacts
            .iter()
            .map(|artifact| artifact.name.clone())
            .collect::<Vec<_>>();
        match self {
            Self::Wasm => components.push("a WebAssembly runtime inside the host".to_owned()),
            Self::Dylib => {}
            Self::Static => components.push("a loader binary rebuilt for every mod set".to_owned()),
            Self::Process => components.push("one operating-system process per mod".to_owned()),
        }
        components
    }

    fn command(self, layout: &Layout, mode: &str) -> Vec<String> {
        let plan = self.plan();
        let mut argv = vec![
            layout.host.display().to_string(),
            "--candidate".to_owned(),
            self.flag().to_owned(),
            "--mode".to_owned(),
            mode.to_owned(),
            "--warmup".to_owned(),
            plan.warmup.to_string(),
            "--samples".to_owned(),
            plan.samples.to_string(),
            "--batch".to_owned(),
            plan.batch.to_string(),
        ];
        if let Some(artifact) = self.artifact(layout) {
            argv.push("--artifact".to_owned());
            argv.push(artifact.display().to_string());
        }
        argv
    }

    /// Unsafe surface a production implementation would have to maintain.
    fn safety(self) -> Safety {
        let (blocks, detail) = match self {
            Self::Static => (
                0,
                "a linked mod is an ordinary Rust call; nothing crosses a raw boundary",
            ),
            Self::Dylib => (
                2,
                "opening the library and taking each entry point out of it, both \
                 encapsulated by libloading but written in Ravel",
            ),
            Self::Wasm => (
                0,
                "the runtime owns every raw operation; the host side is safe Rust",
            ),
            Self::Process => (
                0,
                "a pipe and a child process are safe standard library operations",
            ),
        };
        Safety {
            maintained_unsafe_blocks: blocks,
            maintained_unsafe_detail: detail.to_owned(),
            audited_boundary_dependencies: match self {
                Self::Dylib => vec![BoundaryDependency {
                    name: "libloading".to_owned(),
                    version: "0.8.9".to_owned(),
                    licence: "ISC".to_owned(),
                }],
                Self::Wasm => vec![BoundaryDependency {
                    name: "wasmtime".to_owned(),
                    version: "47.0.2".to_owned(),
                    licence: "Apache-2.0 WITH LLVM-exception".to_owned(),
                }],
                Self::Static | Self::Process => Vec::new(),
            },
        }
    }

    fn operability(self) -> Operability {
        match self {
            Self::Static => Operability {
                diagnostics: "one binary, one backtrace, every frame from mod to loader".to_owned(),
                debug_workflow: "one debugger over the whole program".to_owned(),
                crash_attribution: "a mod fault is indistinguishable from a loader fault"
                    .to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
            Self::Dylib => Operability {
                diagnostics: "a mod backtrace crosses the library boundary and keeps its \
                              frames while the library stays loaded"
                    .to_owned(),
                debug_workflow: "one native debugger, with symbols loaded per library".to_owned(),
                crash_attribution: "the faulting address names the mod's library, which \
                                    attributes the crash without naming the mod's function"
                    .to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
            Self::Wasm => Operability {
                diagnostics: "a trap names the module and the guest function, and the host \
                              keeps running to report it"
                    .to_owned(),
                debug_workflow: "guest debugging needs the runtime's own tooling rather \
                                 than a native debugger"
                    .to_owned(),
                crash_attribution: "the runtime attributes the trap to the module before \
                                    the host decides what to do"
                    .to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
            Self::Process => Operability {
                diagnostics: "the mod's own standard error is separate from the game's, and \
                              its exit status survives it"
                    .to_owned(),
                debug_workflow: "the mod can be attached to, restarted, or run under a \
                                 different debugger than the game"
                    .to_owned(),
                crash_attribution: "the operating system attributes the crash to the mod \
                                    process by construction"
                    .to_owned(),
                prism_launch: PrismLaunch::NotEvaluated,
            },
        }
    }

    /// How a mod built against a contract this host no longer offers is handled.
    fn abi_evolution(self, observed: &str) -> Note {
        let detail = match self {
            Self::Static => format!(
                "{observed}; a linked mod cannot drift, because a contract change that \
                 breaks it fails the build instead of the launch"
            ),
            Self::Dylib => format!(
                "{observed}; the C ABI is checked by symbol name only, so a changed \
                 signature is undetectable at load and the host must carry an explicit \
                 version symbol"
            ),
            Self::Wasm => format!(
                "{observed}; the module declares the type of every import and export, so \
                 the runtime rejects a mismatched signature at instantiation"
            ),
            Self::Process => format!(
                "{observed}; the contract is the wire protocol, which can carry its own \
                 version and refuse an unknown opcode without loading anything"
            ),
        };
        Note {
            status: NoteStatus::Observed,
            detail,
        }
    }
}

/// How much work one trial does.
#[derive(Clone, Copy)]
struct Plan {
    warmup: u64,
    samples: u64,
    batch: u64,
}

/// Everything one measured binding produced.
struct Evidence {
    trial: ChildTrial,
    started_at: String,
    command_sha256: String,
    failures: Vec<FailureOutcome>,
    isolation: Note,
    abi_evolution: Note,
}

fn measure(binding: Binding, layout: &Layout) -> Result<Evidence, String> {
    let argv = binding.command(layout, "measure");
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let trial = harness::run_child_trial(&argv[0], &argv[1..], proc_root(), TRIAL_TIMEOUT)
        .map_err(|error| error.to_string())?;
    if trial.report.candidate != binding.flag() {
        return Err(format!(
            "the trial reported candidate {} instead of {}",
            trial.report.candidate,
            binding.flag()
        ));
    }

    let failing = survival_probe(binding, layout, "fail")?;
    let crashing = survival_probe(binding, layout, "crash")?;
    let mut failures = in_process_failures(&trial.report)?;
    failures.push(FailureOutcome {
        scenario: FailureScenario::CallbackFailure,
        outcome: failing.outcome(),
        detail: failing.detail.clone(),
    });
    failures.push(FailureOutcome {
        scenario: FailureScenario::Crash,
        outcome: crashing.outcome(),
        detail: crashing.detail.clone(),
    });
    failures.push(timeout_probe(binding, layout));

    Ok(Evidence {
        trial,
        started_at,
        command_sha256: harness::command_sha256(&argv),
        failures,
        isolation: Note {
            status: NoteStatus::Observed,
            detail: format!(
                "a mod whose body fails: {}. A mod that ends its own process: {}",
                failing.detail, crashing.detail
            ),
        },
        abi_evolution: binding.abi_evolution(&abi_probe(binding, layout)?),
    })
}

/// What a deliberately failing mod did to its host.
struct Survival {
    isolated: bool,
    detail: String,
}

impl Survival {
    fn outcome(&self) -> FailureBehaviour {
        if self.isolated {
            FailureBehaviour::FailedClosed
        } else {
            FailureBehaviour::Continued
        }
    }
}

/// Runs one failure probe and reports whether the host outlived the mod.
///
/// The host prints one line if it is still running afterwards, so isolation is
/// observed rather than argued from the mechanism.
fn survival_probe(binding: Binding, layout: &Layout, mode: &str) -> Result<Survival, String> {
    let argv = binding.command(layout, mode);
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(&layout.work)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run the {mode} probe: {error}"))?;

    let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let survived = reported.starts_with("survived");
    let detail = match (survived, output.status.code(), output.status.signal()) {
        (true, _, _) => format!("the host kept running and reported that it {reported}"),
        (false, _, Some(signal)) => {
            format!("it took the host process down with it on signal {signal}")
        }
        (false, Some(code), None) => {
            format!("the host exited with status {code} and never reported surviving")
        }
        (false, None, None) => "the probe produced no readable outcome".to_owned(),
    };
    Ok(Survival {
        isolated: survived,
        detail,
    })
}

/// Asks the host what happens when a mod is missing an entry point it needs.
fn abi_probe(binding: Binding, layout: &Layout) -> Result<String, String> {
    let argv = binding.command(layout, "abi");
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(&layout.work)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run the ABI probe: {error}"))?;
    let line = String::from_utf8_lossy(&output.stdout).trim().to_owned();

    let mut words = line.split_whitespace();
    Ok(match (words.next(), words.next()) {
        (Some("refused"), Some(version)) => format!(
            "the host read contract version {version} and refused the absent entry point \
             before calling it"
        ),
        (Some("accepted"), Some(version)) => format!(
            "the host read contract version {version} and accepted an entry point the mod \
             does not have"
        ),
        _ => "the ABI probe produced no readable outcome".to_owned(),
    })
}

/// Reads the three scenarios the host observes inside its own process.
fn in_process_failures(report: &TrialReport) -> Result<Vec<FailureOutcome>, String> {
    [
        ("invalid-input", FailureScenario::InvalidInput),
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

/// Observes what a binding that never reaches readiness costs the harness.
fn timeout_probe(binding: Binding, layout: &Layout) -> FailureOutcome {
    let argv = binding.command(layout, "hang");
    let (outcome, detail) =
        match harness::run_child_trial(&argv[0], &argv[1..], proc_root(), PROBE_TIMEOUT) {
            Err(HarnessError::Timeout(_)) => (
                FailureBehaviour::FailedClosed,
                format!(
                    "the harness killed a host that never announced readiness within {} seconds",
                    PROBE_TIMEOUT.as_secs()
                ),
            ),
            Ok(_) => (
                FailureBehaviour::Continued,
                "the hang mode reported a measurement".to_owned(),
            ),
            Err(HarnessError::Command(_)) => (
                FailureBehaviour::FailedClosed,
                "the hung host died before it could report".to_owned(),
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
    binding: Binding,
    evidence: &Evidence,
    layout: &Layout,
    pinned: &PinnedInputs,
    revision: &Revision,
) -> Result<SpikeResult, String> {
    let report = &evidence.trial.report;
    let plan = binding.plan();
    let invocations = plan.samples * plan.batch;
    let artifacts = hashed(binding, layout)?;
    let boundary_bytes = as_float(report.boundary_bytes_per_call);

    Ok(SpikeResult {
        schema_version: 1,
        protocol_version: "spike-v1".to_owned(),
        gate: Gate::ModExecution,
        candidate: Candidate {
            id: binding.id().to_owned(),
            title: binding.title().to_owned(),
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
            measured_invocations: invocations,
        },
        identity: Identity {
            target: Target::X8664Linux,
            hardware_profile: HARDWARE_PROFILE.to_owned(),
            minecraft: pinned.minecraft.clone(),
            prism: pinned.prism.clone(),
            // These trials host no JVM: the execution mechanism is measured on
            // its own, and ADG-1 measured the cost of reaching it.
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
                "every candidate is its own process here, so there is no shared control to \
                 subtract",
            ),
        },
        calls: Calls {
            invocation_count: invocations,
            callback_nanoseconds: Distribution::from_batches(
                "spike.host",
                &report.batch_nanos,
                plan.batch,
            ),
        },
        boundary: Boundary {
            bytes_per_call: Measurement::exact("spike.host", boundary_bytes, Unit::BytesPerCall),
            total_bytes: Measurement::exact(
                "spike.host",
                boundary_bytes * as_float(invocations),
                Unit::Bytes,
            ),
        },
        allocation: Allocation {
            allocations_per_call: match report.allocations_per_call {
                Some(count) => Measurement::estimated("spike.host", count, Unit::CountPerCall),
                None => Measurement::unavailable("spike.host", "the host counted no allocations"),
            },
            allocated_bytes_per_call: match report.allocated_bytes_per_call {
                Some(bytes) => Measurement::estimated("spike.host", bytes, Unit::BytesPerCall),
                None => Measurement::unavailable("spike.host", "the host counted no allocations"),
            },
        },
        packaging: Packaging {
            installed_bytes: installed_bytes(binding, layout)?,
            artifact_count: u64::try_from(artifacts.len()).unwrap_or(u64::MAX),
            required_components: binding.required_components(&artifacts),
        },
        failures: evidence.failures.clone(),
        safety: binding.safety(),
        operability: binding.operability(),
        artifacts,
        isolation: Some(evidence.isolation.clone()),
        abi_evolution: Some(evidence.abi_evolution.clone()),
        mod_to_mod_calls: Some(peer_cost(report, plan.batch)),
        binding_generation: None,
        state_ownership: None,
        prism_integration: None,
        deviations: deviations(binding, report),
    })
}

/// Bootstrap and retained cost observed at the host's ready boundary.
fn memory(trial: &ChildTrial) -> Memory {
    // A binding that puts the mod in another process would report half its
    // memory if the second process were left out.
    let auxiliary = trial.report.auxiliary_rss_bytes.unwrap_or(0);
    let retained = Measurement::exact(
        "spike.harness",
        as_float(trial.retained_rss_bytes + auxiliary),
        Unit::Bytes,
    );
    Memory {
        bootstrap_peak_rss: Measurement::exact(
            "spike.harness",
            as_float(trial.bootstrap_peak_rss_bytes + auxiliary),
            Unit::Bytes,
        ),
        retained_rss: retained,
        retained_classes: Measurement::unavailable(
            "spike.jmx",
            "these trials host no class loader",
        ),
    }
}

/// Cost of one mod-to-mod call, which is two crossings of the same boundary.
fn peer_cost(report: &TrialReport, batch: u64) -> Measurement {
    let Some(percentiles) = harness::percentiles(&report.peer_batch_nanos) else {
        return Measurement::unavailable("spike.host", "the trial collected no mod-to-mod samples");
    };
    Measurement::estimated(
        "spike.host",
        percentiles.median / as_float(batch),
        Unit::Nanoseconds,
    )
}

fn deviations(binding: Binding, report: &TrialReport) -> Vec<Deviation> {
    let mut deviations = vec![Deviation {
        summary: "these trials host no JVM, so the measured cost is the mod-execution \
                  boundary alone; reaching it from the game costs what ADG-1 measured, and \
                  the two add"
            .to_owned(),
        classification: Classification::NonBlocking,
    }];
    if binding == Binding::Process {
        deviations.push(Deviation {
            summary: "the process candidate is measured unbatched, one synchronous request \
                      per call, because that is what a hook needs; batching would amortize \
                      the round trip but cannot make a hook's answer arrive in time"
                .to_owned(),
            classification: Classification::NonBlocking,
        });
    }
    if report.auxiliary_rss_bytes.is_some() {
        deviations.push(Deviation {
            summary: "memory is the sum of two processes sampled at the same boundary by two \
                      collectors, so it is a close total rather than one reading"
                .to_owned(),
            classification: Classification::NonBlocking,
        });
    }
    deviations
}

fn hashed(binding: Binding, layout: &Layout) -> Result<Vec<HashedArtifact>, String> {
    binding
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

fn installed_bytes(binding: Binding, layout: &Layout) -> Result<u64, String> {
    let mut total = 0;
    for path in binding.artifacts(layout) {
        let name = base_name(&path)?;
        total += fs::metadata(&path)
            .map_err(|error| format!("cannot size {name}: {error}"))?
            .len();
    }
    Ok(total)
}

/// Where the trial inputs live.
struct Layout {
    root: PathBuf,
    work: PathBuf,
    host: PathBuf,
    library: PathBuf,
    wasm: PathBuf,
    worker: PathBuf,
}

impl Layout {
    fn discover() -> Result<Self, String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .map_err(|error| format!("cannot resolve the repository root: {error}"))?;
        let executable = std::env::current_exe()
            .map_err(|error| format!("cannot resolve this executable: {error}"))?;
        let profile = executable
            .parent()
            .ok_or_else(|| "this executable has no directory".to_owned())?
            .to_path_buf();
        let target = profile
            .parent()
            .ok_or_else(|| "this executable has no target directory".to_owned())?;

        let work = root.join("target/spike/adg-2");
        fs::create_dir_all(&work)
            .map_err(|error| format!("cannot create a work directory: {error}"))?;

        let layout = Self {
            host: profile.join(HOST_BINARY),
            library: profile.join(GUEST_LIBRARY),
            worker: profile.join(GUEST_WORKER),
            wasm: target.join(GUEST_WASM),
            root,
            work,
        };
        for (path, name) in [
            (&layout.host, HOST_BINARY),
            (&layout.library, GUEST_LIBRARY),
            (&layout.worker, GUEST_WORKER),
            (&layout.wasm, GUEST_WASM),
        ] {
            if !path.is_file() {
                return Err(format!(
                    "{name} is missing; build the ADG-2 guest and host before measuring them"
                ));
            }
        }
        Ok(layout)
    }
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
