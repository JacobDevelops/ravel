#![forbid(unsafe_code)]

//! Developer-facing fixture and benchmark operations.

mod linux_supervisor;
mod performance_markers;

use std::collections::HashSet;
use std::fmt::{self, Write as _};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use linux_supervisor::{exec_initial_linux_child, supervise_initial_linux_process};
pub use performance_markers::{
    InitialLinuxPerformanceCounts, InitialLinuxPerformanceMarkerCommand,
    InitialPerformanceMarkerCommand, InitialPerformanceMarkerRecorder,
    InitialPerformanceMarkerStream, PerformanceEvent, PerformanceEventKind, PerformancePhase,
    parse_initial_linux_performance_marker_command, parse_initial_performance_marker_command,
    parse_initial_performance_marker_csv, record_initial_linux_performance,
    record_initial_performance_markers, send_initial_linux_performance_marker,
    send_initial_performance_marker, verify_initial_performance_marker_csv,
};

const ALLOWED_DOWNLOAD_PREFIX: &str = "https://mediafilez.forgecdn.net/";
const LINUX_MEMORY_CSV_HEADER: &str = "sample_index,elapsed_ns,process_rss_bytes,\
system_used_memory_bytes,memory_pressure_level,swap_in_total_bytes,swap_out_total_bytes";
const TRACY_FRAME_CSV_HEADER: &str = "frame_set,frame_index,start_ns,end_ns,duration_ns";

/// A normalized frame stream emitted by Minecraft's bundled Tracy client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TracyFrameMetric {
    /// One complete Minecraft client render loop.
    FrameTime,
    /// One complete integrated-server tick.
    IntegratedServerTickTime,
}

impl TracyFrameMetric {
    fn frame_set_name(self) -> &'static str {
        match self {
            Self::FrameTime => "Client Tick",
            Self::IntegratedServerTickTime => "Server Tick",
        }
    }
}

/// One exact frame or tick duration extracted from a Tracy trace.
#[derive(Debug, Eq, PartialEq)]
pub struct TracyFrameSample {
    /// Normalized metric represented by the named Tracy frame set.
    pub metric: TracyFrameMetric,
    /// Zero-based index within the Tracy frame set.
    pub frame_index: u64,
    /// Nanoseconds from Tracy initialization to the frame start.
    pub start_ns: u64,
    /// Exact frame duration in nanoseconds.
    pub duration_ns: u64,
}

/// Validated sample totals for a complete Tracy frame CSV.
#[derive(Debug, Eq, PartialEq)]
pub struct TracyFrameCounts {
    /// Complete client-loop frames.
    pub client_frames: usize,
    /// Complete integrated-server ticks.
    pub integrated_server_ticks: usize,
}

/// A performance-collector parsing or normalization failure.
#[derive(Debug, Eq, PartialEq)]
pub enum PerformanceError {
    /// Reading raw collector output failed.
    Io(String),
    /// A pinned Tracy collector or exporter operation failed.
    TracyTool(String),
    /// Tracy frame CSV violated the pinned collector contract.
    InvalidTracyFrameCsv(String),
    /// Linux procfs memory counters violated the pinned collector contract.
    InvalidLinuxProc(String),
    /// Linux normalized memory CSV violated the pinned collector contract.
    InvalidLinuxMemoryCsv(String),
    /// Lifecycle marker CSV violated the initial profile contract.
    InvalidPerformanceMarkerCsv(String),
}

impl fmt::Display for PerformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message)
            | Self::TracyTool(message)
            | Self::InvalidTracyFrameCsv(message)
            | Self::InvalidLinuxProc(message)
            | Self::InvalidLinuxMemoryCsv(message)
            | Self::InvalidPerformanceMarkerCsv(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for PerformanceError {}

/// Categorical Linux memory-pressure level derived from one sampling interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinuxMemoryPressure {
    /// Neither partial nor full memory-stall time increased.
    Normal,
    /// At least one task stalled, but full-stall time did not increase.
    Warning,
    /// All non-idle tasks were stalled simultaneously during the interval.
    Critical,
}

impl LinuxMemoryPressure {
    /// Returns the performance collector schema's numeric level.
    #[must_use]
    pub const fn level(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Warning => 1,
            Self::Critical => 2,
        }
    }
}

/// Parsed Linux memory values and cumulative counters captured at one point.
#[derive(Debug, Eq, PartialEq)]
pub struct LinuxMemorySnapshot {
    process_rss_bytes: u64,
    system_used_memory_bytes: u64,
    pressure_some_total_microseconds: u64,
    pressure_full_total_microseconds: u64,
    swap_in_total_bytes: u64,
    swap_out_total_bytes: u64,
}

/// Stable Linux process identity used to detect numeric PID reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxProcessIdentity {
    /// Numeric process identifier.
    pub pid: u32,
    /// `/proc/<pid>/stat` field 22, measured in clock ticks since boot.
    pub start_time_ticks: u64,
}

/// Linux memory metrics normalized for the performance collector schema.
#[derive(Debug, Eq, PartialEq)]
pub struct LinuxNormalizedMemorySample {
    /// Process resident set size from `/proc/<pid>/smaps_rollup`.
    pub process_rss_bytes: u64,
    /// Estimated used system memory, `MemTotal - MemAvailable`.
    pub system_used_memory_bytes: u64,
    /// Pressure level observed between the previous and current snapshots.
    pub memory_pressure: LinuxMemoryPressure,
    /// Cumulative pages swapped in, converted to bytes.
    pub swap_in_total_bytes: u64,
    /// Cumulative pages swapped out, converted to bytes.
    pub swap_out_total_bytes: u64,
}

/// One timestamped row from the focus-free Linux memory collector.
#[derive(Debug, Eq, PartialEq)]
pub struct LinuxTimedMemorySample {
    /// Zero-based contiguous sample index.
    pub sample_index: u64,
    /// Nanoseconds elapsed on the collector's monotonic clock.
    pub elapsed_ns: u64,
    /// Process resident set size.
    pub process_rss_bytes: u64,
    /// Estimated system-used memory.
    pub system_used_memory_bytes: u64,
    /// Memory pressure observed during this sampling interval.
    pub memory_pressure: LinuxMemoryPressure,
    /// Cumulative swap-in bytes.
    pub swap_in_total_bytes: u64,
    /// Cumulative swap-out bytes.
    pub swap_out_total_bytes: u64,
}

/// Incremental Linux memory CSV writer driven by an external monotonic clock.
///
/// The recorder owns procfs identity and normalization state, while its caller
/// supplies elapsed timestamps from the clock shared with lifecycle markers.
/// An unfinished destination is removed when the recorder is dropped.
pub struct LinuxMemoryCsvRecorder {
    proc_root: PathBuf,
    pid: u32,
    page_size_bytes: u64,
    interval_ns: u64,
    next_deadline_ns: u64,
    process_identity: LinuxProcessIdentity,
    previous: LinuxMemorySnapshot,
    writer: Option<io::BufWriter<File>>,
    destination: PathBuf,
    output_identity: LinuxOutputIdentity,
    sample_count: usize,
    finalized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LinuxOutputIdentity {
    device: u64,
    inode: u64,
}

impl LinuxMemoryCsvRecorder {
    /// Creates an incremental capture at one externally supplied clock origin.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero or overflowing interval, deadline overflow,
    /// an existing destination, procfs drift, or output I/O failure.
    pub fn new(
        proc_root: &Path,
        pid: u32,
        page_size_bytes: u64,
        interval: Duration,
        start_elapsed_ns: u64,
        destination: &Path,
    ) -> Result<Self, PerformanceError> {
        if interval.is_zero() {
            return Err(PerformanceError::InvalidLinuxProc(
                "Linux memory sampling interval must be greater than zero".to_owned(),
            ));
        }
        let interval_ns = u64::try_from(interval.as_nanos()).map_err(|error| {
            PerformanceError::InvalidLinuxProc(format!(
                "Linux memory sampling interval exceeds u64 nanoseconds: {error}"
            ))
        })?;
        let next_deadline_ns = start_elapsed_ns.checked_add(interval_ns).ok_or_else(|| {
            PerformanceError::InvalidLinuxProc("Linux memory sample deadline overflowed".to_owned())
        })?;
        let output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(destination)
            .map_err(|error| {
                PerformanceError::Io(format!(
                    "cannot create Linux memory CSV {}: {error}",
                    destination.display()
                ))
            })?;
        let output_metadata = output.metadata().map_err(|error| {
            PerformanceError::Io(format!(
                "cannot inspect Linux memory CSV {}: {error}",
                destination.display()
            ))
        })?;
        let output_identity = LinuxOutputIdentity {
            device: output_metadata.dev(),
            inode: output_metadata.ino(),
        };

        let initialized = (|| {
            let mut writer = io::BufWriter::new(output);
            writeln!(writer, "{LINUX_MEMORY_CSV_HEADER}").map_err(|error| {
                PerformanceError::Io(format!("cannot write Linux memory CSV header: {error}"))
            })?;
            let process_identity = read_linux_process_identity(proc_root, pid)?;
            let previous = read_linux_memory_snapshot_for_process(
                proc_root,
                pid,
                page_size_bytes,
                process_identity,
            )?;
            Ok(Self {
                proc_root: proc_root.to_path_buf(),
                pid,
                page_size_bytes,
                interval_ns,
                next_deadline_ns,
                process_identity,
                previous,
                writer: Some(writer),
                destination: destination.to_path_buf(),
                output_identity,
                sample_count: 0,
                finalized: false,
            })
        })();
        if initialized.is_err() {
            remove_linux_output_if_owned(destination, output_identity);
        }
        initialized
    }

