use std::ffi::{OsStr, OsString};
use std::os::unix::process::CommandExt as _;
use std::os::unix::process::ExitStatusExt as _;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};

use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};

use crate::error::PerformanceError;
use crate::performance_markers::{
    InitialLinuxPerformanceMarkerCommand, send_initial_linux_performance_marker,
};

/// Runs the JVM through a same-PID marker child and reports its final status.
/// Emits `process-exit` for a successful JVM or `abort` for a failed one.
///
/// # Errors
///
/// Returns an error when the current executable cannot be resolved, the marker
/// child cannot be started or waited for, or a successful JVM cannot complete
/// the marker stream.
pub fn supervise_initial_linux_process(
    socket_path: &Path,
    program: &OsStr,
    arguments: &[OsString],
) -> Result<ExitStatus, PerformanceError> {
    let current_executable = std::env::current_exe().map_err(|error| {
        aborting_supervisor_error(
            socket_path,
            format!("cannot locate the Ravel CLI for Linux process supervision: {error}"),
        )
    })?;
    supervise_initial_linux_process_with_executable(
        socket_path,
        &current_executable,
        program,
        arguments,
    )
}

fn supervise_initial_linux_process_with_executable(
    socket_path: &Path,
    current_executable: &Path,
    program: &OsStr,
    arguments: &[OsString],
) -> Result<ExitStatus, PerformanceError> {
    let mut child = Command::new(current_executable)
        .arg("performance")
        .arg("exec-initial-linux-child")
        .arg(socket_path)
        .arg("--")
        .arg(program)
        .args(arguments)
        .spawn()
        .map_err(|error| {
            aborting_supervisor_error(
                socket_path,
                format!("cannot start the supervised Linux marker child: {error}"),
            )
        })?;
    let observed = match wait_for_linux_child_exit_without_reaping(&child) {
        Ok(status) => status,
        Err(error) => {
            abort_capture(socket_path);
            let _ = child.wait();
            return Err(error);
        }
    };

    let marker_result = if observed.success() {
        send_initial_linux_performance_marker(
            socket_path,
            &InitialLinuxPerformanceMarkerCommand::ProcessExit,
        )
        .and_then(|complete| {
            if complete {
                Ok(())
            } else {
                Err(PerformanceError::InvalidPerformanceMarkerCsv(
                    "successful supervised Linux process did not complete the marker stream"
                        .to_owned(),
                ))
            }
        })
    } else {
        abort_capture(socket_path);
        Ok(())
    };
    let reaped_status = child.wait().map_err(|error| {
        aborting_supervisor_error(
            socket_path,
            format!("cannot reap the supervised Linux process: {error}"),
        )
    })?;
    marker_result?;
    if !observed.matches(reaped_status) {
        return Err(PerformanceError::Io(format!(
            "supervised Linux process status changed while being reaped: \
             observed {observed:?}, reaped {reaped_status}"
        )));
    }

    Ok(reaped_status)
}

fn wait_for_linux_child_exit_without_reaping(
    child: &Child,
) -> Result<ObservedExit, PerformanceError> {
    let raw_pid = i32::try_from(child.id()).map_err(|error| {
        PerformanceError::InvalidLinuxProc(format!(
            "supervised Linux process ID cannot be represented by waitid: {error}"
        ))
    })?;
    let pid = Pid::from_raw(raw_pid).ok_or_else(|| {
        PerformanceError::InvalidLinuxProc(
            "supervised Linux process ID must be greater than zero".to_owned(),
        )
    })?;
    let status = waitid(
        WaitId::Pid(pid),
        WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
    )
    .map_err(|error| {
        PerformanceError::Io(format!(
            "cannot wait for the supervised Linux process without reaping it: {error}"
        ))
    })?
    .ok_or_else(|| {
        PerformanceError::Io(
            "waitid returned no supervised Linux process status while blocking".to_owned(),
        )
    })?;

    if let Some(code) = status.exit_status() {
        return Ok(ObservedExit::Code(code));
    }
    if let Some(signal) = status.terminating_signal() {
        return Ok(ObservedExit::Signal(signal));
    }
    Err(PerformanceError::InvalidLinuxProc(
        "waitid returned a non-terminal supervised Linux process status".to_owned(),
    ))
}

#[derive(Debug, Eq, PartialEq)]
enum ObservedExit {
    Code(i32),
    Signal(i32),
}

impl ObservedExit {
    const fn success(&self) -> bool {
        matches!(self, Self::Code(0))
    }

    fn matches(&self, status: ExitStatus) -> bool {
        match self {
            Self::Code(code) => status.code() == Some(*code),
            Self::Signal(signal) => status.signal() == Some(*signal),
        }
    }
}

fn abort_capture(socket_path: &Path) {
    let _ = send_initial_linux_performance_marker(
        socket_path,
        &InitialLinuxPerformanceMarkerCommand::Abort,
    );
}

fn aborting_supervisor_error(socket_path: &Path, message: String) -> PerformanceError {
    abort_capture(socket_path);
    PerformanceError::Io(message)
}

/// Marks this process as the capture subject, then replaces it with Java.
/// Public for the binary target's internal supervisor handoff.
///
/// # Errors
///
/// Returns an error when process-start is rejected, unexpectedly completes the
/// stream, or the operating system cannot replace this process.
pub fn exec_initial_linux_child(
    socket_path: &Path,
    program: &OsStr,
    arguments: &[OsString],
) -> Result<(), PerformanceError> {
    let complete = send_initial_linux_performance_marker(
        socket_path,
        &InitialLinuxPerformanceMarkerCommand::ProcessStart {
            pid: std::process::id(),
        },
    )?;
    if complete {
        return Err(PerformanceError::InvalidPerformanceMarkerCsv(
            "process-start unexpectedly completed the Linux marker stream".to_owned(),
        ));
    }

    let error = Command::new(program).args(arguments).exec();
    Err(PerformanceError::Io(format!(
        "cannot replace the Linux marker child with the supervised process: {error}"
    )))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::thread;

    use super::supervise_initial_linux_process_with_executable;

    #[test]
    fn child_spawn_failure_aborts_the_pending_capture() {
        let root = std::env::temp_dir().join(format!(
            "ravel-linux-supervisor-spawn-failure-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test root should be removable");
        }
        fs::create_dir_all(&root).expect("test root should be creatable");
        let socket = root.join("markers.sock");
        let listener = UnixListener::bind(&socket).expect("fake recorder should bind");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("abort marker should connect");
            let mut request = String::new();
            stream
                .read_to_string(&mut request)
                .expect("abort marker should be readable");
            stream
                .write_all(b"error:launch failed\n")
                .expect("abort response should write");
            request
        });

        assert!(
            supervise_initial_linux_process_with_executable(
                &socket,
                Path::new("/missing/ravel-cli"),
                OsStr::new("/bin/true"),
                &[],
            )
            .is_err()
        );
        assert_eq!(
            server.join().expect("fake recorder should join"),
            "abort,\n"
        );

        fs::remove_dir_all(root).expect("test root should be removable");
    }
}
