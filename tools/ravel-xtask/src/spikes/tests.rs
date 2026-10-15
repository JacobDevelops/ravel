use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::spikes::verify_spikes;
use crate::test_support::temporary_root;

#[test]
fn an_absent_spike_tree_is_not_a_violation() {
    let root = temporary_root("spikes-absent");

    assert_eq!(verify_spikes(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn spike_results_must_cover_every_failure_scenario() {
    let root = temporary_root("spikes-failures");
    let gate = root.join("evidence/m1/spikes/adg-1");
    fs::create_dir_all(&gate).expect("spike directory should be creatable");
    fs::write(
        gate.join("candidate.json"),
        r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
            "candidate":{"disposition":"measured"},
            "failures":[{"scenario":"crash"}],
            "memory":{"bootstrapPeakRss":{"status":"measured","value":1},
                "retainedRss":{"status":"measured","value":1},
                "retainedClasses":{"status":"measured","value":1}},
            "startup":{"processToReady":{"status":"measured","value":1}},
            "calls":{"callbackNanoseconds":{"status":"measured"}},
            "trial":{"measuredInvocations":1},"deviations":[]}"#,
    )
    .expect("spike result should be writable");

    let violations = verify_spikes(&root);

    assert!(violations.iter().any(|violation| {
        violation.ends_with("must exercise every required failure scenario exactly once")
    }));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_spike_result_does_not_require_a_historical_change_identifier() {
    let root = temporary_root("spikes-no-change-id");
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::create_dir_all(root.join("schemas")).expect("schema directory should be creatable");
    fs::copy(
        repository.join("schemas/spike-result.schema.json"),
        root.join("schemas/spike-result.schema.json"),
    )
    .expect("spike schema should be copied");
    let gate = root.join("evidence/m1/spikes/adg-5");
    fs::create_dir_all(&gate).expect("spike directory should be creatable");
    let output = Command::new("jq")
        .arg("del(.trial.revision.changeId)")
        .arg(repository.join("evidence/m1/spikes/adg-5/adg5-stock-jvm-arguments.json"))
        .output()
        .expect("recorded spike result should be readable by jq");
    assert!(output.status.success(), "spike mutation should succeed");
    fs::write(gate.join("candidate.json"), output.stdout).expect("spike result should be writable");

    assert!(
        verify_spikes(&root).is_empty(),
        "a spike result without the historical identifier should pass"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_claimed_stock_launch_without_the_redacted_log_hash_is_refused() {
    let root = temporary_root("spikes-launch-log");
    fs::create_dir_all(root.join("schemas")).expect("schema directory should be creatable");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/spike-result.schema.json"),
        root.join("schemas/spike-result.schema.json"),
    )
    .expect("spike schema should be copied");
    let gate = root.join("evidence/m1/spikes/adg-5");
    fs::create_dir_all(&gate).expect("spike directory should be creatable");
    let recorded = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../evidence/m1/spikes/adg-5/adg5-stock-jvm-arguments.json"),
    )
    .expect("the recorded ADG-5 result should be readable");
    let target = gate.join("candidate.json");

    fs::write(&target, &recorded).expect("spike result should be writable");
    assert!(
        verify_spikes(&root).is_empty(),
        "the recorded result should pass unaltered"
    );

    fs::write(
        &target,
        recorded.replace("redacted-instance-launch.log", "some-other-capture.log"),
    )
    .expect("altered spike result should be writable");
    assert!(
        verify_spikes(&root)
            .iter()
            .any(|violation| violation.ends_with("does not conform to the spike schema")),
        "a stock launch with no redacted log hash should be refused"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn spike_results_must_not_substitute_zero_for_a_missing_measurement() {
    let root = temporary_root("spikes-zero");
    let gate = root.join("evidence/m1/spikes/adg-1");
    fs::create_dir_all(&gate).expect("spike directory should be creatable");
    fs::write(
        gate.join("candidate.json"),
        r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
            "candidate":{"disposition":"measured"},
            "failures":[{"scenario":"callback-failure"},{"scenario":"crash"},
                {"scenario":"invalid-input"},{"scenario":"timeout"},
                {"scenario":"unavailable-runtime"}],
            "memory":{"bootstrapPeakRss":{"status":"measured","value":0},
                "retainedRss":{"status":"measured","value":1},
                "retainedClasses":{"status":"measured","value":1}},
            "startup":{"processToReady":{"status":"measured","value":1}},
            "calls":{"callbackNanoseconds":{"status":"measured"}},
            "trial":{"measuredInvocations":1},"deviations":[]}"#,
    )
    .expect("spike result should be writable");

    let violations = verify_spikes(&root);

    assert!(violations.iter().any(|violation| {
        violation.ends_with("must not report a measured bootstrap or startup value of zero")
    }));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
