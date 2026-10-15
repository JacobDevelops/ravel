use std::fs;
use std::io::Cursor;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::time::Duration;

use super::common::{ravel_cli_command, temporary_root, write_proc_snapshot};

use ravel_cli::{
    LinuxMemoryCsvRecorder, LinuxMemoryPressure, capture_linux_memory_csv,
    normalize_linux_memory_sample, parse_linux_memory_csv, parse_linux_memory_snapshot,
    read_linux_memory_snapshot_for_process, read_linux_process_identity,
};

const PREVIOUS_SMAPS: &str = "\
00400000-00452000 r--p 00000000 00:00 0 [rollup]
Rss:                2048 kB
Pss:                1024 kB
";
const CURRENT_SMAPS: &str = "\
00400000-00452000 r--p 00000000 00:00 0 [rollup]
Rss:                3072 kB
Pss:                1536 kB
";
const MEMINFO: &str = "\
MemTotal:       16777216 kB
MemFree:         1048576 kB
MemAvailable:    4194304 kB
";

#[test]
fn parses_kernel_counters_and_normalizes_partial_memory_stall() {
    let previous = parse_linux_memory_snapshot(
        PREVIOUS_SMAPS,
        MEMINFO,
        "\
some avg10=0.00 avg60=0.00 avg300=0.00 total=100
full avg10=0.00 avg60=0.00 avg300=0.00 total=10
",
        "pswpin 2\npswpout 3\n",
        4096,
    )
    .expect("previous snapshot should parse");
    let current = parse_linux_memory_snapshot(
        CURRENT_SMAPS,
        MEMINFO,
        "\
some avg10=0.50 avg60=0.10 avg300=0.01 total=150
full avg10=0.00 avg60=0.00 avg300=0.00 total=10
",
        "pswpin 3\npswpout 4\n",
        4096,
    )
    .expect("current snapshot should parse");

    let sample =
        normalize_linux_memory_sample(&previous, &current).expect("counters should normalize");

    assert_eq!(sample.process_rss_bytes, 3 * 1024 * 1024);
    assert_eq!(sample.system_used_memory_bytes, 12 * 1024 * 1024 * 1024);
    assert_eq!(sample.memory_pressure, LinuxMemoryPressure::Warning);
    assert_eq!(sample.memory_pressure.level(), 1);
    assert_eq!(sample.swap_in_total_bytes, 3 * 4096);
    assert_eq!(sample.swap_out_total_bytes, 4 * 4096);
}

#[test]
fn distinguishes_no_stall_from_full_memory_stall() {
    let previous = parse_linux_memory_snapshot(
        PREVIOUS_SMAPS,
        MEMINFO,
        "\
some avg10=0.00 avg60=0.00 avg300=0.00 total=100
full avg10=0.00 avg60=0.00 avg300=0.00 total=10
",
        "pswpin 2\npswpout 3\n",
        4096,
    )
    .expect("previous snapshot should parse");
    let unchanged = parse_linux_memory_snapshot(
        CURRENT_SMAPS,
        MEMINFO,
        "\
some avg10=0.00 avg60=0.00 avg300=0.00 total=100
full avg10=0.00 avg60=0.00 avg300=0.00 total=10
",
        "pswpin 2\npswpout 3\n",
        4096,
    )
    .expect("unchanged snapshot should parse");
    let full_stall = parse_linux_memory_snapshot(
        CURRENT_SMAPS,
        MEMINFO,
        "\
some avg10=1.00 avg60=0.20 avg300=0.02 total=200
full avg10=0.25 avg60=0.05 avg300=0.01 total=25
",
        "pswpin 4\npswpout 5\n",
        4096,
    )
    .expect("full-stall snapshot should parse");

    assert_eq!(
        normalize_linux_memory_sample(&previous, &unchanged)
            .expect("unchanged counters should normalize")
            .memory_pressure,
        LinuxMemoryPressure::Normal
    );
    assert_eq!(
        normalize_linux_memory_sample(&previous, &full_stall)
            .expect("increasing counters should normalize")
            .memory_pressure,
        LinuxMemoryPressure::Critical
    );
}

