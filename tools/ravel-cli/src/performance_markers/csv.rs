//! Lifecycle-marker CSV I/O and profile validation.

use std::fs::File;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use super::invalid_marker;
use super::model::{
    ALIGNMENT_TOLERANCE_NS, COOL_DOWN_NS, EXPECTED_EVENT_COUNT, EXPECTED_KINDS, EXPECTED_ROUTES,
    InitialPerformanceMarkerStream, MEASUREMENT_NS, PerformanceEvent, PerformanceEventKind,
    SECOND_NS, WARM_UP_NS,
};
use crate::error::PerformanceError;

const MARKER_CSV_HEADER: &str = "event_index,elapsed_ns,kind,route_step,dimension";

/// Parses and validates a lifecycle-marker protocol version 2 CSV.
///
/// # Errors
///
/// Returns an error for I/O failure, header or column drift, invalid metadata,
/// missing or reordered events, incorrect profile routes or dimensions, phase
/// duration drift, or insufficient cool-down.
pub fn parse_initial_performance_marker_csv(
    reader: impl BufRead,
) -> Result<InitialPerformanceMarkerStream, PerformanceError> {
    let mut lines = reader.lines();
    let header = lines
        .next()
        .transpose()
        .map_err(|error| marker_read_error(&error))?
        .ok_or_else(|| invalid_marker("performance marker CSV is empty"))?;
    if header != MARKER_CSV_HEADER {
        return Err(invalid_marker(format!(
            "performance marker CSV header must be `{MARKER_CSV_HEADER}`"
        )));
    }

    let mut events = Vec::new();
    let mut previous_elapsed_ns = None;
    for (offset, line) in lines.enumerate() {
        let line_number = offset + 2;
        let line = line.map_err(|error| marker_read_error(&error))?;
        let columns = line.split(',').collect::<Vec<_>>();
        if columns.len() != 5 {
            return Err(invalid_marker(format!(
                "line {line_number} has {} columns, expected 5",
                columns.len()
            )));
        }
        let expected_index = u64::try_from(events.len())
            .map_err(|_| invalid_marker("performance marker count exceeds u64"))?;
        let event_index = parse_marker_u64(columns[0], line_number, "event index")?;
        if event_index != expected_index {
            return Err(invalid_marker(format!(
                "line {line_number} has event index {event_index}, expected {expected_index}"
            )));
        }
        let elapsed_ns = parse_marker_u64(columns[1], line_number, "elapsed nanoseconds")?;
        if previous_elapsed_ns.is_some_and(|previous| elapsed_ns < previous) {
            return Err(invalid_marker(format!(
                "line {line_number} elapsed timestamp {elapsed_ns} regresses"
            )));
        }
        previous_elapsed_ns = Some(elapsed_ns);
        let kind = PerformanceEventKind::parse(columns[2], line_number)?;
        let route_step = parse_optional_marker_field(columns[3], line_number, "route step")?;
        let dimension = parse_optional_marker_field(columns[4], line_number, "dimension")?;
        validate_marker_metadata(
            kind,
            route_step.as_deref(),
            dimension.as_deref(),
            line_number,
        )?;
        events.push(PerformanceEvent {
            event_index,
            elapsed_ns,
            kind,
            route_step,
            dimension,
        });
    }

    validate_initial_events(&events)?;
    Ok(InitialPerformanceMarkerStream { events })
}

/// Reopens and validates a lifecycle-marker protocol version 2 CSV.
///
/// # Errors
///
/// Returns an error when the file cannot be opened or violates the pinned
/// marker contract.
pub fn verify_initial_performance_marker_csv(path: &Path) -> Result<usize, PerformanceError> {
    let input = File::open(path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot open performance marker CSV {}: {error}",
            path.display()
        ))
    })?;
    let markers = parse_initial_performance_marker_csv(io::BufReader::new(input))?;
    Ok(markers.events().len())
}

