//! Fixed-profile lifecycle markers, CSV validation, and Unix-socket capture.

mod csv;
mod model;
mod protocol;
mod schedule;
mod socket;

pub use csv::{parse_initial_performance_marker_csv, verify_initial_performance_marker_csv};
pub use model::{
    InitialLinuxPerformanceCounts, InitialLinuxPerformanceMarkerCommand,
    InitialPerformanceMarkerCommand, InitialPerformanceMarkerStream, PerformanceEvent,
    PerformanceEventKind, PerformancePhase,
};
pub use protocol::{
    parse_initial_linux_performance_marker_command, parse_initial_performance_marker_command,
    send_initial_linux_performance_marker, send_initial_performance_marker,
};
pub use schedule::InitialPerformanceMarkerRecorder;
pub use socket::{record_initial_linux_performance, record_initial_performance_markers};

use crate::error::PerformanceError;

fn invalid_marker(message: impl Into<String>) -> PerformanceError {
    PerformanceError::InvalidPerformanceMarkerCsv(message.into())
}