    /// Returns the next fixed sample deadline on the caller's clock.
    #[must_use]
    pub const fn next_deadline_ns(&self) -> u64 {
        self.next_deadline_ns
    }

    /// Captures one sample when the supplied shared-clock time reaches its
    /// fixed deadline.
    ///
    /// Returns `false` before the deadline and `true` after writing one sample.
    ///
    /// # Errors
    ///
    /// Returns an error when the deadline is missed beyond the supplied
    /// tolerance, procfs or process identity drifts, counters regress, deadline
    /// arithmetic overflows, or the row cannot be written.
    pub fn sample_due_at(
        &mut self,
        elapsed_ns: u64,
        alignment_tolerance_ns: u64,
    ) -> Result<bool, PerformanceError> {
        if elapsed_ns < self.next_deadline_ns {
            return Ok(false);
        }
        let lateness_ns = elapsed_ns - self.next_deadline_ns;
        if lateness_ns > alignment_tolerance_ns {
            return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
                "Linux memory sample at {elapsed_ns} ns missed deadline {} ns by \
                 {lateness_ns} ns, beyond alignment tolerance {alignment_tolerance_ns} ns",
                self.next_deadline_ns
            )));
        }

        let current = read_linux_memory_snapshot_for_process(
            &self.proc_root,
            self.pid,
            self.page_size_bytes,
            self.process_identity,
        )?;
        let sample = normalize_linux_memory_sample(&self.previous, &current)?;
        let sample_index = u64::try_from(self.sample_count).map_err(|error| {
            PerformanceError::InvalidLinuxProc(format!(
                "Linux memory sample index exceeds u64: {error}"
            ))
        })?;
        let writer = self.writer.as_mut().ok_or_else(|| {
            PerformanceError::InvalidLinuxMemoryCsv(
                "Linux memory recorder is already finalized".to_owned(),
            )
        })?;
        writeln!(
            writer,
            "{sample_index},{elapsed_ns},{},{},{},{},{}",
            sample.process_rss_bytes,
            sample.system_used_memory_bytes,
            sample.memory_pressure.level(),
            sample.swap_in_total_bytes,
            sample.swap_out_total_bytes
        )
        .map_err(|error| {
            PerformanceError::Io(format!("cannot write Linux memory CSV sample: {error}"))
        })?;
        self.previous = current;
        self.sample_count += 1;
        self.next_deadline_ns = self
            .next_deadline_ns
            .checked_add(self.interval_ns)
            .ok_or_else(|| {
                PerformanceError::InvalidLinuxProc(
                    "Linux memory sample deadline overflowed".to_owned(),
                )
            })?;
        Ok(true)
    }

    /// Flushes, reparses, and retains a completed nonempty capture.
    ///
    /// # Errors
    ///
    /// Returns an error when no samples were captured, flushing or reopening
    /// fails, or the verified sample count differs from the written count.
    pub fn finish(mut self) -> Result<usize, PerformanceError> {
        let verified = self.verify_finished()?;
        self.finalized = true;
        Ok(verified)
    }

    pub(crate) fn verify_finished(&mut self) -> Result<usize, PerformanceError> {
        let mut writer = self.writer.take().ok_or_else(|| {
            PerformanceError::InvalidLinuxMemoryCsv(
                "Linux memory recorder is already finalized".to_owned(),
            )
        })?;
        writer.flush().map_err(|error| {
            PerformanceError::Io(format!("cannot flush Linux memory CSV: {error}"))
        })?;
        drop(writer);
        let verified = verify_linux_memory_csv(&self.destination)?;
        if verified != self.sample_count {
            return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
                "captured {} Linux memory samples but verified {verified}",
                self.sample_count
            )));
        }
        Ok(verified)
    }

    pub(crate) const fn retain_finished(&mut self) {
        self.finalized = true;
    }
}

impl Drop for LinuxMemoryCsvRecorder {
    fn drop(&mut self) {
        if !self.finalized {
            remove_linux_output_if_owned(&self.destination, self.output_identity);
        }
    }
}

/// Parses one Linux memory snapshot from the kernel's procfs interfaces.
///
/// `smaps_rollup` must be `/proc/<pid>/smaps_rollup`, `meminfo` must be
/// `/proc/meminfo`, `memory_pressure` must be `/proc/pressure/memory`, and
/// `vmstat` must be `/proc/vmstat`. Swap page counters are converted with the
/// supplied base page size.
///
/// # Errors
///
/// Returns an error when a required field or unit is missing, a value is
/// malformed, memory arithmetic is impossible, or byte conversion overflows.
pub fn parse_linux_memory_snapshot(
    smaps_rollup: &str,
    meminfo: &str,
    memory_pressure: &str,
    vmstat: &str,
    page_size_bytes: u64,
) -> Result<LinuxMemorySnapshot, PerformanceError> {
    if page_size_bytes == 0 || !page_size_bytes.is_power_of_two() {
        return Err(PerformanceError::InvalidLinuxProc(format!(
            "Linux base page size must be a nonzero power of two, observed {page_size_bytes}"
        )));
    }

    let process_rss_bytes =
        checked_kilobytes_to_bytes(parse_proc_kilobytes(smaps_rollup, "Rss")?, "Rss")?;
    let memory_total_kib = parse_proc_kilobytes(meminfo, "MemTotal")?;
    let memory_available_kib = parse_proc_kilobytes(meminfo, "MemAvailable")?;
    let system_used_kib = memory_total_kib
        .checked_sub(memory_available_kib)
        .ok_or_else(|| {
            PerformanceError::InvalidLinuxProc(format!(
                "MemAvailable {memory_available_kib} kB exceeds MemTotal {memory_total_kib} kB"
            ))
        })?;
    let system_used_memory_bytes =
        checked_kilobytes_to_bytes(system_used_kib, "system used memory")?;

    let pressure_some_total_microseconds = parse_psi_total(memory_pressure, "some")?;
    let pressure_full_total_microseconds = parse_psi_total(memory_pressure, "full")?;
    let swap_in_pages = parse_proc_counter(vmstat, "pswpin")?;
    let swap_out_pages = parse_proc_counter(vmstat, "pswpout")?;
    let swap_in_total_bytes = swap_in_pages.checked_mul(page_size_bytes).ok_or_else(|| {
        PerformanceError::InvalidLinuxProc(format!(
            "pswpin value {swap_in_pages} overflows bytes at page size {page_size_bytes}"
        ))
    })?;
    let swap_out_total_bytes = swap_out_pages.checked_mul(page_size_bytes).ok_or_else(|| {
        PerformanceError::InvalidLinuxProc(format!(
            "pswpout value {swap_out_pages} overflows bytes at page size {page_size_bytes}"
        ))
    })?;

    Ok(LinuxMemorySnapshot {
        process_rss_bytes,
        system_used_memory_bytes,
        pressure_some_total_microseconds,
        pressure_full_total_microseconds,
        swap_in_total_bytes,
        swap_out_total_bytes,
    })
}