pub(super) fn write_initial_marker_csv(
    events: &[PerformanceEvent],
    destination: &Path,
) -> Result<usize, PerformanceError> {
    validate_initial_events(events)?;
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .map_err(|error| {
            PerformanceError::Io(format!(
                "cannot create performance marker CSV {}: {error}",
                destination.display()
            ))
        })?;
    let metadata = output.metadata().map_err(|error| {
        PerformanceError::Io(format!(
            "cannot inspect performance marker CSV {}: {error}",
            destination.display()
        ))
    })?;
    let mut output_guard = MarkerOutputGuard {
        path: destination,
        identity: MarkerFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        },
        retained: false,
    };
    let result = (|| {
        let mut writer = io::BufWriter::new(output);
        writeln!(writer, "{MARKER_CSV_HEADER}").map_err(|error| {
            PerformanceError::Io(format!(
                "cannot write performance marker CSV header: {error}"
            ))
        })?;
        for event in events {
            writeln!(
                writer,
                "{},{},{},{},{}",
                event.event_index,
                event.elapsed_ns,
                event.kind.as_str(),
                event.route_step.as_deref().unwrap_or_default(),
                event.dimension.as_deref().unwrap_or_default()
            )
            .map_err(|error| {
                PerformanceError::Io(format!("cannot write performance marker CSV row: {error}"))
            })?;
        }
        writer.flush().map_err(|error| {
            PerformanceError::Io(format!("cannot flush performance marker CSV: {error}"))
        })?;
        verify_initial_performance_marker_csv(destination)
    })();
    if result.is_ok() {
        output_guard.retained = true;
    }
    result
}

struct MarkerOutputGuard<'a> {
    path: &'a Path,
    identity: MarkerFileIdentity,
    retained: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MarkerFileIdentity {
    device: u64,
    inode: u64,
}

impl Drop for MarkerOutputGuard<'_> {
    fn drop(&mut self) {
        if self.retained {
            return;
        }
        let Ok(metadata) = std::fs::symlink_metadata(self.path) else {
            return;
        };
        let identity = MarkerFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        if metadata.file_type().is_file() && identity == self.identity {
            let _ = std::fs::remove_file(self.path);
        }
    }
}

pub(super) fn validate_partial_event(
    event: &PerformanceEvent,
    position: usize,
) -> Result<(), PerformanceError> {
    validate_marker_metadata(
        event.kind,
        event.route_step.as_deref(),
        event.dimension.as_deref(),
        position + 2,
    )?;

    let route_index = match position {
        6 => Some(0),
        7 => Some(1),
        8 => Some(2),
        11 => Some(3),
        12 => Some(4),
        _ => None,
    };
    if let Some(route_index) = route_index {
        let (expected_step, expected_dimension, _, _) = EXPECTED_ROUTES[route_index];
        if event.route_step.as_deref() != Some(expected_step)
            || event.dimension.as_deref() != Some(expected_dimension)
        {
            return Err(invalid_marker(format!(
                "route event {position} must be `{expected_step}` in `{expected_dimension}`"
            )));
        }
    }

    let expected_dimension = match position {
        3 | 13 | 14 => Some("minecraft:overworld"),
        9 | 10 => Some("minecraft:the_nether"),
        _ => None,
    };
    if let Some(expected_dimension) = expected_dimension {
        require_dimension(event, expected_dimension)?;
    }
    Ok(())
}

pub(super) fn validate_initial_events(events: &[PerformanceEvent]) -> Result<(), PerformanceError> {
    if events.len() != EXPECTED_EVENT_COUNT {
        return Err(invalid_marker(format!(
            "initial-v2 requires exactly {EXPECTED_EVENT_COUNT} lifecycle markers, observed {}",
            events.len()
        )));
    }
    for (position, (event, expected_kind)) in events.iter().zip(EXPECTED_KINDS).enumerate() {
        if event.kind != expected_kind {
            return Err(invalid_marker(format!(
                "event {position} has kind {:?}, expected {expected_kind:?}",
                event.kind
            )));
        }
    }

    validate_strict_lifecycle_order(events)?;
    validate_initial_route(events)?;
    validate_initial_durations(events)?;
    Ok(())
}

fn validate_strict_lifecycle_order(events: &[PerformanceEvent]) -> Result<(), PerformanceError> {
    for (earlier, later, label) in [
        (0, 1, "process start before game ready"),
        (1, 2, "game ready before world load"),
        (2, 3, "world load start before world ready"),
        (3, 4, "world ready before warm-up"),
        (9, 10, "outbound dimension change before dimension ready"),
        (13, 14, "return dimension change before dimension ready"),
        (15, 16, "measurement end before process exit"),
    ] {
        if events[earlier].elapsed_ns >= events[later].elapsed_ns {
            return Err(invalid_marker(format!(
                "initial-v2 requires {label}, observed {} then {}",
                events[earlier].elapsed_ns, events[later].elapsed_ns
            )));
        }
    }
    Ok(())
}

