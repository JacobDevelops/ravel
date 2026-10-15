use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::time::Duration;

use super::{
    ALIGNMENT_TOLERANCE_NS, CAPTURE_DEADLINE_NS, InitialPerformanceMarkerCommand,
    InitialPerformanceMarkerRecorder, MarkerSocketGuard, SECOND_NS, TERMINAL_MARKER_PRIORITY_NS,
    acknowledge_completion, finish_combined_linux_capture, invalid_marker, refuse_request,
    sample_combined_linux_memory, sample_combined_linux_memory_for_command,
    should_defer_socket_accept, should_reserve_terminal_marker_priority, socket_identity,
    validate_capture_deadline,
};
use crate::error::PerformanceError;
use crate::linux_memory::LinuxMemoryCsvRecorder;

#[test]
fn a_recorder_that_outlives_its_profile_gives_up_and_says_what_it_waited_for() {
    let recorder = InitialPerformanceMarkerRecorder::new();

    validate_capture_deadline(&recorder, CAPTURE_DEADLINE_NS)
        .expect("a capture inside its deadline should continue");

    let error = validate_capture_deadline(&recorder, CAPTURE_DEADLINE_NS + 1)
        .expect_err("a capture past its deadline should end");
    let message = error.to_string();
    assert!(message.contains("gave up"), "{message}");
    assert!(message.contains("event 0 (`process-start`)"), "{message}");
}

#[test]
fn a_request_the_recorder_could_not_read_ends_the_capture_and_a_bad_one_does_not() {
    refuse_request(invalid_marker("unsupported event kind"))
        .expect("a protocol violation should leave the recorder serving");

    assert!(matches!(
        refuse_request(PerformanceError::Io("request did not arrive".to_owned())),
        Err(PerformanceError::Io(_))
    ));
}

#[test]
fn socket_accepts_reserve_the_alignment_window_for_scheduled_markers() {
    let mut recorder = InitialPerformanceMarkerRecorder::new();
    for (command, elapsed_ns) in [
        (InitialPerformanceMarkerCommand::ProcessStart, 0),
        (InitialPerformanceMarkerCommand::GameReady, SECOND_NS),
        (
            InitialPerformanceMarkerCommand::WorldLoadStart,
            2 * SECOND_NS,
        ),
        (
            InitialPerformanceMarkerCommand::WorldReady {
                dimension: "minecraft:overworld".to_owned(),
            },
            3 * SECOND_NS,
        ),
        (InitialPerformanceMarkerCommand::WarmUpStart, 4 * SECOND_NS),
    ] {
        recorder
            .record(command, elapsed_ns)
            .expect("pre-measurement marker should record");
    }

    let measurement_start_ns = 304 * SECOND_NS;
    assert!(
        !should_defer_socket_accept(&recorder, measurement_start_ns - ALIGNMENT_TOLERANCE_NS - 1)
            .expect("deadline lookup should succeed")
    );
    assert!(
        should_defer_socket_accept(&recorder, measurement_start_ns - ALIGNMENT_TOLERANCE_NS)
            .expect("deadline lookup should succeed")
    );
    assert!(
        should_defer_socket_accept(&recorder, measurement_start_ns - 1)
            .expect("deadline lookup should succeed")
    );
}

#[test]
fn socket_guard_preserves_a_replacement_path() {
    let root = std::env::temp_dir().join(format!(
        "ravel-marker-guard-replacement-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale test root should be removable");
    }
    fs::create_dir_all(&root).expect("test root should be creatable");
    let socket = root.join("markers.sock");
    let listener = UnixListener::bind(&socket).expect("test socket should bind");
    let guard = MarkerSocketGuard {
        path: &socket,
        identity: socket_identity(&socket).expect("socket identity should be readable"),
    };

    fs::remove_file(&socket).expect("bound socket path should be replaceable");
    fs::write(&socket, "replacement").expect("replacement path should be writable");
    drop(guard);

    assert_eq!(
        fs::read_to_string(&socket).expect("replacement should survive guard cleanup"),
        "replacement"
    );
    drop(listener);
    fs::remove_dir_all(root).expect("test root should be removable");
}

#[test]
fn combined_capture_finishes_both_private_outputs_without_waiting_on_wall_clock() {
    let root = std::env::temp_dir().join(format!("ravel-combined-finish-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale test root should be removable");
    }
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let memory_csv = root.join("linux-memory.csv");
    let marker_csv = root.join("markers.csv");
    let mut memory = Some(
        LinuxMemoryCsvRecorder::new(
            &proc_root,
            42,
            4096,
            Duration::from_millis(10),
            0,
            &memory_csv,
        )
        .expect("memory recorder should initialize"),
    );
    assert!(
        memory
            .as_mut()
            .expect("memory recorder should remain attached")
            .sample_due_at(10_000_000, ALIGNMENT_TOLERANCE_NS)
            .expect("exact shared-clock deadline should sample")
    );

    let markers = complete_recorder();
    let counts = finish_combined_linux_capture(&markers, &mut memory, &marker_csv)
        .expect("combined outputs should finish");
    assert_eq!(counts.lifecycle_markers, 17);
    assert_eq!(counts.linux_memory_samples, 1);
    assert!(memory.is_none());
    for output in [&marker_csv, &memory_csv] {
        assert_eq!(
            fs::metadata(output)
                .expect("combined output metadata should be readable")
                .mode()
                & 0o777,
            0o600
        );
    }

    fs::remove_dir_all(root).expect("test root should be removable");
}

