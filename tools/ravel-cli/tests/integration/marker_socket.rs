use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

use ravel_cli::{
    InitialPerformanceMarkerCommand, PerformanceError, record_initial_performance_markers,
    send_initial_performance_marker,
};

use super::common::{ravel_cli_command, temporary_root, wait_for_socket};

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

    wait_for_socket(&socket);
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

    wait_for_socket(&socket);

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
