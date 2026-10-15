//! Procfs parsing, normalization, and process identity checks.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::PerformanceError;

use super::{
    LinuxMemoryPressure, LinuxMemorySnapshot, LinuxNormalizedMemorySample, LinuxProcessIdentity,
};

/// Parses one Linux memory snapshot from the kernel's procfs interfaces.
/// Inputs are `/proc/<pid>/smaps_rollup`, `/proc/meminfo`, `/proc/pressure/memory`,
/// and `/proc/vmstat`, in that order; swap counters use `page_size_bytes`.
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
