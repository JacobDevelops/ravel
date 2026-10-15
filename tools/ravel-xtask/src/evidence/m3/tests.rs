use serde_json::Value;

use super::{
    CampaignInventory, M3_RECORDS, campaign_document_violations, verify_campaign_inventory,
    verify_harness_lifecycles,
};
use crate::evidence::accounting::verify_review_round_counts;
use crate::json::read_json;

#[test]
fn the_tracked_m3_records_leave_the_round_count_to_the_record_that_counts() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_review_round_counts(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_tracked_m3_records_agree_with_the_harnesss_declared_world_lifecycles() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_harness_lifecycles(&root, &mut violations);

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
}
#[test]
fn the_tracked_campaign_document_agrees_with_the_tracked_launch_record() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_campaign_inventory(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

fn tracked_record(name: &str) -> Value {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    read_json(&root.join("evidence/m3").join(name)).expect("tracked M3 record should parse")
}

#[test]
fn every_tracked_m3_record_satisfies_its_closure_rule() {
    for (name, rule, _) in M3_RECORDS {
        assert!(rule(&tracked_record(name)), "{name} fails its rule");
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
        let (_, rule, _) = M3_RECORDS
            .iter()
            .find(|(record, _, _)| *record == name)
            .expect("record should have a rule");

        let mut nulled = tracked_record(name);
        *nulled
            .pointer_mut(pointer)
            .expect("tracked record should hold the field") = Value::Null;
        assert!(!rule(&nulled), "{name} accepted null {pointer}");

        let mut missing = tracked_record(name);
        let (parent, field) = pointer
            .rsplit_once('/')
            .expect("pointer should have a parent");
        missing
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .expect("parent should be an object")
            .remove(field);
        assert!(!rule(&missing), "{name} accepted missing {pointer}");
    }
}

#[test]
fn the_m3_matrix_refuses_another_loader_and_a_zero_rewrite() {
    let (_, rule, _) = M3_RECORDS
        .iter()
        .find(|(record, _, _)| *record == "linux-process-matrix.json")
        .expect("matrix should have a rule");
    let mut other_loader = tracked_record("linux-process-matrix.json");
    other_loader["instance"]["otherLoaderComponents"] = serde_json::json!(1);
    assert!(!rule(&other_loader));
    let mut zero_rewrite = tracked_record("linux-process-matrix.json");
    zero_rewrite["rewrite"]["classesRewritten"] = serde_json::json!(0);
    assert!(!rule(&zero_rewrite));
}
