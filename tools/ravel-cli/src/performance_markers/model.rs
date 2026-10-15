//! Lifecycle events, operator commands, and phase classification.

use super::invalid_marker;
use crate::error::PerformanceError;

pub(super) const EXPECTED_EVENT_COUNT: usize = 17;
pub(super) const ALIGNMENT_TOLERANCE_NS: u64 = 5_000_000;
pub(super) const SECOND_NS: u64 = 1_000_000_000;
pub(super) const WARM_UP_NS: u64 = 300 * SECOND_NS;
pub(super) const MEASUREMENT_NS: u64 = 900 * SECOND_NS;
pub(super) const COOL_DOWN_NS: u64 = 120 * SECOND_NS;
pub(super) const SCHEDULED_MARKER_OFFSETS_SECONDS: [u64; 7] =
    [300, 300, 480, 720, 840, 1_020, 1_200];

pub(super) const EXPECTED_KINDS: [PerformanceEventKind; EXPECTED_EVENT_COUNT] = [
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

pub(super) const EXPECTED_ROUTES: [(&str, &str, u64, PerformancePhase); 5] = [
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
    pub(super) fn parse(value: &str, line_number: usize) -> Result<Self, PerformanceError> {
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

    pub(super) const fn as_str(self) -> &'static str {
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
    pub(super) events: Vec<PerformanceEvent>,
}

/// One operator-supplied event accepted by the initial marker recorder.
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
    pub(super) fn event_kind(&self) -> PerformanceEventKind {
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

    pub(super) fn dimension(&self) -> Option<&str> {
        match self {
            Self::WorldReady { dimension }
            | Self::DimensionChangeStart { dimension }
            | Self::DimensionReady { dimension } => Some(dimension),
            _ => None,
        }
    }

    pub(super) fn into_event(self) -> (PerformanceEventKind, Option<String>) {
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
