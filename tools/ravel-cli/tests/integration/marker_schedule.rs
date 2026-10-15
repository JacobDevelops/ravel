use std::fs;
use std::io::Cursor;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use ravel_cli::{
    InitialPerformanceMarkerCommand, InitialPerformanceMarkerRecorder, PerformanceError,
    PerformanceEventKind, PerformancePhase, parse_initial_performance_marker_command,
    parse_initial_performance_marker_csv, send_initial_performance_marker,
};

use super::common::{ravel_cli_command, temporary_root};

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
    assert_eq!(output.stderr, [] as [u8; 0]);

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
