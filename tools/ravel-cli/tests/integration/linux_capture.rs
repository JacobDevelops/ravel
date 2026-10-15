use std::fs;
use std::os::unix::net::UnixStream;
#[cfg(target_os = "linux")]
use std::process::Stdio;
use std::thread;
use std::time::Duration;

use ravel_cli::{
    InitialLinuxPerformanceMarkerCommand, PerformanceError,
    parse_initial_linux_performance_marker_command, record_initial_linux_performance,
    send_initial_linux_performance_marker,
};

#[cfg(target_os = "linux")]
use super::common::ravel_cli_command;
use super::common::{temporary_root, wait_for_socket, write_proc_snapshot};

#[test]
fn combined_linux_commands_require_a_positive_process_pid() {
    for (kind, value, expected) in [
        (
            "process-start",
            Some("42"),
            InitialLinuxPerformanceMarkerCommand::ProcessStart { pid: 42 },
        ),
        (
            "game-ready",
            None,
            InitialLinuxPerformanceMarkerCommand::GameReady,
        ),
        (
            "world-load-start",
            None,
            InitialLinuxPerformanceMarkerCommand::WorldLoadStart,
        ),
        (
            "world-ready",
            Some("minecraft:overworld"),
            InitialLinuxPerformanceMarkerCommand::WorldReady {
                dimension: "minecraft:overworld".to_owned(),
            },
        ),
        (
            "warm-up-start",
            None,
            InitialLinuxPerformanceMarkerCommand::WarmUpStart,
        ),
        (
            "dimension-change-start",
            Some("minecraft:the_nether"),
            InitialLinuxPerformanceMarkerCommand::DimensionChangeStart {
                dimension: "minecraft:the_nether".to_owned(),
            },
        ),
        (
            "dimension-ready",
            Some("minecraft:the_nether"),
            InitialLinuxPerformanceMarkerCommand::DimensionReady {
                dimension: "minecraft:the_nether".to_owned(),
            },
        ),
        (
            "process-exit",
            None,
            InitialLinuxPerformanceMarkerCommand::ProcessExit,
        ),
        ("abort", None, InitialLinuxPerformanceMarkerCommand::Abort),
    ] {
        assert_eq!(
            parse_initial_linux_performance_marker_command(kind, value)
                .expect("supported combined Linux marker should parse"),
            expected
        );
    }

    for value in [None, Some("0"), Some("not-a-pid")] {
        assert!(matches!(
            parse_initial_linux_performance_marker_command("process-start", value),
            Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
        ));
    }
    for (kind, value) in [
        ("measurement-start", None),
        ("route-step-start", Some("unexpected")),
        ("measurement-end", None),
        ("world-ready", None),
        ("world-ready", Some("")),
        ("dimension-change-start", None),
        ("dimension-change-start", Some("")),
        ("dimension-ready", None),
        ("dimension-ready", Some("")),
        ("game-ready", Some("unexpected")),
        ("world-load-start", Some("unexpected")),
        ("warm-up-start", Some("unexpected")),
        ("process-exit", Some("unexpected")),
        ("abort", Some("unexpected")),
        ("unknown", None),
    ] {
        assert!(matches!(
            parse_initial_linux_performance_marker_command(kind, value),
            Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
        ));
    }
}

