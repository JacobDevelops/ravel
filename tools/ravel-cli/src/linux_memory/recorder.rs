//! Fixed-deadline Linux memory capture and CSV validation.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::PerformanceError;
use crate::evidence::remove_partial_performance_output;

use super::{
    LinuxMemoryPressure, LinuxMemorySnapshot, LinuxProcessIdentity, LinuxTimedMemorySample,
    normalize_linux_memory_sample, read_linux_memory_snapshot_for_process,
    read_linux_process_identity,
};

const LINUX_MEMORY_CSV_HEADER: &str = "sample_index,elapsed_ns,process_rss_bytes,\
system_used_memory_bytes,memory_pressure_level,swap_in_total_bytes,swap_out_total_bytes";

/// Incremental Linux memory CSV writer driven by an external monotonic clock.
/// Dropping an unfinished recorder removes its destination.
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
    /// Returns `false` before the deadline and `true` after writing a sample.
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

/// Captures a fixed number of normalized Linux memory samples to a new CSV.
/// Partial output is removed on failure.
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
