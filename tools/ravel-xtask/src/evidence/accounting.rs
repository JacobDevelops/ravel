//! Cross-milestone review and documentation arithmetic.

use std::path::{Path, PathBuf};

use crate::evidence::rules::{Milestone, milestone_manifests};
use crate::text::{numbers_after, numbers_before, read_text, verify_stated_counts};

const ROUND_COUNT_EXEMPT: [Milestone; 2] = [Milestone::M0, Milestone::M1];
const PLAN_DOCUMENT: &str = "ROADMAP.md";

const ROUND_NOUNS: [&str; 5] = ["rounds", "verdicts", "audits", "reviews", "iterations"];

fn verify_review_round_counts_for(
    root: &Path,
    manifests: &[(PathBuf, Milestone)],
    violations: &mut Vec<String>,
) {
    for (record, documents) in round_count_subjects(manifests) {
        if let Err(error) = crate::text::require_file(root, &record) {
            violations.push(error);
        }
        for relative in documents {
            if !root.join(&relative).exists() {
                continue;
            }
            match read_text(root, Path::new(&relative)) {
                Ok(document) => {
                    for (count, noun) in round_count_restatements(&document) {
                        violations.push(format!(
                            "{relative} restates that {count} review {noun} exist; \
                             {record} is the one record that counts them"
                        ));
                    }
                }
                Err(error) => violations.push(error),
            }
        }
    }

    match read_text(root, Path::new(PLAN_DOCUMENT)) {
        Ok(document) => {
            for (count, noun) in round_count_restatements(&document) {
                violations.push(format!(
                    "{PLAN_DOCUMENT} restates that {count} review {noun} exist; each milestone's \
                     review.md is the one record that counts its own rounds, and a count written \
                     in the plan is falsified by the next round"
                ));
            }
        }
        Err(error) => violations.push(error),
    }
}

fn round_count_subjects(manifests: &[(PathBuf, Milestone)]) -> Vec<(String, [String; 2])> {
    manifests
        .iter()
        .filter(|(_, milestone)| !ROUND_COUNT_EXEMPT.contains(milestone))
        .map(|(manifest, _)| {
            let directory = manifest
                .parent()
                .expect("manifest has a parent")
                .display()
                .to_string();
            (
                format!("{directory}/review.md"),
                [
                    format!("{directory}/README.md"),
                    manifest.display().to_string(),
                ],
            )
        })
        .collect()
}

fn round_count_restatements(document: &str) -> Vec<(usize, &'static str)> {
    ROUND_NOUNS
        .into_iter()
        .flat_map(|noun| {
            let mut counts = numbers_before(document, &[noun]);
            counts.extend(numbers_after(document, &[noun]));
            counts.into_iter().map(move |count| (count, noun))
        })
        .collect()
}

fn round_heading_blocking_counts(document: &str) -> Vec<(String, Option<usize>)> {
    document
        .lines()
        .filter(|line| line.starts_with(ROUND_HEADING))
        .map(|line| {
            (
                line.to_owned(),
                numbers_before(line, &["blocking"]).first().copied(),
            )
        })
        .collect()
}

const ROUND_HEADING: &str = "### Round ";

// These records predate per-round tallies; their `### Round` lines are subsections.
const UNTALLIED_RECORDS: [&str; 2] = ["evidence/m3/review.md", "evidence/m4/review.md"];

fn verify_review_blocking_counts_for(
    root: &Path,
    manifests: &[(PathBuf, Milestone)],
    violations: &mut Vec<String>,
) {
    for (record, _) in round_count_subjects(manifests) {
        if UNTALLIED_RECORDS.contains(&record.as_str()) {
            continue;
        }
        let path = root.join(&record);
        if !path.is_file() {
            continue;
        }

        let document = match read_text(root, &path) {
            Ok(document) => document,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        let per_round = round_heading_blocking_counts(&document);
        if per_round.is_empty() {
            continue;
        }

        for (heading, _) in per_round.iter().filter(|(_, count)| count.is_none()) {
            violations.push(format!(
                "{record} has a round heading that declares no blocking count, so the round is \
                 absent from the tally it belongs to: {}",
                heading.trim()
            ));
        }
        let total: usize = per_round.iter().filter_map(|(_, count)| *count).sum();
        let rounds = per_round.len();
        let summary = document
            .split_once(ROUND_HEADING)
            .map_or(document.as_str(), |(summary, _)| summary);

        let mut stated = numbers_before(summary, &["blocking", "findings"]);
        stated.extend(numbers_after(summary, &["blocking", "findings"]));
        verify_stated_counts(
            &stated,
            total,
            || {
                format!(
                    "{record} states no tally of the blocking findings its {rounds} round headings \
                 declare; the summary carries that figure so a reader is not left to sum them"
                )
            },
            |count| {
                format!(
                    "{record} states that {count} blocking findings were raised; its own round \
                 headings declare {total} across {rounds} rounds, and the headings are where \
                 that figure is written"
                )
            },
            violations,
        );
    }
}

fn verify_manifest_percentages_for(
    root: &Path,
    manifests: &[(PathBuf, Milestone)],
    violations: &mut Vec<String>,
) {
    for (relative, milestone) in manifests {
        let relative = relative.display().to_string();
        match read_text(root, Path::new(&relative)) {
            Ok(manifest) => {
                for line in manifest.lines().filter(|line| line.contains('%')) {
                    let head = line.split('%').next().unwrap_or("");
                    let start = head.len()
                        - head
                            .chars()
                            .rev()
                            .take_while(|character| character.is_ascii_digit() || *character == '.')
                            .count();
                    let figure = if start == head.len() {
                        "a percentage"
                    } else {
                        &head[start..]
                    };
                    violations.push(format!(
                        "{relative} writes {figure}% into {milestone}'s prose; no gate recomputes \
                         a percentage a manifest states, so it belongs in the record that \
                         measured it"
                    ));
                }
            }
            Err(error) => violations.push(error),
        }
    }
}

pub(crate) fn verify_accounting(
    root: &Path,
    milestone: Option<Milestone>,
    violations: &mut Vec<String>,
) {
    let manifests = match milestone_manifests(root) {
        Ok(manifests) => manifests,
        Err(error) => {
            violations.push(error);
            Vec::new()
        }
    };
    let manifests: Vec<_> = manifests
        .into_iter()
        .filter(|(_, each)| milestone.is_none_or(|selected| selected == *each))
        .collect();
    verify_review_round_counts_for(root, &manifests, violations);
    verify_review_blocking_counts_for(root, &manifests, violations);
    verify_manifest_percentages_for(root, &manifests, violations);
}

#[cfg(test)]
mod tests;