#[test]
fn rejects_missing_units_invalid_memory_and_counter_regression() {
    let missing_unit = parse_linux_memory_snapshot(
        "Rss: 3072\n",
        MEMINFO,
        "some total=100\nfull total=10\n",
        "pswpin 2\npswpout 3\n",
        4096,
    )
    .expect_err("RSS without kB must fail");
    assert!(missing_unit.to_string().contains("Rss"));

    let impossible_memory = parse_linux_memory_snapshot(
        CURRENT_SMAPS,
        "MemTotal: 1024 kB\nMemAvailable: 2048 kB\n",
        "some total=100\nfull total=10\n",
        "pswpin 2\npswpout 3\n",
        4096,
    )
    .expect_err("available memory above total must fail");
    assert!(impossible_memory.to_string().contains("MemAvailable"));

    let previous = parse_linux_memory_snapshot(
        PREVIOUS_SMAPS,
        MEMINFO,
        "some total=100\nfull total=10\n",
        "pswpin 3\npswpout 4\n",
        4096,
    )
    .expect("previous snapshot should parse");
    let regressed = parse_linux_memory_snapshot(
        CURRENT_SMAPS,
        MEMINFO,
        "some total=99\nfull total=9\n",
        "pswpin 2\npswpout 3\n",
        4096,
    )
    .expect("individual snapshot should parse");
    let error = normalize_linux_memory_sample(&previous, &regressed)
        .expect_err("cumulative counters must not regress");
    assert!(error.to_string().contains("regressed"));
}

#[test]
fn rejects_zero_page_size_and_byte_overflow() {
    let zero_page = parse_linux_memory_snapshot(
        CURRENT_SMAPS,
        MEMINFO,
        "some total=100\nfull total=10\n",
        "pswpin 2\npswpout 3\n",
        0,
    )
    .expect_err("zero page size must fail");
    assert!(zero_page.to_string().contains("page size"));

    let overflow = parse_linux_memory_snapshot(
        "Rss: 18014398509481984 kB\n",
        MEMINFO,
        "some total=100\nfull total=10\n",
        "pswpin 2\npswpout 3\n",
        4096,
    )
    .expect_err("kilobyte conversion overflow must fail");
    assert!(overflow.to_string().contains("overflows"));
}

#[test]
fn rejects_swap_overflow_and_swap_only_counter_regression() {
    for (vmstat, field) in [
        ("pswpin 18446744073709551615\npswpout 3\n", "pswpin"),
        ("pswpin 2\npswpout 18446744073709551615\n", "pswpout"),
    ] {
        let error = parse_linux_memory_snapshot(
            CURRENT_SMAPS,
            MEMINFO,
            "some total=100\nfull total=10\n",
            vmstat,
            4096,
        )
        .expect_err("swap byte conversion overflow must fail");
        assert!(error.to_string().contains(field));
        assert!(error.to_string().contains("overflows"));
    }

    let previous = parse_linux_memory_snapshot(
        PREVIOUS_SMAPS,
        MEMINFO,
        "some total=100\nfull total=10\n",
        "pswpin 3\npswpout 4\n",
        4096,
    )
    .expect("previous snapshot should parse");
    for (vmstat, field) in [
        ("pswpin 2\npswpout 4\n", "pswpin"),
        ("pswpin 3\npswpout 3\n", "pswpout"),
    ] {
        let current = parse_linux_memory_snapshot(
            CURRENT_SMAPS,
            MEMINFO,
            "some total=100\nfull total=10\n",
            vmstat,
            4096,
        )
        .expect("individual snapshot should parse");
        let error = normalize_linux_memory_sample(&previous, &current)
            .expect_err("swap counter regression must fail");
        assert!(error.to_string().contains(field));
        assert!(error.to_string().contains("regressed"));
    }
}