/// Normalizes two Linux snapshots into one interval sample.
///
/// The current RSS and system-used-memory values are retained. PSI cumulative
/// totals classify the interval as normal, warning, or critical, while swap
/// totals remain cumulative for later throughput calculation.
///
/// # Errors
///
/// Returns an error if any cumulative PSI or swap counter regresses.
pub fn normalize_linux_memory_sample(
    previous: &LinuxMemorySnapshot,
    current: &LinuxMemorySnapshot,
) -> Result<LinuxNormalizedMemorySample, PerformanceError> {
    let some_delta = monotonic_counter_delta(
        previous.pressure_some_total_microseconds,
        current.pressure_some_total_microseconds,
        "memory PSI some total",
    )?;
    let full_delta = monotonic_counter_delta(
        previous.pressure_full_total_microseconds,
        current.pressure_full_total_microseconds,
        "memory PSI full total",
    )?;
    monotonic_counter_delta(
        previous.swap_in_total_bytes,
        current.swap_in_total_bytes,
        "pswpin",
    )?;
    monotonic_counter_delta(
        previous.swap_out_total_bytes,
        current.swap_out_total_bytes,
        "pswpout",
    )?;

    let memory_pressure = if full_delta > 0 {
        LinuxMemoryPressure::Critical
    } else if some_delta > 0 {
        LinuxMemoryPressure::Warning
    } else {
        LinuxMemoryPressure::Normal
    };
    Ok(LinuxNormalizedMemorySample {
        process_rss_bytes: current.process_rss_bytes,
        system_used_memory_bytes: current.system_used_memory_bytes,
        memory_pressure,
        swap_in_total_bytes: current.swap_in_total_bytes,
        swap_out_total_bytes: current.swap_out_total_bytes,
    })
}

/// Reads one Linux memory snapshot from a procfs root.
///
/// # Errors
///
/// Returns an error when a required procfs file cannot be read or parsed.
pub fn read_linux_memory_snapshot(
    proc_root: &Path,
    pid: u32,
    page_size_bytes: u64,
) -> Result<LinuxMemorySnapshot, PerformanceError> {
    if pid == 0 {
        return Err(PerformanceError::InvalidLinuxProc(
            "Linux process ID must be greater than zero".to_owned(),
        ));
    }
    let process_root = proc_root.join(pid.to_string());
    let smaps_rollup = read_linux_proc_file(&process_root.join("smaps_rollup"))?;
    let meminfo = read_linux_proc_file(&proc_root.join("meminfo"))?;
    let memory_pressure = read_linux_proc_file(&proc_root.join("pressure/memory"))?;
    let vmstat = read_linux_proc_file(&proc_root.join("vmstat"))?;
    parse_linux_memory_snapshot(
        &smaps_rollup,
        &meminfo,
        &memory_pressure,
        &vmstat,
        page_size_bytes,
    )
}

/// Reads a stable Linux process identity from `/proc/<pid>/stat`.
///
/// # Errors
///
/// Returns an error when the stat file cannot be read, has an invalid PID or
/// field layout, or has a malformed start-time counter.
pub fn read_linux_process_identity(
    proc_root: &Path,
    pid: u32,
) -> Result<LinuxProcessIdentity, PerformanceError> {
    if pid == 0 {
        return Err(PerformanceError::InvalidLinuxProc(
            "Linux process ID must be greater than zero".to_owned(),
        ));
    }
    let stat_path = proc_root.join(pid.to_string()).join("stat");
    let stat = read_linux_proc_file(&stat_path)?;
    parse_linux_process_identity(&stat, pid)
}

/// Reads a Linux memory snapshot while proving the numeric PID still refers to
/// the expected process.
///
/// Identity is checked both before and after the procfs reads so process exit
/// and PID reuse cannot silently switch the RSS source.
///
/// # Errors
///
/// Returns an error when the identity changes or any required procfs source
/// cannot be read or parsed.
pub fn read_linux_memory_snapshot_for_process(
    proc_root: &Path,
    pid: u32,
    page_size_bytes: u64,
    expected: LinuxProcessIdentity,
) -> Result<LinuxMemorySnapshot, PerformanceError> {
    validate_linux_process_identity(expected, read_linux_process_identity(proc_root, pid)?)?;
    let snapshot = read_linux_memory_snapshot(proc_root, pid, page_size_bytes)?;
    validate_linux_process_identity(expected, read_linux_process_identity(proc_root, pid)?)?;
    Ok(snapshot)
}

/// Queries the host base page size through the standard `getconf` interface.
///
/// # Errors
///
/// Returns an error when `getconf` fails or emits an invalid page size.
pub fn query_linux_page_size() -> Result<u64, PerformanceError> {
    let output = Command::new("getconf")
        .arg("PAGESIZE")
        .output()
        .map_err(|error| PerformanceError::Io(format!("cannot run getconf PAGESIZE: {error}")))?;
    if !output.status.success() {
        return Err(PerformanceError::InvalidLinuxProc(format!(
            "getconf PAGESIZE failed with status {}",
            output.status
        )));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        PerformanceError::InvalidLinuxProc(format!(
            "getconf PAGESIZE emitted non-UTF-8 output: {error}"
        ))
    })?;
    let page_size = parse_linux_u64(stdout.trim(), "base page size")?;
    if page_size == 0 || !page_size.is_power_of_two() {
        return Err(PerformanceError::InvalidLinuxProc(format!(
            "Linux base page size must be a nonzero power of two, observed {page_size}"
        )));
    }
    Ok(page_size)
}

/// Captures a fixed number of normalized Linux memory samples to a new CSV.
///
/// Sampling uses a monotonic clock and fixed deadlines, so collector work does
/// not accumulate interval drift. The partial destination is removed on any
/// read, normalization, timing, or write failure.
///
/// # Errors
///
/// Returns an error for an invalid interval or sample count, an existing
/// destination, procfs drift, counter regression, timing overflow, or I/O
/// failure.
pub fn capture_linux_memory_csv(
    proc_root: &Path,
    pid: u32,
    page_size_bytes: u64,
    interval: Duration,
    sample_count: usize,
    destination: &Path,
) -> Result<usize, PerformanceError> {
    if interval.is_zero() {
        return Err(PerformanceError::InvalidLinuxProc(
            "Linux memory sampling interval must be greater than zero".to_owned(),
        ));
    }
    if sample_count == 0 {
        return Err(PerformanceError::InvalidLinuxProc(
            "Linux memory sample count must be greater than zero".to_owned(),
        ));
    }

    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| {
            PerformanceError::Io(format!(
                "cannot create Linux memory CSV {}: {error}",
                destination.display()
            ))
        })?;
    let result = capture_linux_memory_csv_inner(
        proc_root,
        pid,
        page_size_bytes,
        interval,
        sample_count,
        output,
    )
    .and_then(|captured| {
        let verified = verify_linux_memory_csv(destination)?;
        if verified != captured {
            return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
                "captured {captured} Linux memory samples but verified {verified}"
            )));
        }
        Ok(verified)
    });
    if result.is_err() {
        remove_partial_performance_output(destination, "Linux memory CSV");
    }
    result
}

fn capture_linux_memory_csv_inner(
    proc_root: &Path,
    pid: u32,
    page_size_bytes: u64,
    interval: Duration,
    sample_count: usize,
    output: File,
) -> Result<usize, PerformanceError> {
    let mut writer = io::BufWriter::new(output);
    writeln!(writer, "{LINUX_MEMORY_CSV_HEADER}").map_err(|error| {
        PerformanceError::Io(format!("cannot write Linux memory CSV header: {error}"))
    })?;
    let start = Instant::now();
    let mut deadline = start.checked_add(interval).ok_or_else(|| {
        PerformanceError::InvalidLinuxProc("Linux memory sample deadline overflowed".to_owned())
    })?;
    let process_identity = read_linux_process_identity(proc_root, pid)?;
    let mut previous =
        read_linux_memory_snapshot_for_process(proc_root, pid, page_size_bytes, process_identity)?;

    for index in 0..sample_count {
        thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let current = read_linux_memory_snapshot_for_process(
            proc_root,
            pid,
            page_size_bytes,
            process_identity,
        )?;
        let sample = normalize_linux_memory_sample(&previous, &current)?;
        let elapsed_ns = u64::try_from(start.elapsed().as_nanos()).map_err(|error| {
            PerformanceError::InvalidLinuxProc(format!(
                "Linux memory elapsed time exceeds u64 nanoseconds: {error}"
            ))
        })?;
        let sample_index = u64::try_from(index).map_err(|error| {
            PerformanceError::InvalidLinuxProc(format!(
                "Linux memory sample index exceeds u64: {error}"
            ))
        })?;
        writeln!(
            writer,
            "{sample_index},{elapsed_ns},{},{},{},{},{}",
            sample.process_rss_bytes,
            sample.system_used_memory_bytes,
            sample.memory_pressure.level(),
            sample.swap_in_total_bytes,
            sample.swap_out_total_bytes
        )
        .map_err(|error| {
            PerformanceError::Io(format!("cannot write Linux memory CSV sample: {error}"))
        })?;
        previous = current;
        deadline = deadline.checked_add(interval).ok_or_else(|| {
            PerformanceError::InvalidLinuxProc("Linux memory sample deadline overflowed".to_owned())
        })?;
    }
    writer
        .flush()
        .map_err(|error| PerformanceError::Io(format!("cannot flush Linux memory CSV: {error}")))?;
    Ok(sample_count)
}

