use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::common::{ravel_cli_command, temporary_root};

use ravel_cli::{
    PerformanceError, TracyFrameMetric, parse_tracy_frame_csv, validate_private_evidence_output,
};

const HEADER: &str = "frame_set,frame_index,start_ns,end_ns,duration_ns\n";

#[test]
fn parses_exact_client_and_server_tick_frames() {
    let csv = format!(
        "{HEADER}\
         Client Tick,0,100,130,30\n\
         Server Tick,0,110,125,15\n\
         Client Tick,1,140,180,40\n\
         Server Tick,1,160,175,15\n"
    );

    let samples = parse_tracy_frame_csv(Cursor::new(csv)).expect("valid Tracy frame CSV");

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].metric, TracyFrameMetric::FrameTime);
    assert_eq!(samples[0].frame_index, 0);
    assert_eq!(samples[0].start_ns, 100);
    assert_eq!(samples[0].duration_ns, 30);
    assert_eq!(
        samples[1].metric,
        TracyFrameMetric::IntegratedServerTickTime
    );
    assert_eq!(samples[3].frame_index, 1);
    assert_eq!(samples[3].start_ns, 160);
    assert_eq!(samples[3].duration_ns, 15);
}

#[test]
fn rejects_a_missing_required_frame_set() {
    let csv = format!("{HEADER}Client Tick,0,100,130,30\n");

    let error = parse_tracy_frame_csv(Cursor::new(csv)).expect_err("server ticks are required");

    assert_eq!(
        error,
        PerformanceError::InvalidTracyFrameCsv(
            "missing required `Server Tick` frame set".to_owned()
        )
    );
}

#[test]
fn rejects_duration_drift() {
    let csv = format!(
        "{HEADER}\
         Client Tick,0,100,130,29\n\
         Server Tick,0,110,125,15\n"
    );

    let error = parse_tracy_frame_csv(Cursor::new(csv)).expect_err("duration must equal end-start");

    assert_eq!(
        error,
        PerformanceError::InvalidTracyFrameCsv(
            "line 2 duration 29 does not equal end-start 30".to_owned()
        )
    );
}

#[test]
fn rejects_non_sequential_frame_indices() {
    let csv = format!(
        "{HEADER}\
         Client Tick,0,100,130,30\n\
         Server Tick,0,110,125,15\n\
         Client Tick,2,140,180,40\n\
         Server Tick,1,160,175,15\n"
    );

    let error =
        parse_tracy_frame_csv(Cursor::new(csv)).expect_err("frame indices must be contiguous");

    assert_eq!(
        error,
        PerformanceError::InvalidTracyFrameCsv(
            "line 4 `Client Tick` frame index 2 does not follow 0".to_owned()
        )
    );
}

#[test]
fn rejects_out_of_order_frame_starts() {
    let csv = format!(
        "{HEADER}\
         Client Tick,0,140,180,40\n\
         Server Tick,0,110,125,15\n\
         Client Tick,1,100,130,30\n\
         Server Tick,1,160,175,15\n"
    );

    let error = parse_tracy_frame_csv(Cursor::new(csv)).expect_err("starts must be monotonic");

    assert_eq!(
        error,
        PerformanceError::InvalidTracyFrameCsv(
            "line 4 `Client Tick` start 100 precedes 140".to_owned()
        )
    );
}

#[test]
fn rejects_unexpected_columns_or_frame_sets() {
    let wrong_header = "frame_set,start_ns,duration_ns\nClient Tick,100,30\n";
    let header_error =
        parse_tracy_frame_csv(Cursor::new(wrong_header)).expect_err("header must be pinned");
    assert_eq!(
        header_error,
        PerformanceError::InvalidTracyFrameCsv(format!(
            "expected header `{}`, observed `frame_set,start_ns,duration_ns`",
            HEADER.trim_end()
        ))
    );

    let unknown = format!(
        "{HEADER}\
         Render Tick,0,100,130,30\n\
         Server Tick,0,110,125,15\n"
    );
    let frame_error =
        parse_tracy_frame_csv(Cursor::new(unknown)).expect_err("only pinned frame sets are valid");
    assert_eq!(
        frame_error,
        PerformanceError::InvalidTracyFrameCsv(
            "line 2 has unsupported frame set `Render Tick`".to_owned()
        )
    );
}

#[test]
fn raw_tracy_output_must_use_an_ignored_workspace_boundary() {
    let root = PathBuf::from("/workspace/ravel");

    for tracked in [
        root.join("capture.tracy"),
        root.join("tracy-frames.csv"),
        PathBuf::from("capture.tracy"),
        PathBuf::from("../capture.tracy"),
    ] {
        let error = validate_private_evidence_output(&root, &tracked)
            .expect_err("tracked or escaping output must fail");
        assert!(matches!(error, PerformanceError::TracyTool(_)));
    }

    for private in [
        root.join("evidence/private/m0/capture.tracy"),
        root.join("evidence/m0/raw/tracy-frames.csv"),
        root.join("evidence/m0/captures/capture.tracy"),
        root.join("tests/fixtures/local/performance/capture.tracy"),
        PathBuf::from("/private/ravel-capture.tracy"),
    ] {
        validate_private_evidence_output(&root, &private)
            .expect("ignored or external output should pass");
    }
}