#[test]
fn combined_linux_recorder_gives_up_on_a_client_that_never_writes() {
    let root = temporary_root("record-markers-linux-silent-client");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let socket = root.join("markers.sock");
    let markers_csv = root.join("markers.csv");
    let memory_csv = root.join("linux-memory.csv");
    let server_socket = socket.clone();
    let server_markers = markers_csv.clone();
    let server_memory = memory_csv.clone();
    let server_proc = proc_root.clone();
    let server = thread::spawn(move || {
        record_initial_linux_performance(
            &server_socket,
            &server_markers,
            &server_memory,
            &server_proc,
            4096,
            Duration::from_secs(1),
        )
    });

    wait_for_socket(&socket);
    let _silent_client = UnixStream::connect(&socket).expect("silent client should connect");

    let error = server
        .join()
        .expect("combined server thread should join")
        .expect_err("a client that says nothing must not hold the recorder open");
    assert!(error.to_string().contains("did not arrive"), "{error}");
    assert!(!socket.exists());
    assert!(!markers_csv.exists());
    assert!(!memory_csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn combined_linux_recorder_cleans_partial_memory_after_marker_order_failure() {
    let root = temporary_root("record-markers-linux-memory");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let socket = root.join("markers.sock");
    let markers_csv = root.join("markers.csv");
    let memory_csv = root.join("linux-memory.csv");
    let server_socket = socket.clone();
    let server_markers = markers_csv.clone();
    let server_memory = memory_csv.clone();
    let server_proc = proc_root.clone();
    let server = thread::spawn(move || {
        record_initial_linux_performance(
            &server_socket,
            &server_markers,
            &server_memory,
            &server_proc,
            4096,
            Duration::from_secs(1),
        )
    });

    wait_for_socket(&socket);
    assert!(
        !send_initial_linux_performance_marker(
            &socket,
            &InitialLinuxPerformanceMarkerCommand::ProcessStart { pid: 42 }
        )
        .expect("process start should initialize memory sampling")
    );
    assert!(memory_csv.exists(), "memory output should be staged");

    assert!(matches!(
        send_initial_linux_performance_marker(
            &socket,
            &InitialLinuxPerformanceMarkerCommand::ProcessExit
        ),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(matches!(
        server.join().expect("combined server thread should join"),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(!socket.exists());
    assert!(!markers_csv.exists());
    assert!(!memory_csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn combined_linux_abort_removes_partial_outputs() {
    let root = temporary_root("record-markers-linux-abort");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let socket = root.join("markers.sock");
    let markers_csv = root.join("markers.csv");
    let memory_csv = root.join("linux-memory.csv");
    let server_socket = socket.clone();
    let server_markers = markers_csv.clone();
    let server_memory = memory_csv.clone();
    let server_proc = proc_root.clone();
    let server = thread::spawn(move || {
        record_initial_linux_performance(
            &server_socket,
            &server_markers,
            &server_memory,
            &server_proc,
            4096,
            Duration::from_secs(1),
        )
    });

    wait_for_socket(&socket);
    assert!(
        !send_initial_linux_performance_marker(
            &socket,
            &InitialLinuxPerformanceMarkerCommand::ProcessStart { pid: 42 }
        )
        .expect("process start should attach memory sampling")
    );
    assert!(
        send_initial_linux_performance_marker(
            &socket,
            &InitialLinuxPerformanceMarkerCommand::Abort
        )
        .is_err()
    );
    assert!(
        server
            .join()
            .expect("combined server thread should join")
            .is_err()
    );
    assert!(!socket.exists());
    assert!(!markers_csv.exists());
    assert!(!memory_csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn combined_linux_recorder_rejects_aliased_existing_and_missing_inputs() {
    let root = temporary_root("record-markers-linux-preflight");
    let socket = root.join("markers.sock");
    let markers_csv = root.join("markers.csv");
    let memory_csv = root.join("linux-memory.csv");

    assert!(matches!(
        record_initial_linux_performance(
            &socket,
            &markers_csv,
            &markers_csv,
            &root.join("proc"),
            4096,
            Duration::from_secs(1)
        ),
        Err(PerformanceError::Io(_))
    ));
    assert!(!socket.exists());

    fs::write(&markers_csv, "preserve markers").expect("existing markers should be writable");
    assert!(matches!(
        record_initial_linux_performance(
            &socket,
            &markers_csv,
            &memory_csv,
            &root.join("proc"),
            4096,
            Duration::from_secs(1)
        ),
        Err(PerformanceError::Io(_))
    ));
    assert_eq!(
        fs::read_to_string(&markers_csv).expect("existing markers should remain readable"),
        "preserve markers"
    );
    fs::remove_file(&markers_csv).expect("existing marker test file should be removable");

    fs::write(&memory_csv, "preserve memory").expect("existing memory should be writable");
    assert!(matches!(
        record_initial_linux_performance(
            &socket,
            &markers_csv,
            &memory_csv,
            &root.join("proc"),
            4096,
            Duration::from_secs(1)
        ),
        Err(PerformanceError::Io(_))
    ));
    assert_eq!(
        fs::read_to_string(&memory_csv).expect("existing memory should remain readable"),
        "preserve memory"
    );
    fs::remove_file(&memory_csv).expect("existing memory test file should be removable");

    let server_socket = socket.clone();
    let server_markers = markers_csv.clone();
    let server_memory = memory_csv.clone();
    let missing_proc = root.join("missing-proc");
    let server = thread::spawn(move || {
        record_initial_linux_performance(
            &server_socket,
            &server_markers,
            &server_memory,
            &missing_proc,
            4096,
            Duration::from_secs(1),
        )
    });
    wait_for_socket(&socket);
    assert!(
        send_initial_linux_performance_marker(
            &socket,
            &InitialLinuxPerformanceMarkerCommand::ProcessStart { pid: 42 }
        )
        .is_err()
    );
    assert!(
        server
            .join()
            .expect("combined server thread should join")
            .is_err()
    );
    assert!(!socket.exists());
    assert!(!markers_csv.exists());
    assert!(!memory_csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[cfg(target_os = "linux")]
#[test]
fn command_line_combines_linux_memory_and_markers_without_desktop_input() {
    let root = temporary_root("record-markers-linux-cli");
    let socket = root.join("markers.sock");
    let markers_csv = root.join("markers.csv");
    let memory_csv = root.join("linux-memory.csv");
    let mut server = ravel_cli_command()
        .args([
            "performance",
            "record-initial-linux",
            socket.to_str().expect("test path should be UTF-8"),
            markers_csv.to_str().expect("test path should be UTF-8"),
            memory_csv.to_str().expect("test path should be UTF-8"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("combined Linux recorder should start");

    for _ in 0..1_000 {
        if socket.exists()
            || server
                .try_wait()
                .expect("server status should be readable")
                .is_some()
        {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        socket.exists(),
        "combined recorder socket should become ready"
    );

    let process_start = ravel_cli_command()
        .args([
            "performance",
            "mark-initial-linux",
            socket.to_str().expect("test path should be UTF-8"),
            "process-start",
            &std::process::id().to_string(),
        ])
        .output()
        .expect("combined marker client should execute");
    assert!(process_start.status.success());
    assert_eq!(
        String::from_utf8_lossy(&process_start.stdout),
        "Recorded combined Linux lifecycle marker.\n"
    );

    let invalid_exit = ravel_cli_command()
        .args([
            "performance",
            "mark-initial-linux",
            socket.to_str().expect("test path should be UTF-8"),
            "process-exit",
        ])
        .output()
        .expect("combined marker client should report invalid order");
    let server_output = server
        .wait_with_output()
        .expect("combined recorder should stop after invalid order");

    assert_eq!(invalid_exit.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&invalid_exit.stderr).contains("expected GameReady"));
    assert_eq!(server_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&server_output.stderr).contains("expected GameReady"));
    assert!(!socket.exists());
    assert!(!markers_csv.exists());
    assert!(!memory_csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}