/// Parses and validates a normalized Linux memory CSV.
///
/// # Errors
///
/// Returns an error when the header or row shape drifts, values are malformed,
/// indices are not contiguous, elapsed timestamps are not strictly increasing,
/// or a pressure level is outside `0..=2`.
pub fn parse_linux_memory_csv(
    mut reader: impl BufRead,
) -> Result<Vec<LinuxTimedMemorySample>, PerformanceError> {
    let mut header = String::new();
    reader.read_line(&mut header).map_err(|error| {
        PerformanceError::Io(format!("cannot read Linux memory CSV header: {error}"))
    })?;
    let observed_header = header.trim_end_matches(&['\r', '\n'][..]);
    if observed_header != LINUX_MEMORY_CSV_HEADER {
        return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
            "expected header `{LINUX_MEMORY_CSV_HEADER}`, observed `{observed_header}`"
        )));
    }

    let mut samples = Vec::new();
    let mut previous_elapsed = None;
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 2;
        let line = line.map_err(|error| {
            PerformanceError::Io(format!(
                "cannot read Linux memory CSV line {line_number}: {error}"
            ))
        })?;
        let fields = line.split(',').collect::<Vec<_>>();
        if fields.len() != 7 {
            return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
                "line {line_number} has {} columns; expected 7",
                fields.len()
            )));
        }
        let sample_index = parse_linux_csv_u64(fields[0], line_number, "sample index")?;
        let expected_index = u64::try_from(index).map_err(|error| {
            PerformanceError::InvalidLinuxMemoryCsv(format!(
                "line {line_number} index cannot fit u64: {error}"
            ))
        })?;
        if sample_index != expected_index {
            return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
                "line {line_number} sample index {sample_index} does not equal {expected_index}"
            )));
        }
        let elapsed_ns = parse_linux_csv_u64(fields[1], line_number, "elapsed time")?;
        if elapsed_ns == 0 || previous_elapsed.is_some_and(|previous| elapsed_ns <= previous) {
            return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
                "line {line_number} elapsed time {elapsed_ns} is not strictly increasing"
            )));
        }
        let pressure_level = parse_linux_csv_u64(fields[4], line_number, "memory pressure level")?;
        let memory_pressure = match pressure_level {
            0 => LinuxMemoryPressure::Normal,
            1 => LinuxMemoryPressure::Warning,
            2 => LinuxMemoryPressure::Critical,
            _ => {
                return Err(PerformanceError::InvalidLinuxMemoryCsv(format!(
                    "line {line_number} memory pressure level {pressure_level} is outside 0..=2"
                )));
            }
        };
        samples.push(LinuxTimedMemorySample {
            sample_index,
            elapsed_ns,
            process_rss_bytes: parse_linux_csv_u64(fields[2], line_number, "process RSS")?,
            system_used_memory_bytes: parse_linux_csv_u64(
                fields[3],
                line_number,
                "system used memory",
            )?,
            memory_pressure,
            swap_in_total_bytes: parse_linux_csv_u64(fields[5], line_number, "swap-in total")?,
            swap_out_total_bytes: parse_linux_csv_u64(fields[6], line_number, "swap-out total")?,
        });
        previous_elapsed = Some(elapsed_ns);
    }
    if samples.is_empty() {
        return Err(PerformanceError::InvalidLinuxMemoryCsv(
            "Linux memory CSV contains no samples".to_owned(),
        ));
    }
    Ok(samples)
}

/// Verifies a normalized Linux memory CSV and returns its sample count.
///
/// # Errors
///
/// Returns an error when the file cannot be opened or violates the normalized
/// Linux memory CSV contract.
pub fn verify_linux_memory_csv(path: &Path) -> Result<usize, PerformanceError> {
    let file = File::open(path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot open Linux memory CSV {}: {error}",
            path.display()
        ))
    })?;
    parse_linux_memory_csv(io::BufReader::new(file)).map(|samples| samples.len())
}

fn parse_proc_kilobytes(input: &str, field: &str) -> Result<u64, PerformanceError> {
    let mut value = None;
    for line in input.lines() {
        let Some((name, contents)) = line.split_once(':') else {
            continue;
        };
        if name != field {
            continue;
        }
        if value.is_some() {
            return Err(PerformanceError::InvalidLinuxProc(format!(
                "duplicate {field} field"
            )));
        }
        let parts = contents.split_whitespace().collect::<Vec<_>>();
        if parts.len() != 2 || parts[1] != "kB" {
            return Err(PerformanceError::InvalidLinuxProc(format!(
                "{field} must contain one integer with the exact kB unit"
            )));
        }
        value = Some(parse_linux_u64(parts[0], field)?);
    }
    value.ok_or_else(|| PerformanceError::InvalidLinuxProc(format!("missing {field} field")))
}

fn parse_proc_counter(input: &str, field: &str) -> Result<u64, PerformanceError> {
    let mut value = None;
    for line in input.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.first().copied() != Some(field) {
            continue;
        }
        if value.is_some() {
            return Err(PerformanceError::InvalidLinuxProc(format!(
                "duplicate {field} counter"
            )));
        }
        if parts.len() != 2 {
            return Err(PerformanceError::InvalidLinuxProc(format!(
                "{field} must contain exactly one integer counter"
            )));
        }
        value = Some(parse_linux_u64(parts[1], field)?);
    }
    value.ok_or_else(|| PerformanceError::InvalidLinuxProc(format!("missing {field} counter")))
}

fn parse_psi_total(input: &str, category: &str) -> Result<u64, PerformanceError> {
    let mut value = None;
    for line in input.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some(category) {
            continue;
        }
        if value.is_some() {
            return Err(PerformanceError::InvalidLinuxProc(format!(
                "duplicate memory PSI {category} line"
            )));
        }
        let totals = parts
            .filter_map(|part| part.strip_prefix("total="))
            .collect::<Vec<_>>();
        if totals.len() != 1 {
            return Err(PerformanceError::InvalidLinuxProc(format!(
                "memory PSI {category} line must contain exactly one total counter"
            )));
        }
        value = Some(parse_linux_u64(
            totals[0],
            &format!("memory PSI {category} total"),
        )?);
    }
    value.ok_or_else(|| {
        PerformanceError::InvalidLinuxProc(format!("missing memory PSI {category} line"))
    })
}

fn parse_linux_u64(value: &str, field: &str) -> Result<u64, PerformanceError> {
    value.parse::<u64>().map_err(|error| {
        PerformanceError::InvalidLinuxProc(format!(
            "invalid Linux {field} value `{value}`: {error}"
        ))
    })
}

fn parse_linux_process_identity(
    stat: &str,
    expected_pid: u32,
) -> Result<LinuxProcessIdentity, PerformanceError> {
    let open = stat.find('(').ok_or_else(|| {
        PerformanceError::InvalidLinuxProc("Linux process stat is missing `(`".to_owned())
    })?;
    let close = stat.rfind(')').ok_or_else(|| {
        PerformanceError::InvalidLinuxProc("Linux process stat is missing `)`".to_owned())
    })?;
    if close <= open {
        return Err(PerformanceError::InvalidLinuxProc(
            "Linux process stat has an invalid command field".to_owned(),
        ));
    }
    let observed_pid = stat[..open].trim().parse::<u32>().map_err(|error| {
        PerformanceError::InvalidLinuxProc(format!(
            "Linux process stat has an invalid PID: {error}"
        ))
    })?;
    if observed_pid != expected_pid {
        return Err(PerformanceError::InvalidLinuxProc(format!(
            "Linux process stat PID {observed_pid} does not equal requested PID {expected_pid}"
        )));
    }
    let fields = stat[close + 1..].split_whitespace().collect::<Vec<_>>();
    let start_time = fields.get(19).ok_or_else(|| {
        PerformanceError::InvalidLinuxProc(
            "Linux process stat is missing field 22 start time".to_owned(),
        )
    })?;
    Ok(LinuxProcessIdentity {
        pid: expected_pid,
        start_time_ticks: parse_linux_u64(start_time, "process start time")?,
    })
}

fn validate_linux_process_identity(
    expected: LinuxProcessIdentity,
    observed: LinuxProcessIdentity,
) -> Result<(), PerformanceError> {
    if observed == expected {
        return Ok(());
    }
    Err(PerformanceError::InvalidLinuxProc(format!(
        "Linux process identity changed for PID {}: start time {} became {}",
        expected.pid, expected.start_time_ticks, observed.start_time_ticks
    )))
}

