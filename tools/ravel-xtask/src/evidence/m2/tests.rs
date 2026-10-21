use serde_json::{Value, json};

use super::verify_m2_budget_limits;

fn limit(metric: &str, measured: Value, upper: Value) -> Value {
    let mut limit = json!({
        "metric": metric,
        "unit": "ms",
        "operator": "less-than-or-equal",
    });
    limit["measured"] = measured;
    limit["limit"] = upper;
    limit
}

fn budget(limits: Vec<Value>) -> Value {
    json!({"budget": {"limits": Value::Array(limits)}})
}

#[test]
fn distinct_limits_above_positive_measurements_are_accepted() {
    assert!(
        verify_m2_budget_limits(&budget(vec![
            limit("startup", json!(10), json!(20)),
            limit("tick", json!(0.5), json!(1.0)),
        ]))
        .is_ok()
    );
}

#[test]
fn a_duplicated_metric_is_refused() {
    assert!(
        verify_m2_budget_limits(&budget(vec![
            limit("startup", json!(10), json!(20)),
            limit("startup", json!(11), json!(20)),
        ]))
        .is_err()
    );
}

#[test]
fn a_limit_not_above_its_measurement_is_refused() {
    assert!(
        verify_m2_budget_limits(&budget(vec![limit("startup", json!(10), json!(10))])).is_err()
    );
    assert!(verify_m2_budget_limits(&budget(vec![limit("startup", json!(10), json!(9))])).is_err());
    assert!(verify_m2_budget_limits(&budget(vec![limit("startup", json!(0), json!(9))])).is_err());
}

#[test]
fn missing_null_or_empty_budget_fields_are_refused() {
    assert!(verify_m2_budget_limits(&budget(Vec::new())).is_err());
    assert!(verify_m2_budget_limits(&json!({})).is_err());
    assert!(
        verify_m2_budget_limits(&budget(vec![limit("startup", Value::Null, json!(20))])).is_err()
    );
    assert!(
        verify_m2_budget_limits(&budget(vec![limit("startup", json!(10), Value::Null)])).is_err()
    );
    assert!(verify_m2_budget_limits(&budget(vec![limit("", json!(10), json!(20))])).is_err());
    for field in ["metric", "unit", "operator", "measured", "limit"] {
        let mut missing = limit("startup", json!(10), json!(20));
        missing
            .as_object_mut()
            .expect("limit should be an object")
            .remove(field);
        assert!(
            verify_m2_budget_limits(&budget(vec![missing])).is_err(),
            "missing {field} was accepted"
        );
    }
}

#[test]
fn integrity_still_requires_the_loader_overhead_record() {
    let root = crate::test_support::temporary_root("m2-required-overhead");
    let mut violations = Vec::new();
    super::verify_m2_integrity(&root, &Value::Null, &mut violations);
    assert_eq!(
        violations,
        ["missing required file: evidence/m2/loader-overhead.json"]
    );
    std::fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn typed_budget_admission_names_the_exact_row_and_field() {
    let record = budget(vec![
        limit("startup", json!(10), json!(20)),
        limit("tick", Value::Null, json!(1)),
    ]);
    let reason = <serde_json::Number as serde::Deserialize>::deserialize(&Value::Null)
        .expect_err("null must not deserialize as a number");
    assert_eq!(
        verify_m2_budget_limits(&record),
        Err(format!("/budget/limits/1/measured: {reason}"))
    );
    let empty_metric = budget(vec![limit("", json!(10), json!(20))]);
    assert_eq!(
        verify_m2_budget_limits(&empty_metric),
        Err("/budget/limits/0/metric: expected a nonempty string".to_owned())
    );
}

#[test]
fn noop_pipeline_refusal_names_the_exact_run() {
    let rule = super::M2_RECORDS
        .iter()
        .find(|definition| definition.name == "noop-pipeline.json")
        .expect("the no-op pipeline must have a record rule")
        .rule;
    let mut record = json!({"runs": [
        {"repeatedRunIsIdentical": true},
        {"repeatedRunIsIdentical": false}
    ]});
    assert_eq!(
        rule(&record),
        Err("/runs/1/repeatedRunIsIdentical".to_owned())
    );
    record["runs"][0]["repeatedRunIsIdentical"] = false.into();
    assert_eq!(
        rule(&record),
        Err("/runs/0/repeatedRunIsIdentical".to_owned())
    );
}
