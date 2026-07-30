use std::fs::File;
use std::io::{self, BufRead, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::{LinuxMemoryCsvRecorder, PerformanceError};

const MARKER_CSV_HEADER: &str = "event_index,elapsed_ns,kind,route_step,dimension";
const EXPECTED_EVENT_COUNT: usize = 17;
const ALIGNMENT_TOLERANCE_NS: u64 = 5_000_000;
const SECOND_NS: u64 = 1_000_000_000;
const WARM_UP_NS: u64 = 300 * SECOND_NS;
const MEASUREMENT_NS: u64 = 900 * SECOND_NS;
const COOL_DOWN_NS: u64 = 120 * SECOND_NS;
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(1);
const SOCKET_REQUEST_TIMEOUT: Duration = Duration::from_millis(1);
const TERMINAL_MARKER_PRIORITY_NS: u64 = 3_000_000;
const MAX_MARKER_REQUEST_BYTES: u64 = 1_024;
const SCHEDULED_MARKER_OFFSETS_SECONDS: [u64; 7] = [300, 300, 480, 720, 840, 1_020, 1_200];

const EXPECTED_KINDS: [PerformanceEventKind; EXPECTED_EVENT_COUNT] = [
    PerformanceEventKind::ProcessStart,
    PerformanceEventKind::GameReady,
    PerformanceEventKind::WorldLoadStart,
    PerformanceEventKind::WorldReady,
    PerformanceEventKind::WarmUpStart,
    PerformanceEventKind::MeasurementStart,
    PerformanceEventKind::RouteStepStart,
    PerformanceEventKind::RouteStepStart,
    PerformanceEventKind::RouteStepStart,
    PerformanceEventKind::DimensionChangeStart,
    PerformanceEventKind::DimensionReady,
    PerformanceEventKind::RouteStepStart,
    PerformanceEventKind::RouteStepStart,
    PerformanceEventKind::DimensionChangeStart,
    PerformanceEventKind::DimensionReady,
    PerformanceEventKind::MeasurementEnd,
    PerformanceEventKind::ProcessExit,
];

const EXPECTED_ROUTES: [(&str, &str, u64, PerformancePhase); 5] = [
    (
        "base-observation",
        "minecraft:overworld",
        0,
        PerformancePhase::SteadyState,
    ),
    (
        "overworld-traverse",
        "minecraft:overworld",
        180,
        PerformancePhase::SteadyState,
    ),
    (
        "outbound-dimension-change",
        "minecraft:the_nether",
        420,
        PerformancePhase::Transient,
    ),
    (
        "dimension-traverse",
        "minecraft:the_nether",
        540,
        PerformancePhase::SteadyState,
    ),
    (
        "return-and-base-observation",
        "minecraft:overworld",
        720,
        PerformancePhase::Transient,
    ),
];

/// One lifecycle event kind accepted by the initial performance profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerformanceEventKind {
    /// The measured game process started.
    ProcessStart,
    /// Minecraft reached its ready game surface.
    GameReady,
    /// The canonical world began loading.
    WorldLoadStart,
    /// The canonical world became ready.
    WorldReady,
    /// The profile warm-up interval began.
    WarmUpStart,
    /// The measured route interval began.
    MeasurementStart,
    /// One canonical route step began.
    RouteStepStart,
    /// A dimension transition began.
    DimensionChangeStart,
    /// The destination dimension became ready.
    DimensionReady,
    /// The measured route interval ended.
    MeasurementEnd,
    /// The measured game process exited cleanly.
    ProcessExit,
}

impl PerformanceEventKind {
    fn parse(value: &str, line_number: usize) -> Result<Self, PerformanceError> {
        match value {
            "process-start" => Ok(Self::ProcessStart),
            "game-ready" => Ok(Self::GameReady),
            "world-load-start" => Ok(Self::WorldLoadStart),
            "world-ready" => Ok(Self::WorldReady),
            "warm-up-start" => Ok(Self::WarmUpStart),
            "measurement-start" => Ok(Self::MeasurementStart),
            "route-step-start" => Ok(Self::RouteStepStart),
            "dimension-change-start" => Ok(Self::DimensionChangeStart),
            "dimension-ready" => Ok(Self::DimensionReady),
            "measurement-end" => Ok(Self::MeasurementEnd),
            "process-exit" => Ok(Self::ProcessExit),
            _ => Err(invalid_marker(format!(
                "line {line_number} has unsupported event kind `{value}`"
            ))),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::ProcessStart => "process-start",
            Self::GameReady => "game-ready",
            Self::WorldLoadStart => "world-load-start",
            Self::WorldReady => "world-ready",
            Self::WarmUpStart => "warm-up-start",
            Self::MeasurementStart => "measurement-start",
            Self::RouteStepStart => "route-step-start",
            Self::DimensionChangeStart => "dimension-change-start",
            Self::DimensionReady => "dimension-ready",
            Self::MeasurementEnd => "measurement-end",
            Self::ProcessExit => "process-exit",
        }
    }
}

/// A normalized performance-sample phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerformancePhase {
    /// Process launch before world loading begins.
    Startup,
    /// Canonical world loading and pre-warm-up content verification.
    WorldLoad,
    /// The fixed pre-measurement warm-up interval.
    WarmUp,
    /// An eligible steady-state measurement route step.
    SteadyState,
    /// A loading or dimension-change measurement route step.
    Transient,
    /// Post-measurement collection before clean process exit.
    CoolDown,
}

/// One ordered lifecycle marker on the collector's monotonic timeline.
#[derive(Debug, Eq, PartialEq)]
pub struct PerformanceEvent {
    /// Zero-based contiguous row index.
    pub event_index: u64,
    /// Nanoseconds elapsed from the marker recorder's monotonic origin.
    pub elapsed_ns: u64,
    /// Lifecycle event kind.
    pub kind: PerformanceEventKind,
    /// Canonical route step for a route-step event.
    pub route_step: Option<String>,
    /// Minecraft dimension for world, route, and transition events.
    pub dimension: Option<String>,
}

/// A marker stream validated against lifecycle-marker protocol version 2.
#[derive(Debug, Eq, PartialEq)]
pub struct InitialPerformanceMarkerStream {
    events: Vec<PerformanceEvent>,
}

/// One operator-supplied event accepted by the initial marker recorder.
///
/// Measurement and route-step boundaries are omitted because the recorder
/// emits them from the fixed profile schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialPerformanceMarkerCommand {
    /// The measured game process started.
    ProcessStart,
    /// Minecraft reached its ready game surface.
    GameReady,
    /// The canonical world began loading.
    WorldLoadStart,
    /// The canonical world became ready in the supplied dimension.
    WorldReady {
        /// Minecraft dimension identifier.
        dimension: String,
    },
    /// Content checks passed and the fixed warm-up began.
    WarmUpStart,
    /// A dimension transition began.
    DimensionChangeStart {
        /// Destination Minecraft dimension identifier.
        dimension: String,
    },
    /// The destination dimension became ready.
    DimensionReady {
        /// Ready Minecraft dimension identifier.
        dimension: String,
    },
    /// The measured game process exited after cool-down.
    ProcessExit,
}

