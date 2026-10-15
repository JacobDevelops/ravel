#![forbid(unsafe_code)]

//! Shared measurement contract for the Ravel M1 architecture spikes.
//!
//! Every candidate of every gate records the same fields through this crate, so
//! comparisons reflect the mechanism rather than the instrument. The crate is a
//! measurement instrument only: no production crate may depend on it.

pub mod result;

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use result::{Revision, SpikeResult};

/// Line a trial child prints once its entry mechanism is in place.
pub const READY_MARKER: &str = "ready";

/// Wall-clock allowance for one measured child trial before it is treated as hung.
pub const TRIAL_TIMEOUT: Duration = Duration::from_secs(90);

/// Allowance for a probe whose expected outcome is that it never reports.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Anything that stops a trial from producing admissible evidence.
#[derive(Debug)]
pub enum HarnessError {
    /// A required command could not be started or failed.
    Command(String),
    /// The trial child produced no usable report.
    Report(String),
    /// A filesystem or `/proc` read failed.
    Io(String),
    /// The child exceeded [`TRIAL_TIMEOUT`].
    Timeout(String),
}

impl fmt::Display for HarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(detail) => write!(formatter, "trial command failed: {detail}"),
            Self::Report(detail) => write!(formatter, "trial report is unusable: {detail}"),
            Self::Io(detail) => write!(formatter, "trial input/output failed: {detail}"),
            Self::Timeout(detail) => write!(formatter, "trial timed out: {detail}"),
        }
    }
}

impl std::error::Error for HarnessError {}

/// Percentiles of a callback-cost sample set.
#[derive(Clone, Copy, Debug)]
pub struct Percentiles {
    /// 50th percentile.
    pub median: f64,
    /// 95th percentile.
    pub p95: f64,
    /// 99th percentile.
    pub p99: f64,
}

/// Summarizes samples with one nearest-rank definition shared by every gate.
///
/// Returns `None` for an empty sample set so callers record an unavailable
/// metric instead of substituting zero.
#[must_use]
pub fn percentiles(samples: &[u64]) -> Option<Percentiles> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    Some(Percentiles {
        median: nearest_rank(&sorted, 0.50),
        p95: nearest_rank(&sorted, 0.95),
        p99: nearest_rank(&sorted, 0.99),
    })
}

fn nearest_rank(sorted: &[u64], fraction: f64) -> f64 {
    // ponytail: nearest-rank, not interpolated; one definition everywhere beats
    // a more precise one that differs between collectors.
    let count = sorted.len();
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "sample counts stay far below f64 integer precision limits"
    )]
    let rank = (fraction * count as f64).ceil() as usize;
    let index = rank.max(1) - 1;
    #[expect(
        clippy::cast_precision_loss,
        reason = "nanosecond samples stay far below f64 integer precision limits"
    )]
    let value = sorted[index.min(count - 1)] as f64;
    value
}

/// Hashes bytes with the same digest the evidence contract uses elsewhere.
#[must_use]
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Hashes a file's contents.
///
/// # Errors
///
/// Returns an error when the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String, HarnessError> {
    let bytes = fs::read(path)
        .map_err(|error| HarnessError::Io(format!("cannot read {}: {error}", path.display())))?;
    Ok(sha256_bytes(&bytes))
}

/// Hashes a NUL-delimited argument vector, matching the evidence convention.
#[must_use]
pub fn command_sha256<S: AsRef<str>>(argv: &[S]) -> String {
    let mut joined = Vec::new();
    for argument in argv {
        joined.extend_from_slice(argument.as_ref().as_bytes());
        joined.push(0);
    }
    sha256_bytes(&joined)
}

