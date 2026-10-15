//! Shared closure-record, arithmetic, and carried-figure checks.

use std::path::Path;

use crate::evidence::manifest::verify_redaction;
use crate::json::{jq_matches, jq_value};

pub(crate) fn verify_record_arithmetic(
    record: &Path,
    record_label: &str,
    rules: &[(&str, &str)],
    violations: &mut Vec<String>,
) {
    for (filter, requirement) in rules {
        if !jq_matches(record, filter).unwrap_or(false) {
            violations.push(format!(
                "{record_label} does not satisfy `{filter}`, and {requirement}"
            ));
        }
    }
}

fn verify_carried_figures(
    manifest_path: &Path,
    manifest_label: &str,
    record: &Path,
    record_label: &str,
    subject: &str,
    figures: &[(&str, &str)],
    violations: &mut Vec<String>,
) {
    for (claimed_at, derived_from) in figures {
        let (Ok(claimed), Ok(derived)) = (
            jq_value(manifest_path, claimed_at),
            jq_value(record, derived_from),
        ) else {
            violations.push(format!(
                "{manifest_label} {claimed_at} and {record_label} \
                 `{derived_from}` cannot both be read, so nothing reconciles them"
            ));
            continue;
        };
        if claimed == "null" || derived == "null" {
            violations.push(format!(
                "{manifest_label} {claimed_at} or the record's `{derived_from}` is \
                 absent; a figure the gate cannot resolve is a figure nothing binds"
            ));
            continue;
        }
        if claimed != derived {
            violations.push(format!(
                "{manifest_label} records {claimed_at} {claimed} while \
                 {record_label} derives {derived} from `{derived_from}`; one \
                 {subject} produced both"
            ));
        }
    }
}

pub(crate) fn verify_closure_records(
    root: &Path,
    milestone: &str,
    directory: &str,
    records: &[(&str, &str, &str)],
    violations: &mut Vec<String>,
) {
    for (file, rule, requirement) in records {
        let path = root.join(directory).join(file);
        if !path.is_file() {
            violations.push(format!(
                "{milestone} closure requires the record {directory}/{file}"
            ));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("{milestone} closure requires {requirement}"));
        }
        verify_redaction(&path, &format!("{directory}/{file}"), violations);
    }
}

pub(crate) struct CarriedFigures {
    pub(crate) manifest: &'static str,
    pub(crate) record: &'static str,
    pub(crate) subject: &'static str,
    pub(crate) figures: &'static [(&'static str, &'static str)],
}

pub(crate) fn verify_milestone_figures(
    root: &Path,
    manifest_path: &Path,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    let record = root.join(carried.record);
    if !record.is_file() {
        return;
    }
    verify_carried_figures(
        manifest_path,
        carried.manifest,
        &record,
        carried.record,
        carried.subject,
        carried.figures,
        violations,
    );
}

#[cfg(test)]
mod tests;