fn parse_linux_csv_u64(
    value: &str,
    line_number: usize,
    field: &str,
) -> Result<u64, PerformanceError> {
    value.parse::<u64>().map_err(|error| {
        PerformanceError::InvalidLinuxMemoryCsv(format!(
            "line {line_number} has invalid {field} `{value}`: {error}"
        ))
    })
}

fn read_linux_proc_file(path: &Path) -> Result<String, PerformanceError> {
    fs::read_to_string(path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot read Linux procfs source {}: {error}",
            path.display()
        ))
    })
}

fn checked_kilobytes_to_bytes(value: u64, field: &str) -> Result<u64, PerformanceError> {
    value.checked_mul(1024).ok_or_else(|| {
        PerformanceError::InvalidLinuxProc(format!(
            "Linux {field} value {value} kB overflows bytes"
        ))
    })
}

fn monotonic_counter_delta(
    previous: u64,
    current: u64,
    field: &str,
) -> Result<u64, PerformanceError> {
    current.checked_sub(previous).ok_or_else(|| {
        PerformanceError::InvalidLinuxProc(format!(
            "Linux cumulative {field} counter regressed from {previous} to {current}"
        ))
    })
}

/// Verifies that raw evidence written inside the workspace uses a repository
/// ignore boundary.
///
/// Paths outside the workspace are allowed. Relative paths are interpreted
/// from the workspace root and cannot contain traversal components.
///
/// # Errors
///
/// Returns an error when a workspace-local destination is tracked or escapes
/// through a relative traversal.
pub fn validate_private_evidence_output(
    workspace_root: &Path,
    destination: &Path,
) -> Result<(), PerformanceError> {
    let relative = if destination.is_absolute() {
        let Ok(relative) = destination.strip_prefix(workspace_root) else {
            return Ok(());
        };
        relative
    } else {
        destination
    };
    let components = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => Err(PerformanceError::TracyTool(format!(
                "raw evidence destination must not traverse the workspace: {}",
                destination.display()
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let ignored_fixture = components.starts_with(&[
        std::ffi::OsStr::new("tests"),
        std::ffi::OsStr::new("fixtures"),
        std::ffi::OsStr::new("local"),
    ]) && components.len() > 3;
    let private_evidence = components.starts_with(&[
        std::ffi::OsStr::new("evidence"),
        std::ffi::OsStr::new("private"),
    ]) && components.len() > 2;
    let raw_evidence = components.first() == Some(&std::ffi::OsStr::new("evidence"))
        && components
            .iter()
            .take(components.len().saturating_sub(1))
            .any(|component| {
                *component == std::ffi::OsStr::new("raw")
                    || *component == std::ffi::OsStr::new("captures")
            });
    if ignored_fixture || private_evidence || raw_evidence {
        return Ok(());
    }

    Err(PerformanceError::TracyTool(format!(
        "raw evidence inside the workspace must use an ignored private boundary: {}",
        destination.display()
    )))
}

/// Parses exact `Client Tick` and `Server Tick` frames from the pinned Tracy
/// exporter format.
///
/// # Errors
///
/// Returns an error when input cannot be read, its header or frame-set names
/// drift, duration arithmetic is inconsistent, indices are not contiguous, or
/// either required frame set is absent.
pub fn parse_tracy_frame_csv(
    mut reader: impl BufRead,
) -> Result<Vec<TracyFrameSample>, PerformanceError> {
    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|error| PerformanceError::Io(format!("cannot read Tracy CSV header: {error}")))?;
    let observed_header = header.trim_end_matches(&['\r', '\n'][..]);
    if observed_header != TRACY_FRAME_CSV_HEADER {
        return Err(PerformanceError::InvalidTracyFrameCsv(format!(
            "expected header `{TRACY_FRAME_CSV_HEADER}`, observed `{observed_header}`"
        )));
    }

    let mut samples = Vec::new();
    let mut client_state = None;
    let mut server_state = None;
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 2;
        let line = line.map_err(|error| {
            PerformanceError::Io(format!("cannot read Tracy CSV line {line_number}: {error}"))
        })?;
        let fields = line.split(',').collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(PerformanceError::InvalidTracyFrameCsv(format!(
                "line {line_number} has {} columns; expected 5",
                fields.len()
            )));
        }

        let metric = match fields[0] {
            "Client Tick" => TracyFrameMetric::FrameTime,
            "Server Tick" => TracyFrameMetric::IntegratedServerTickTime,
            frame_set => {
                return Err(PerformanceError::InvalidTracyFrameCsv(format!(
                    "line {line_number} has unsupported frame set `{frame_set}`"
                )));
            }
        };
        let frame_index = parse_tracy_u64(fields[1], line_number, "frame index")?;
        let start_ns = parse_tracy_u64(fields[2], line_number, "start")?;
        let end_ns = parse_tracy_u64(fields[3], line_number, "end")?;
        let duration_ns = parse_tracy_u64(fields[4], line_number, "duration")?;
        let calculated_duration = end_ns.checked_sub(start_ns).ok_or_else(|| {
            PerformanceError::InvalidTracyFrameCsv(format!(
                "line {line_number} end {end_ns} precedes start {start_ns}"
            ))
        })?;
        if duration_ns != calculated_duration {
            return Err(PerformanceError::InvalidTracyFrameCsv(format!(
                "line {line_number} duration {duration_ns} does not equal end-start \
                 {calculated_duration}"
            )));
        }

        let state = match metric {
            TracyFrameMetric::FrameTime => &mut client_state,
            TracyFrameMetric::IntegratedServerTickTime => &mut server_state,
        };
        validate_tracy_frame_order(*state, metric, frame_index, start_ns, line_number)?;
        *state = Some((frame_index, start_ns));
        samples.push(TracyFrameSample {
            metric,
            frame_index,
            start_ns,
            duration_ns,
        });
    }

    for (metric, state) in [
        (TracyFrameMetric::FrameTime, client_state),
        (TracyFrameMetric::IntegratedServerTickTime, server_state),
    ] {
        if state.is_none() {
            return Err(PerformanceError::InvalidTracyFrameCsv(format!(
                "missing required `{}` frame set",
                metric.frame_set_name()
            )));
        }
    }

    Ok(samples)
}

/// Verifies a pinned Tracy frame CSV and reports its exact sample totals.
///
/// # Errors
///
/// Returns an error when the file cannot be opened or its contents violate the
/// pinned Tracy frame contract.
pub fn verify_tracy_frame_csv(path: &Path) -> Result<TracyFrameCounts, PerformanceError> {
    let file = File::open(path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot open Tracy frame CSV {}: {error}",
            path.display()
        ))
    })?;
    let samples = parse_tracy_frame_csv(io::BufReader::new(file))?;
    let client_frames = samples
        .iter()
        .filter(|sample| sample.metric == TracyFrameMetric::FrameTime)
        .count();
    let integrated_server_ticks = samples.len() - client_frames;
    Ok(TracyFrameCounts {
        client_frames,
        integrated_server_ticks,
    })
}

/// Exports and validates Minecraft's pinned Tracy frame sets without desktop
/// automation.
///
/// The destination is created exclusively and removed if the exporter fails or
/// emits invalid data.
///
/// # Errors
///
/// Returns an error when the destination already exists, the pinned exporter
/// cannot run, or the generated CSV violates the Tracy frame contract.
pub fn export_tracy_frame_csv(
    trace: &Path,
    destination: &Path,
) -> Result<TracyFrameCounts, PerformanceError> {
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| {
            PerformanceError::Io(format!(
                "cannot create Tracy frame CSV {}: {error}",
                destination.display()
            ))
        })?;
    let status = Command::new("tracy-csvexport")
        .args(["--frames", "--case", "--filter= Tick"])
        .arg(trace)
        .stdout(Stdio::from(output))
        .status();
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            remove_partial_performance_output(destination, "Tracy frame CSV");
            return Err(PerformanceError::Io(format!(
                "cannot run tracy-csvexport: {error}"
            )));
        }
    };
    if !status.success() {
        remove_partial_performance_output(destination, "Tracy frame CSV");
        return Err(PerformanceError::InvalidTracyFrameCsv(format!(
            "tracy-csvexport failed with status {status}"
        )));
    }

    match verify_tracy_frame_csv(destination) {
        Ok(counts) => Ok(counts),
        Err(error) => {
            remove_partial_performance_output(destination, "Tracy frame CSV");
            Err(error)
        }
    }
}