#[test]
fn captures_and_reparses_a_focus_free_linux_memory_stream() {
    let root = temporary_root("capture");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let csv = root.join("linux-memory.csv");

    let count = capture_linux_memory_csv(&proc_root, 42, 4096, Duration::from_millis(1), 2, &csv)
        .expect("synthetic procfs should capture");
    assert_eq!(count, 2);

    let samples = parse_linux_memory_csv(Cursor::new(
        fs::read_to_string(&csv).expect("capture should be readable"),
    ))
    .expect("generated CSV should reparse");
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].sample_index, 0);
    assert_eq!(samples[1].sample_index, 1);
    assert!(samples[0].elapsed_ns > 0);
    assert!(samples[1].elapsed_ns > samples[0].elapsed_ns);
    assert_eq!(samples[0].process_rss_bytes, 3 * 1024 * 1024);
    assert_eq!(samples[0].memory_pressure, LinuxMemoryPressure::Normal);

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn records_linux_memory_on_a_supplied_shared_clock() {
    let root = temporary_root("shared-clock");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let csv = root.join("linux-memory.csv");
    let mut recorder =
        LinuxMemoryCsvRecorder::new(&proc_root, 42, 4096, Duration::from_millis(1), 25, &csv)
            .expect("shared-clock recorder should initialize");

    assert_eq!(recorder.next_deadline_ns(), 1_000_025);
    assert!(
        !recorder
            .sample_due_at(1_000_024, 5_000_000)
            .expect("an early poll should be harmless")
    );
    assert!(
        recorder
            .sample_due_at(1_000_025, 5_000_000)
            .expect("the exact deadline should sample")
    );
    assert_eq!(
        recorder
            .finish()
            .expect("shared-clock capture should verify"),
        1
    );

    let samples = parse_linux_memory_csv(Cursor::new(
        fs::read_to_string(&csv).expect("shared-clock CSV should be readable"),
    ))
    .expect("shared-clock CSV should parse");
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].elapsed_ns, 1_000_025);

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn shared_clock_memory_rejects_late_samples_and_removes_partial_output() {
    let root = temporary_root("shared-clock-late");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let csv = root.join("linux-memory.csv");
    let mut recorder =
        LinuxMemoryCsvRecorder::new(&proc_root, 42, 4096, Duration::from_millis(1), 0, &csv)
            .expect("shared-clock recorder should initialize");

    let error = recorder
        .sample_due_at(6_000_001, 5_000_000)
        .expect_err("a sample outside alignment tolerance must fail");
    assert!(error.to_string().contains("alignment tolerance"));
    drop(recorder);
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn shared_clock_memory_is_private_and_preserves_a_replacement_path() {
    let root = temporary_root("shared-clock-replacement");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let csv = root.join("linux-memory.csv");
    let recorder =
        LinuxMemoryCsvRecorder::new(&proc_root, 42, 4096, Duration::from_millis(1), 0, &csv)
            .expect("shared-clock recorder should initialize");
    assert_eq!(
        fs::metadata(&csv)
            .expect("partial output metadata should be readable")
            .mode()
            & 0o777,
        0o600
    );

    fs::remove_file(&csv).expect("partial output should be replaceable");
    fs::write(&csv, "replacement").expect("replacement output should be writable");
    drop(recorder);
    assert_eq!(
        fs::read_to_string(&csv).expect("replacement output should survive cleanup"),
        "replacement"
    );

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn shared_clock_memory_rejects_invalid_setup_and_empty_capture() {
    let root = temporary_root("shared-clock-invalid-setup");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);

    let zero_interval_csv = root.join("zero-interval.csv");
    assert!(
        LinuxMemoryCsvRecorder::new(&proc_root, 42, 4096, Duration::ZERO, 0, &zero_interval_csv)
            .is_err()
    );
    assert!(!zero_interval_csv.exists());

    let overflow_csv = root.join("overflow.csv");
    assert!(
        LinuxMemoryCsvRecorder::new(
            &proc_root,
            42,
            4096,
            Duration::from_nanos(1),
            u64::MAX,
            &overflow_csv
        )
        .is_err()
    );
    assert!(!overflow_csv.exists());

    let existing_csv = root.join("existing.csv");
    fs::write(&existing_csv, "preserve").expect("existing output should be writable");
    assert!(
        LinuxMemoryCsvRecorder::new(
            &proc_root,
            42,
            4096,
            Duration::from_secs(1),
            0,
            &existing_csv
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(&existing_csv).expect("existing output should remain readable"),
        "preserve"
    );

    let missing_proc_csv = root.join("missing-proc.csv");
    assert!(
        LinuxMemoryCsvRecorder::new(
            &root.join("missing-proc"),
            42,
            4096,
            Duration::from_secs(1),
            0,
            &missing_proc_csv
        )
        .is_err()
    );
    assert!(!missing_proc_csv.exists());

    let empty_csv = root.join("empty.csv");
    let recorder =
        LinuxMemoryCsvRecorder::new(&proc_root, 42, 4096, Duration::from_secs(1), 0, &empty_csv)
            .expect("empty recorder should initialize");
    assert!(recorder.finish().is_err());
    assert!(!empty_csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn rejects_linux_memory_csv_drift_and_existing_output() {
    let wrong_header = "\
elapsed_ns,process_rss_bytes
1,2
";
    let error =
        parse_linux_memory_csv(Cursor::new(wrong_header)).expect_err("header drift must fail");
    assert!(error.to_string().contains("expected header"));

    let root = temporary_root("existing");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let csv = root.join("linux-memory.csv");
    fs::write(&csv, "keep me").expect("existing output should be writable");
    let error = capture_linux_memory_csv(&proc_root, 42, 4096, Duration::from_millis(1), 1, &csv)
        .expect_err("existing output must not be overwritten");
    assert!(error.to_string().contains("cannot create"));
    assert_eq!(
        fs::read_to_string(&csv).expect("existing output should remain readable"),
        "keep me"
    );

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn rejects_invalid_linux_rows_and_removes_partial_capture() {
    let header = "\
sample_index,elapsed_ns,process_rss_bytes,system_used_memory_bytes,memory_pressure_level,swap_in_total_bytes,swap_out_total_bytes
";
    for (row, expected) in [
        ("1,1,2,3,0,4,5\n", "sample index"),
        ("0,1,2,3,0,4,5\n1,1,2,3,0,4,5\n", "strictly increasing"),
        ("0,1,2,3,3,4,5\n", "outside 0..=2"),
    ] {
        let error = parse_linux_memory_csv(Cursor::new(format!("{header}{row}")))
            .expect_err("invalid normalized row must fail");
        assert!(error.to_string().contains(expected));
    }
    let empty = parse_linux_memory_csv(Cursor::new(header))
        .expect_err("header-only capture must not validate");
    assert!(empty.to_string().contains("no samples"));

    let root = temporary_root("partial");
    let csv = root.join("linux-memory.csv");
    let error = capture_linux_memory_csv(
        &root.join("missing-proc"),
        42,
        4096,
        Duration::from_millis(1),
        1,
        &csv,
    )
    .expect_err("missing procfs source must fail");
    assert!(error.to_string().contains("cannot read"));
    assert!(!csv.exists());

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn rejects_reused_numeric_pid_during_snapshot() {
    let root = temporary_root("pid-reuse");
    let proc_root = root.join("proc");
    write_proc_snapshot(&proc_root, 42);
    let expected =
        read_linux_process_identity(&proc_root, 42).expect("initial identity should parse");
    assert_eq!(expected.start_time_ticks, 100);

    fs::write(
        proc_root.join("42/stat"),
        "42 (java worker) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 101\n",
    )
    .expect("replacement identity should be writable");
    let error = read_linux_memory_snapshot_for_process(&proc_root, 42, 4096, expected)
        .expect_err("same numeric PID with a new start time must fail");
    assert!(error.to_string().contains("identity changed"));

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[cfg(target_os = "linux")]
#[test]
fn command_line_captures_linux_memory_without_desktop_input() {
    let root = temporary_root("cli");
    let csv = root.join("linux-memory.csv");
    let output = ravel_cli_command()
        .args([
            "performance",
            "capture-linux-memory",
            "self",
            "1",
            csv.to_str().expect("test path should be UTF-8"),
        ])
        .output()
        .expect("ravel-cli should execute");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "Captured 1 Linux memory samples without desktop input: {}\n",
            csv.display()
        )
    );
    assert_eq!(output.stderr, [] as [u8; 0]);
    assert_eq!(
        parse_linux_memory_csv(Cursor::new(
            fs::read_to_string(&csv).expect("capture should be readable")
        ))
        .expect("live capture should validate")
        .len(),
        1
    );

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[cfg(target_os = "linux")]
#[test]
fn command_line_rejects_tracked_linux_memory_output_before_sampling() {
    let csv = PathBuf::from(format!(
        "linux-memory-tracked-test-{}.csv",
        std::process::id()
    ));
    assert!(!csv.exists());
    let output = ravel_cli_command()
        .args([
            "performance",
            "capture-linux-memory",
            "self",
            "1",
            csv.to_str().expect("test path should be UTF-8"),
        ])
        .output()
        .expect("ravel-cli should execute");

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("ignored private boundary"));
    assert!(!csv.exists());
}
