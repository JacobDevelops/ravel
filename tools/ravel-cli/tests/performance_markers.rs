use std::fs;
use std::io::{Cursor, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use ravel_cli::{
    InitialLinuxPerformanceMarkerCommand, InitialPerformanceMarkerCommand,
    InitialPerformanceMarkerRecorder, PerformanceError, PerformanceEventKind, PerformancePhase,
    parse_initial_linux_performance_marker_command, parse_initial_performance_marker_command,
    parse_initial_performance_marker_csv, record_initial_linux_performance,
    record_initial_performance_markers, send_initial_linux_performance_marker,
    send_initial_performance_marker,
};

const HEADER: &str = "event_index,elapsed_ns,kind,route_step,dimension\n";

#[test]
fn parses_initial_v2_markers_and_assigns_every_phase() {
    let markers = parse_initial_performance_marker_csv(Cursor::new(canonical_markers()))
        .expect("canonical marker stream should parse");

    assert_eq!(markers.events().len(), 17);
    assert_eq!(
        markers.events()[8].kind,
        PerformanceEventKind::RouteStepStart
    );
    assert_eq!(
        markers.events()[8].route_step.as_deref(),
        Some("outbound-dimension-change")
    );
    assert_eq!(
        markers.events()[8].dimension.as_deref(),
        Some("minecraft:the_nether")
    );

    for (elapsed_ns, expected) in [
        (0, PerformancePhase::Startup),
        (19_999_999_999, PerformancePhase::Startup),
        (20_000_000_000, PerformancePhase::WorldLoad),
        (39_999_999_999, PerformancePhase::WorldLoad),
        (40_000_000_000, PerformancePhase::WarmUp),
        (339_999_999_999, PerformancePhase::WarmUp),
        (340_000_000_000, PerformancePhase::SteadyState),
        (759_999_999_999, PerformancePhase::SteadyState),
        (760_000_000_000, PerformancePhase::Transient),
        (880_000_000_000, PerformancePhase::SteadyState),
        (1_060_000_000_000, PerformancePhase::Transient),
        (1_240_000_000_000, PerformancePhase::CoolDown),
        (1_359_999_999_999, PerformancePhase::CoolDown),
    ] {
        assert_eq!(
            markers
                .phase_at(elapsed_ns)
                .expect("sample should fall inside the process lifetime"),
            expected
        );
    }

    for outside in [1_360_000_000_000, u64::MAX] {
        assert!(matches!(
            markers.phase_at(outside),
            Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
        ));
    }
}

#[test]
fn rejects_marker_csv_shape_index_and_clock_drift() {
    for invalid in [
        canonical_markers().replacen(HEADER, "kind,elapsed_ns,route_step,dimension\n", 1),
        canonical_markers().replacen(
            "1,10000000000,game-ready,,",
            "2,10000000000,game-ready,,",
            1,
        ),
        canonical_markers().replacen(
            "2,20000000000,world-load-start,,",
            "2,9000000000,world-load-start,,",
            1,
        ),
        canonical_markers().replacen(
            "1,10000000000,game-ready,,",
            "1,10000000000,unknown-ready,,",
            1,
        ),
    ] {
        assert!(matches!(
            parse_initial_performance_marker_csv(Cursor::new(invalid)),
            Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
        ));
    }
}

#[test]
fn rejects_marker_metadata_route_and_duration_drift() {
    for invalid in [
        canonical_markers().replacen(
            "1,10000000000,game-ready,,",
            "1,10000000000,game-ready,unexpected,",
            1,
        ),
        canonical_markers().replacen(
            "3,30000000000,world-ready,,minecraft:overworld",
            "3,30000000000,world-ready,,",
            1,
        ),
        canonical_markers().replacen(
            "7,520000000000,route-step-start,overworld-traverse,minecraft:overworld",
            "7,520000000000,route-step-start,wrong-step,minecraft:overworld",
            1,
        ),
        canonical_markers().replacen(
            "9,780000000000,dimension-change-start,,minecraft:the_nether",
            "9,780000000000,dimension-change-start,,minecraft:overworld",
            1,
        ),
        canonical_markers().replacen(
            "7,520000000000,route-step-start,overworld-traverse,minecraft:overworld",
            "7,521000000000,route-step-start,overworld-traverse,minecraft:overworld",
            1,
        ),
        canonical_markers().replacen(
            "16,1360000000000,process-exit,,",
            "16,1359000000000,process-exit,,",
            1,
        ),
    ] {
        assert!(matches!(
            parse_initial_performance_marker_csv(Cursor::new(invalid)),
            Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
        ));
    }
}

#[test]
fn rejects_non_atomic_first_route_and_marker_count_or_order_drift() {
    for invalid in [
        canonical_markers().replacen(
            "6,340000000000,route-step-start,base-observation,minecraft:overworld",
            "6,340001000000,route-step-start,base-observation,minecraft:overworld",
            1,
        ),
        canonical_markers().replacen("16,1360000000000,process-exit,,\n", "", 1),
        canonical_markers() + "17,1370000000000,process-exit,,\n",
        canonical_markers()
            .replacen(
                "1,10000000000,game-ready,,",
                "1,10000000000,world-load-start,,",
                1,
            )
            .replacen(
                "2,20000000000,world-load-start,,",
                "2,20000000000,game-ready,,",
                1,
            ),
    ] {
        assert!(matches!(
            parse_initial_performance_marker_csv(Cursor::new(invalid)),
            Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
        ));
    }
}

#[test]
fn command_line_verifies_markers_without_desktop_input() {
    let root = temporary_root("verify-markers");
    let csv = root.join("markers.csv");
    fs::write(&csv, canonical_markers()).expect("marker CSV should be writable");

    let output = ravel_cli_command()
        .args([
            "performance",
            "verify-initial-markers",
            csv.to_str().expect("test path should be UTF-8"),
        ])
        .output()
        .expect("ravel-cli should execute");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Verified 17 initial-v2 lifecycle markers.\n"
    );
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn recorder_owns_the_clock_and_emits_the_fixed_route_schedule() {
    let mut recorder = InitialPerformanceMarkerRecorder::new();
    recorder
        .record(InitialPerformanceMarkerCommand::ProcessStart, 0)
        .expect("process start should record");
    recorder
        .record(InitialPerformanceMarkerCommand::GameReady, 10_000_000_000)
        .expect("game ready should record");
    recorder
        .record(
            InitialPerformanceMarkerCommand::WorldLoadStart,
            20_000_000_000,
        )
        .expect("world load should record");
    recorder
        .record(
            InitialPerformanceMarkerCommand::WorldReady {
                dimension: "minecraft:overworld".to_owned(),
            },
            30_000_000_000,
        )
        .expect("world ready should record");
    recorder
        .record(InitialPerformanceMarkerCommand::WarmUpStart, 40_000_000_000)
        .expect("warm-up should record");

    for elapsed_ns in [340_000_000_000, 520_000_000_000, 760_000_000_000] {
        recorder
            .advance_to(elapsed_ns)
            .expect("scheduled route boundary should record");
    }
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionChangeStart {
                dimension: "minecraft:the_nether".to_owned(),
            },
            780_000_000_000,
        )
        .expect("outbound dimension change should record");
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionReady {
                dimension: "minecraft:the_nether".to_owned(),
            },
            800_000_000_000,
        )
        .expect("nether ready should record");
    recorder
        .advance_to(880_000_000_000)
        .expect("dimension route should record");
    recorder
        .advance_to(1_060_000_000_000)
        .expect("return route should record");
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionChangeStart {
                dimension: "minecraft:overworld".to_owned(),
            },
            1_100_000_000_000,
        )
        .expect("return dimension change should record");
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionReady {
                dimension: "minecraft:overworld".to_owned(),
            },
            1_120_000_000_000,
        )
        .expect("overworld ready should record");
    recorder
        .advance_to(1_240_000_000_000)
        .expect("measurement end should record");
    let complete = recorder
        .record(
            InitialPerformanceMarkerCommand::ProcessExit,
            1_360_000_000_000,
        )
        .expect("process exit should complete the stream");

    assert!(complete);
    assert_eq!(recorder.events().len(), 17);
    assert_eq!(recorder.events()[5].elapsed_ns, 340_000_000_000);
    assert_eq!(recorder.events()[6].elapsed_ns, 340_000_000_000);
    assert_eq!(recorder.events()[15].elapsed_ns, 1_240_000_000_000);

    assert_private_marker_csv(&recorder);
}