#[test]
fn command_line_verifies_tracy_frames_without_desktop_input() {
    let root = temporary_root("verify-tracy");
    let csv = root.join("tracy-frames.csv");
    fs::write(
        &csv,
        format!(
            "{HEADER}\
             Client Tick,0,100,130,30\n\
             Server Tick,0,110,125,15\n\
             Client Tick,1,140,180,40\n\
             Server Tick,1,160,175,15\n"
        ),
    )
    .expect("test CSV should be writable");

    let output = ravel_cli_command()
        .args([
            "performance",
            "verify-tracy-frames",
            csv.to_str().expect("test path should be UTF-8"),
        ])
        .output()
        .expect("ravel-cli should execute");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Verified 2 client frames and 2 integrated-server ticks.\n"
    );
    assert_eq!(output.stderr, [] as [u8; 0]);

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn command_line_exports_only_the_pinned_tracy_frame_sets() {
    let root = temporary_root("export-tracy");
    let exporter = root.join("tracy-csvexport");
    fs::write(
        &exporter,
        format!(
            "#!/bin/sh\n\
             set -eu\n\
             test \"$1\" = '--frames'\n\
             test \"$2\" = '--case'\n\
             test \"$3\" = '--filter= Tick'\n\
             test \"$4\" = '{}'\n\
             printf '{}\\n'\n\
             printf 'Client Tick,0,100,130,30\\n'\n\
             printf 'Server Tick,0,110,125,15\\n'\n",
            root.join("capture.tracy").display(),
            HEADER.trim_end()
        ),
    )
    .expect("fake exporter should be writable");
    make_executable(&exporter);

    let trace = root.join("capture.tracy");
    fs::write(&trace, b"synthetic trace").expect("synthetic trace should be writable");
    let csv = root.join("tracy-frames.csv");
    let output = ravel_cli_command()
        .args([
            "performance",
            "export-tracy-frames",
            trace.to_str().expect("test path should be UTF-8"),
            csv.to_str().expect("test path should be UTF-8"),
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.display(),
                std::env::var("PATH").expect("PATH should be set")
            ),
        )
        .output()
        .expect("ravel-cli should execute");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "Exported and verified 1 client frames and 1 integrated-server ticks: {}\n",
            csv.display()
        )
    );
    assert_eq!(output.stderr, [] as [u8; 0]);
    assert_eq!(
        fs::read_to_string(&csv).expect("exported CSV should be readable"),
        format!(
            "{HEADER}\
             Client Tick,0,100,130,30\n\
             Server Tick,0,110,125,15\n"
        )
    );

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn command_line_captures_tracy_without_desktop_input() {
    let root = temporary_root("capture-tracy");
    let collector = root.join("tracy-capture");
    fs::write(
        &collector,
        "#!/bin/sh\n\
         set -eu\n\
         test \"$1\" = '-o'\n\
         test \"$3\" = '-s'\n\
         test \"$4\" = '3'\n\
         printf 'synthetic trace' > \"$2\"\n",
    )
    .expect("fake collector should be writable");
    make_executable(&collector);

    let trace = root.join("capture.tracy");
    let output = ravel_cli_command()
        .args([
            "performance",
            "capture-tracy",
            "3",
            trace.to_str().expect("test path should be UTF-8"),
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.display(),
                std::env::var("PATH").expect("PATH should be set")
            ),
        )
        .output()
        .expect("ravel-cli should execute");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "Captured Tracy trace without desktop input: {}\n",
            trace.display()
        )
    );
    assert_eq!(output.stderr, [] as [u8; 0]);
    assert_eq!(
        fs::read_to_string(&trace).expect("trace should be readable"),
        "synthetic trace"
    );

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn failed_tracy_tools_leave_no_partial_evidence() {
    let root = temporary_root("partial-tracy");
    let exporter = root.join("tracy-csvexport");
    fs::write(
        &exporter,
        format!(
            "#!/bin/sh\n\
             set -eu\n\
             printf '{}\\n'\n",
            HEADER.trim_end()
        ),
    )
    .expect("fake exporter should be writable");
    make_executable(&exporter);
    let trace = root.join("input.tracy");
    fs::write(&trace, b"synthetic trace").expect("synthetic trace should be writable");
    let csv = root.join("partial.csv");
    let path = format!(
        "{}:{}",
        root.display(),
        std::env::var("PATH").expect("PATH should be set")
    );

    let export = ravel_cli_command()
        .args([
            "performance",
            "export-tracy-frames",
            trace.to_str().expect("test path should be UTF-8"),
            csv.to_str().expect("test path should be UTF-8"),
        ])
        .env("PATH", &path)
        .output()
        .expect("ravel-cli should execute");
    assert_eq!(export.status.code(), Some(1));
    assert!(!csv.exists());

    let collector = root.join("tracy-capture");
    fs::write(
        &collector,
        "#!/bin/sh\n\
         set -eu\n\
         printf 'partial trace' > \"$2\"\n\
         exit 1\n",
    )
    .expect("fake collector should be writable");
    make_executable(&collector);
    let partial_trace = root.join("partial.tracy");
    let capture = ravel_cli_command()
        .args([
            "performance",
            "capture-tracy",
            "3",
            partial_trace.to_str().expect("test path should be UTF-8"),
        ])
        .env("PATH", path)
        .output()
        .expect("ravel-cli should execute");
    assert_eq!(capture.status.code(), Some(1));
    assert!(!partial_trace.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn make_executable(path: &std::path::Path) {
    let mut permissions = fs::metadata(path)
        .expect("tool metadata should be readable")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("tool should be executable");
}