/// One operator event accepted by the combined Linux marker and memory
/// recorder.
///
/// Process start carries the stable PID that the recorder samples. A launcher
/// wrapper can send it immediately before replacing itself with the JVM, so
/// `/proc/<pid>/stat` identity remains stable across `exec`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialLinuxPerformanceMarkerCommand {
    /// The measured process is about to become the Minecraft JVM.
    ProcessStart {
        /// Stable process identifier sampled for RSS.
        pid: u32,
    },
    /// Minecraft reached its ready game surface.
    GameReady,
    /// The canonical world began loading.
    WorldLoadStart,
    /// The canonical world became ready in the supplied dimension.
    WorldReady {
        /// Minecraft dimension identifier.
        dimension: String,
    },
    /// Content checks passed and the fixed warm-up began.
    WarmUpStart,
    /// A dimension transition began.
    DimensionChangeStart {
        /// Destination Minecraft dimension identifier.
        dimension: String,
    },
    /// The destination dimension became ready.
    DimensionReady {
        /// Ready Minecraft dimension identifier.
        dimension: String,
    },
    /// The measured game process exited after cool-down.
    ProcessExit,
    /// The supervised process failed and the partial capture must be removed.
    Abort,
}

/// Verified output totals from one combined Linux capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitialLinuxPerformanceCounts {
    /// Validated lifecycle marker count.
    pub lifecycle_markers: usize,
    /// Validated one-second Linux memory sample count.
    pub linux_memory_samples: usize,
}

impl InitialPerformanceMarkerCommand {
    fn event_kind(&self) -> PerformanceEventKind {
        match self {
            Self::ProcessStart => PerformanceEventKind::ProcessStart,
            Self::GameReady => PerformanceEventKind::GameReady,
            Self::WorldLoadStart => PerformanceEventKind::WorldLoadStart,
            Self::WorldReady { .. } => PerformanceEventKind::WorldReady,
            Self::WarmUpStart => PerformanceEventKind::WarmUpStart,
            Self::DimensionChangeStart { .. } => PerformanceEventKind::DimensionChangeStart,
            Self::DimensionReady { .. } => PerformanceEventKind::DimensionReady,
            Self::ProcessExit => PerformanceEventKind::ProcessExit,
        }
    }

    fn dimension(&self) -> Option<&str> {
        match self {
            Self::WorldReady { dimension }
            | Self::DimensionChangeStart { dimension }
            | Self::DimensionReady { dimension } => Some(dimension),
            _ => None,
        }
    }

    fn into_event(self) -> (PerformanceEventKind, Option<String>) {
        match self {
            Self::ProcessStart => (PerformanceEventKind::ProcessStart, None),
            Self::GameReady => (PerformanceEventKind::GameReady, None),
            Self::WorldLoadStart => (PerformanceEventKind::WorldLoadStart, None),
            Self::WorldReady { dimension } => (PerformanceEventKind::WorldReady, Some(dimension)),
            Self::WarmUpStart => (PerformanceEventKind::WarmUpStart, None),
            Self::DimensionChangeStart { dimension } => {
                (PerformanceEventKind::DimensionChangeStart, Some(dimension))
            }
            Self::DimensionReady { dimension } => {
                (PerformanceEventKind::DimensionReady, Some(dimension))
            }
            Self::ProcessExit => (PerformanceEventKind::ProcessExit, None),
        }
    }

    fn to_wire(&self) -> Result<String, PerformanceError> {
        let dimension = self.dimension().unwrap_or_default();
        if dimension.contains([',', '\n', '\r']) {
            return Err(invalid_marker(
                "marker dimensions cannot contain commas or line breaks",
            ));
        }
        Ok(format!("{},{}\n", self.event_kind().as_str(), dimension))
    }
}

impl InitialLinuxPerformanceMarkerCommand {
    fn into_marker(self) -> Option<(InitialPerformanceMarkerCommand, Option<u32>)> {
        Some(match self {
            Self::ProcessStart { pid } => {
                (InitialPerformanceMarkerCommand::ProcessStart, Some(pid))
            }
            Self::GameReady => (InitialPerformanceMarkerCommand::GameReady, None),
            Self::WorldLoadStart => (InitialPerformanceMarkerCommand::WorldLoadStart, None),
            Self::WorldReady { dimension } => (
                InitialPerformanceMarkerCommand::WorldReady { dimension },
                None,
            ),
            Self::WarmUpStart => (InitialPerformanceMarkerCommand::WarmUpStart, None),
            Self::DimensionChangeStart { dimension } => (
                InitialPerformanceMarkerCommand::DimensionChangeStart { dimension },
                None,
            ),
            Self::DimensionReady { dimension } => (
                InitialPerformanceMarkerCommand::DimensionReady { dimension },
                None,
            ),
            Self::ProcessExit => (InitialPerformanceMarkerCommand::ProcessExit, None),
            Self::Abort => return None,
        })
    }

    fn to_wire(&self) -> Result<String, PerformanceError> {
        match self {
            Self::ProcessStart { pid } => {
                if *pid == 0 {
                    return Err(invalid_marker(
                        "combined Linux process start requires a PID greater than zero",
                    ));
                }
                Ok(format!("process-start,{pid}\n"))
            }
            Self::GameReady => InitialPerformanceMarkerCommand::GameReady.to_wire(),
            Self::WorldLoadStart => InitialPerformanceMarkerCommand::WorldLoadStart.to_wire(),
            Self::WorldReady { dimension } => InitialPerformanceMarkerCommand::WorldReady {
                dimension: dimension.clone(),
            }
            .to_wire(),
            Self::WarmUpStart => InitialPerformanceMarkerCommand::WarmUpStart.to_wire(),
            Self::DimensionChangeStart { dimension } => {
                InitialPerformanceMarkerCommand::DimensionChangeStart {
                    dimension: dimension.clone(),
                }
                .to_wire()
            }
            Self::DimensionReady { dimension } => InitialPerformanceMarkerCommand::DimensionReady {
                dimension: dimension.clone(),
            }
            .to_wire(),
            Self::ProcessExit => InitialPerformanceMarkerCommand::ProcessExit.to_wire(),
            Self::Abort => Ok("abort,\n".to_owned()),
        }
    }
}

/// Pure initial-v2 marker schedule driven by recorder-owned timestamps.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct InitialPerformanceMarkerRecorder {
    events: Vec<PerformanceEvent>,
    warm_up_start_ns: Option<u64>,
    scheduled_markers_emitted: usize,
    last_observed_ns: Option<u64>,
}

