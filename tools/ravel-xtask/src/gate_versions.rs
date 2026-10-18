//! Gate versions and recorded harness sections.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ravel_minecraft_generate::Eligibility;

use crate::version_boundary::{
    VersionEligibility, adapter_eligibility, missing_mapping_input, read_tracked_adapter,
    tracked_versions,
};

struct Record {
    path: &'static str,
    verb: &'static str,
    task: &'static str,
}

const GATE_RECORDS: [Record; 3] = [
    Record {
        path: "target/ravel-jvm/tested-versions.txt",
        verb: "tested",
        task: "ravel:test",
    },
    Record {
        path: "target/ravel-jvm/compiled-versions.txt",
        verb: "compiled",
        task: "ravel:verify-java",
    },
    Record {
        path: "target/ravel-jvm/harness-versions.txt",
        verb: "ran",
        task: "ravel:verify-jvm",
    },
];

const HARNESS_RECORD: &str = "target/ravel-jvm/harness-versions.txt";

const REQUIRED_SECTIONS: [&str; 13] = [
    "cluster",
    "content",
    "hierarchy",
    "initialization",
    "load",
    "members",
    "outward",
    "persistence",
    "refusal",
    "screens",
    "settings",
    "side",
    "transformation",
];

/// Separates incomplete local proof from invalid coverage records.
#[derive(Debug, Default)]
pub struct GateVersionsReport {
    /// Missing or contradictory records that fail the gate.
    pub violations: Vec<String>,
    /// Explicit skips and generation-only obligations, not live-support evidence.
    pub notices: Vec<String>,
}

/// Holds every gate task that loops versions to having looped all of them.
#[must_use]
pub fn verify_gate_versions(root: &Path) -> GateVersionsReport {
    let mut report = GateVersionsReport::default();
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };
    if versions.is_empty() {
        report
            .violations
            .push("version matrix tracks no Minecraft version, so it proves nothing".to_owned());
        return report;
    }

    let mut eligibility = VersionEligibility::new();
    let mut unavailable = BTreeSet::new();
    for version in &versions {
        let parsed = match read_tracked_adapter(root, version) {
            Ok(parsed) => parsed,
            Err(error) => {
                report.violations.push(error);
                continue;
            }
        };
        let value = match adapter_eligibility(&parsed, version) {
            Ok(value) => value,
            Err(error) => {
                report.violations.push(error);
                continue;
            }
        };
        eligibility.insert(version.clone(), value);
        if missing_mapping_input(root, &parsed).is_some() {
            unavailable.insert(version.as_str());
        }
        let generation_only = value == Eligibility::GenerationOnly;
        if generation_only {
            report.notices.push(format!(
                "Minecraft {version} is generation-only: adapter-test obligation not yet met"
            ));
            for section in REQUIRED_SECTIONS {
                report.notices.push(format!(
                    "Minecraft {version} is generation-only: harness {section} obligation \
                     not yet met; generation is not live support"
                ));
            }
        }
    }
    for record in &GATE_RECORDS {
        verify_record(
            root,
            record,
            &versions,
            &eligibility,
            &unavailable,
            &mut report,
        );
    }

    verify_harness_sections(root, &versions, &mut report.violations);
    report
}

fn verify_record(
    root: &Path,
    record: &Record,
    versions: &[String],
    eligibility: &VersionEligibility,
    unavailable: &BTreeSet<&str>,
    report: &mut GateVersionsReport,
) {
    let Ok(text) = fs::read_to_string(root.join(record.path)) else {
        report.violations.push(format!(
            "{} is absent, so nothing says which versions {} covered; run it before this check",
            record.path, record.task
        ));
        return;
    };
    let mut reached_live = false;
    for version in versions {
        let generation_only = eligibility.get(version) == Some(&Eligibility::GenerationOnly);
        let verb = match (generation_only, record.verb) {
            (true, "tested") => "built",
            (true, "ran") => "NOT-YET-RUN",
            _ => record.verb,
        };
        let prefix = format!("{verb}: ");
        let mut named = text
            .lines()
            .filter(|line| names_version(line, version))
            .peekable();
        if generation_only && verb != record.verb {
            let live_prefix = format!("{}: ", record.verb);
            if named.clone().any(|line| line.starts_with(&live_prefix)) {
                report.violations.push(format!(
                    "{} falsely claims Minecraft {version} {} despite generation-only eligibility",
                    record.path, record.verb
                ));
            }
        }
        if named.peek().is_none() {
            report.violations.push(format!(
                "{} names no outcome for Minecraft {version}, so {} either stopped looping \
                 the tracked versions or covered one without recording it",
                record.path, record.task
            ));
        } else if unavailable.contains(version.as_str())
            && named.clone().any(|line| line.starts_with("SKIPPED: "))
        {
            report.notices.extend(
                named
                    .filter(|line| line.starts_with("SKIPPED: "))
                    .map(str::to_owned),
            );
        } else if !named.any(|line| line.starts_with(&prefix)) {
            report.violations.push(format!(
                "{}: Minecraft {version} {} obligation not yet met by {}; no accepted \
                 outcome was recorded",
                record.path, verb, record.task
            ));
        } else if !generation_only {
            reached_live = true;
        }
    }
    if !reached_live {
        report.violations.push(format!(
            "{} records no live-eligible version {} actually reached, so no live coverage \
             was demonstrated",
            record.path, record.verb
        ));
    }
}

fn verify_harness_sections(root: &Path, versions: &[String], violations: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(root.join(HARNESS_RECORD)) else {
        return;
    };

    for version in versions {
        let Some(line) = text
            .lines()
            .find(|line| line.starts_with("ran: ") && names_version(line, version))
        else {
            continue;
        };
        let ran = line
            .split_once("sections=")
            .map(|(_, sections)| {
                sections
                    .split(',')
                    .map(str::trim)
                    .filter(|section| !section.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if ran.is_empty() {
            violations.push(format!(
                "{HARNESS_RECORD} says the harness ran for Minecraft {version} and names no \
                 section it ran, so what it covered is unrecorded"
            ));
            continue;
        }

        for section in &ran {
            if !REQUIRED_SECTIONS.contains(section) {
                violations.push(format!(
                    "{HARNESS_RECORD} says Minecraft {version} ran a {section} section and the \
                     harness has no such section, so a renamed section answers for the one it \
                     replaced and its absence goes unnoticed"
                ));
            }
        }

        for section in REQUIRED_SECTIONS {
            if !ran.contains(&section) {
                violations.push(format!(
                    "the harness did not run the {section} section for Minecraft {version}, and \
                     every tracked version runs every section: find out what broke rather than \
                     recording the gap, and raise a work package in STATUS.md and ROADMAP.md if \
                     a version turns out not to be able to run one"
                ));
            }
        }
    }
}

fn names_version(line: &str, version: &str) -> bool {
    let needle = format!("Minecraft {version}");
    line.match_indices(&needle).any(|(at, _)| {
        line[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_alphanumeric() && !matches!(next, '.' | '-' | '_' | '+'))
    })
}

#[cfg(test)]
mod tests;
