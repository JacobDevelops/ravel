use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;

use super::common::{ravel_cli_command, temporary_root};

#[test]
fn command_line_supervisor_marks_the_pid_that_executes_java() {
    let root = temporary_root("supervise-initial-linux");
    let socket = root.join("markers.sock");
    let pid_file = root.join("java.pid");
    let listener = std::os::unix::net::UnixListener::bind(&socket)
        .expect("fake combined recorder should bind");
    let server_pid_file = pid_file.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("fake recorder should accept process start");
        let mut process_start = String::new();
        stream
            .read_to_string(&mut process_start)
            .expect("process start should be readable");
        let pid = process_start
            .strip_suffix('\n')
            .expect("process start should end in newline")
            .strip_prefix("process-start,")
            .expect("first command should be process start")
            .parse::<u32>()
            .expect("process start should carry a PID");
        assert!(
            !server_pid_file.exists(),
            "Java must remain blocked until process start is accepted"
        );
        stream
            .write_all(b"ok\n")
            .expect("process start acknowledgement should write");
        drop(stream);
        let (mut stream, _) = listener
            .accept()
            .expect("fake recorder should accept process exit");
        let mut process_exit = String::new();
        stream
            .read_to_string(&mut process_exit)
            .expect("process exit should be readable");
        assert_eq!(process_exit, "process-exit,\n");
        assert!(
            PathBuf::from("/proc").join(pid.to_string()).exists(),
            "process exit must be marked before the supervised JVM is reaped"
        );
        stream
            .write_all(b"complete\n")
            .expect("completion acknowledgement should write");
        pid
    });

    let output = ravel_cli_command()
        .args([
            "performance",
            "supervise-initial-linux",
            socket.to_str().expect("test socket path should be UTF-8"),
            "--",
            "/bin/sh",
            "-c",
            "printf '%s' \"$$\" > \"$1\"",
            "sh",
            pid_file.to_str().expect("PID output path should be UTF-8"),
        ])
        .output()
        .expect("Linux supervisor should execute");

    assert!(output.status.success());
    let marked_pid = server.join().expect("fake recorder should join");
    let executed_pid = fs::read_to_string(&pid_file)
        .expect("fake Java PID should be readable")
        .parse::<u32>()
        .expect("fake Java PID should parse");
    assert_eq!(marked_pid, executed_pid);
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn command_line_supervisor_aborts_capture_when_java_fails() {
    let root = temporary_root("supervise-initial-linux-failure");
    let socket = root.join("markers.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket)
        .expect("fake combined recorder should bind");
    let server = thread::spawn(move || {
        let process_start = accept_fake_marker(&listener, "ok\n");
        assert!(process_start.starts_with("process-start,"));
        let abort = accept_fake_marker(&listener, "error:supervised process failed\n");
        assert_eq!(abort, "abort,");
    });

    let output = ravel_cli_command()
        .args([
            "performance",
            "supervise-initial-linux",
            socket.to_str().expect("test socket path should be UTF-8"),
            "--",
            "/bin/sh",
            "-c",
            "exit 7",
        ])
        .output()
        .expect("failing Linux supervisor should execute");

    assert_eq!(output.status.code(), Some(7));
    server.join().expect("fake recorder should join");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.is_empty());
    assert!(!stderr.contains("exit 7"));

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn command_line_supervisor_preserves_java_signal_status() {
    let root = temporary_root("supervise-initial-linux-signal");
    let socket = root.join("markers.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket)
        .expect("fake combined recorder should bind");
    let server = thread::spawn(move || {
        let process_start = accept_fake_marker(&listener, "ok\n");
        assert!(process_start.starts_with("process-start,"));
        let abort = accept_fake_marker(&listener, "error:supervised process failed\n");
        assert_eq!(abort, "abort,");
    });

    let output = ravel_cli_command()
        .args([
            "performance",
            "supervise-initial-linux",
            socket.to_str().expect("test socket path should be UTF-8"),
            "--",
            "/bin/sh",
            "-c",
            "kill -TERM $$",
        ])
        .output()
        .expect("signalled Linux supervisor should execute");

    assert_eq!(output.status.code(), Some(143));
    assert_eq!(output.stderr, [] as [u8; 0]);
    server.join().expect("fake recorder should join");

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn accept_fake_marker(listener: &std::os::unix::net::UnixListener, response: &str) -> String {
    let (mut stream, _) = listener
        .accept()
        .expect("fake recorder should accept marker");
    let mut request = String::new();
    stream
        .read_to_string(&mut request)
        .expect("fake recorder should read marker");
    stream
        .write_all(response.as_bytes())
        .expect("fake recorder should respond");
    request
        .strip_suffix('\n')
        .expect("marker request should end in newline")
        .to_owned()
}
