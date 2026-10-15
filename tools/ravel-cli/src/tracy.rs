//! Tracy frame parsing, export, and timed capture.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::PerformanceError;
use crate::evidence::remove_partial_performance_output;

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
/// Partial output is removed if export or validation fails.
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
