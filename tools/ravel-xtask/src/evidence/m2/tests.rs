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
    assert!(m2_budget_limits(&budget(vec![
        limit("startup", json!(10), json!(20)),
        limit("tick", json!(0.5), json!(1.0)),
    ])));
}

#[test]
fn a_duplicated_metric_is_refused() {
    assert!(!m2_budget_limits(&budget(vec![
        limit("startup", json!(10), json!(20)),
        limit("startup", json!(11), json!(20)),
    ])));
}

#[test]
fn a_limit_not_above_its_measurement_is_refused() {
    assert!(!m2_budget_limits(&budget(vec![limit(
        "startup",
        json!(10),
        json!(10)
    )])));
    assert!(!m2_budget_limits(&budget(vec![limit(
        "startup",
        json!(10),
        json!(9)
    )])));
    assert!(!m2_budget_limits(&budget(vec![limit(
        "startup",
        json!(0),
        json!(9)
    )])));
}

#[test]
fn missing_null_or_empty_budget_fields_are_refused() {
    assert!(!m2_budget_limits(&budget(Vec::new())));
    assert!(!m2_budget_limits(&json!({})));
    assert!(!m2_budget_limits(&budget(vec![limit(
        "startup",
        Value::Null,
        json!(20)
    )])));
    assert!(!m2_budget_limits(&budget(vec![limit(
        "startup",
        json!(10),
        Value::Null
    )])));
    assert!(!m2_budget_limits(&budget(vec![limit(
        "",
        json!(10),
        json!(20)
    )])));
    for field in ["metric", "unit", "operator", "measured", "limit"] {
        let mut missing = limit("startup", json!(10), json!(20));
        missing
            .as_object_mut()
            .expect("limit should be an object")
            .remove(field);
        assert!(
            !m2_budget_limits(&budget(vec![missing])),
            "missing {field} was accepted"
        );
    }
}
