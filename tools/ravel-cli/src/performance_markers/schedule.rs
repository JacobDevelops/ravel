//! Fixed-profile scheduling on a caller-supplied monotonic clock.

use std::path::Path;

use super::csv::{validate_initial_events, validate_partial_event, write_initial_marker_csv};
use super::invalid_marker;
use super::model::{
    EXPECTED_EVENT_COUNT, EXPECTED_KINDS, EXPECTED_ROUTES, InitialPerformanceMarkerCommand,
    PerformanceEvent, PerformanceEventKind, SCHEDULED_MARKER_OFFSETS_SECONDS, SECOND_NS,
};
use crate::error::PerformanceError;

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

    pub(super) fn awaiting(&self) -> String {
        EXPECTED_KINDS.get(self.events.len()).map_or_else(
            || "nothing; the stream is already complete".to_owned(),
            |kind| format!("event {} (`{}`)", self.events.len(), kind.as_str()),
        )
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

    pub(super) fn next_scheduled_ns(&self) -> Result<Option<u64>, PerformanceError> {
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