/// Captures an attached Tracy client for a fixed duration without desktop
/// input.
///
/// # Errors
///
/// Returns an error when the duration is zero, the destination already exists,
/// the collector fails, or no nonempty trace is produced.
pub fn capture_tracy(duration_seconds: u64, destination: &Path) -> Result<(), PerformanceError> {
    if duration_seconds == 0 {
        return Err(PerformanceError::TracyTool(
            "Tracy capture duration must be greater than zero".to_owned(),
        ));
    }
    if destination.try_exists().map_err(|error| {
        PerformanceError::Io(format!(
            "cannot inspect Tracy trace destination {}: {error}",
            destination.display()
        ))
    })? {
        return Err(PerformanceError::TracyTool(format!(
            "Tracy trace destination already exists: {}",
            destination.display()
        )));
    }

    let status = Command::new("tracy-capture")
        .arg("-o")
        .arg(destination)
        .args(["-s", &duration_seconds.to_string()])
        .status()
        .map_err(|error| PerformanceError::Io(format!("cannot run tracy-capture: {error}")))?;
    if !status.success() {
        remove_partial_performance_output(destination, "Tracy trace");
        return Err(PerformanceError::TracyTool(format!(
            "tracy-capture failed with status {status}"
        )));
    }

    let metadata = fs::metadata(destination).map_err(|error| {
        PerformanceError::Io(format!(
            "tracy-capture did not produce {}: {error}",
            destination.display()
        ))
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        remove_partial_performance_output(destination, "Tracy trace");
        return Err(PerformanceError::TracyTool(format!(
            "tracy-capture produced an empty or non-file trace: {}",
            destination.display()
        )));
    }
    Ok(())
}

fn remove_partial_performance_output(path: &Path, label: &str) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        eprintln!(
            "warning: cannot remove partial {label} {}: {error}",
            path.display()
        );
    }
}

fn remove_linux_output_if_owned(path: &Path, expected: LinuxOutputIdentity) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    let observed = LinuxOutputIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    if metadata.file_type().is_file()
        && observed == expected
        && let Err(error) = fs::remove_file(path)
    {
        eprintln!(
            "warning: cannot remove partial Linux memory CSV {}: {error}",
            path.display()
        );
    }
}

fn parse_tracy_u64(value: &str, line_number: usize, field: &str) -> Result<u64, PerformanceError> {
    value.parse::<u64>().map_err(|error| {
        PerformanceError::InvalidTracyFrameCsv(format!(
            "line {line_number} has invalid {field} `{value}`: {error}"
        ))
    })
}

fn validate_tracy_frame_order(
    state: Option<(u64, u64)>,
    metric: TracyFrameMetric,
    frame_index: u64,
    start_ns: u64,
    line_number: usize,
) -> Result<(), PerformanceError> {
    let frame_set = metric.frame_set_name();
    let Some((previous_index, previous_start)) = state else {
        if frame_index != 0 {
            return Err(PerformanceError::InvalidTracyFrameCsv(format!(
                "line {line_number} `{frame_set}` frame index {frame_index} does not start at 0"
            )));
        }
        return Ok(());
    };

    let expected_index = previous_index.checked_add(1).ok_or_else(|| {
        PerformanceError::InvalidTracyFrameCsv(format!(
            "line {line_number} `{frame_set}` previous frame index cannot be incremented"
        ))
    })?;
    if frame_index != expected_index {
        return Err(PerformanceError::InvalidTracyFrameCsv(format!(
            "line {line_number} `{frame_set}` frame index {frame_index} does not follow \
             {previous_index}"
        )));
    }
    if start_ns < previous_start {
        return Err(PerformanceError::InvalidTracyFrameCsv(format!(
            "line {line_number} `{frame_set}` start {start_ns} precedes {previous_start}"
        )));
    }
    Ok(())
}

/// Public metadata needed to reconstruct an ignored local fixture.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureDescriptor {
    /// Descriptor format version.
    pub schema_version: u32,
    /// Stable local fixture identifier.
    pub id: String,
    /// Authorized source metadata.
    pub source: FixtureSource,
    /// Expected metadata inside the source archive.
    pub manifest: ExpectedManifest,
}

/// Authorized source and integrity metadata.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureSource {
    /// Source service kind.
    pub kind: String,
    /// `CurseForge` project identifier.
    pub project_id: u64,
    /// `CurseForge` file identifier.
    pub file_id: u64,
    /// Human-readable source page.
    pub page_url: String,
    /// Direct publisher download URL.
    pub download_url: String,
    /// Terms governing the source.
    pub terms_url: String,
    /// Local archive filename.
    pub file_name: String,
    /// Expected archive SHA-256.
    pub sha256: String,
}

/// Expected fields in a `CurseForge` client-pack manifest.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedManifest {
    /// Archive member containing the manifest.
    pub entry: String,
    /// Expected manifest SHA-256.
    pub sha256: String,
    /// `CurseForge` manifest type.
    pub manifest_type: String,
    /// `CurseForge` manifest format version.
    pub manifest_version: u32,
    /// Pack display name.
    pub name: String,
    /// Pack version.
    pub version: String,
    /// Pack author.
    pub author: String,
    /// Minecraft version.
    pub minecraft: String,
    /// Primary mod-loader identifier.
    pub mod_loader: String,
    /// Number of referenced `CurseForge` files.
    pub expected_reference_count: usize,
}

/// Admitted identity fields from a verified fixture archive.
#[derive(Debug, Eq, PartialEq)]
pub struct FixtureVerification {
    /// Minecraft version.
    pub minecraft: String,
    /// Primary mod-loader identifier.
    pub mod_loader: String,
    /// Pack version.
    pub pack_version: String,
    /// Referenced `CurseForge` file count.
    pub reference_count: usize,
}

/// A fixture reconstruction or verification failure.
#[derive(Debug, Eq, PartialEq)]
pub enum FixtureError {
    /// A local file operation failed.
    Io(String),
    /// JSON parsing failed.
    Json(String),
    /// A descriptor field violates the public fixture policy.
    InvalidDescriptor(String),
    /// An artifact did not match its recorded digest.
    HashMismatch {
        /// Artifact being checked.
        artifact: &'static str,
        /// Recorded SHA-256.
        expected: String,
        /// Observed SHA-256.
        actual: String,
    },
    /// The archive could not provide its manifest.
    Archive(String),
    /// A pinned manifest field drifted.
    MetadataMismatch {
        /// Field being checked.
        field: &'static str,
        /// Recorded value.
        expected: String,
        /// Observed value.
        actual: String,
    },
    /// The `CurseForge` reference total drifted.
    ReferenceCountMismatch {
        /// Recorded reference total.
        expected: usize,
        /// Observed reference total.
        actual: usize,
    },
    /// The manifest contains the same project/file pair more than once.
    DuplicateReference {
        /// `CurseForge` project identifier.
        project_id: u64,
        /// `CurseForge` file identifier.
        file_id: u64,
    },
    /// The download process failed.
    Download(String),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message)
            | Self::Json(message)
            | Self::InvalidDescriptor(message)
            | Self::Archive(message)
            | Self::Download(message) => formatter.write_str(message),
            Self::HashMismatch {
                artifact,
                expected,
                actual,
            } => write!(
                formatter,
                "{artifact} SHA-256 mismatch: expected {expected}, observed {actual}"
            ),
            Self::MetadataMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "fixture manifest {field} mismatch: expected {expected}, observed {actual}"
            ),
            Self::ReferenceCountMismatch { expected, actual } => write!(
                formatter,
                "fixture manifest reference count mismatch: expected {expected}, observed {actual}"
            ),
            Self::DuplicateReference {
                project_id,
                file_id,
            } => write!(
                formatter,
                "fixture manifest repeats CurseForge project {project_id} file {file_id}"
            ),
        }
    }
}

impl std::error::Error for FixtureError {}

/// Loads and validates a public fixture descriptor.
///
/// # Errors
///
/// Returns an error when the descriptor cannot be read, parsed, or violates
/// Ravel's fixed source and local-path policy.
pub fn load_fixture_descriptor(path: &Path) -> Result<FixtureDescriptor, FixtureError> {
    let source = fs::read(path)
        .map_err(|error| FixtureError::Io(format!("cannot read {}: {error}", path.display())))?;
    let descriptor: FixtureDescriptor = serde_json::from_slice(&source).map_err(|error| {
        FixtureError::Json(format!(
            "cannot parse fixture descriptor {}: {error}",
            path.display()
        ))
    })?;
    validate_descriptor(&descriptor)?;
    Ok(descriptor)
}

