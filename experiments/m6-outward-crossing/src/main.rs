//! Measures what carrying a mod's game action outward costs, three ways.
//!
//! `ADG-6` rejected drawing a screen from Rust and said what would reopen the
//! question: "if Ravel ever needs to call into the game for another reason,
//! that is a new gate with its own measurement, and this decision should be
//! revisited then rather than assumed". Playing a sound a mod declared is that
//! other reason, and this is that measurement.
//!
//! It differs from ADG-6's own gate in the way that matters. ADG-6 recorded
//! that it could not measure, because one of its two candidates did not exist
//! and building it would have been the decision rather than evidence for it.
//! Both candidates here exist: JNI provides the outward call, and ADG-7's
//! persistence adapter already carries an instruction the other way. So this
//! gate is measurable, and a decision taken without measuring it would be a
//! choice rather than a finding.
//!
//! The runner itself needs no unsafe.

#![forbid(unsafe_code)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use ravel_spike_harness as harness;
use serde_json::{Value, json};

use harness::TRIAL_TIMEOUT;
use harness::result::Distribution;

/// Name `System.loadLibrary` resolves.
const LIBRARY: &str = "ravel_spike_outward";
/// File that name resolves to.
const LIBRARY_FILE: &str = "libravel_spike_outward.so";
/// Shared trial entry point.
const TRIAL_CLASS: &str = "dev.ravel.outward.Action";
/// Requests discarded before the measured window.
const WARMUP: u64 = 100_000;
/// Timed batches in the measured window.
const SAMPLES: u64 = 20_000;
/// Requests per timed batch.
const BATCH: u64 = 64;
/// Actions per request, measured at both ends of the range that matters.
///
/// One is a mod asking for a single thing, where a deferred instruction pays
/// for an array it did not need. Eight is a mod asking for several in one tick,
/// where the direct candidates pay per action and the deferred one does not.
const ACTION_COUNTS: [u64; 2] = [1, 8];
/// The hardware profile every Ravel Linux measurement is compared on.
const HARDWARE_PROFILE: &str = "tests/performance/hardware/linux-reference.toml";
/// Where the record belongs for the M6 gate to find it.
const RESULT: &str = "evidence/m6/outward-crossing.json";

/// The three shapes an outward crossing could take.
#[derive(Clone, Copy)]
enum Candidate {
    /// Rust calls the game method, resolving it on every call.
    DirectLookup,
    /// Rust calls the game method through an identifier resolved once.
    DirectCached,
    /// Rust answers with an instruction and the Java adapter makes the call.
    Deferred,
}

impl Candidate {
    const ALL: [Self; 3] = [Self::DirectLookup, Self::DirectCached, Self::Deferred];

    fn flag(self) -> &'static str {
        match self {
            Self::DirectLookup => "direct-lookup",
            Self::DirectCached => "direct-cached",
            Self::Deferred => "deferred",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::DirectLookup => "Rust calls the game method through JNI, resolving it per action",
            Self::DirectCached => {
                "Rust calls the game method through JNI, through a cached identifier"
            }
            Self::Deferred => "Rust answers with an instruction and the Java adapter performs it",
        }
    }

    /// What this candidate costs beyond nanoseconds.
    fn risk(self) -> &'static str {
        match self {
            Self::DirectLookup | Self::DirectCached => {
                "a refused action leaves a pending Java exception on the calling thread, and every \
                 JNI call the native side makes before clearing it is undefined; the obligation is \
                 the loader's and no type in the workspace expresses it"
            }
            Self::Deferred => {
                "the action happens after the answer, so nothing a mod asked for can be reported \
                 back to the mod in the same call, and an adapter that drops an instruction fails \
                 silently unless something checks"
            }
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(written) => {
            println!("wrote the M6 outward-crossing record to {written}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("M6 outward-crossing measurement failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let root = repository_root()?;
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;
    let pinned = harness::pinned_inputs(&root).map_err(|error| error.to_string())?;
    let java = harness::java_runtime(&pinned.java).map_err(|error| error.to_string())?;
    let profile =
        harness::sha256_file(&root.join(HARDWARE_PROFILE)).map_err(|error| error.to_string())?;

    let layout = Layout::discover(&root, &java.program)?;
    layout.build()?;

    let mut candidates = Vec::new();
    let mut failures = Vec::new();
    let mut reported_java = None;
    for candidate in Candidate::ALL {
        let mut measurements = Vec::new();
        for actions in ACTION_COUNTS {
            let argv = layout.command(candidate, actions);
            let trial = harness::run_child_trial(&argv[0], &argv[1..], proc_root(), TRIAL_TIMEOUT)
                .map_err(|error| error.to_string())?;
            if trial.report.candidate != candidate.flag() {
                return Err(format!(
                    "the trial reported candidate {} instead of {}",
                    trial.report.candidate,
                    candidate.flag()
                ));
            }
            reported_java = trial.report.java_version.clone().or(reported_java);
            if failures.is_empty() {
                failures = trial
                    .report
                    .failures
                    .iter()
                    .map(|failure| {
                        json!({
                            "scenario": failure.scenario,
                            "outcome": failure.outcome,
                            "detail": failure.detail,
                        })
                    })
                    .collect();
            }
            measurements.push(measurement(&trial.report, actions)?);
        }
        candidates.push(json!({
            "id": candidate.flag(),
            "title": candidate.title(),
            "perActionCount": measurements,
            "residualRisk": candidate.risk(),
        }));
    }

    let record = json!({
        "schemaVersion": 1,
        "record": "m6-w2-outward-crossing",
        "milestone": "M6",
        "workPackage": "M6-W2",
        "summary": "What it costs to carry a game action a mod asked for outward into the running \
                    game, measured across the three mechanisms that could carry it, so that \
                    ADG-6's deferred question is decided on a number rather than on a preference.",
        "identity": {
            "target": "x86_64-linux",
            "hardwareProfile": "linux-reference-v1",
            "hardwareProfileSha256": profile,
            "minecraft": pinned.minecraft,
            "javaPinned": pinned.java,
            "javaMeasured": reported_java,
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
            "warmupRequests": WARMUP,
            "measuredRequests": SAMPLES * BATCH,
            "commandSha256": harness::command_sha256(&[
                "cargo",
                "run",
                "--release",
                "-p",
                "ravel-spike-m6-outward-crossing",
            ]),
        },
        "candidates": candidates,
        "failures": failures,
        "whatThisDoesNotSay": caveats(),
    });

    let mut text = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("the record does not serialize: {error}"))?;
    text.push('\n');
    let path = root.join(RESULT);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create the record directory: {error}"))?;
    }
    fs::write(&path, text).map_err(|error| format!("cannot write {RESULT}: {error}"))?;
    Ok(RESULT.to_owned())
}

