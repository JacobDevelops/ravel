//! Linux memory samples and procfs collection.

mod procfs;
mod recorder;

pub use procfs::{
    normalize_linux_memory_sample, parse_linux_memory_snapshot, query_linux_page_size,
    read_linux_memory_snapshot, read_linux_memory_snapshot_for_process,
    read_linux_process_identity,
};
pub use recorder::{
    LinuxMemoryCsvRecorder, capture_linux_memory_csv, parse_linux_memory_csv,
    verify_linux_memory_csv,
};

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