fn validate_initial_route(events: &[PerformanceEvent]) -> Result<(), PerformanceError> {
    require_dimension(&events[3], "minecraft:overworld")?;

    let route_events = [6, 7, 8, 11, 12].map(|index| &events[index]);
    for (event, (expected_step, expected_dimension, _, _)) in
        route_events.into_iter().zip(EXPECTED_ROUTES)
    {
        if event.route_step.as_deref() != Some(expected_step)
            || event.dimension.as_deref() != Some(expected_dimension)
        {
            return Err(invalid_marker(format!(
                "route event {} must be `{expected_step}` in `{expected_dimension}`",
                event.event_index
            )));
        }
    }

    for (event_index, expected_dimension) in [
        (9, "minecraft:the_nether"),
        (10, "minecraft:the_nether"),
        (13, "minecraft:overworld"),
        (14, "minecraft:overworld"),
    ] {
        require_dimension(&events[event_index], expected_dimension)?;
    }
    Ok(())
}

fn validate_initial_durations(events: &[PerformanceEvent]) -> Result<(), PerformanceError> {
    require_duration(
        events[4].elapsed_ns,
        events[5].elapsed_ns,
        WARM_UP_NS,
        "warm-up",
    )?;

    let measurement_start = events[5].elapsed_ns;
    if events[6].elapsed_ns != measurement_start {
        return Err(invalid_marker(format!(
            "measurement start and first route step must be atomic, observed \
             {measurement_start} and {}",
            events[6].elapsed_ns
        )));
    }
    for (event_index, (_, _, offset_seconds, _)) in
        [6, 7, 8, 11, 12].into_iter().zip(EXPECTED_ROUTES)
    {
        let expected = measurement_start
            .checked_add(offset_seconds * SECOND_NS)
            .ok_or_else(|| invalid_marker("route timestamp overflows u64"))?;
        require_aligned(events[event_index].elapsed_ns, expected, "route step start")?;
    }
    require_duration(
        measurement_start,
        events[15].elapsed_ns,
        MEASUREMENT_NS,
        "measurement",
    )?;

    let cool_down = events[16]
        .elapsed_ns
        .checked_sub(events[15].elapsed_ns)
        .ok_or_else(|| invalid_marker("process exit precedes measurement end"))?;
    if cool_down < COOL_DOWN_NS {
        return Err(invalid_marker(format!(
            "cool-down spans {cool_down} ns, below required {COOL_DOWN_NS} ns"
        )));
    }
    Ok(())
}

fn require_duration(
    start_ns: u64,
    end_ns: u64,
    expected_ns: u64,
    label: &str,
) -> Result<(), PerformanceError> {
    let observed = end_ns
        .checked_sub(start_ns)
        .ok_or_else(|| invalid_marker(format!("{label} end precedes its start")))?;
    require_aligned(observed, expected_ns, label)
}

fn require_aligned(
    observed_ns: u64,
    expected_ns: u64,
    label: &str,
) -> Result<(), PerformanceError> {
    if observed_ns.abs_diff(expected_ns) > ALIGNMENT_TOLERANCE_NS {
        return Err(invalid_marker(format!(
            "{label} timestamp {observed_ns} differs from expected {expected_ns} \
             by more than {ALIGNMENT_TOLERANCE_NS} ns"
        )));
    }
    Ok(())
}

fn require_dimension(event: &PerformanceEvent, expected: &str) -> Result<(), PerformanceError> {
    if event.dimension.as_deref() != Some(expected) {
        return Err(invalid_marker(format!(
            "event {} must use dimension `{expected}`",
            event.event_index
        )));
    }
    Ok(())
}

fn validate_marker_metadata(
    kind: PerformanceEventKind,
    route_step: Option<&str>,
    dimension: Option<&str>,
    line_number: usize,
) -> Result<(), PerformanceError> {
    let valid = match kind {
        PerformanceEventKind::RouteStepStart => route_step.is_some() && dimension.is_some(),
        PerformanceEventKind::WorldReady
        | PerformanceEventKind::DimensionChangeStart
        | PerformanceEventKind::DimensionReady => route_step.is_none() && dimension.is_some(),
        _ => route_step.is_none() && dimension.is_none(),
    };
    if !valid {
        return Err(invalid_marker(format!(
            "line {line_number} has invalid metadata for event kind {kind:?}"
        )));
    }
    Ok(())
}

fn parse_optional_marker_field(
    value: &str,
    line_number: usize,
    field: &str,
) -> Result<Option<String>, PerformanceError> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.trim() != value {
        return Err(invalid_marker(format!(
            "line {line_number} {field} has leading or trailing whitespace"
        )));
    }
    Ok(Some(value.to_owned()))
}

fn parse_marker_u64(value: &str, line_number: usize, field: &str) -> Result<u64, PerformanceError> {
    value.parse::<u64>().map_err(|error| {
        invalid_marker(format!(
            "line {line_number} has invalid {field} `{value}`: {error}"
        ))
    })
}

fn marker_read_error(error: &io::Error) -> PerformanceError {
    PerformanceError::Io(format!("cannot read performance marker CSV: {error}"))
}