/// Returns the current UTC timestamp in the evidence date-time format.
///
/// # Errors
///
/// Returns an error when the clock cannot be read.
pub fn utc_timestamp() -> Result<String, HarnessError> {
    // ponytail: coreutils already formats this correctly; hand-rolled calendar
    // arithmetic is the classic 3am bug.
    let output = Command::new("date")
        .args(["--utc", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .map_err(|error| HarnessError::Command(format!("cannot read the clock: {error}")))?;
    if !output.status.success() {
        return Err(HarnessError::Command("cannot read the clock".to_owned()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Returns the described revision under test and whether the tree is dirty.
///
/// # Errors
///
/// Returns an error when the revision cannot be queried.
pub fn revision() -> Result<Revision, HarnessError> {
    let commit = jj(&["log", "--no-graph", "-r", "@-", "-T", "commit_id"])?;
    // The working copy is the change the result is being written into, so it is
    // the one that will hold it. Squashing the result elsewhere afterwards makes
    // this wrong, which is what `ravel:verify-spikes` recomputes.
    let change = jj(&["log", "--no-graph", "-r", "@", "-T", "change_id"])?;
    let pending = jj(&["diff", "--summary"])?;
    Ok(Revision {
        id: commit.trim().to_owned(),
        change_id: change.trim().to_owned(),
        dirty: !pending.trim().is_empty(),
    })
}

fn jj(arguments: &[&str]) -> Result<String, HarnessError> {
    let output = Command::new("jj")
        .args(arguments)
        .output()
        .map_err(|error| HarnessError::Command(format!("cannot run jj: {error}")))?;
    if !output.status.success() {
        return Err(HarnessError::Command(format!(
            "jj {} failed",
            arguments.join(" ")
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Path to the descriptor that pins every runtime input M1 measures against.
pub const RUNTIME_INPUTS: &str = "tests/fixtures/descriptors/runtime-inputs-1.21.1.json";

/// Environment variable naming the pinned game Java runtime to measure with.
///
/// The pinned Mojang runtime lives inside the operator's ignored launcher root,
/// so its path can never be committed. Without it, trials fall back to the
/// development JDK and record the difference as a deviation.
pub const PINNED_JAVA_ENV: &str = "RAVEL_SPIKE_JAVA";

/// Runtime versions the pinned descriptor requires.
#[derive(Clone, Debug)]
pub struct PinnedInputs {
    /// Pinned Minecraft version.
    pub minecraft: String,
    /// Pinned Prism Launcher version.
    pub prism: String,
    /// Pinned Linux `x86_64` game Java runtime version.
    pub java: String,
}

/// Reads the pinned runtime inputs instead of restating them in spike code.
///
/// # Errors
///
/// Returns an error when the descriptor is missing or incomplete.
pub fn pinned_inputs(root: &Path) -> Result<PinnedInputs, HarnessError> {
    let path = root.join(RUNTIME_INPUTS);
    let text = fs::read_to_string(&path)
        .map_err(|error| HarnessError::Io(format!("cannot read {}: {error}", path.display())))?;
    let document: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        HarnessError::Report(format!("cannot parse {}: {error}", path.display()))
    })?;

    let java = document["platforms"]
        .as_array()
        .and_then(|platforms| {
            platforms
                .iter()
                .find(|platform| platform["id"] == "linux-x86_64")
        })
        .and_then(|platform| platform["java"]["version"].as_str())
        .ok_or_else(|| {
            HarnessError::Report("the descriptor pins no Linux x86_64 Java runtime".to_owned())
        })?;
    let minecraft = document["minecraft"]["version"].as_str().ok_or_else(|| {
        HarnessError::Report("the descriptor pins no Minecraft version".to_owned())
    })?;
    let prism = document["prism"]["version"]
        .as_str()
        .ok_or_else(|| HarnessError::Report("the descriptor pins no Prism version".to_owned()))?;

    Ok(PinnedInputs {
        minecraft: minecraft.to_owned(),
        prism: prism.to_owned(),
        java: java.to_owned(),
    })
}

/// The Java runtime a trial will actually execute.
#[derive(Clone, Debug)]
pub struct JavaRuntime {
    /// Program to launch, either the pinned runtime or `java` from the shell.
    pub program: String,
    /// Reported feature version, such as `25.0.4`.
    pub version: String,
    /// Human-readable runtime identity.
    pub runtime: String,
    /// Whether this is the runtime the pinned game uses.
    pub pinned: bool,
}

/// Resolves the Java runtime for a trial and states plainly whether it is pinned.
///
/// # Errors
///
/// Returns an error when the runtime cannot be started or identified.
pub fn java_runtime(pinned_version: &str) -> Result<JavaRuntime, HarnessError> {
    let program = std::env::var(PINNED_JAVA_ENV).unwrap_or_else(|_| "java".to_owned());
    let output = Command::new(&program)
        .arg("-version")
        .output()
        .map_err(|error| HarnessError::Command(format!("cannot run {program}: {error}")))?;
    if !output.status.success() {
        return Err(HarnessError::Command(format!("{program} -version failed")));
    }

    // `java -version` writes to standard error and starts with the version line.
    let banner = String::from_utf8_lossy(&output.stderr);
    let first = banner.lines().next().unwrap_or_default();
    let version = first
        .split('"')
        .nth(1)
        .ok_or_else(|| HarnessError::Report("cannot read the Java version banner".to_owned()))?
        .to_owned();
    let runtime = banner
        .lines()
        .nth(1)
        .unwrap_or(first)
        .split(" (build")
        .next()
        .unwrap_or(first)
        .trim()
        .to_owned();

    Ok(JavaRuntime {
        pinned: version == pinned_version,
        program,
        version,
        runtime,
    })
}

/// Resident-set sizes of a live process, in bytes.
#[derive(Clone, Copy, Debug)]
pub struct RssSnapshot {
    /// Current resident set size.
    pub current_bytes: u64,
    /// High-water mark since process start.
    pub peak_bytes: u64,
}

/// Reads a live process's current and peak resident set size from `/proc`.
///
/// # Errors
///
/// Returns an error when the process status is missing or malformed.
pub fn linux_rss(proc_root: &Path, pid: u32) -> Result<RssSnapshot, HarnessError> {
    let status_path = proc_root.join(pid.to_string()).join("status");
    let status = fs::read_to_string(&status_path).map_err(|error| {
        HarnessError::Io(format!("cannot read {}: {error}", status_path.display()))
    })?;
    Ok(RssSnapshot {
        current_bytes: status_kibibytes(&status, "VmRSS")?,
        peak_bytes: status_kibibytes(&status, "VmHWM")?,
    })
}

fn status_kibibytes(status: &str, field: &str) -> Result<u64, HarnessError> {
    let prefix = format!("{field}:");
    let line = status
        .lines()
        .find(|line| line.starts_with(&prefix))
        .ok_or_else(|| HarnessError::Io(format!("process status has no {field} field")))?;
    let kibibytes = line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| HarnessError::Io(format!("process status field {field} is malformed")))?;
    kibibytes
        .checked_mul(1024)
        .ok_or_else(|| HarnessError::Io(format!("process status field {field} overflowed")))
}

/// One failure scenario a trial child exercised inside its own process.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportedFailure {
    /// Scenario identifier matching the spike-result schema.
    pub scenario: String,
    /// Observed behaviour identifier matching the spike-result schema.
    pub outcome: String,
    /// Human-readable observation.
    pub detail: String,
}

/// The measurement report a trial child prints before it parks.
///
/// A child written in Rust builds and serializes this type directly; a child in
/// another language writes the same shape by hand.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrialReport {
    /// Candidate identifier, cross-checked against the expected candidate.
    pub candidate: String,
    /// Feature version of the JVM the child ran on, when it hosted one.
    ///
    /// A candidate that creates its own VM chooses its runtime, so runtime
    /// identity has to come from inside the trial rather than from a separate
    /// `java -version` call the candidate never sees. A trial that hosts no JVM
    /// omits both fields.
    #[serde(default)]
    pub java_version: Option<String>,
    /// Human-readable identity of that JVM.
    #[serde(default)]
    pub java_runtime: Option<String>,
    /// Milliseconds from child start to its ready boundary.
    pub ready_ms: u64,
    /// Classes the runtime had loaded at the ready boundary, when it has classes.
    #[serde(default)]
    pub loaded_classes: Option<u64>,
    /// Whether the candidate rewrote a hook target, where rewriting is the point.
    #[serde(default)]
    pub transformed: bool,
    /// Warm-up invocations excluded from the distribution.
    pub warmup: u64,
    /// Measured batch totals, in nanoseconds.
    ///
    /// One boundary transition is cheaper than the clock's resolution, so a
    /// trial times a batch of them and reports the batch total undivided.
    pub batch_nanos: Vec<u64>,
    /// Bytes the candidate transfers for one callback.
    pub boundary_bytes_per_call: u64,
    /// Allocations attributable to one callback, when the runtime exposes them.
    pub allocations_per_call: Option<f64>,
    /// Allocated bytes attributable to one callback, when exposed.
    pub allocated_bytes_per_call: Option<f64>,
    /// Failure scenarios the child exercised.
    pub failures: Vec<ReportedFailure>,
    /// Batch totals for a mod-to-mod call, for the gates that have one.
    #[serde(default)]
    pub peer_batch_nanos: Vec<u64>,
    /// Gate-specific numbers whose meaning that gate's runner defines.
    ///
    /// A shared report should not grow a field per gate. What every gate has in
    /// common is above; what only one gate measures goes here under a key its
    /// runner documents.
    #[serde(default)]
    pub gate_metrics: BTreeMap<String, f64>,
    /// Gate-specific observations, keyed the same way.
    #[serde(default)]
    pub gate_notes: BTreeMap<String, String>,
    /// Resident set size of processes this child owns, sampled at its own ready
    /// boundary.
    ///
    /// A candidate whose work happens in a second process would otherwise report
    /// half its memory, because the harness can only see the child it started.
    #[serde(default)]
    pub auxiliary_rss_bytes: Option<u64>,
}

/// A completed child trial: the child's report plus externally observed cost.
#[derive(Clone, Debug)]
pub struct ChildTrial {
    /// What the child measured about itself.
    pub report: TrialReport,
    /// Nanoseconds from spawn to the child's ready marker, on the harness clock.
    ///
    /// Nanoseconds because a candidate that only has to exec and link is ready
    /// in well under a millisecond, and a rounded zero is not a measurement.
    pub ready_nanos: u64,
    /// Peak resident set size observed at the ready boundary.
    pub bootstrap_peak_rss_bytes: u64,
    /// Resident set size retained at the ready boundary.
    pub retained_rss_bytes: u64,
    /// Wall-clock duration of the whole trial.
    pub duration_ms: u64,
}

/// Runs one trial child and observes its memory at a defined boundary.
///
/// The child prints `ready` once its entry mechanism is in place, then a JSON
/// [`TrialReport`] line once measurement finishes, then parks on standard input
/// so the harness can read `/proc` while the process is still alive. Writing a
/// newline releases it. Any other behaviour is a failed trial, never a zero.
///
/// Memory is sampled at the ready marker rather than after measurement, so
/// bootstrap and retained cost describe the entry mechanism instead of the
/// workload.
///
/// Pass [`TRIAL_TIMEOUT`] for a measured trial and [`PROBE_TIMEOUT`] for a probe
/// that is expected to hang, so proving one candidate fails closed does not cost
/// the gate a minute and a half.
///
/// # Errors
///
/// Returns an error when the child cannot start, reports nothing usable, times
/// out, or exits unsuccessfully.
pub fn run_child_trial<S: AsRef<std::ffi::OsStr>>(
    program: &str,
    arguments: &[S],
    proc_root: &Path,
    timeout: Duration,
) -> Result<ChildTrial, HarnessError> {
    let started = Instant::now();
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| HarnessError::Command(format!("cannot start {program}: {error}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| HarnessError::Report("child has no standard output".to_owned()))?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line).is_err() {
                return;
            }
        }
    });

    let ready = next_line(&receiver, &mut child, program, timeout)?;
    if ready.trim() != READY_MARKER {
        drop(child.kill());
        return Err(HarnessError::Report(format!(
            "{program} did not announce readiness before reporting"
        )));
    }
    let ready_nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let memory = linux_rss(proc_root, child.id())?;

    let line = next_line(&receiver, &mut child, program, timeout)?;
    let report: TrialReport = serde_json::from_str(line.trim())
        .map_err(|error| HarnessError::Report(format!("cannot parse the child report: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        drop(stdin.write_all(b"\n"));
    }
    let status = child
        .wait()
        .map_err(|error| HarnessError::Command(format!("cannot wait for {program}: {error}")))?;
    if !status.success() {
        return Err(HarnessError::Command(format!(
            "{program} exited with {status}"
        )));
    }

    Ok(ChildTrial {
        report,
        ready_nanos,
        bootstrap_peak_rss_bytes: memory.peak_bytes,
        retained_rss_bytes: memory.current_bytes,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

fn next_line(
    receiver: &mpsc::Receiver<std::io::Result<String>>,
    child: &mut std::process::Child,
    program: &str,
    timeout: Duration,
) -> Result<String, HarnessError> {
    match receiver.recv_timeout(timeout) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(error)) => {
            drop(child.kill());
            Err(HarnessError::Report(format!(
                "cannot read from {program}: {error}"
            )))
        }
        Err(_) => {
            drop(child.kill());
            Err(HarnessError::Timeout(format!(
                "{program} went quiet for more than {} seconds",
                timeout.as_secs()
            )))
        }
    }
}

/// Writes one spike result where the milestone gate expects to find it, unless a
/// record is already there.
///
/// Existing records are digest-bound by milestone manifests, so a rerun must
/// delete the record deliberately.
///
/// Returns whether the record was written.
///
/// # Errors
///
/// Returns an error when the result cannot be serialized or written.
pub fn write_result(path: &Path, spike_result: &SpikeResult) -> Result<bool, HarnessError> {
    if path.exists() {
        // The name only: the gate redacts local paths from everything it prints,
        // and a runner is no more entitled to leak one than a check is.
        let name = path.file_name().unwrap_or(path.as_os_str());
        println!(
            "- {}: already recorded; this run measured but recorded nothing",
            Path::new(name).display()
        );
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            HarnessError::Io(format!("cannot create {}: {error}", parent.display()))
        })?;
    }
    let mut json = serde_json::to_string_pretty(spike_result)
        .map_err(|error| HarnessError::Report(format!("cannot serialize the result: {error}")))?;
    json.push('\n');
    fs::write(path, json)
        .map_err(|error| HarnessError::Io(format!("cannot write {}: {error}", path.display())))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{
        TRIAL_TIMEOUT, command_sha256, linux_rss, percentiles, result::Distribution, sha256_bytes,
        write_result,
    };
    use std::path::Path;

    #[test]
    fn percentiles_use_one_nearest_rank_definition() {
        let samples = (1..=100).collect::<Vec<u64>>();

        let summary = percentiles(&samples).expect("a hundred samples should summarize");

        assert!((summary.median - 50.0).abs() < f64::EPSILON);
        assert!((summary.p95 - 95.0).abs() < f64::EPSILON);
        assert!((summary.p99 - 99.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_single_sample_is_its_own_distribution() {
        let summary = percentiles(&[7]).expect("one sample should summarize");

        assert!((summary.median - 7.0).abs() < f64::EPSILON);
        assert!((summary.p99 - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn missing_samples_are_unavailable_rather_than_zero() {
        assert!(percentiles(&[]).is_none());

        let serialized = serde_json::to_string(&Distribution::measured("spike.harness", &[]))
            .expect("distribution should serialize");

        assert!(serialized.contains("\"status\":\"unavailable\""));
        assert!(!serialized.contains("\"median\""));
    }

    #[test]
    fn batch_totals_divide_after_the_percentiles_are_taken() {
        // Two nanoseconds a call would truncate to zero if the trial divided.
        let batches = vec![128, 130, 132];

        let serialized =
            serde_json::to_string(&Distribution::from_batches("spike.trial", &batches, 64))
                .expect("distribution should serialize");

        assert!(serialized.contains("\"median\":2.03125"));
        assert!(serialized.contains("\"quality\":\"estimated\""));
        assert!(
            serde_json::to_string(&Distribution::from_batches("spike.trial", &batches, 0))
                .expect("distribution should serialize")
                .contains("\"status\":\"unavailable\"")
        );
    }

    #[test]
    fn command_hashes_are_nul_delimited() {
        let expected = sha256_bytes(b"java\0-version\0");

        assert_eq!(command_sha256(&["java", "-version"]), expected);
    }

    #[test]
    fn process_memory_is_read_from_proc_in_bytes() {
        let root = temporary_root("proc");
        let process = root.join("41");
        fs::create_dir_all(&process).expect("fake process directory should be creatable");
        fs::write(
            process.join("status"),
            "Name:\tjava\nVmHWM:\t   2048 kB\nVmRSS:\t   1024 kB\n",
        )
        .expect("fake status should be writable");

        let snapshot = linux_rss(&root, 41).expect("fake status should parse");

        assert_eq!(snapshot.peak_bytes, 2_097_152);
        assert_eq!(snapshot.current_bytes, 1_048_576);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn malformed_process_memory_fails_instead_of_defaulting() {
        let root = temporary_root("proc-malformed");
        let process = root.join("42");
        fs::create_dir_all(&process).expect("fake process directory should be creatable");
        fs::write(
            process.join("status"),
            "Name:\tjava\nVmRSS:\tnot-a-number kB\n",
        )
        .expect("fake status should be writable");

        let error = linux_rss(&root, 42).expect_err("a malformed status must fail");

        assert!(error.to_string().contains("VmRSS is malformed"));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_child_trial_is_sampled_at_its_ready_marker() {
        let report = concat!(
            r#"{"candidate":"probe","javaVersion":"25.0.4","javaRuntime":"probe jdk","readyMs":1,"loadedClasses":2,"transformed":false,"#,
            r#""warmup":1,"batchNanos":[128,192],"boundaryBytesPerCall":16,"#,
            r#""allocationsPerCall":null,"allocatedBytesPerCall":0.5,"failures":[]}"#
        );
        let script = format!("echo ready; echo '{report}'; read -r _release");

        let trial =
            super::run_child_trial("sh", &["-c", &script], Path::new("/proc"), TRIAL_TIMEOUT)
                .expect("the fake child should complete its protocol");

        assert_eq!(trial.report.candidate, "probe");
        assert_eq!(trial.report.batch_nanos, [128, 192]);
        assert!(trial.retained_rss_bytes > 0);
        assert!(trial.bootstrap_peak_rss_bytes >= trial.retained_rss_bytes);
    }

    #[test]
    fn a_child_that_reports_without_announcing_readiness_fails() {
        let error = super::run_child_trial(
            "sh",
            &["-c", "echo '{}'; read -r _"],
            Path::new("/proc"),
            TRIAL_TIMEOUT,
        )
        .expect_err("a child that skips the ready marker must fail");

        assert!(error.to_string().contains("did not announce readiness"));
    }

    #[test]
    fn a_child_that_never_reports_is_killed_at_its_own_timeout() {
        let started = std::time::Instant::now();

        let error = super::run_child_trial(
            "sh",
            &["-c", "sleep 60"],
            Path::new("/proc"),
            super::PROBE_TIMEOUT,
        )
        .expect_err("a hung child must fail closed rather than hang the gate");

        assert!(error.to_string().contains("went quiet"));
        assert!(started.elapsed() < TRIAL_TIMEOUT);
    }

    #[test]
    fn a_files_hash_is_the_hash_of_its_bytes() {
        let root = temporary_root("hash");
        let path = root.join("payload");
        fs::write(&path, b"ravel").expect("payload should be writable");

        let hashed = super::sha256_file(&path).expect("payload should hash");

        assert_eq!(hashed, sha256_bytes(b"ravel"));
        assert!(super::sha256_file(&root.join("absent")).is_err());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn timestamps_are_recorded_in_the_evidence_format() {
        let stamp = super::utc_timestamp().expect("the clock should be readable");

        assert_eq!(stamp.len(), 20, "{stamp} is not an evidence timestamp");
        assert!(stamp.ends_with('Z'));
        assert_eq!(stamp.chars().filter(|c| *c == '-').count(), 2);
        assert_eq!(stamp.chars().filter(|c| *c == ':').count(), 2);
    }

    #[test]
    fn pinned_inputs_come_from_the_descriptor_rather_than_from_code() {
        let root = repository_root();

        let pinned = super::pinned_inputs(&root).expect("the descriptor should parse");

        assert_eq!(pinned.minecraft, "1.21.1");
        assert_eq!(pinned.prism, "11.0.3");
        assert_eq!(pinned.java, "21.0.7");
    }

    #[test]
    fn the_java_pin_is_the_one_this_platform_runs_rather_than_the_first_listed() {
        let root = temporary_root("platform-order");
        let descriptor = root.join(super::RUNTIME_INPUTS);
        fs::create_dir_all(descriptor.parent().expect("descriptor has a parent"))
            .expect("descriptor directory should be creatable");
        fs::write(
            &descriptor,
            r#"{"minecraft":{"version":"1.21.1"},"prism":{"version":"11.0.3"},
                "platforms":[{"id":"macos-aarch64","java":{"version":"mac"}},
                             {"id":"linux-x86_64","java":{"version":"linux"}}]}"#,
        )
        .expect("descriptor should be writable");

        let pinned = super::pinned_inputs(&root).expect("the descriptor should parse");

        assert_eq!(pinned.java, "linux");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_descriptor_without_this_target_is_refused() {
        let root = temporary_root("descriptor");
        let descriptor = root.join(super::RUNTIME_INPUTS);
        fs::create_dir_all(descriptor.parent().expect("descriptor has a parent"))
            .expect("descriptor directory should be creatable");
        fs::write(
            &descriptor,
            r#"{"minecraft":{"version":"26.1.2"},"prism":{"version":"11.0.3"},"platforms":[]}"#,
        )
        .expect("descriptor should be writable");

        let error = super::pinned_inputs(&root).expect_err("no platform means no measurement");

        assert!(error.to_string().contains("no Linux x86_64 Java runtime"));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn the_runtime_a_trial_will_use_is_identified_and_compared_to_the_pin() {
        let runtime = super::java_runtime("0.0.0").expect("a JDK should be on the path");

        assert_ne!(runtime.version, "");
        assert_ne!(runtime.runtime, "");
        assert!(!runtime.pinned, "0.0.0 cannot be the pinned runtime");

        let same = super::java_runtime(&runtime.version).expect("a JDK should be on the path");
        assert!(same.pinned, "the runtime should match its own version");
    }

    #[test]
    fn every_failure_says_which_part_of_a_trial_broke() {
        for (error, expected) in [
            (super::HarnessError::Command("x".to_owned()), "command"),
            (super::HarnessError::Report("x".to_owned()), "report"),
            (super::HarnessError::Io("x".to_owned()), "input/output"),
            (super::HarnessError::Timeout("x".to_owned()), "timed out"),
        ] {
            assert!(
                error.to_string().contains(expected),
                "{error:?} should mention {expected}"
            );
        }
    }

    #[test]
    fn a_result_that_cannot_be_written_is_an_error_rather_than_a_silent_loss() {
        let root = temporary_root("unwritable");
        let blocker = root.join("blocker");
        fs::write(&blocker, b"not a directory").expect("blocker should be writable");

        let error = write_result(
            &blocker.join("nested/result.json"),
            &crate::tests::support::minimal_result(),
        )
        .expect_err("writing under a file must fail");

        assert!(error.to_string().contains("cannot create"));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The repository this crate lives in, for reading tracked descriptors.
    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository root should resolve")
    }

    #[test]
    fn results_are_written_with_their_parent_directory() {
        let root = temporary_root("write");
        let path = root.join("nested/adg-1/candidate.json");

        write_result(&path, &crate::tests::support::minimal_result())
            .expect("result should be writable");

        let written = fs::read_to_string(&path).expect("result should be readable");
        assert!(written.ends_with('\n'));
        assert!(written.contains("\"protocolVersion\": \"spike-v1\""));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_record_already_on_disk_is_not_rewritten_by_a_later_run() {
        let root = temporary_root("existing");
        let path = root.join("candidate.json");
        let mut first = crate::tests::support::minimal_result();
        first.candidate.title = "the run that recorded this".to_owned();

        assert!(write_result(&path, &first).expect("the first result should be written"));
        let recorded = fs::read_to_string(&path).expect("result should be readable");

        let mut second = crate::tests::support::minimal_result();
        second.candidate.title = "a later run with a different revision".to_owned();
        second.trial.revision.change_id = "znvkxxvvymxlulrmpnvpqrwmzuwtmwqk".to_owned();

        assert!(
            !write_result(&path, &second)
                .expect("a second write should report it recorded nothing")
        );
        assert_eq!(
            fs::read_to_string(&path).expect("result should still be readable"),
            recorded,
            "the record on disk must remain exactly what was recorded"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-spike-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }

    pub(super) mod support {
        use crate::result::{
            Allocation, Boundary, Calls, Candidate, Classification, Clock, Deviation, Disposition,
            Distribution, FailureBehaviour, FailureOutcome, FailureScenario, Gate, HashedArtifact,
            Identity, JavaIdentity, Measurement, Memory, Operability, Packaging, PrismLaunch,
            Revision, Safety, SpikeResult, Startup, Target, Trial, TrialKind, Unit,
        };

        pub fn minimal_result() -> SpikeResult {
            SpikeResult {
                schema_version: 1,
                protocol_version: "spike-v1".to_owned(),
                gate: Gate::JvmEntry,
                candidate: Candidate {
                    id: "test.candidate".to_owned(),
                    title: "Test candidate".to_owned(),
                    disposition: Disposition::Measured,
                    rejection_reason: None,
                },
                trial: Trial {
                    revision: Revision {
                        id: "0".repeat(40),
                        change_id: "k".repeat(32),
                        dirty: false,
                    },
                    command_sha256: "0".repeat(64),
                    started_at: "2026-07-30T00:00:00Z".to_owned(),
                    duration_ms: 1,
                    clock: Clock::Monotonic,
                    warmup_invocations: 1,
                    measured_invocations: 1,
                },
                identity: Identity {
                    target: Target::X8664Linux,
                    hardware_profile: "linux-reference-v1".to_owned(),
                    minecraft: "26.1.2".to_owned(),
                    prism: "11.0.3".to_owned(),
                    java: Some(JavaIdentity {
                        version: "25.0.4".to_owned(),
                        runtime: "nixpkgs openjdk".to_owned(),
                        pinned: false,
                    }),
                    trial_kind: TrialKind::Harness,
                },
                memory: Memory {
                    bootstrap_peak_rss: Measurement::exact("spike.harness", 1.0, Unit::Bytes),
                    retained_rss: Measurement::exact("spike.harness", 1.0, Unit::Bytes),
                    retained_classes: Measurement::exact("spike.jmx", 1.0, Unit::Classes),
                },
                startup: Startup {
                    process_to_ready: Measurement::exact("spike.harness", 1.0, Unit::Milliseconds),
                    candidate_delta: Measurement::exact("spike.harness", 0.0, Unit::Milliseconds),
                },
                calls: Calls {
                    invocation_count: 1,
                    callback_nanoseconds: Distribution::measured("spike.harness", &[1]),
                },
                boundary: Boundary {
                    bytes_per_call: Measurement::exact("spike.harness", 1.0, Unit::BytesPerCall),
                    total_bytes: Measurement::exact("spike.harness", 1.0, Unit::Bytes),
                },
                allocation: Allocation {
                    allocations_per_call: Measurement::unavailable("spike.jmx", "no collector"),
                    allocated_bytes_per_call: Measurement::unavailable("spike.jmx", "no collector"),
                },
                packaging: Packaging {
                    installed_bytes: 1,
                    artifact_count: 1,
                    required_components: vec!["test".to_owned()],
                },
                failures: vec![FailureOutcome {
                    scenario: FailureScenario::InvalidInput,
                    outcome: FailureBehaviour::FailedClosed,
                    detail: "rejected".to_owned(),
                }],
                safety: Safety {
                    maintained_unsafe_blocks: 0,
                    maintained_unsafe_detail: "none".to_owned(),
                    audited_boundary_dependencies: Vec::new(),
                },
                operability: Operability {
                    diagnostics: "none".to_owned(),
                    debug_workflow: "none".to_owned(),
                    crash_attribution: "none".to_owned(),
                    prism_launch: PrismLaunch::NotEvaluated,
                },
                artifacts: vec![HashedArtifact {
                    name: "test".to_owned(),
                    sha256: "0".repeat(64),
                }],
                isolation: None,
                abi_evolution: None,
                mod_to_mod_calls: None,
                binding_generation: None,
                state_ownership: None,
                prism_integration: None,
                deviations: vec![Deviation {
                    summary: "test".to_owned(),
                    classification: Classification::NonBlocking,
                }],
            }
        }
    }
}