/// Computes the lowercase SHA-256 digest of a file.
///
/// # Errors
///
/// Returns an error when the file cannot be opened or read.
pub fn sha256_file(path: &Path) -> Result<String, FixtureError> {
    let mut file = File::open(path)
        .map_err(|error| FixtureError::Io(format!("cannot open {}: {error}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            FixtureError::Io(format!("cannot hash {}: {error}", path.display()))
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex_digest(&hasher.finalize()))
}

/// Verifies an archive digest and its pinned `CurseForge` manifest.
///
/// # Errors
///
/// Returns an error for descriptor, hash, archive, metadata, count, or
/// duplicate-reference drift.
pub fn verify_fixture_archive(
    descriptor_path: &Path,
    archive_path: &Path,
) -> Result<FixtureVerification, FixtureError> {
    let descriptor = load_fixture_descriptor(descriptor_path)?;
    let archive_hash = sha256_file(archive_path)?;
    require_hash("fixture archive", &descriptor.source.sha256, &archive_hash)?;

    let output = Command::new("unzip")
        .arg("-p")
        .arg(archive_path)
        .arg(&descriptor.manifest.entry)
        .output()
        .map_err(|error| FixtureError::Archive(format!("cannot execute unzip: {error}")))?;
    if !output.status.success() {
        return Err(FixtureError::Archive(format!(
            "cannot read {} from {}",
            descriptor.manifest.entry,
            archive_path.display()
        )));
    }

    let manifest_hash = sha256_bytes(&output.stdout);
    require_hash(
        "fixture manifest",
        &descriptor.manifest.sha256,
        &manifest_hash,
    )?;
    let manifest: CurseForgeManifest = serde_json::from_slice(&output.stdout).map_err(|error| {
        FixtureError::Json(format!(
            "cannot parse {} from {}: {error}",
            descriptor.manifest.entry,
            archive_path.display()
        ))
    })?;

    verify_manifest(&descriptor.manifest, &manifest)
}

/// Downloads and verifies the pinned archive under the ignored fixture root.
///
/// Existing verified archives are reused. Existing mismatched archives are
/// preserved and reported instead of overwritten.
///
/// # Errors
///
/// Returns an error when paths escape the workspace, the download fails, or
/// verification fails.
pub fn reconstruct_fixture(
    workspace_root: &Path,
    descriptor_path: &Path,
) -> Result<PathBuf, FixtureError> {
    let descriptor = load_fixture_descriptor(descriptor_path)?;
    let workspace_root = workspace_root.canonicalize().map_err(|error| {
        FixtureError::Io(format!(
            "cannot resolve workspace root {}: {error}",
            workspace_root.display()
        ))
    })?;
    let local_root = workspace_root.join("tests/fixtures/local");
    fs::create_dir_all(&local_root).map_err(|error| {
        FixtureError::Io(format!(
            "cannot create local fixture root {}: {error}",
            local_root.display()
        ))
    })?;
    let local_root = local_root.canonicalize().map_err(|error| {
        FixtureError::Io(format!(
            "cannot resolve local fixture root {}: {error}",
            local_root.display()
        ))
    })?;
    if !local_root.starts_with(&workspace_root) {
        return Err(FixtureError::InvalidDescriptor(
            "local fixture root resolves outside the workspace".to_owned(),
        ));
    }

    let source_root = local_root.join(&descriptor.id).join("source");
    fs::create_dir_all(&source_root).map_err(|error| {
        FixtureError::Io(format!(
            "cannot create fixture source root {}: {error}",
            source_root.display()
        ))
    })?;
    let source_root = source_root.canonicalize().map_err(|error| {
        FixtureError::Io(format!(
            "cannot resolve fixture source root {}: {error}",
            source_root.display()
        ))
    })?;
    if !source_root.starts_with(&local_root) {
        return Err(FixtureError::InvalidDescriptor(
            "fixture source root resolves outside tests/fixtures/local".to_owned(),
        ));
    }

    let destination = source_root.join(&descriptor.source.file_name);
    if destination.exists() {
        verify_fixture_archive(descriptor_path, &destination)?;
        return Ok(destination);
    }

    let partial = source_root.join(format!("{}.partial", descriptor.source.file_name));
    if partial.exists() {
        fs::remove_file(&partial).map_err(|error| {
            FixtureError::Io(format!(
                "cannot remove stale partial download {}: {error}",
                partial.display()
            ))
        })?;
    }

    let status = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--max-redirs",
            "0",
            "--output",
        ])
        .arg(&partial)
        .arg(&descriptor.source.download_url)
        .status()
        .map_err(|error| FixtureError::Download(format!("cannot execute curl: {error}")))?;
    if !status.success() {
        remove_partial(&partial);
        return Err(FixtureError::Download(format!(
            "fixture download failed with {status}"
        )));
    }

    if let Err(error) = verify_fixture_archive(descriptor_path, &partial) {
        remove_partial(&partial);
        return Err(error);
    }
    fs::rename(&partial, &destination).map_err(|error| {
        FixtureError::Io(format!(
            "cannot admit verified fixture archive {}: {error}",
            destination.display()
        ))
    })?;

    Ok(destination)
}

fn validate_descriptor(descriptor: &FixtureDescriptor) -> Result<(), FixtureError> {
    if descriptor.schema_version != 1 {
        return Err(FixtureError::InvalidDescriptor(
            "fixture descriptor schemaVersion must equal 1".to_owned(),
        ));
    }
    if !is_safe_component(&descriptor.id) {
        return Err(FixtureError::InvalidDescriptor(
            "fixture descriptor id is not a safe path component".to_owned(),
        ));
    }
    if descriptor.source.kind != "curseforge-client-pack"
        || descriptor.source.project_id == 0
        || descriptor.source.file_id == 0
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture source must identify a CurseForge client-pack file".to_owned(),
        ));
    }
    if !descriptor
        .source
        .download_url
        .starts_with(ALLOWED_DOWNLOAD_PREFIX)
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture download URL is outside the allowed CurseForge CDN".to_owned(),
        ));
    }
    if !descriptor
        .source
        .page_url
        .starts_with("https://www.curseforge.com/")
        || !descriptor.source.terms_url.starts_with("https://")
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture source pages must use the recorded HTTPS origins".to_owned(),
        ));
    }
    if !is_safe_component(&descriptor.source.file_name)
        || !Path::new(&descriptor.source.file_name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture filename must be a local .zip path component".to_owned(),
        ));
    }
    if descriptor.manifest.entry != "manifest.json"
        || descriptor.manifest.expected_reference_count == 0
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture must pin a non-empty CurseForge manifest.json".to_owned(),
        ));
    }
    for (field, hash) in [
        ("source.sha256", descriptor.source.sha256.as_str()),
        ("manifest.sha256", descriptor.manifest.sha256.as_str()),
    ] {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(FixtureError::InvalidDescriptor(format!(
                "fixture descriptor {field} must be a 64-character hexadecimal SHA-256"
            )));
        }
    }
    Ok(())
}

fn verify_manifest(
    expected: &ExpectedManifest,
    actual: &CurseForgeManifest,
) -> Result<FixtureVerification, FixtureError> {
    require_metadata(
        "manifestType",
        &expected.manifest_type,
        &actual.manifest_type,
    )?;
    require_metadata(
        "manifestVersion",
        &expected.manifest_version.to_string(),
        &actual.manifest_version.to_string(),
    )?;
    require_metadata("name", &expected.name, &actual.name)?;
    require_metadata("version", &expected.version, &actual.version)?;
    require_metadata("author", &expected.author, &actual.author)?;
    require_metadata(
        "minecraft.version",
        &expected.minecraft,
        &actual.minecraft.version,
    )?;

    let primary_loaders = actual
        .minecraft
        .mod_loaders
        .iter()
        .filter(|loader| loader.primary)
        .collect::<Vec<_>>();
    let actual_loader = match primary_loaders.as_slice() {
        [loader] => &loader.id,
        _ => {
            return Err(FixtureError::MetadataMismatch {
                field: "minecraft.modLoaders.primary",
                expected: expected.mod_loader.clone(),
                actual: format!("{} primary entries", primary_loaders.len()),
            });
        }
    };
    require_metadata("minecraft.modLoaders", &expected.mod_loader, actual_loader)?;

    if actual.files.len() != expected.expected_reference_count {
        return Err(FixtureError::ReferenceCountMismatch {
            expected: expected.expected_reference_count,
            actual: actual.files.len(),
        });
    }
    let mut references = HashSet::with_capacity(actual.files.len());
    for reference in &actual.files {
        if !references.insert((reference.project_id, reference.file_id)) {
            return Err(FixtureError::DuplicateReference {
                project_id: reference.project_id,
                file_id: reference.file_id,
            });
        }
    }

    Ok(FixtureVerification {
        minecraft: actual.minecraft.version.clone(),
        mod_loader: actual_loader.clone(),
        pack_version: actual.version.clone(),
        reference_count: actual.files.len(),
    })
}