#[test]
fn terminal_marker_priority_precedes_a_due_memory_sample() {
    let root = std::env::temp_dir().join(format!("ravel-terminal-priority-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale test root should be removable");
    }
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let memory_csv = root.join("linux-memory.csv");
    let mut memory = LinuxMemoryCsvRecorder::new(
        &proc_root,
        42,
        4096,
        Duration::from_millis(10),
        0,
        &memory_csv,
    )
    .expect("memory recorder should initialize");

    assert!(should_reserve_terminal_marker_priority(
        Some(&memory),
        10_000_000
    ));
    assert!(should_reserve_terminal_marker_priority(
        Some(&memory),
        10_000_000 + TERMINAL_MARKER_PRIORITY_NS - 1
    ));
    assert!(!should_reserve_terminal_marker_priority(
        Some(&memory),
        10_000_000 + TERMINAL_MARKER_PRIORITY_NS
    ));

    fs::remove_dir_all(proc_root.join("42")).expect("synthetic process should be removable");
    assert!(
        sample_combined_linux_memory(Some(&mut memory), 10_000_000).is_err(),
        "sampling after process exit would fail without terminal priority"
    );
    assert!(
        !sample_combined_linux_memory_for_command(
            &InitialPerformanceMarkerCommand::ProcessExit,
            Some(&mut memory),
            10_000_000,
        )
        .expect("a pending terminal marker must bypass the due sample")
    );
    drop(memory);
    assert!(
        !memory_csv.exists(),
        "failed partial output should be removed"
    );
    fs::remove_dir_all(root).expect("test root should be removable");
}

#[test]
fn finalized_capture_ignores_a_disconnected_completion_client() {
    let (mut server, client) =
        std::os::unix::net::UnixStream::pair().expect("socket pair should open");
    drop(client);

    acknowledge_completion(&mut server);
}

fn complete_recorder() -> InitialPerformanceMarkerRecorder {
    let mut recorder = InitialPerformanceMarkerRecorder::new();
    for (command, elapsed_ns) in [
        (InitialPerformanceMarkerCommand::ProcessStart, 0),
        (InitialPerformanceMarkerCommand::GameReady, SECOND_NS),
        (
            InitialPerformanceMarkerCommand::WorldLoadStart,
            2 * SECOND_NS,
        ),
        (
            InitialPerformanceMarkerCommand::WorldReady {
                dimension: "minecraft:overworld".to_owned(),
            },
            3 * SECOND_NS,
        ),
        (InitialPerformanceMarkerCommand::WarmUpStart, 4 * SECOND_NS),
    ] {
        recorder
            .record(command, elapsed_ns)
            .expect("pre-measurement marker should record");
    }
    for elapsed_ns in [304, 484, 724] {
        recorder
            .advance_to(elapsed_ns * SECOND_NS)
            .expect("scheduled outbound route should record");
    }
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionChangeStart {
                dimension: "minecraft:the_nether".to_owned(),
            },
            744 * SECOND_NS,
        )
        .expect("outbound dimension start should record");
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionReady {
                dimension: "minecraft:the_nether".to_owned(),
            },
            764 * SECOND_NS,
        )
        .expect("nether ready should record");
    for elapsed_ns in [844, 1_024] {
        recorder
            .advance_to(elapsed_ns * SECOND_NS)
            .expect("scheduled return route should record");
    }
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionChangeStart {
                dimension: "minecraft:overworld".to_owned(),
            },
            1_064 * SECOND_NS,
        )
        .expect("return dimension start should record");
    recorder
        .record(
            InitialPerformanceMarkerCommand::DimensionReady {
                dimension: "minecraft:overworld".to_owned(),
            },
            1_084 * SECOND_NS,
        )
        .expect("overworld ready should record");
    recorder
        .advance_to(1_204 * SECOND_NS)
        .expect("measurement end should record");
    assert!(
        recorder
            .record(
                InitialPerformanceMarkerCommand::ProcessExit,
                1_324 * SECOND_NS
            )
            .expect("process exit should complete markers")
    );
    recorder
}

fn write_proc_snapshot(proc_root: &Path, pid: u32) {
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
