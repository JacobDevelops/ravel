use serde_json::{Value, json};

use super::m2_budget_limits;

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
        m2_budget_limits(&budget(vec![
            limit("startup", json!(10), json!(20)),
            limit("tick", json!(0.5), json!(1.0)),
        ]))
        .is_ok()
    );
}

#[test]
fn a_duplicated_metric_is_refused() {
    assert!(
        m2_budget_limits(&budget(vec![
            limit("startup", json!(10), json!(20)),
            limit("startup", json!(11), json!(20)),
        ]))
        .is_err()
    );
}

#[test]
fn a_limit_not_above_its_measurement_is_refused() {
    assert!(m2_budget_limits(&budget(vec![limit("startup", json!(10), json!(10))])).is_err());
    assert!(m2_budget_limits(&budget(vec![limit("startup", json!(10), json!(9))])).is_err());
    assert!(m2_budget_limits(&budget(vec![limit("startup", json!(0), json!(9))])).is_err());
}

#[test]
fn missing_null_or_empty_budget_fields_are_refused() {
    assert!(m2_budget_limits(&budget(Vec::new())).is_err());
    assert!(m2_budget_limits(&json!({})).is_err());
    assert!(m2_budget_limits(&budget(vec![limit("startup", Value::Null, json!(20))])).is_err());
    assert!(m2_budget_limits(&budget(vec![limit("startup", json!(10), Value::Null)])).is_err());
    assert!(m2_budget_limits(&budget(vec![limit("", json!(10), json!(20))])).is_err());
    for field in ["metric", "unit", "operator", "measured", "limit"] {
        let mut missing = limit("startup", json!(10), json!(20));
        missing
            .as_object_mut()
            .expect("limit should be an object")
            .remove(field);
        assert!(
            m2_budget_limits(&budget(vec![missing])).is_err(),
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
