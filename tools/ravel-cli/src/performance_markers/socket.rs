//! Unix-socket capture loops, shared-clock Linux sampling, and socket cleanup.

use std::io::{self, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use super::invalid_marker;
use super::model::{
    ALIGNMENT_TOLERANCE_NS, COOL_DOWN_NS, InitialLinuxPerformanceCounts,
    InitialPerformanceMarkerCommand, MEASUREMENT_NS, SECOND_NS, WARM_UP_NS,
};
use super::protocol::{
    SOCKET_POLL_INTERVAL, marker_socket_write_error, read_linux_marker_command,
    read_marker_command, refuse_request,
};
use super::schedule::InitialPerformanceMarkerRecorder;
use crate::error::PerformanceError;
use crate::linux_memory::LinuxMemoryCsvRecorder;

const TERMINAL_MARKER_PRIORITY_NS: u64 = 3_000_000;
// Allow five minutes for launcher startup beyond the fixed capture schedule.
const CAPTURE_DEADLINE_NS: u64 = WARM_UP_NS + MEASUREMENT_NS + COOL_DOWN_NS + 300 * SECOND_NS;

/// Runs a focus-free Unix-socket recorder until a complete initial-v2 stream.
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
        validate_capture_deadline(&recorder, elapsed_ns)?;
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
                        refuse_request(error)?;
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
        validate_capture_deadline(&markers, elapsed_ns)?;
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
                        refuse_request(error)?;
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

fn acknowledge_completion(stream: &mut UnixStream) {
    let _ = stream.write_all(b"complete\n");
}

fn validate_capture_deadline(
    recorder: &InitialPerformanceMarkerRecorder,
    elapsed_ns: u64,
) -> Result<(), PerformanceError> {
    if elapsed_ns <= CAPTURE_DEADLINE_NS {
        return Ok(());
    }
    Err(PerformanceError::Io(format!(
        "performance marker recorder gave up at {elapsed_ns} ns, past its \
         {CAPTURE_DEADLINE_NS} ns capture deadline, while waiting for {}",
        recorder.awaiting()
    )))
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

fn instant_elapsed_ns(origin: Instant) -> Result<u64, PerformanceError> {
    u64::try_from(origin.elapsed().as_nanos())
        .map_err(|_| invalid_marker("marker recorder monotonic timestamp exceeds u64"))
}

struct MarkerSocketGuard<'a> {
    path: &'a Path,
    identity: MarkerSocketIdentity,
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
    let staging_path = socket_path.with_extension("pending");
    if staging_path.exists() {
        return Err(PerformanceError::Io(format!(
            "performance marker socket is already being bound: {}",
            staging_path.display()
        )));
    }
    let listener = stage_marker_socket(&staging_path, socket_path).inspect_err(|_| {
        let _ = std::fs::remove_file(&staging_path);
    })?;
    let socket_guard = MarkerSocketGuard {
        path: socket_path,
        identity: socket_identity(socket_path)?,
    };
    Ok((listener, socket_guard))
}

// Publish only after setting permissions; bind initially inherits the process umask.
fn stage_marker_socket(
    staging_path: &Path,
    socket_path: &Path,
) -> Result<UnixListener, PerformanceError> {
    let listener = UnixListener::bind(staging_path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot bind performance marker socket {}: {error}",
            socket_path.display()
        ))
    })?;
    std::fs::set_permissions(staging_path, std::fs::Permissions::from_mode(0o600)).map_err(
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
    std::fs::rename(staging_path, socket_path).map_err(|error| {
        PerformanceError::Io(format!(
            "cannot publish performance marker socket {}: {error}",
            socket_path.display()
        ))
    })?;
    Ok(listener)
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

#[cfg(test)]
mod tests;
