//! Gate versions and recorded harness sections.

use std::fs;
use std::path::Path;

use crate::version_boundary::tracked_versions;

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

/// Holds every gate task that loops versions to having looped all of them.
#[must_use]
pub fn verify_gate_versions(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => return vec![error],
    };

    for record in &GATE_RECORDS {
        let Ok(text) = fs::read_to_string(root.join(record.path)) else {
            violations.push(format!(
                "{} is absent, so nothing says which versions {} covered; run it before this \
                 check",
                record.path, record.task
            ));
            continue;
        };
        let lines = text.lines().collect::<Vec<_>>();
        let mut reached = 0;
        for version in &versions {
            let named = lines.iter().filter(|line| names_version(line, version));
            let mut positive = false;
            let mut mentioned = false;
            for line in named {
                mentioned = true;
                if line.starts_with(&format!("{}: ", record.verb)) {
                    positive = true;
                }
            }
            if !mentioned {
                violations.push(format!(
                    "{} names no outcome for Minecraft {version}, so {} either stopped looping \
                     the tracked versions or covered one without recording it",
                    record.path, record.task
                ));
                continue;
            }
            if positive {
                reached += 1;
            }
        }
        if reached == 0 {
            violations.push(format!(
                "{} records no version {} actually reached, so the task exited zero having \
                 proved nothing",
                record.path, record.verb
            ));
        }
    }

    verify_harness_sections(root, &versions, &mut violations);
    violations
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