impl InitialPerformanceMarkerRecorder {
    /// Creates an empty initial-v2 marker recorder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: Vec::new(),
            warm_up_start_ns: None,
            scheduled_markers_emitted: 0,
            last_observed_ns: None,
        }
    }

    /// Returns all events recorded or scheduled so far.
    #[must_use]
    pub fn events(&self) -> &[PerformanceEvent] {
        &self.events
    }

    /// Creates and verifies a CSV from a complete recorder stream.
    ///
    /// # Errors
    ///
    /// Returns an error when the stream is incomplete or invalid, the
    /// destination exists, or writing and reparsing the CSV fails.
    pub fn write_csv(&self, destination: &Path) -> Result<usize, PerformanceError> {
        write_initial_marker_csv(&self.events, destination)
    }

    /// Records one operator event at the recorder's monotonic timestamp.
    ///
    /// Scheduled boundaries strictly before the supplied timestamp are emitted
    /// first. Boundaries at the same timestamp are emitted after the operator
    /// event, allowing a dimension-ready marker to precede a route boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for clock regression, event order or metadata drift,
    /// an event after completion, or a profile arithmetic overflow.
    pub fn record(
        &mut self,
        command: InitialPerformanceMarkerCommand,
        elapsed_ns: u64,
    ) -> Result<bool, PerformanceError> {
        self.observe_time(elapsed_ns)?;
        self.emit_scheduled(elapsed_ns, false)?;
        let (kind, dimension) = command.into_event();
        self.append_event(kind, elapsed_ns, None, dimension)?;
        if kind == PerformanceEventKind::WarmUpStart {
            self.warm_up_start_ns = Some(elapsed_ns);
        }
        self.emit_scheduled(elapsed_ns, true)?;
        if self.events.len() == EXPECTED_EVENT_COUNT {
            validate_initial_events(&self.events)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Advances the recorder clock and emits every due fixed boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for clock regression, a missed required operator event,
    /// or profile arithmetic overflow.
    pub fn advance_to(&mut self, elapsed_ns: u64) -> Result<(), PerformanceError> {
        self.observe_time(elapsed_ns)?;
        self.emit_scheduled(elapsed_ns, true)
    }

    fn observe_time(&mut self, elapsed_ns: u64) -> Result<(), PerformanceError> {
        if self
            .last_observed_ns
            .is_some_and(|previous| elapsed_ns < previous)
        {
            return Err(invalid_marker(format!(
                "marker recorder clock regressed to {elapsed_ns} ns"
            )));
        }
        self.last_observed_ns = Some(elapsed_ns);
        Ok(())
    }

    fn emit_scheduled(
        &mut self,
        elapsed_ns: u64,
        include_equal: bool,
    ) -> Result<(), PerformanceError> {
        while let Some(target_ns) = self.next_scheduled_ns()? {
            let due = if include_equal {
                target_ns <= elapsed_ns
            } else {
                target_ns < elapsed_ns
            };
            if !due {
                break;
            }
            let scheduled_index = self.scheduled_markers_emitted;
            let (kind, route_step, dimension) = scheduled_marker(scheduled_index)?;
            self.append_event(
                kind,
                target_ns,
                route_step.map(str::to_owned),
                dimension.map(str::to_owned),
            )?;
            self.scheduled_markers_emitted += 1;
        }
        Ok(())
    }

    fn next_scheduled_ns(&self) -> Result<Option<u64>, PerformanceError> {
        let Some(warm_up_start_ns) = self.warm_up_start_ns else {
            return Ok(None);
        };
        let Some(offset_seconds) =
            SCHEDULED_MARKER_OFFSETS_SECONDS.get(self.scheduled_markers_emitted)
        else {
            return Ok(None);
        };
        let offset_ns = offset_seconds
            .checked_mul(SECOND_NS)
            .ok_or_else(|| invalid_marker("scheduled marker offset overflows u64"))?;
        warm_up_start_ns
            .checked_add(offset_ns)
            .map(Some)
            .ok_or_else(|| invalid_marker("scheduled marker timestamp overflows u64"))
    }

    fn append_event(
        &mut self,
        kind: PerformanceEventKind,
        elapsed_ns: u64,
        route_step: Option<String>,
        dimension: Option<String>,
    ) -> Result<(), PerformanceError> {
        let position = self.events.len();
        let Some(expected_kind) = EXPECTED_KINDS.get(position) else {
            return Err(invalid_marker("marker stream is already complete"));
        };
        if kind != *expected_kind {
            return Err(invalid_marker(format!(
                "event {position} has kind {kind:?}, expected {expected_kind:?}"
            )));
        }
        if self
            .events
            .last()
            .is_some_and(|previous| elapsed_ns < previous.elapsed_ns)
        {
            return Err(invalid_marker(format!(
                "event {position} timestamp {elapsed_ns} regresses"
            )));
        }
        let event_index = u64::try_from(position)
            .map_err(|_| invalid_marker("performance marker count exceeds u64"))?;
        let event = PerformanceEvent {
            event_index,
            elapsed_ns,
            kind,
            route_step,
            dimension,
        };
        validate_partial_event(&event, position)?;
        self.events.push(event);
        Ok(())
    }
}

impl InitialPerformanceMarkerStream {
    /// Returns the 17 validated lifecycle events in capture order.
    #[must_use]
    pub fn events(&self) -> &[PerformanceEvent] {
        &self.events
    }

    /// Assigns the initial profile phase at one monotonic timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error when the timestamp falls before process start or at or
    /// after process exit.
    pub fn phase_at(&self, elapsed_ns: u64) -> Result<PerformancePhase, PerformanceError> {
        let process_start = self.events[0].elapsed_ns;
        let world_load_start = self.events[2].elapsed_ns;
        let warm_up_start = self.events[4].elapsed_ns;
        let measurement_start = self.events[5].elapsed_ns;
        let measurement_end = self.events[15].elapsed_ns;
        let process_exit = self.events[16].elapsed_ns;

        if elapsed_ns < process_start || elapsed_ns >= process_exit {
            return Err(invalid_marker(format!(
                "sample timestamp {elapsed_ns} is outside process lifetime \
                 [{process_start}, {process_exit})"
            )));
        }
        if elapsed_ns >= measurement_end {
            return Ok(PerformancePhase::CoolDown);
        }
        if elapsed_ns >= measurement_start {
            return self.measurement_phase_at(elapsed_ns);
        }
        if elapsed_ns >= warm_up_start {
            return Ok(PerformancePhase::WarmUp);
        }
        if elapsed_ns >= world_load_start {
            return Ok(PerformancePhase::WorldLoad);
        }
        Ok(PerformancePhase::Startup)
    }

    fn measurement_phase_at(&self, elapsed_ns: u64) -> Result<PerformancePhase, PerformanceError> {
        let route = self
            .events
            .iter()
            .filter(|event| event.kind == PerformanceEventKind::RouteStepStart)
            .take_while(|event| event.elapsed_ns <= elapsed_ns)
            .last()
            .ok_or_else(|| invalid_marker("sample precedes the first active route step"))?;
        let route_step = route
            .route_step
            .as_deref()
            .ok_or_else(|| invalid_marker("validated route event has no route step"))?;
        EXPECTED_ROUTES
            .iter()
            .find_map(|(expected, _, _, phase)| (route_step == *expected).then_some(*phase))
            .ok_or_else(|| {
                invalid_marker(format!(
                    "validated marker stream has unknown route step `{route_step}`"
                ))
            })
    }
}

/// Parses one operator marker command from CLI or socket fields.
///
/// # Errors
///
/// Returns an error for an automated event kind, missing or extra dimension,
/// or an unsupported event name.
pub fn parse_initial_performance_marker_command(
    kind: &str,
    dimension: Option<&str>,
) -> Result<InitialPerformanceMarkerCommand, PerformanceError> {
    let parsed = match (kind, dimension) {
        ("process-start", None) => InitialPerformanceMarkerCommand::ProcessStart,
        ("game-ready", None) => InitialPerformanceMarkerCommand::GameReady,
        ("world-load-start", None) => InitialPerformanceMarkerCommand::WorldLoadStart,
        ("world-ready", Some(dimension)) => InitialPerformanceMarkerCommand::WorldReady {
            dimension: dimension.to_owned(),
        },
        ("warm-up-start", None) => InitialPerformanceMarkerCommand::WarmUpStart,
        ("dimension-change-start", Some(dimension)) => {
            InitialPerformanceMarkerCommand::DimensionChangeStart {
                dimension: dimension.to_owned(),
            }
        }
        ("dimension-ready", Some(dimension)) => InitialPerformanceMarkerCommand::DimensionReady {
            dimension: dimension.to_owned(),
        },
        ("process-exit", None) => InitialPerformanceMarkerCommand::ProcessExit,
        ("measurement-start" | "route-step-start" | "measurement-end", _) => {
            return Err(invalid_marker(format!(
                "`{kind}` is recorder-scheduled and cannot be supplied by an operator"
            )));
        }
        ("world-ready" | "dimension-change-start" | "dimension-ready", None) => {
            return Err(invalid_marker(format!("`{kind}` requires one dimension")));
        }
        (
            "process-start" | "game-ready" | "world-load-start" | "warm-up-start" | "process-exit",
            Some(_),
        ) => {
            return Err(invalid_marker(format!(
                "`{kind}` does not accept a dimension"
            )));
        }
        _ => {
            return Err(invalid_marker(format!(
                "unsupported initial marker command `{kind}`"
            )));
        }
    };
    Ok(parsed)
}

/// Parses one operator command for the combined Linux marker and memory
/// recorder.
///
/// # Errors
///
/// Returns an error when process start lacks a positive PID, a dimensional
/// marker lacks its dimension, a scheduled marker is supplied, or the kind is
/// unsupported.
pub fn parse_initial_linux_performance_marker_command(
    kind: &str,
    value: Option<&str>,
) -> Result<InitialLinuxPerformanceMarkerCommand, PerformanceError> {
    match (kind, value) {
        ("process-start", Some(pid)) => {
            let pid = pid.parse::<u32>().map_err(|error| {
                invalid_marker(format!(
                    "invalid combined Linux process ID `{pid}`: {error}"
                ))
            })?;
            if pid == 0 {
                return Err(invalid_marker(
                    "combined Linux process start requires a PID greater than zero",
                ));
            }
            Ok(InitialLinuxPerformanceMarkerCommand::ProcessStart { pid })
        }
        ("process-start", None) => Err(invalid_marker(
            "combined Linux process start requires one PID",
        )),
        ("game-ready", None) => Ok(InitialLinuxPerformanceMarkerCommand::GameReady),
        ("world-load-start", None) => Ok(InitialLinuxPerformanceMarkerCommand::WorldLoadStart),
        ("world-ready", Some(dimension)) if !dimension.is_empty() => {
            Ok(InitialLinuxPerformanceMarkerCommand::WorldReady {
                dimension: dimension.to_owned(),
            })
        }
        ("warm-up-start", None) => Ok(InitialLinuxPerformanceMarkerCommand::WarmUpStart),
        ("dimension-change-start", Some(dimension)) if !dimension.is_empty() => {
            Ok(InitialLinuxPerformanceMarkerCommand::DimensionChangeStart {
                dimension: dimension.to_owned(),
            })
        }
        ("dimension-ready", Some(dimension)) if !dimension.is_empty() => {
            Ok(InitialLinuxPerformanceMarkerCommand::DimensionReady {
                dimension: dimension.to_owned(),
            })
        }
        ("process-exit", None) => Ok(InitialLinuxPerformanceMarkerCommand::ProcessExit),
        ("abort", None) => Ok(InitialLinuxPerformanceMarkerCommand::Abort),
        ("measurement-start" | "route-step-start" | "measurement-end", _) => Err(invalid_marker(
            format!("`{kind}` is recorder-scheduled and cannot be supplied by an operator"),
        )),
        ("world-ready" | "dimension-change-start" | "dimension-ready", None | Some("")) => {
            Err(invalid_marker(format!("`{kind}` requires one dimension")))
        }
        (
            "game-ready" | "world-load-start" | "warm-up-start" | "process-exit" | "abort",
            Some(_),
        ) => Err(invalid_marker(format!("`{kind}` does not accept a value"))),
        _ => Err(invalid_marker(format!(
            "unsupported initial Linux marker command `{kind}`"
        ))),
    }
}

/// Runs a focus-free Unix-socket recorder until a complete initial-v2 stream.
///
/// The server owns the monotonic origin, schedules measurement and route
/// boundaries, creates the destination only after a complete valid stream, and
/// removes its socket on every normal Rust return path.
///
/// # Errors
///
/// Returns an error for an existing destination or socket, socket I/O, marker
/// protocol drift, a scheduling delay above 5 milliseconds, or an invalid
/// completed stream.
pub fn record_initial_performance_markers(
    socket_path: &Path,
    destination: &Path,
) -> Result<usize, PerformanceError> {
    if destination.exists() {
        return Err(PerformanceError::Io(format!(
            "performance marker CSV already exists: {}",
            destination.display()
        )));
    }
    let (listener, socket_guard) = bind_marker_socket(socket_path)?;

    let origin = Instant::now();
    let mut recorder = InitialPerformanceMarkerRecorder::new();
    loop {
        let elapsed_ns = instant_elapsed_ns(origin)?;
        validate_recorder_wake_deadline(&recorder, elapsed_ns)?;
        recorder.advance_to(elapsed_ns)?;
        if should_defer_socket_accept(&recorder, elapsed_ns)? {
            thread::sleep(SOCKET_POLL_INTERVAL);
            continue;
        }

        match listener.accept() {
            Ok((mut stream, _)) => {
                let command = match read_marker_command(&mut stream) {
                    Ok(command) => command,
                    Err(error) => {
                        let _ = writeln!(stream, "error:{error}");
                        continue;
                    }
                };
                let elapsed_ns = instant_elapsed_ns(origin)?;
                validate_recorder_wake_deadline(&recorder, elapsed_ns)?;
                match recorder.record(command, elapsed_ns) {
                    Ok(false) => {
                        stream
                            .write_all(b"ok\n")
                            .map_err(|error| marker_socket_write_error(&error))?;
                    }
                    Ok(true) => {
                        let verified = match recorder.write_csv(destination) {
                            Ok(verified) => verified,
                            Err(error) => {
                                let _ = writeln!(stream, "error:{error}");
                                return Err(error);
                            }
                        };
                        acknowledge_completion(&mut stream);
                        drop(socket_guard);
                        return Ok(verified);
                    }
                    Err(error) => {
                        let _ = writeln!(stream, "error:{error}");
                        return Err(error);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(SOCKET_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(PerformanceError::Io(format!(
                    "cannot accept performance marker command: {error}"
                )));
            }
        }
    }
}

/// Runs one focus-free Linux recorder for lifecycle markers and procfs memory
/// samples on the same monotonic clock.
///
/// The recorder starts before Prism. Its process-start command supplies the PID
/// that will become the Minecraft JVM, then one-second memory deadlines and all
/// lifecycle events share this server's `Instant` origin.
///
/// # Errors
///
/// Returns an error for aliased or existing outputs, an unprotected socket
/// directory, socket protocol or lifecycle drift, late sampling or marker
/// deadlines, procfs identity drift, counter regression, or output I/O.
pub fn record_initial_linux_performance(
    socket_path: &Path,
    marker_destination: &Path,
    memory_destination: &Path,
    proc_root: &Path,
    page_size_bytes: u64,
    memory_interval: Duration,
) -> Result<InitialLinuxPerformanceCounts, PerformanceError> {
    validate_combined_linux_paths(socket_path, marker_destination, memory_destination)?;
    let (listener, socket_guard) = bind_marker_socket(socket_path)?;

    let origin = Instant::now();
    let mut markers = InitialPerformanceMarkerRecorder::new();
    let mut memory = None;
    loop {
        let elapsed_ns = instant_elapsed_ns(origin)?;
        advance_combined_linux_markers(&mut markers, elapsed_ns)?;
        if should_defer_socket_accept(&markers, elapsed_ns)? {
            thread::sleep(SOCKET_POLL_INTERVAL);
            continue;
        }

        match listener.accept() {
            Ok((mut stream, _)) => {
                let command = match read_linux_marker_command(&mut stream) {
                    Ok(command) => command,
                    Err(error) => {
                        let _ = writeln!(stream, "error:{error}");
                        continue;
                    }
                };
                let elapsed_ns = instant_elapsed_ns(origin)?;
                let Some((command, process_pid)) = command.into_marker() else {
                    let error = invalid_marker(
                        "combined Linux capture aborted because the supervised process failed",
                    );
                    let _ = writeln!(stream, "error:{error}");
                    return Err(error);
                };
                advance_combined_linux_markers(&mut markers, elapsed_ns)?;
                while sample_combined_linux_memory_for_command(
                    &command,
                    memory.as_mut(),
                    elapsed_ns,
                )? {}
                if let Err(error) = attach_combined_linux_memory(
                    &mut memory,
                    process_pid,
                    proc_root,
                    page_size_bytes,
                    memory_interval,
                    elapsed_ns,
                    memory_destination,
                ) {
                    let _ = writeln!(stream, "error:{error}");
                    return Err(error);
                }

                match markers.record(command, elapsed_ns) {
                    Ok(false) => {
                        stream
                            .write_all(b"ok\n")
                            .map_err(|error| marker_socket_write_error(&error))?;
                    }
                    Ok(true) => {
                        let counts = match finish_combined_linux_capture(
                            &markers,
                            &mut memory,
                            marker_destination,
                        ) {
                            Ok(counts) => counts,
                            Err(error) => {
                                let _ = writeln!(stream, "error:{error}");
                                return Err(error);
                            }
                        };
                        acknowledge_completion(&mut stream);
                        drop(socket_guard);
                        return Ok(counts);
                    }
                    Err(error) => {
                        let _ = writeln!(stream, "error:{error}");
                        return Err(error);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if should_reserve_terminal_marker_priority(memory.as_ref(), elapsed_ns) {
                    thread::sleep(SOCKET_POLL_INTERVAL);
                    continue;
                }
                while sample_combined_linux_memory(memory.as_mut(), elapsed_ns)? {}
                thread::sleep(SOCKET_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(PerformanceError::Io(format!(
                    "cannot accept performance marker command: {error}"
                )));
            }
        }
    }
}

fn attach_combined_linux_memory(
    memory: &mut Option<LinuxMemoryCsvRecorder>,
    process_pid: Option<u32>,
    proc_root: &Path,
    page_size_bytes: u64,
    memory_interval: Duration,
    elapsed_ns: u64,
    memory_destination: &Path,
) -> Result<(), PerformanceError> {
    let Some(pid) = process_pid else {
        return Ok(());
    };
    if memory.is_some() {
        return Err(invalid_marker(
            "combined Linux memory sampling is already attached to a process",
        ));
    }
    *memory = Some(LinuxMemoryCsvRecorder::new(
        proc_root,
        pid,
        page_size_bytes,
        memory_interval,
        elapsed_ns,
        memory_destination,
    )?);
    Ok(())
}

/// Sends one event to a running focus-free marker recorder.
///
/// Returns `true` when this command completed the 17-event stream.
///
/// # Errors
///
/// Returns an error when the socket cannot be reached, the request cannot be
/// written, or the recorder rejects the command.
pub fn send_initial_performance_marker(
    socket_path: &Path,
    command: &InitialPerformanceMarkerCommand,
) -> Result<bool, PerformanceError> {
    let request = command.to_wire()?;
    send_marker_wire(socket_path, &request)
}

/// Sends one event to a running combined Linux marker and memory recorder.
///
/// Returns `true` when this command completed and verified both CSV streams.
///
/// # Errors
///
/// Returns an error when the command is invalid, the socket cannot be reached,
/// the request cannot be written, or the recorder rejects the event.
pub fn send_initial_linux_performance_marker(
    socket_path: &Path,
    command: &InitialLinuxPerformanceMarkerCommand,
) -> Result<bool, PerformanceError> {
    let request = command.to_wire()?;
    send_marker_wire(socket_path, &request)
}

fn send_marker_wire(socket_path: &Path, request: &str) -> Result<bool, PerformanceError> {
    let mut stream = UnixStream::connect(socket_path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot connect to performance marker socket {}: {error}",
            socket_path.display()
        ))
    })?;
    stream
        .write_all(request.as_bytes())
        .map_err(|error| marker_socket_write_error(&error))?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| marker_socket_write_error(&error))?;
    let mut response = String::new();
    stream
        .take(MAX_MARKER_REQUEST_BYTES + 1)
        .read_to_string(&mut response)
        .map_err(|error| {
            PerformanceError::Io(format!("cannot read performance marker response: {error}"))
        })?;
    match response.trim_end() {
        "ok" => Ok(false),
        "complete" => Ok(true),
        error if error.starts_with("error:") => Err(invalid_marker(
            error
                .strip_prefix("error:")
                .unwrap_or("marker recorder rejected the command"),
        )),
        other => Err(invalid_marker(format!(
            "performance marker recorder returned invalid response `{other}`"
        ))),
    }
}

fn acknowledge_completion(stream: &mut UnixStream) {
    let _ = stream.write_all(b"complete\n");
}

/// Parses and validates a lifecycle-marker protocol version 2 CSV.
///
/// Rows remain in capture order. Event indices must be contiguous and elapsed
/// timestamps may be equal but never decrease, allowing one logical boundary
/// to emit measurement and route markers atomically.
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

fn scheduled_marker(
    index: usize,
) -> Result<
    (
        PerformanceEventKind,
        Option<&'static str>,
        Option<&'static str>,
    ),
    PerformanceError,
> {
    match index {
        0 => Ok((PerformanceEventKind::MeasurementStart, None, None)),
        1..=5 => {
            let route_index = index - 1;
            let (route_step, dimension, _, _) = EXPECTED_ROUTES[route_index];
            Ok((
                PerformanceEventKind::RouteStepStart,
                Some(route_step),
                Some(dimension),
            ))
        }
        6 => Ok((PerformanceEventKind::MeasurementEnd, None, None)),
        _ => Err(invalid_marker(format!(
            "unknown scheduled marker index {index}"
        ))),
    }
}

fn validate_partial_event(
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

fn validate_recorder_wake_deadline(
    recorder: &InitialPerformanceMarkerRecorder,
    elapsed_ns: u64,
) -> Result<(), PerformanceError> {
    let Some(deadline_ns) = recorder.next_scheduled_ns()? else {
        return Ok(());
    };
    let latest_ns = deadline_ns
        .checked_add(ALIGNMENT_TOLERANCE_NS)
        .ok_or_else(|| invalid_marker("scheduled marker tolerance overflows u64"))?;
    if elapsed_ns > latest_ns {
        return Err(invalid_marker(format!(
            "marker recorder woke at {elapsed_ns} ns, after scheduled deadline \
             {deadline_ns} ns plus {ALIGNMENT_TOLERANCE_NS} ns tolerance"
        )));
    }
    Ok(())
}

fn advance_combined_linux_markers(
    markers: &mut InitialPerformanceMarkerRecorder,
    elapsed_ns: u64,
) -> Result<(), PerformanceError> {
    validate_recorder_wake_deadline(markers, elapsed_ns)?;
    markers.advance_to(elapsed_ns)
}

fn sample_combined_linux_memory(
    memory: Option<&mut LinuxMemoryCsvRecorder>,
    elapsed_ns: u64,
) -> Result<bool, PerformanceError> {
    memory.map_or(Ok(false), |memory| {
        memory.sample_due_at(elapsed_ns, ALIGNMENT_TOLERANCE_NS)
    })
}

fn sample_combined_linux_memory_for_command(
    command: &InitialPerformanceMarkerCommand,
    memory: Option<&mut LinuxMemoryCsvRecorder>,
    elapsed_ns: u64,
) -> Result<bool, PerformanceError> {
    if *command == InitialPerformanceMarkerCommand::ProcessExit {
        return Ok(false);
    }
    sample_combined_linux_memory(memory, elapsed_ns)
}

fn finish_combined_linux_capture(
    markers: &InitialPerformanceMarkerRecorder,
    memory: &mut Option<LinuxMemoryCsvRecorder>,
    marker_destination: &Path,
) -> Result<InitialLinuxPerformanceCounts, PerformanceError> {
    let mut memory_recorder = memory.take().ok_or_else(|| {
        invalid_marker("combined Linux capture completed without a memory recorder")
    })?;
    let linux_memory_samples = memory_recorder.verify_finished()?;
    let lifecycle_markers = markers.write_csv(marker_destination)?;
    memory_recorder.retain_finished();
    Ok(InitialLinuxPerformanceCounts {
        lifecycle_markers,
        linux_memory_samples,
    })
}

fn should_defer_socket_accept(
    recorder: &InitialPerformanceMarkerRecorder,
    elapsed_ns: u64,
) -> Result<bool, PerformanceError> {
    let Some(deadline_ns) = recorder.next_scheduled_ns()? else {
        return Ok(false);
    };
    Ok(
        elapsed_ns < deadline_ns
            && deadline_ns.saturating_sub(elapsed_ns) <= ALIGNMENT_TOLERANCE_NS,
    )
}

fn should_reserve_terminal_marker_priority(
    memory: Option<&LinuxMemoryCsvRecorder>,
    elapsed_ns: u64,
) -> bool {
    memory.is_some_and(|memory| {
        let deadline_ns = memory.next_deadline_ns();
        elapsed_ns >= deadline_ns
            && elapsed_ns.saturating_sub(deadline_ns) < TERMINAL_MARKER_PRIORITY_NS
    })
}

fn read_marker_command(
    stream: &mut UnixStream,
) -> Result<InitialPerformanceMarkerCommand, PerformanceError> {
    let (kind, value) = read_marker_fields(stream)?;
    parse_initial_performance_marker_command(&kind, value.as_deref())
}

fn read_linux_marker_command(
    stream: &mut UnixStream,
) -> Result<InitialLinuxPerformanceMarkerCommand, PerformanceError> {
    let (kind, value) = read_marker_fields(stream)?;
    parse_initial_linux_performance_marker_command(&kind, value.as_deref())
}

fn read_marker_fields(
    stream: &mut UnixStream,
) -> Result<(String, Option<String>), PerformanceError> {
    stream
        .set_read_timeout(Some(SOCKET_REQUEST_TIMEOUT))
        .map_err(|error| {
            PerformanceError::Io(format!(
                "cannot configure performance marker request timeout: {error}"
            ))
        })?;
    let mut bytes = Vec::new();
    let request_started = Instant::now();
    for _ in 0..=MAX_MARKER_REQUEST_BYTES {
        if request_started.elapsed() >= SOCKET_REQUEST_TIMEOUT {
            return Err(PerformanceError::Io(format!(
                "performance marker request exceeded {} ms",
                SOCKET_REQUEST_TIMEOUT.as_millis()
            )));
        }
        let mut byte = [0_u8; 1];
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                bytes.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            Err(error) => {
                return Err(PerformanceError::Io(format!(
                    "cannot read performance marker request: {error}"
                )));
            }
        }
    }
    let request = String::from_utf8(bytes).map_err(|error| {
        invalid_marker(format!(
            "performance marker request is not valid UTF-8: {error}"
        ))
    })?;
    if u64::try_from(request.len()).unwrap_or(u64::MAX) > MAX_MARKER_REQUEST_BYTES {
        return Err(invalid_marker(format!(
            "performance marker request exceeds {MAX_MARKER_REQUEST_BYTES} bytes"
        )));
    }
    let line = request
        .strip_suffix('\n')
        .ok_or_else(|| invalid_marker("performance marker request must end with one newline"))?;
    if line.contains(['\n', '\r']) {
        return Err(invalid_marker(
            "performance marker request contains an embedded line break",
        ));
    }
    let (kind, dimension) = line
        .split_once(',')
        .ok_or_else(|| invalid_marker("performance marker request must contain two columns"))?;
    if dimension.contains(',') {
        return Err(invalid_marker(
            "performance marker request must contain exactly two columns",
        ));
    }
    Ok((
        kind.to_owned(),
        (!dimension.is_empty()).then(|| dimension.to_owned()),
    ))
}

fn write_initial_marker_csv(
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

fn instant_elapsed_ns(origin: Instant) -> Result<u64, PerformanceError> {
    u64::try_from(origin.elapsed().as_nanos())
        .map_err(|_| invalid_marker("marker recorder monotonic timestamp exceeds u64"))
}

fn marker_socket_write_error(error: &io::Error) -> PerformanceError {
    PerformanceError::Io(format!(
        "cannot write performance marker socket response: {error}"
    ))
}

struct MarkerSocketGuard<'a> {
    path: &'a Path,
    identity: MarkerSocketIdentity,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MarkerSocketIdentity {
    device: u64,
    inode: u64,
}

impl Drop for MarkerSocketGuard<'_> {
    fn drop(&mut self) {
        if socket_identity(self.path).is_ok_and(|current| current == self.identity) {
            let _ = std::fs::remove_file(self.path);
        }
    }
}

fn validate_combined_linux_paths(
    socket_path: &Path,
    marker_destination: &Path,
    memory_destination: &Path,
) -> Result<(), PerformanceError> {
    if socket_path == marker_destination
        || socket_path == memory_destination
        || marker_destination == memory_destination
    {
        return Err(PerformanceError::Io(
            "combined Linux socket, marker CSV, and memory CSV paths must be distinct".to_owned(),
        ));
    }
    for (label, destination) in [
        ("performance marker CSV", marker_destination),
        ("Linux memory CSV", memory_destination),
    ] {
        if destination.exists() {
            return Err(PerformanceError::Io(format!(
                "{label} already exists: {}",
                destination.display()
            )));
        }
    }
    Ok(())
}

fn bind_marker_socket(
    socket_path: &Path,
) -> Result<(UnixListener, MarkerSocketGuard<'_>), PerformanceError> {
    require_owner_only_socket_parent(socket_path)?;
    if socket_path.exists() {
        return Err(PerformanceError::Io(format!(
            "performance marker socket already exists: {}",
            socket_path.display()
        )));
    }
    let listener = UnixListener::bind(socket_path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot bind performance marker socket {}: {error}",
            socket_path.display()
        ))
    })?;
    let socket_guard = MarkerSocketGuard {
        path: socket_path,
        identity: socket_identity(socket_path)?,
    };
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600)).map_err(
        |error| {
            PerformanceError::Io(format!(
                "cannot protect performance marker socket {}: {error}",
                socket_path.display()
            ))
        },
    )?;
    listener.set_nonblocking(true).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot configure performance marker socket {}: {error}",
            socket_path.display()
        ))
    })?;
    Ok((listener, socket_guard))
}