/// One candidate's numbers at one action count.
fn measurement(report: &harness::TrialReport, actions: u64) -> Result<Value, String> {
    let per_request = Distribution::from_batches("spike.trial", &report.batch_nanos, BATCH);
    let per_action =
        Distribution::from_batches("spike.trial", &report.batch_nanos, BATCH * actions);
    Ok(json!({
        "actionsPerRequest": actions,
        "crossingsPerRequest": metric(report, "outward.crossings_per_request")?,
        "boundaryBytesPerRequest": metric(report, "outward.boundary_bytes_per_request")?,
        "nanosecondsPerRequest": per_request,
        "nanosecondsPerAction": per_action,
    }))
}

/// A number the trial reported under a documented key.
fn metric(report: &harness::TrialReport, key: &str) -> Result<f64, String> {
    report
        .gate_metrics
        .get(key)
        .copied()
        .ok_or_else(|| format!("the trial reported no {key}"))
}

/// What a reader must not take this record to have shown.
fn caveats() -> Value {
    json!([
        "The performed action is arithmetic, not a game method. A real game call does work both \
         candidates pay identically, so the difference measured here is the crossing and the \
         difference a player would feel is smaller.",
        "The deferred candidate's Java side calls the action directly. An adapter that had to \
         resolve the target reflectively, as `dev.ravel.jvm.Persistence` does, would pay a \
         reflective dispatch this record does not include.",
        "Nothing here measures ordering. A deferred instruction happens after the answer, and \
         whether that is acceptable is a question about each surface rather than a number.",
        "One machine, one JVM, one action shape. These are orders of magnitude for a decision, \
         not a budget anything is held to.",
    ])
}

/// Where the trial inputs live and how they are built.
struct Layout {
    classes: PathBuf,
    sources: PathBuf,
    trial_jar: PathBuf,
    library: PathBuf,
    library_dir: PathBuf,
    java: String,
}

impl Layout {
    fn discover(root: &Path, java: &str) -> Result<Self, String> {
        let library_dir = std::env::current_exe()
            .map_err(|error| format!("cannot resolve this executable: {error}"))?
            .parent()
            .ok_or_else(|| "this executable has no directory".to_owned())?
            .to_path_buf();
        let work = root.join("target/spike/m6-outward");
        Ok(Self {
            sources: root.join("experiments/m6-outward-crossing/java"),
            classes: work.join("classes"),
            trial_jar: work.join("action.jar"),
            library: library_dir.join(LIBRARY_FILE),
            library_dir,
            java: java.to_owned(),
        })
    }

    /// Compiles the shared trial class and packages it.
    fn build(&self) -> Result<(), String> {
        fs::create_dir_all(&self.classes)
            .map_err(|error| format!("cannot create the work directory: {error}"))?;

        let mut sources = fs::read_dir(self.sources.join("dev/ravel/outward"))
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

    fn command(&self, candidate: Candidate, actions: u64) -> Vec<String> {
        let mut argv = vec![
            self.java.clone(),
            "-cp".to_owned(),
            self.trial_jar.display().to_string(),
            format!("-Djava.library.path={}", self.library_dir.display()),
            "--enable-native-access=ALL-UNNAMED".to_owned(),
            TRIAL_CLASS.to_owned(),
        ];
        argv.extend(["--candidate", candidate.flag(), "--library", LIBRARY].map(str::to_owned));
        for (name, value) in [
            ("--warmup", WARMUP),
            ("--samples", SAMPLES),
            ("--batch", BATCH),
            ("--actions", actions),
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

fn proc_root() -> &'static Path {
    Path::new("/proc")
}
