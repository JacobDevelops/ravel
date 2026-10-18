//! Shared record predicates and carried-figure comparisons.

use std::collections::{BTreeMap, BTreeSet};
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
    milestone: Milestone,
    manifest: &Value,
    record: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    let directory = milestone.directory();
    let manifest_label = format!("{directory}/manifest.json");
    let record_label = format!("{directory}/{}", carried.record);
    let subject = carried.subject;
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
    values: BTreeMap<&'static str, Value>,
    reported: BTreeSet<&'static str>,
}

impl LoadedRecords {
    pub(crate) fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    pub(crate) fn carried(&self, figures: &CarriedFigures) -> Option<&Value> {
        self.get(figures.record)
    }

    /// Whether loading already reported a failure for this record.
    pub(crate) fn already_reported(&self, name: &str) -> bool {
        self.reported.contains(name)
    }
}
pub(crate) struct RecordDefinition {
    pub(crate) name: &'static str,
    pub(crate) rule: RecordRule,
    pub(crate) description: &'static str,
    pub(crate) required: bool,
}

pub(crate) fn read_records(
    root: &Path,
    milestone: Milestone,
    definitions: &[RecordDefinition],
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let directory = milestone.directory();
    let mut records = LoadedRecords::default();
    for RecordDefinition { name, required, .. } in definitions {
        let relative = format!("{directory}/{name}");
        let path = match crate::text::require_file(root, &relative) {
            Ok(path) => path,
            Err(error) => {
                if *required {
                    records.reported.insert(*name);
                    violations.push(error);
                }
                continue;
            }
        };
        match read_json(root, &path) {
            Ok(record) => {
                records.values.insert(*name, record);
            }
            Err(error) => {
                records.reported.insert(*name);
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
    let directory = milestone.directory();
    for RecordDefinition {
        name,
        rule,
        description,
        ..
    } in definitions
    {
        let relative = format!("{directory}/{name}");
        let Some(record) = records.get(name) else {
            if !records.already_reported(name) {
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

pub(crate) fn verify_manifest_closure(
    manifest: &Value,
    rules: &[(Rule, &str)],
    violations: &mut Vec<String>,
) {
    for (rule, violation) in rules {
        super::rules::require_manifest_rule(manifest, rule, violation, violations);
    }
}

pub(crate) struct CarriedFigures {
    pub(crate) record: &'static str,
    pub(crate) subject: &'static str,
    pub(crate) figures: &'static [(&'static str, Figure)],
    pub(crate) arithmetic: &'static [(Rule, &'static str)],
}

pub(crate) fn verify_milestone_figures(
    milestone: Milestone,
    record: &Value,
    manifest: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    let label = format!("{}/{}", milestone.directory(), carried.record);
    verify_arithmetic(record, &label, carried.arithmetic, violations);
    verify_carried_figures(milestone, manifest, record, carried, violations);
}

#[cfg(test)]
mod tests;
