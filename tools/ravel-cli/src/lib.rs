#![forbid(unsafe_code)]

//! Developer-facing fixture and benchmark operations.

mod error;
mod evidence;
mod fixture;
mod host;
mod linux_memory;
mod linux_supervisor;
mod performance_markers;
mod tracy;

pub use error::{FixtureError, PerformanceError};
pub use evidence::validate_private_evidence_output;
pub use fixture::{
    ExpectedManifest, FixtureDescriptor, FixtureSource, FixtureVerification,
    load_fixture_descriptor, reconstruct_fixture, sha256_file, verify_fixture_archive,
};
pub use host::{BuiltHost, HostError, build_host};
pub use linux_memory::{
    LinuxMemoryCsvRecorder, LinuxMemoryPressure, LinuxMemorySnapshot, LinuxNormalizedMemorySample,
    LinuxProcessIdentity, LinuxTimedMemorySample, capture_linux_memory_csv,
    normalize_linux_memory_sample, parse_linux_memory_csv, parse_linux_memory_snapshot,
    query_linux_page_size, read_linux_memory_snapshot, read_linux_memory_snapshot_for_process,
    read_linux_process_identity, verify_linux_memory_csv,
};
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
pub use tracy::{
    TracyFrameCounts, TracyFrameMetric, TracyFrameSample, capture_tracy, export_tracy_frame_csv,
    parse_tracy_frame_csv, verify_tracy_frame_csv,
};
