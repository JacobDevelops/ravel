use serde_json::Value;

use super::{
    CampaignInventory, M3_RECORDS, campaign_document_violations, verify_campaign_inventory,
    verify_harness_lifecycles,
};
use crate::json::read_json;

#[test]
fn the_tracked_m3_records_agree_with_the_harnesss_declared_world_lifecycles() {
    let root = crate::test_support::repository_root();
    let mut violations = Vec::new();

    super::verify_m3_integrity(&root, &Value::Null, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}
#[test]
fn a_campaign_document_stating_the_wrong_inventory_anywhere_is_reported() {
    let derived = CampaignInventory {
        classes: 15,
        targets: 33,
        prior: 31,
        launches: 3,
    };

    let violations = campaign_document_violations(
        "Rewriting all 33 callable targets, over the same 15 classes in the reviewed \
         inventory. The count moved from 31 to 33, over three surface launches. A later \
         launch at 34 callable targets over 16 classes in the reviewed inventory.",
        &derived,
    );

    assert_eq!(
        violations,
        [
            "evidence/m3/README.md states 34 callable targets where \
             evidence/m3/linux-process-matrix.json records 33"
                .to_owned(),
            "evidence/m3/README.md states 16 classes in the reviewed inventory where \
             evidence/m3/linux-process-matrix.json records 15"
                .to_owned(),
        ]
    );
}

#[test]
fn a_campaign_figure_split_across_a_wrapped_line_is_still_checked() {
    let derived = CampaignInventory {
        classes: 15,
        targets: 33,
        prior: 31,
        launches: 3,
    };

    let violations = campaign_document_violations(
        "The count moved from 31 to 33 callable targets over the same 16 classes in the\n  \
         reviewed inventory, across three surface launches.",
        &derived,
    );

    assert_eq!(
        violations,
        [
            "evidence/m3/README.md states 16 classes in the reviewed inventory where \
             evidence/m3/linux-process-matrix.json records 15"
                .to_owned()
        ]
    );
}

#[test]
fn a_campaign_document_stating_no_count_at_all_is_not_read_as_agreement() {
    let derived = CampaignInventory {
        classes: 15,
        targets: 33,
        prior: 31,
        launches: 3,
    };

    let violations = campaign_document_violations("The campaigns are recorded.", &derived);

    assert_eq!(violations.len(), 4, "{violations:?}");
    assert!(
        violations
            .iter()
            .all(|violation| violation.contains("no longer writes a count against")),
        "{violations:?}"
    );
    assert_eq!(
        violations[3],
        "evidence/m3/README.md no longer writes a count against `surface launches`, so nothing in it \
         is held to evidence/m3/linux-process-matrix.json's 3 launches in the surface campaign",
    );
}
#[test]
fn the_tracked_campaign_document_agrees_with_the_tracked_launch_record() {
    let root = crate::test_support::repository_root();
    let mut violations = Vec::new();

    verify_campaign_inventory(
        &root,
        &read_json(&root.join(super::CAMPAIGN_MATRIX)).expect("matrix should parse"),
        &mut violations,
    );

    assert_eq!(violations, Vec::<String>::new());
}

fn tracked_record(name: &str) -> Value {
    let root = crate::test_support::repository_root();
    read_json(&root.join("evidence/m3").join(name)).expect("tracked M3 record should parse")
}

#[test]
fn every_tracked_m3_record_satisfies_its_closure_rule() {
    for record in M3_RECORDS {
        let name = record.name;
        assert!(
            (record.rule)(&tracked_record(name)).is_ok(),
            "{name} fails its rule"
        );
    }
}

#[test]
fn every_m3_record_rule_refuses_a_missing_or_null_required_field() {
    for (name, pointer) in [
        ("class-loader-identity.json", "/observed/sameObject"),
        ("class-loader-identity.json", "/rewrite/nativeInvocations"),
        ("class-loader-identity.json", "/rewrite/reachedNativeCode"),
        ("pack-compile.json", "/compile/repeats"),
        ("pack-compile.json", "/disk/toolchainBytes"),
        ("profiling-overhead.json", "/budget/limits"),
        ("profiling-overhead.json", "/collectors/unsupported"),
        (
            "linux-process-matrix.json",
            "/instance/withoutAnotherLoader",
        ),
        (
            "linux-process-matrix.json",
            "/instance/otherLoaderComponents",
        ),
        ("linux-process-matrix.json", "/rewrite/classesRewritten"),
    ] {
        let rule = M3_RECORDS
            .iter()
            .find(|record| record.name == name)
            .expect("record should have a rule")
            .rule;

        let mut nulled = tracked_record(name);
        *nulled
            .pointer_mut(pointer)
            .expect("tracked record should hold the field") = Value::Null;
        assert!(rule(&nulled).is_err(), "{name} accepted null {pointer}");

        let mut missing = tracked_record(name);
        let (parent, field) = pointer
            .rsplit_once('/')
            .expect("pointer should have a parent");
        missing
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .expect("parent should be an object")
            .remove(field);
        assert!(rule(&missing).is_err(), "{name} accepted missing {pointer}");
    }
}

#[test]
fn the_m3_matrix_refuses_another_loader_and_a_zero_rewrite() {
    let rule = M3_RECORDS
        .iter()
        .find(|record| record.name == "linux-process-matrix.json")
        .expect("matrix should have a rule")
        .rule;
    let mut other_loader = tracked_record("linux-process-matrix.json");
    other_loader["instance"]["otherLoaderComponents"] = serde_json::json!(1);
    assert!(rule(&other_loader).is_err());
    let mut zero_rewrite = tracked_record("linux-process-matrix.json");
    zero_rewrite["rewrite"]["classesRewritten"] = serde_json::json!(0);
    assert!(rule(&zero_rewrite).is_err());
}

#[test]
fn integrity_still_requires_the_campaign_matrix_record() {
    let root = lifecycle_root("m3-required-campaign");
    let mut violations = Vec::new();
    super::verify_m3_integrity(&root, &Value::Null, &mut violations);
    assert_eq!(
        violations,
        ["missing required file: evidence/m3/linux-process-matrix.json"]
    );
    std::fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn lifecycle_root(label: &str) -> std::path::PathBuf {
    let root = crate::test_support::temporary_root(label);
    let harness = root.join(super::PERSISTENCE_HARNESS);
    std::fs::create_dir_all(harness.parent().expect("harness should have a parent"))
        .expect("harness directory should be creatable");
    std::fs::write(harness, "persistence_lifecycles_declared=1\n").expect("harness should write");
    std::fs::create_dir_all(root.join("evidence/m3"))
        .expect("evidence directory should be creatable");
    std::fs::write(root.join(super::CAMPAIGN_DOCUMENT), "1 world lifecycles")
        .expect("campaign document should write");
    root
}

#[test]
fn cached_matrix_still_checks_lifecycle_counts_without_rereading_the_file() {
    let root = lifecycle_root("m3-cached-lifecycle-counts");
    let matrix = root.join(super::CAMPAIGN_MATRIX);
    std::fs::write(&matrix, r#"{"scope":"2 world lifecycles"}"#).expect("matrix should write");
    let mut violations = Vec::new();
    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M3,
        M3_RECORDS,
        &mut violations,
    );
    assert!(violations.is_empty(), "{violations:?}");
    std::fs::remove_file(matrix).expect("loaded matrix should be removable");
    verify_harness_lifecycles(&root, &records, &mut violations);
    assert_eq!(
        violations,
        [format!(
            "{} states 2 world lifecycles where {} declares 1",
            super::CAMPAIGN_MATRIX,
            super::PERSISTENCE_HARNESS,
        )]
    );
    std::fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn compact_and_pretty_matrices_check_every_string_leaf() {
    let record = serde_json::json!({
        "scope": "1 world lifecycles",
        "nested": ["2 world lifecycles", {"mixed": "1 world lifecycles and 3 world lifecycles"}],
    });
    for (label, text) in [
        ("m3-compact-lifecycles", record.to_string()),
        (
            "m3-pretty-lifecycles",
            serde_json::to_string_pretty(&record).expect("matrix should serialize"),
        ),
    ] {
        let root = lifecycle_root(label);
        std::fs::write(root.join(super::CAMPAIGN_MATRIX), text).expect("matrix should write");
        let expected = [2, 3].map(|count| {
            format!(
                "{} states {count} world lifecycles where {} declares 1",
                super::CAMPAIGN_MATRIX,
                super::PERSISTENCE_HARNESS,
            )
        });
        let mut cached = Vec::new();
        let records = crate::evidence::closure::read_records(
            &root,
            crate::evidence::rules::Milestone::M3,
            M3_RECORDS,
            &mut cached,
        );
        assert!(cached.is_empty(), "{cached:?}");
        std::fs::remove_file(root.join(super::CAMPAIGN_MATRIX))
            .expect("cached matrix should be removable");
        verify_harness_lifecycles(&root, &records, &mut cached);
        assert_eq!(cached, expected, "{label}");
        std::fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}