#[test]
fn recorder_rejects_a_missed_dimension_boundary() {
    let mut recorder = recorder_through_warm_up();
    let root = temporary_root("incomplete-recorded-markers");
    let csv = root.join("markers.csv");
    assert!(matches!(
        recorder.write_csv(&csv),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(!csv.exists());

    recorder
        .advance_to(760_000_000_000)
        .expect("first three route steps should be scheduled");

    assert!(matches!(
        recorder.advance_to(880_000_000_000),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn rejects_operator_scheduled_events_metadata_drift_and_wire_injection() {
    for (kind, dimension) in [
        ("measurement-start", None),
        ("route-step-start", Some("minecraft:overworld")),
        ("world-ready", None),
        ("process-start", Some("minecraft:overworld")),
        ("unknown", None),
    ] {
        assert!(matches!(
            parse_initial_performance_marker_command(kind, dimension),
            Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
        ));
    }

    let injected = InitialPerformanceMarkerCommand::WorldReady {
        dimension: "minecraft:overworld\nprocess-exit".to_owned(),
    };
    assert!(matches!(
        send_initial_performance_marker(
            PathBuf::from("/missing/ravel-markers.sock").as_path(),
            &injected
        ),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
}

#[test]
fn parses_every_operator_marker_command() {
    for (kind, dimension, expected) in [
        (
            "process-start",
            None,
            InitialPerformanceMarkerCommand::ProcessStart,
        ),
        (
            "game-ready",
            None,
            InitialPerformanceMarkerCommand::GameReady,
        ),
        (
            "world-load-start",
            None,
            InitialPerformanceMarkerCommand::WorldLoadStart,
        ),
        (
            "world-ready",
            Some("minecraft:overworld"),
            InitialPerformanceMarkerCommand::WorldReady {
                dimension: "minecraft:overworld".to_owned(),
            },
        ),
        (
            "warm-up-start",
            None,
            InitialPerformanceMarkerCommand::WarmUpStart,
        ),
        (
            "dimension-change-start",
            Some("minecraft:the_nether"),
            InitialPerformanceMarkerCommand::DimensionChangeStart {
                dimension: "minecraft:the_nether".to_owned(),
            },
        ),
        (
            "dimension-ready",
            Some("minecraft:the_nether"),
            InitialPerformanceMarkerCommand::DimensionReady {
                dimension: "minecraft:the_nether".to_owned(),
            },
        ),
        (
            "process-exit",
            None,
            InitialPerformanceMarkerCommand::ProcessExit,
        ),
    ] {
        assert_eq!(
            parse_initial_performance_marker_command(kind, dimension)
                .expect("supported operator marker should parse"),
            expected
        );
    }
}

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
fn recorder_rejects_clock_regression() {
    let mut recorder = InitialPerformanceMarkerRecorder::new();
    recorder
        .advance_to(10)
        .expect("the first clock observation should succeed");

    assert!(matches!(
        recorder.advance_to(9),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
}

#[test]
fn socket_recorder_never_overwrites_existing_paths() {
    let root = temporary_root("record-markers-existing-paths");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");

    fs::write(&csv, "preserve this evidence").expect("existing CSV should be writable");
    assert!(matches!(
        record_initial_performance_markers(&socket, &csv),
        Err(PerformanceError::Io(_))
    ));
    assert!(!socket.exists());
    assert_eq!(
        fs::read_to_string(&csv).expect("existing CSV should remain readable"),
        "preserve this evidence"
    );

    fs::remove_file(&csv).expect("test CSV should be removable");
    fs::write(&socket, "preserve this endpoint").expect("existing socket path should be writable");
    assert!(matches!(
        record_initial_performance_markers(&socket, &csv),
        Err(PerformanceError::Io(_))
    ));
    assert!(!csv.exists());
    assert_eq!(
        fs::read_to_string(&socket).expect("existing socket path should remain readable"),
        "preserve this endpoint"
    );

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn socket_recorder_requires_an_owner_only_parent() {
    let root = temporary_root("record-markers-public-parent");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755))
        .expect("test parent permissions should be writable");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");

    let error = record_initial_performance_markers(&socket, &csv)
        .expect_err("a group-readable socket parent must be rejected");
    assert!(error.to_string().contains("owner-only"));
    assert!(!socket.exists());
    assert!(!csv.exists());

    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .expect("test parent permissions should be restorable");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn socket_recorder_rejects_invalid_order_and_removes_private_endpoints() {
    let root = temporary_root("record-markers");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");
    let server_socket = socket.clone();
    let server_csv = csv.clone();
    let server =
        thread::spawn(move || record_initial_performance_markers(&server_socket, &server_csv));

    for _ in 0..1_000 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(socket.exists(), "recorder socket should become ready");
    let socket_metadata =
        fs::metadata(&socket).expect("recorder socket metadata should be readable");
    assert_eq!(socket_metadata.mode() & 0o777, 0o600);

    assert!(matches!(
        send_initial_performance_marker(&socket, &InitialPerformanceMarkerCommand::GameReady),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(matches!(
        server.join().expect("server thread should join"),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(!socket.exists());
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn socket_recorder_survives_bad_wire_requests_and_reads_one_line() {
    let root = temporary_root("record-markers-wire");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");
    let server_socket = socket.clone();
    let server_csv = csv.clone();
    let server =
        thread::spawn(move || record_initial_performance_markers(&server_socket, &server_csv));

    for _ in 0..1_000 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(socket.exists(), "recorder socket should become ready");

    let mut bad_client = UnixStream::connect(&socket).expect("bad client should connect");
    bad_client
        .write_all(b"not-a-marker,\n")
        .expect("bad request should write");
    bad_client
        .shutdown(std::net::Shutdown::Write)
        .expect("bad client should close writes");
    let mut bad_response = String::new();
    bad_client
        .read_to_string(&mut bad_response)
        .expect("bad response should read");
    assert!(bad_response.starts_with("error:"));
    assert!(socket.exists(), "recorder should keep listening");

    let mut open_client = UnixStream::connect(&socket).expect("open client should connect");
    open_client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("open client timeout should configure");
    open_client
        .write_all(b"process-start,\n")
        .expect("newline-terminated request should write");
    let mut open_response = [0_u8; 3];
    open_client
        .read_exact(&mut open_response)
        .expect("recorder should answer without waiting for EOF");
    assert_eq!(&open_response, b"ok\n");

    assert!(matches!(
        send_initial_performance_marker(&socket, &InitialPerformanceMarkerCommand::ProcessExit),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(matches!(
        server.join().expect("server thread should join"),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(!socket.exists());
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

/// The endpoint is published already protected, so its private staging name is
/// as reserved as the socket path itself.
#[test]
fn socket_recorder_refuses_an_endpoint_another_bind_is_holding() {
    let root = temporary_root("record-markers-staging");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");
    let staging = socket.with_extension("pending");
    fs::write(&staging, "another bind holds this").expect("staging path should be writable");

    assert!(matches!(
        record_initial_performance_markers(&socket, &csv),
        Err(PerformanceError::Io(_))
    ));
    assert_eq!(
        fs::read_to_string(&staging).expect("staging path should remain readable"),
        "another bind holds this"
    );
    assert!(!socket.exists());
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

/// A connected client that says nothing must end the recorder, not outlast it.
///
/// The recorder answers the silent peer and returns, rather than writing
/// `error:` and polling on for a command that is never coming.
#[test]
fn socket_recorder_gives_up_on_a_client_that_never_writes() {
    let root = temporary_root("record-markers-silent-client");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");
    let server_socket = socket.clone();
    let server_csv = csv.clone();
    let server =
        thread::spawn(move || record_initial_performance_markers(&server_socket, &server_csv));

    wait_for_socket(&socket);
    let mut silent_client = UnixStream::connect(&socket).expect("silent client should connect");

    let error = server
        .join()
        .expect("server thread should join")
        .expect_err("a client that says nothing must not hold the recorder open");
    assert!(error.to_string().contains("did not arrive"), "{error}");

    let mut response = String::new();
    silent_client
        .read_to_string(&mut response)
        .expect("silent client should read its refusal");
    assert!(response.starts_with("error:"), "{response}");
    assert!(!socket.exists());
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

/// A request that arrives late is still a request.
///
/// The client here connects and then stalls far longer than one read timeout,
/// which is what a descheduled process does under load. The recorder retries
/// the read rather than refusing a command it has not seen yet.
#[test]
fn socket_recorder_answers_a_client_that_writes_late() {
    let root = temporary_root("record-markers-late-client");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");
    let server_socket = socket.clone();
    let server_csv = csv.clone();
    let server =
        thread::spawn(move || record_initial_performance_markers(&server_socket, &server_csv));

    wait_for_socket(&socket);
    let mut late_client = UnixStream::connect(&socket).expect("late client should connect");
    late_client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("late client timeout should configure");
    thread::sleep(Duration::from_millis(50));
    late_client
        .write_all(b"process-start,\n")
        .expect("late request should write");
    let mut response = [0_u8; 3];
    late_client
        .read_exact(&mut response)
        .expect("a late request should still be answered");
    assert_eq!(&response, b"ok\n");

    assert!(matches!(
        send_initial_performance_marker(&socket, &InitialPerformanceMarkerCommand::ProcessExit),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(matches!(
        server.join().expect("server thread should join"),
        Err(PerformanceError::InvalidPerformanceMarkerCsv(_))
    ));
    assert!(!socket.exists());
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

/// The combined recorder is the one the gate strands, so it gets the same proof.
///
/// A capture that answered `error:` and kept polling held its thread until
/// nextest killed the run at two minutes. It now returns, and takes its
/// endpoint with it.
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

    for _ in 0..1_000 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        socket.exists(),
        "combined recorder socket should become ready"
    );
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
    for _ in 0..1_000 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        socket.exists(),
        "combined recorder socket should become ready"
    );
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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
    assert!(output.stderr.is_empty());
    server.join().expect("fake recorder should join");

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn command_line_records_markers_over_a_focus_free_socket() {
    let root = temporary_root("record-markers-cli");
    let socket = root.join("markers.sock");
    let csv = root.join("markers.csv");
    let mut server = ravel_cli_command()
        .args([
            "performance",
            "record-initial-markers",
            socket.to_str().expect("test path should be UTF-8"),
            csv.to_str().expect("test path should be UTF-8"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("marker recorder should start");

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
    assert!(socket.exists(), "recorder socket should become ready");

    let client = ravel_cli_command()
        .args([
            "performance",
            "mark-initial",
            socket.to_str().expect("test path should be UTF-8"),
            "game-ready",
        ])
        .output()
        .expect("marker client should execute");
    let server_output = server
        .wait_with_output()
        .expect("marker recorder should stop after invalid order");

    assert_eq!(client.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&client.stderr).contains("expected ProcessStart"));
    assert_eq!(server_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&server_output.stderr).contains("expected ProcessStart"));
    assert!(!socket.exists());
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn recorder_through_warm_up() -> InitialPerformanceMarkerRecorder {
    let mut recorder = InitialPerformanceMarkerRecorder::new();
    for (command, elapsed_ns) in [
        (InitialPerformanceMarkerCommand::ProcessStart, 0),
        (InitialPerformanceMarkerCommand::GameReady, 10_000_000_000),
        (
            InitialPerformanceMarkerCommand::WorldLoadStart,
            20_000_000_000,
        ),
        (
            InitialPerformanceMarkerCommand::WorldReady {
                dimension: "minecraft:overworld".to_owned(),
            },
            30_000_000_000,
        ),
        (InitialPerformanceMarkerCommand::WarmUpStart, 40_000_000_000),
    ] {
        recorder
            .record(command, elapsed_ns)
            .expect("pre-measurement command should record");
    }
    recorder
}

fn canonical_markers() -> String {
    format!(
        "{HEADER}\
         0,0,process-start,,\n\
         1,10000000000,game-ready,,\n\
         2,20000000000,world-load-start,,\n\
         3,30000000000,world-ready,,minecraft:overworld\n\
         4,40000000000,warm-up-start,,\n\
         5,340000000000,measurement-start,,\n\
         6,340000000000,route-step-start,base-observation,minecraft:overworld\n\
         7,520000000000,route-step-start,overworld-traverse,minecraft:overworld\n\
         8,760000000000,route-step-start,outbound-dimension-change,minecraft:the_nether\n\
         9,780000000000,dimension-change-start,,minecraft:the_nether\n\
         10,800000000000,dimension-ready,,minecraft:the_nether\n\
         11,880000000000,route-step-start,dimension-traverse,minecraft:the_nether\n\
         12,1060000000000,route-step-start,return-and-base-observation,minecraft:overworld\n\
         13,1100000000000,dimension-change-start,,minecraft:overworld\n\
         14,1120000000000,dimension-ready,,minecraft:overworld\n\
         15,1240000000000,measurement-end,,\n\
         16,1360000000000,process-exit,,\n"
    )
}

fn write_proc_snapshot(proc_root: &std::path::Path, pid: u32) {
    fs::create_dir_all(proc_root.join(pid.to_string()))
        .expect("synthetic process directory should be creatable");
    fs::create_dir_all(proc_root.join("pressure"))
        .expect("synthetic pressure directory should be creatable");
    fs::write(
        proc_root.join(pid.to_string()).join("smaps_rollup"),
        "Rss: 3072 kB\n",
    )
    .expect("synthetic smaps should be writable");
    fs::write(
        proc_root.join(pid.to_string()).join("stat"),
        format!("{pid} (java worker) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 100\n"),
    )
    .expect("synthetic process identity should be writable");
    fs::write(
        proc_root.join("meminfo"),
        "MemTotal: 16777216 kB\nMemAvailable: 4194304 kB\n",
    )
    .expect("synthetic meminfo should be writable");
    fs::write(
        proc_root.join("pressure/memory"),
        "some total=100\nfull total=10\n",
    )
    .expect("synthetic PSI should be writable");
    fs::write(proc_root.join("vmstat"), "pswpin 2\npswpout 3\n")
        .expect("synthetic vmstat should be writable");
}

fn assert_private_marker_csv(recorder: &InitialPerformanceMarkerRecorder) {
    let root = temporary_root("write-recorded-markers");
    let csv = root.join("markers.csv");
    assert_eq!(
        recorder
            .write_csv(&csv)
            .expect("complete stream should serialize"),
        17
    );
    assert_eq!(
        parse_initial_performance_marker_csv(Cursor::new(
            fs::read_to_string(&csv).expect("recorded CSV should be readable")
        ))
        .expect("recorded CSV should reparse")
        .events()
        .len(),
        17
    );
    assert_eq!(
        fs::metadata(&csv)
            .expect("recorded CSV metadata should be readable")
            .mode()
            & 0o777,
        0o600
    );
    assert!(recorder.write_csv(&csv).is_err());
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

fn wait_for_socket(socket: &std::path::Path) {
    for _ in 0..1_000 {
        if socket.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("recorder socket should become ready");
}

fn ravel_cli_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ravel-cli"));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        let profile = PathBuf::from(profile);
        let file_name = profile
            .file_name()
            .expect("coverage profile should have a filename")
            .to_string_lossy();
        command.env(
            "LLVM_PROFILE_FILE",
            profile.with_file_name(format!("child-%p-{file_name}")),
        );
    }
    command
}

fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "ravel-cli-performance-{label}-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temporary root should be removable");
    }
    fs::create_dir_all(&root).expect("temporary root should be creatable");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .expect("temporary root should be owner-only");
    root
}
