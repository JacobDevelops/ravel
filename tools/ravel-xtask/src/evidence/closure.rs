//! Shared record predicates and carried-figure comparisons.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::verify_redaction_value;
use crate::evidence::rules::{Figure, Milestone, RecordRule, Rule};
use crate::json::read_json;

pub(crate) fn verify_arithmetic(
    record: &Value,
    label: &str,
    rules: &[(Rule, &str)],
    violations: &mut Vec<String>,
) {
    for (rule, description) in rules {
        if !rule(record) {
            violations.push(format!("{label}: {description}"));
        }
    }
}

fn verify_carried_figures(
    manifest: &Value,
    record: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    let CarriedFigures {
        manifest: manifest_label,
        record: record_label,
        subject,
        ..
    } = carried;
    for (pointer, derive) in carried.figures {
        let claimed = manifest.pointer(pointer).filter(|value| !value.is_null());
        let measured = derive(record).filter(|value| !value.is_null());
        let (Some(claimed), Some(measured)) = (claimed, measured) else {
            violations.push(format!(
                "{manifest_label}: cannot compare {subject} {pointer} with {record_label}: \
                 a required figure is missing or null"
            ));
            continue;
        };
        if !same_figure(claimed, &measured) {
            violations.push(format!(
                "{manifest_label} records {pointer} {claimed} while {record_label} derives \
                 {measured}; one {subject} produced both"
            ));
        }
    }
}

fn same_figure(claimed: &Value, measured: &Value) -> bool {
    claimed == measured
        || claimed
            .as_f64()
            .zip(measured.as_f64())
            .is_some_and(|(claimed, measured)| {
                claimed.partial_cmp(&measured) == Some(std::cmp::Ordering::Equal)
            })
}

#[derive(Default)]
pub(crate) struct LoadedRecords {
    values: BTreeMap<&'static str, Option<Value>>,
}

impl LoadedRecords {
    pub(crate) fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name).and_then(Option::as_ref)
    }

    pub(crate) fn was_loaded(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }
}
pub(crate) type RecordDefinition = (&'static str, RecordRule, &'static str, bool);

pub(crate) fn read_records(
    root: &Path,
    milestone: Milestone,
    definitions: &[RecordDefinition],
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let directory = format!("evidence/{}", milestone.as_str().to_ascii_lowercase());
    let mut records = LoadedRecords::default();
    for (name, _, _, required) in definitions {
        let relative = format!("{directory}/{name}");
        let path = root.join(&relative);
        if !path.is_file() {
            if *required {
                records.values.insert(*name, None);
                violations.push(format!("missing required file: {relative}"));
            }
            continue;
        }
        match read_json(&path) {
            Ok(record) => {
                records.values.insert(*name, Some(record));
            }
            Err(error) => {
                records.values.insert(*name, None);
                violations.push(error);
            }
        }
    }
    records
}

pub(crate) fn verify_closure_records(
    milestone: Milestone,
    records: &LoadedRecords,
    definitions: &[RecordDefinition],
    violations: &mut Vec<String>,
) {
    let directory = format!("evidence/{}", milestone.as_str().to_ascii_lowercase());
    for (name, rule, description, _) in definitions {
        let relative = format!("{directory}/{name}");
        let Some(record) = records.get(name) else {
            if !records.was_loaded(name) {
                violations.push(format!(
                    "{milestone} closure requires the record {relative}"
                ));
            }
            continue;
        };
        let result = if record["milestone"].as_str() == Some(milestone.as_str()) {
            rule(record)
        } else {
            Err(format!("/milestone must be {milestone}"))
        };
        if let Err(requirement) = result {
            violations.push(format!(
                "{milestone} closure requires {description}: {relative}: failed {requirement}"
            ));
        }
        verify_redaction_value(record, &relative, violations);
    }
}

pub(crate) struct CarriedFigures {
    pub(crate) manifest: &'static str,
    pub(crate) record: &'static str,
    pub(crate) subject: &'static str,
    pub(crate) figures: &'static [(&'static str, Figure)],
    pub(crate) arithmetic: &'static [(Rule, &'static str)],
}

pub(crate) fn verify_milestone_figures(
    record: &Value,
    manifest: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    verify_arithmetic(record, carried.record, carried.arithmetic, violations);
    verify_carried_figures(manifest, record, carried, violations);
}

#[cfg(test)]
mod tests;
