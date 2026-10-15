//! Shared record predicates and carried-figure comparisons.

use std::path::Path;

use serde_json::Value;

use crate::evidence::manifest::verify_redaction_value;
use crate::json::{Figure, Rule, read_json};

pub(crate) fn verify_record_arithmetic(
    root: &Path,
    record_label: &str,
    rules: &[(Rule, &str)],
    violations: &mut Vec<String>,
) {
    let path = root.join(record_label);
    if !path.is_file() {
        return;
    }
    match read_json(&path) {
        Ok(record) => {
            for (rule, description) in rules {
                if !rule(&record) {
                    violations.push(format!("{record_label}: {description}"));
                }
            }
        }
        Err(error) => violations.push(error),
    }
}

pub(crate) fn verify_carried_figures(
    manifest: &Value,
    manifest_label: &str,
    record: &Value,
    record_label: &str,
    subject: &str,
    figures: &[(&str, Figure)],
    violations: &mut Vec<String>,
) {
    for (pointer, derive) in figures {
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

pub(crate) fn verify_closure_records(
    root: &Path,
    milestone: &str,
    directory: &str,
    records: &[(&str, Rule, &str)],
    violations: &mut Vec<String>,
) {
    for (name, rule, description) in records {
        let relative = format!("{directory}/{name}");
        let path = root.join(&relative);
        if !path.is_file() {
            violations.push(format!(
                "{milestone} closure requires the record {relative}"
            ));
            continue;
        }
        match read_json(&path) {
            Ok(record) => {
                if !rule(&record) {
                    violations.push(format!(
                        "{milestone} closure requires {description}: {relative}"
                    ));
                }
                verify_redaction_value(&record, &relative, violations);
            }
            Err(error) => violations.push(error),
        }
    }
}

pub(crate) struct CarriedFigures {
    pub(crate) manifest: &'static str,
    pub(crate) record: &'static str,
    pub(crate) subject: &'static str,
    pub(crate) figures: &'static [(&'static str, Figure)],
}

pub(crate) fn verify_milestone_figures(
    root: &Path,
    manifest: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    let path = root.join(carried.record);
    if !path.is_file() {
        return;
    }
    match read_json(&path) {
        Ok(record) => verify_carried_figures(
            manifest,
            carried.manifest,
            &record,
            carried.record,
            carried.subject,
            carried.figures,
            violations,
        ),
        Err(error) => violations.push(error),
    }
}

#[cfg(test)]
mod tests;
