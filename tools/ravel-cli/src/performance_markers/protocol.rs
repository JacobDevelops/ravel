//! Operator command parsing and bounded Unix-socket request framing.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use super::invalid_marker;
use super::model::{InitialLinuxPerformanceMarkerCommand, InitialPerformanceMarkerCommand};
use crate::error::PerformanceError;

pub(super) const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(1);
const SOCKET_REQUEST_BUDGET: Duration = Duration::from_millis(250);
const MAX_MARKER_REQUEST_BYTES: u64 = 1_024;

impl InitialPerformanceMarkerCommand {
    pub(super) fn to_wire(&self) -> Result<String, PerformanceError> {
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
    pub(super) fn into_marker(self) -> Option<(InitialPerformanceMarkerCommand, Option<u32>)> {
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

    pub(super) fn to_wire(&self) -> Result<String, PerformanceError> {
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

/// Sends one event to a running focus-free marker recorder.
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

// Protocol errors reject one request; transport failures terminate capture.
pub(super) fn refuse_request(error: PerformanceError) -> Result<(), PerformanceError> {
    if matches!(error, PerformanceError::Io(_)) {
        return Err(error);
    }
    Ok(())
}

fn is_retryable_read(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
    )
}

pub(super) fn read_marker_command(
    stream: &mut UnixStream,
) -> Result<InitialPerformanceMarkerCommand, PerformanceError> {
    let (kind, value) = read_marker_fields(stream)?;
    parse_initial_performance_marker_command(&kind, value.as_deref())
}

pub(super) fn read_linux_marker_command(
    stream: &mut UnixStream,
) -> Result<InitialLinuxPerformanceMarkerCommand, PerformanceError> {
    let (kind, value) = read_marker_fields(stream)?;
    parse_initial_linux_performance_marker_command(&kind, value.as_deref())
}

// Read timeouts are retry granularity; SOCKET_REQUEST_BUDGET bounds the request.
fn read_marker_fields(
    stream: &mut UnixStream,
) -> Result<(String, Option<String>), PerformanceError> {
    stream
        .set_read_timeout(Some(SOCKET_POLL_INTERVAL))
        .map_err(|error| {
            PerformanceError::Io(format!(
                "cannot configure performance marker request timeout: {error}"
            ))
        })?;
    let mut bytes = Vec::new();
    let request_started = Instant::now();
    while u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= MAX_MARKER_REQUEST_BYTES {
        let mut byte = [0_u8; 1];
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                bytes.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            Err(error) if is_retryable_read(&error) => {}
            Err(error) => {
                return Err(PerformanceError::Io(format!(
                    "cannot read performance marker request: {error}"
                )));
            }
        }
        if request_started.elapsed() >= SOCKET_REQUEST_BUDGET {
            return Err(PerformanceError::Io(format!(
                "performance marker request did not arrive within {} ms; the recorder was \
                 waiting for the rest of a request and read {} of its bytes",
                SOCKET_REQUEST_BUDGET.as_millis(),
                bytes.len()
            )));
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

pub(super) fn marker_socket_write_error(error: &io::Error) -> PerformanceError {
    PerformanceError::Io(format!(
        "cannot write performance marker socket response: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{InitialLinuxPerformanceMarkerCommand, InitialPerformanceMarkerCommand};

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
}