fn require_owner_only_socket_parent(socket_path: &Path) -> Result<(), PerformanceError> {
    let parent = socket_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let metadata = std::fs::metadata(parent).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot inspect performance marker socket parent {}: {error}",
            parent.display()
        ))
    })?;
    if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
        return Err(PerformanceError::Io(format!(
            "performance marker socket parent must be an owner-only directory: {}",
            parent.display()
        )));
    }
    Ok(())
}

fn socket_identity(path: &Path) -> Result<MarkerSocketIdentity, PerformanceError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot inspect performance marker socket {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_socket() {
        return Err(PerformanceError::Io(format!(
            "performance marker endpoint is not a Unix socket: {}",
            path.display()
        )));
    }
    Ok(MarkerSocketIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn validate_initial_events(events: &[PerformanceEvent]) -> Result<(), PerformanceError> {
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

fn invalid_marker(message: impl Into<String>) -> PerformanceError {
    PerformanceError::InvalidPerformanceMarkerCsv(message.into())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::time::Duration;

    use super::{
        ALIGNMENT_TOLERANCE_NS, InitialLinuxPerformanceMarkerCommand,
        InitialPerformanceMarkerCommand, InitialPerformanceMarkerRecorder, MarkerSocketGuard,
        SECOND_NS, TERMINAL_MARKER_PRIORITY_NS, acknowledge_completion,
        finish_combined_linux_capture, sample_combined_linux_memory,
        sample_combined_linux_memory_for_command, should_defer_socket_accept,
        should_reserve_terminal_marker_priority, socket_identity,
    };
    use crate::LinuxMemoryCsvRecorder;

    #[test]
    fn socket_accepts_reserve_the_alignment_window_for_scheduled_markers() {
        let mut recorder = InitialPerformanceMarkerRecorder::new();
        for (command, elapsed_ns) in [
            (InitialPerformanceMarkerCommand::ProcessStart, 0),
            (InitialPerformanceMarkerCommand::GameReady, SECOND_NS),
            (
                InitialPerformanceMarkerCommand::WorldLoadStart,
                2 * SECOND_NS,
            ),
            (
                InitialPerformanceMarkerCommand::WorldReady {
                    dimension: "minecraft:overworld".to_owned(),
                },
                3 * SECOND_NS,
            ),
            (InitialPerformanceMarkerCommand::WarmUpStart, 4 * SECOND_NS),
        ] {
            recorder
                .record(command, elapsed_ns)
                .expect("pre-measurement marker should record");
        }

        let measurement_start_ns = 304 * SECOND_NS;
        assert!(
            !should_defer_socket_accept(
                &recorder,
                measurement_start_ns - ALIGNMENT_TOLERANCE_NS - 1
            )
            .expect("deadline lookup should succeed")
        );
        assert!(
            should_defer_socket_accept(&recorder, measurement_start_ns - ALIGNMENT_TOLERANCE_NS)
                .expect("deadline lookup should succeed")
        );
        assert!(
            should_defer_socket_accept(&recorder, measurement_start_ns - 1)
                .expect("deadline lookup should succeed")
        );
    }

    #[test]
    fn socket_guard_preserves_a_replacement_path() {
        let root = std::env::temp_dir().join(format!(
            "ravel-marker-guard-replacement-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test root should be removable");
        }
        fs::create_dir_all(&root).expect("test root should be creatable");
        let socket = root.join("markers.sock");
        let listener = UnixListener::bind(&socket).expect("test socket should bind");
        let guard = MarkerSocketGuard {
            path: &socket,
            identity: socket_identity(&socket).expect("socket identity should be readable"),
        };

        fs::remove_file(&socket).expect("bound socket path should be replaceable");
        fs::write(&socket, "replacement").expect("replacement path should be writable");
        drop(guard);

        assert_eq!(
            fs::read_to_string(&socket).expect("replacement should survive guard cleanup"),
            "replacement"
        );
        drop(listener);
        fs::remove_dir_all(root).expect("test root should be removable");
    }

    #[test]
    fn combined_commands_map_and_serialize_every_operator_event() {
        for (command, expected_marker, expected_pid, expected_wire) in [
            (
                InitialLinuxPerformanceMarkerCommand::ProcessStart { pid: 42 },
                InitialPerformanceMarkerCommand::ProcessStart,
                Some(42),
                "process-start,42\n",
            ),
            (
                InitialLinuxPerformanceMarkerCommand::GameReady,
                InitialPerformanceMarkerCommand::GameReady,
                None,
                "game-ready,\n",
            ),
            (
                InitialLinuxPerformanceMarkerCommand::WorldLoadStart,
                InitialPerformanceMarkerCommand::WorldLoadStart,
                None,
                "world-load-start,\n",
            ),
            (
                InitialLinuxPerformanceMarkerCommand::WorldReady {
                    dimension: "minecraft:overworld".to_owned(),
                },
                InitialPerformanceMarkerCommand::WorldReady {
                    dimension: "minecraft:overworld".to_owned(),
                },
                None,
                "world-ready,minecraft:overworld\n",
            ),
            (
                InitialLinuxPerformanceMarkerCommand::WarmUpStart,
                InitialPerformanceMarkerCommand::WarmUpStart,
                None,
                "warm-up-start,\n",
            ),
            (
                InitialLinuxPerformanceMarkerCommand::DimensionChangeStart {
                    dimension: "minecraft:the_nether".to_owned(),
                },
                InitialPerformanceMarkerCommand::DimensionChangeStart {
                    dimension: "minecraft:the_nether".to_owned(),
                },
                None,
                "dimension-change-start,minecraft:the_nether\n",
            ),
            (
                InitialLinuxPerformanceMarkerCommand::DimensionReady {
                    dimension: "minecraft:the_nether".to_owned(),
                },
                InitialPerformanceMarkerCommand::DimensionReady {
                    dimension: "minecraft:the_nether".to_owned(),
                },
                None,
                "dimension-ready,minecraft:the_nether\n",
            ),
            (
                InitialLinuxPerformanceMarkerCommand::ProcessExit,
                InitialPerformanceMarkerCommand::ProcessExit,
                None,
                "process-exit,\n",
            ),
        ] {
            assert_eq!(
                command
                    .to_wire()
                    .expect("combined command should serialize"),
                expected_wire
            );
            assert_eq!(command.into_marker(), Some((expected_marker, expected_pid)));
        }

        assert_eq!(
            InitialLinuxPerformanceMarkerCommand::Abort
                .to_wire()
                .expect("abort should serialize"),
            "abort,\n"
        );
        assert_eq!(
            InitialLinuxPerformanceMarkerCommand::Abort.into_marker(),
            None
        );
        assert!(
            InitialLinuxPerformanceMarkerCommand::ProcessStart { pid: 0 }
                .to_wire()
                .is_err()
        );
        assert!(
            InitialLinuxPerformanceMarkerCommand::WorldReady {
                dimension: "minecraft:overworld\nprocess-exit".to_owned(),
            }
            .to_wire()
            .is_err()
        );
        assert!(
            super::send_initial_linux_performance_marker(
                Path::new("/missing/ravel-markers.sock"),
                &InitialLinuxPerformanceMarkerCommand::GameReady
            )
            .is_err()
        );
    }

    #[test]
    fn combined_capture_finishes_both_private_outputs_without_waiting_on_wall_clock() {
        let root =
            std::env::temp_dir().join(format!("ravel-combined-finish-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test root should be removable");
        }
        let proc_root = root.join("proc");
        write_proc_snapshot(&proc_root, 42);
        let memory_csv = root.join("linux-memory.csv");
        let marker_csv = root.join("markers.csv");
        let mut memory = Some(
            LinuxMemoryCsvRecorder::new(
                &proc_root,
                42,
                4096,
                Duration::from_millis(10),
                0,
                &memory_csv,
            )
            .expect("memory recorder should initialize"),
        );
        assert!(
            memory
                .as_mut()
                .expect("memory recorder should remain attached")
                .sample_due_at(10_000_000, ALIGNMENT_TOLERANCE_NS)
                .expect("exact shared-clock deadline should sample")
        );

        let markers = complete_recorder();
        let counts = finish_combined_linux_capture(&markers, &mut memory, &marker_csv)
            .expect("combined outputs should finish");
        assert_eq!(counts.lifecycle_markers, 17);
        assert_eq!(counts.linux_memory_samples, 1);
        assert!(memory.is_none());
        for output in [&marker_csv, &memory_csv] {
            assert_eq!(
                fs::metadata(output)
                    .expect("combined output metadata should be readable")
                    .mode()
                    & 0o777,
                0o600
            );
        }

        fs::remove_dir_all(root).expect("test root should be removable");
    }

    #[test]
    fn terminal_marker_priority_precedes_a_due_memory_sample() {
        let root =
            std::env::temp_dir().join(format!("ravel-terminal-priority-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test root should be removable");
        }
        let proc_root = root.join("proc");
        write_proc_snapshot(&proc_root, 42);
        let memory_csv = root.join("linux-memory.csv");
        let mut memory = LinuxMemoryCsvRecorder::new(
            &proc_root,
            42,
            4096,
            Duration::from_millis(10),
            0,
            &memory_csv,
        )
        .expect("memory recorder should initialize");

        assert!(should_reserve_terminal_marker_priority(
            Some(&memory),
            10_000_000
        ));
        assert!(should_reserve_terminal_marker_priority(
            Some(&memory),
            10_000_000 + TERMINAL_MARKER_PRIORITY_NS - 1
        ));
        assert!(!should_reserve_terminal_marker_priority(
            Some(&memory),
            10_000_000 + TERMINAL_MARKER_PRIORITY_NS
        ));

        fs::remove_dir_all(proc_root.join("42")).expect("synthetic process should be removable");
        assert!(
            sample_combined_linux_memory(Some(&mut memory), 10_000_000).is_err(),
            "sampling after process exit would fail without terminal priority"
        );
        assert!(
            !sample_combined_linux_memory_for_command(
                &InitialPerformanceMarkerCommand::ProcessExit,
                Some(&mut memory),
                10_000_000,
            )
            .expect("a pending terminal marker must bypass the due sample")
        );
        drop(memory);
        assert!(
            !memory_csv.exists(),
            "failed partial output should be removed"
        );
        fs::remove_dir_all(root).expect("test root should be removable");
    }

    #[test]
    fn finalized_capture_ignores_a_disconnected_completion_client() {
        let (mut server, client) =
            std::os::unix::net::UnixStream::pair().expect("socket pair should open");
        drop(client);

        acknowledge_completion(&mut server);
    }

    fn complete_recorder() -> InitialPerformanceMarkerRecorder {
        let mut recorder = InitialPerformanceMarkerRecorder::new();
        for (command, elapsed_ns) in [
            (InitialPerformanceMarkerCommand::ProcessStart, 0),
            (InitialPerformanceMarkerCommand::GameReady, SECOND_NS),
            (
                InitialPerformanceMarkerCommand::WorldLoadStart,
                2 * SECOND_NS,
            ),
            (
                InitialPerformanceMarkerCommand::WorldReady {
                    dimension: "minecraft:overworld".to_owned(),
                },
                3 * SECOND_NS,
            ),
            (InitialPerformanceMarkerCommand::WarmUpStart, 4 * SECOND_NS),
        ] {
            recorder
                .record(command, elapsed_ns)
                .expect("pre-measurement marker should record");
        }
        for elapsed_ns in [304, 484, 724] {
            recorder
                .advance_to(elapsed_ns * SECOND_NS)
                .expect("scheduled outbound route should record");
        }
        recorder
            .record(
                InitialPerformanceMarkerCommand::DimensionChangeStart {
                    dimension: "minecraft:the_nether".to_owned(),
                },
                744 * SECOND_NS,
            )
            .expect("outbound dimension start should record");
        recorder
            .record(
                InitialPerformanceMarkerCommand::DimensionReady {
                    dimension: "minecraft:the_nether".to_owned(),
                },
                764 * SECOND_NS,
            )
            .expect("nether ready should record");
        for elapsed_ns in [844, 1_024] {
            recorder
                .advance_to(elapsed_ns * SECOND_NS)
                .expect("scheduled return route should record");
        }
        recorder
            .record(
                InitialPerformanceMarkerCommand::DimensionChangeStart {
                    dimension: "minecraft:overworld".to_owned(),
                },
                1_064 * SECOND_NS,
            )
            .expect("return dimension start should record");
        recorder
            .record(
                InitialPerformanceMarkerCommand::DimensionReady {
                    dimension: "minecraft:overworld".to_owned(),
                },
                1_084 * SECOND_NS,
            )
            .expect("overworld ready should record");
        recorder
            .advance_to(1_204 * SECOND_NS)
            .expect("measurement end should record");
        assert!(
            recorder
                .record(
                    InitialPerformanceMarkerCommand::ProcessExit,
                    1_324 * SECOND_NS
                )
                .expect("process exit should complete markers")
        );
        recorder
    }

    fn write_proc_snapshot(proc_root: &Path, pid: u32) {
        fs::create_dir_all(proc_root.join(pid.to_string()))
            .expect("synthetic process directory should be creatable");
        fs::create_dir_all(proc_root.join("pressure"))
            .expect("synthetic pressure directory should be creatable");
        fs::write(
            proc_root.join(pid.to_string()).join("smaps_rollup"),
            "Rss: 3072 kB\n",
        )
        .expect("synthetic smaps should be writable");
        fs::write(
            proc_root.join(pid.to_string()).join("stat"),
            format!("{pid} (java worker) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 100\n"),
        )
        .expect("synthetic process identity should be writable");
        fs::write(
            proc_root.join("meminfo"),
            "MemTotal: 16777216 kB\nMemAvailable: 4194304 kB\n",
        )
        .expect("synthetic meminfo should be writable");
        fs::write(
            proc_root.join("pressure/memory"),
            "some total=100\nfull total=10\n",
        )
        .expect("synthetic PSI should be writable");
        fs::write(proc_root.join("vmstat"), "pswpin 2\npswpout 3\n")
            .expect("synthetic vmstat should be writable");
    }
}