fn require_hash(artifact: &'static str, expected: &str, actual: &str) -> Result<(), FixtureError> {
    if expected.eq_ignore_ascii_case(actual) {
        Ok(())
    } else {
        Err(FixtureError::HashMismatch {
            artifact,
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn require_metadata(field: &'static str, expected: &str, actual: &str) -> Result<(), FixtureError> {
    if expected == actual {
        Ok(())
    } else {
        Err(FixtureError::MetadataMismatch {
            field,
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex_digest(&Sha256::digest(bytes))
}

fn hex_digest(digest: &[u8]) -> String {
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn is_safe_component(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && !value.contains(['/', '\\'])
}

fn remove_partial(path: &Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        eprintln!(
            "warning: cannot remove partial fixture {}: {error}",
            path.display()
        );
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeManifest {
    minecraft: MinecraftManifest,
    manifest_type: String,
    manifest_version: u32,
    name: String,
    version: String,
    author: String,
    files: Vec<CurseForgeReference>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftManifest {
    version: String,
    mod_loaders: Vec<ModLoader>,
}

#[derive(Debug, Deserialize)]
struct ModLoader {
    id: String,
    primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeReference {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        CurseForgeManifest, CurseForgeReference, FixtureDescriptor, FixtureError,
        MinecraftManifest, ModLoader, is_safe_component, reconstruct_fixture, remove_partial,
        sha256_file, validate_descriptor, verify_manifest,
    };

    #[test]
    fn descriptor_policy_rejects_each_unsafe_boundary() {
        let mut descriptor = valid_descriptor();
        descriptor.schema_version = 2;
        assert_invalid(&descriptor, "schemaVersion");

        let mut descriptor = valid_descriptor();
        descriptor.id = "../escape".to_owned();
        assert_invalid(&descriptor, "safe path component");

        let mut descriptor = valid_descriptor();
        descriptor.source.project_id = 0;
        assert_invalid(&descriptor, "CurseForge client-pack");

        let mut descriptor = valid_descriptor();
        descriptor.source.download_url = "https://example.invalid/fixture.zip".to_owned();
        assert_invalid(&descriptor, "allowed CurseForge CDN");

        let mut descriptor = valid_descriptor();
        descriptor.source.page_url = "http://www.curseforge.com/project".to_owned();
        assert_invalid(&descriptor, "HTTPS origins");

        let mut descriptor = valid_descriptor();
        descriptor.source.file_name = "fixture.jar".to_owned();
        assert_invalid(&descriptor, "local .zip");

        let mut descriptor = valid_descriptor();
        descriptor.manifest.expected_reference_count = 0;
        assert_invalid(&descriptor, "non-empty");

        let mut descriptor = valid_descriptor();
        descriptor.source.sha256 = "not-a-digest".to_owned();
        assert_invalid(&descriptor, "source.sha256");
    }

    #[test]
    fn manifest_policy_reports_identity_loader_and_duplicate_drift() {
        let expected = valid_descriptor().manifest;
        let mut manifest = valid_manifest();
        manifest.name = "Changed".to_owned();
        assert!(matches!(
            verify_manifest(&expected, &manifest),
            Err(FixtureError::MetadataMismatch { field: "name", .. })
        ));

        let mut manifest = valid_manifest();
        manifest.minecraft.mod_loaders.push(ModLoader {
            id: "another-loader".to_owned(),
            primary: true,
        });
        assert!(matches!(
            verify_manifest(&expected, &manifest),
            Err(FixtureError::MetadataMismatch {
                field: "minecraft.modLoaders.primary",
                ..
            })
        ));

        let mut manifest = valid_manifest();
        manifest.files.push(CurseForgeReference {
            project_id: 1,
            file_id: 2,
        });
        let mut expected = expected;
        expected.expected_reference_count = 2;
        assert_eq!(
            verify_manifest(&expected, &manifest),
            Err(FixtureError::DuplicateReference {
                project_id: 1,
                file_id: 2
            })
        );
    }

    #[test]
    fn fixture_errors_have_actionable_messages() {
        let errors = [
            FixtureError::Io("io".to_owned()),
            FixtureError::Json("json".to_owned()),
            FixtureError::InvalidDescriptor("descriptor".to_owned()),
            FixtureError::Archive("archive".to_owned()),
            FixtureError::Download("download".to_owned()),
            FixtureError::HashMismatch {
                artifact: "archive",
                expected: "a".to_owned(),
                actual: "b".to_owned(),
            },
            FixtureError::MetadataMismatch {
                field: "name",
                expected: "a".to_owned(),
                actual: "b".to_owned(),
            },
            FixtureError::ReferenceCountMismatch {
                expected: 235,
                actual: 1,
            },
            FixtureError::DuplicateReference {
                project_id: 1,
                file_id: 2,
            },
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn safe_components_exclude_hidden_and_nested_paths() {
        assert!(is_safe_component("fixture.zip"));
        assert!(!is_safe_component(""));
        assert!(!is_safe_component(".hidden"));
        assert!(!is_safe_component("../escape"));
        assert!(!is_safe_component("nested/file.zip"));
        assert!(!is_safe_component(r"nested\file.zip"));
    }

    #[test]
    fn removing_an_absent_partial_is_idempotent() {
        remove_partial(Path::new("/definitely/not/a/ravel/partial"));
    }

    #[test]
    fn reconstruction_admits_an_existing_verified_fixture() {
        let root = temporary_root("unit-reconstruct");
        let source_root = root.join("tests/fixtures/local/fixture/source");
        fs::create_dir_all(&source_root).expect("fixture root should be creatable");
        let manifest = source_root.join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({
                "minecraft": {
                    "version": "26.1.2",
                    "modLoaders": [{"id": "neoforge-26.1.2.78", "primary": true}]
                },
                "manifestType": "minecraftModpack",
                "manifestVersion": 1,
                "name": "All the Mods 11",
                "version": "0.2.1",
                "author": "ATMTeam",
                "files": [{"projectID": 1, "fileID": 2, "required": true}],
                "overrides": "overrides"
            }))
            .expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
        let status = Command::new("zip")
            .args(["-q", "fixture.zip", "manifest.json"])
            .current_dir(&source_root)
            .status()
            .expect("zip should execute");
        assert!(status.success());
        let archive = source_root.join("fixture.zip");
        let descriptor = root.join("descriptor.json");
        let mut value =
            serde_json::to_value(valid_descriptor()).expect("descriptor should serialize");
        value["source"]["sha256"] = sha256_file(&archive)
            .expect("archive should be hashable")
            .into();
        value["manifest"]["sha256"] = sha256_file(&manifest)
            .expect("manifest should be hashable")
            .into();
        fs::write(
            &descriptor,
            serde_json::to_vec(&value).expect("descriptor should serialize"),
        )
        .expect("descriptor should be writable");

        let admitted =
            reconstruct_fixture(&root, &descriptor).expect("verified fixture should be admitted");

        assert_eq!(admitted, archive);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn assert_invalid(descriptor: &FixtureDescriptor, expected_message: &str) {
        let error = validate_descriptor(descriptor)
            .expect_err("the altered descriptor should violate fixture policy");
        assert!(error.to_string().contains(expected_message));
    }

    fn valid_descriptor() -> FixtureDescriptor {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "id": "fixture",
            "source": {
                "kind": "curseforge-client-pack",
                "projectId": 1,
                "fileId": 2,
                "pageUrl": "https://www.curseforge.com/project",
                "downloadUrl": "https://mediafilez.forgecdn.net/files/1/2/fixture.zip",
                "termsUrl": "https://support.curseforge.com/terms",
                "fileName": "fixture.zip",
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "manifest": {
                "entry": "manifest.json",
                "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "manifestType": "minecraftModpack",
                "manifestVersion": 1,
                "name": "All the Mods 11",
                "version": "0.2.1",
                "author": "ATMTeam",
                "minecraft": "26.1.2",
                "modLoader": "neoforge-26.1.2.78",
                "expectedReferenceCount": 1
            }
        }))
        .expect("test descriptor should deserialize")
    }

    fn valid_manifest() -> CurseForgeManifest {
        CurseForgeManifest {
            minecraft: MinecraftManifest {
                version: "26.1.2".to_owned(),
                mod_loaders: vec![ModLoader {
                    id: "neoforge-26.1.2.78".to_owned(),
                    primary: true,
                }],
            },
            manifest_type: "minecraftModpack".to_owned(),
            manifest_version: 1,
            name: "All the Mods 11".to_owned(),
            version: "0.2.1".to_owned(),
            author: "ATMTeam".to_owned(),
            files: vec![CurseForgeReference {
                project_id: 1,
                file_id: 2,
            }],
        }
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-cli-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }
}
