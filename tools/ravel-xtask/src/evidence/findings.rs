//! Finding anchors against repository sources.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::manifest::Milestone;
use crate::json::{read_text, relative_label};
use crate::tracked::tracked_paths;

const RUST_DEFINITION_KEYWORDS: [&str; 8] = [
    "fn", "struct", "enum", "trait", "union", "const", "static", "type",
];

#[derive(Deserialize)]
struct Finding {
    summary: String,
    #[serde(default)]
    anchor: Option<Anchor>,
}

#[derive(Deserialize)]
struct Anchor {
    symbol: String,
    paths: Vec<String>,
}

pub(crate) fn verify_finding_anchors_value(
    root: &Path,
    manifest_path: &Path,
    milestone: Milestone,
    document: &Value,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let tracked = match tracked_paths(root) {
        Ok(tracked) => tracked,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let sources = tracked
        .into_iter()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
        })
        .collect::<Vec<_>>();
    let defined = defined_rust_symbols(root, &sources);
    let mut findings = Vec::new();
    for field in ["deviations", "risks"] {
        let Some(rows) = document[field].as_array() else {
            violations.push(format!("{label}: {field} must be an array"));
            return;
        };
        for row in rows {
            match serde_json::from_value::<Finding>(row.clone()) {
                Ok(finding) => findings.push(finding),
                Err(error) => violations.push(format!("{label}: invalid {field} finding: {error}")),
            }
        }
    }
    let anchored_from = match milestone {
        Milestone::M0 | Milestone::M1 | Milestone::M2 | Milestone::M3 | Milestone::M4 => false,
        Milestone::M5
        | Milestone::M6
        | Milestone::M7
        | Milestone::M8
        | Milestone::M9
        | Milestone::M10
        | Milestone::M11
        | Milestone::M12
        | Milestone::M13 => true,
    };
    verify_finding_anchors_rows(
        root,
        &label,
        &sources,
        &defined,
        &findings,
        anchored_from,
        violations,
    );
}

fn verify_finding_anchors_rows(
    root: &Path,
    label: &str,
    sources: &[String],
    defined: &BTreeSet<String>,
    findings: &[Finding],
    anchored_from: bool,
    violations: &mut Vec<String>,
) {
    for finding in findings {
        let summary = &finding.summary;
        let symbol = finding
            .anchor
            .as_ref()
            .map_or("", |anchor| anchor.symbol.as_str());
        let paths = finding
            .anchor
            .as_ref()
            .map_or(&[][..], |anchor| anchor.paths.as_slice());
        let named = named_rust_symbols(summary, defined);

        if symbol.is_empty() {
            if anchored_from && !named.is_empty() {
                violations.push(format!(
                    "{label} carries a finding naming the Rust {} {} with no anchor; a finding \
                     naming a symbol must claim something a grep can falsify",
                    if named.len() == 1 {
                        "symbol"
                    } else {
                        "symbols"
                    },
                    named.join(", ")
                ));
            }
            continue;
        }

        if !named.iter().any(|candidate| candidate == symbol) {
            violations.push(format!(
                "{label} anchors a finding on {symbol}, which the finding's own summary does not \
                 name; an anchor over a symbol nobody wrote about constrains nothing"
            ));
        }
        if !defined.contains(symbol) {
            violations.push(format!(
                "{label} anchors a finding on {symbol}, which no tracked Rust source defines"
            ));
            continue;
        }
        let allowed = paths
            .iter()
            .map(String::as_str)
            .filter(|prefix| !prefix.is_empty())
            .collect::<Vec<_>>();
        for source in sources_naming(root, sources, symbol) {
            if !allowed.iter().any(|prefix| source.starts_with(prefix)) {
                violations.push(format!(
                    "{label} anchors a finding on {symbol} outside {paths:?}, and \
                     {source} names it; the finding has outlived the tree it describes"
                ));
            }
        }
    }
}

pub(crate) fn defined_rust_symbols(root: &Path, sources: &[String]) -> BTreeSet<String> {
    let mut defined = BTreeSet::new();
    for relative in sources {
        let Ok(source) = read_text(root, Path::new(relative)) else {
            continue;
        };
        for line in source.lines() {
            let mut words = line.split_whitespace();
            while let Some(word) = words.next() {
                let keyword = word.trim_end_matches('!');
                if keyword == "macro_rules" || RUST_DEFINITION_KEYWORDS.contains(&keyword) {
                    let Some(name) = words.clone().next() else {
                        continue;
                    };
                    let name = name
                        .split(|character: char| !is_identifier_character(character))
                        .next()
                        .unwrap_or_default();
                    if distinguishable_symbol(name) {
                        defined.insert(name.to_owned());
                    }
                }
            }
        }
    }
    defined
}

pub(crate) fn named_rust_symbols(summary: &str, defined: &BTreeSet<String>) -> Vec<String> {
    summary
        .split(|character: char| !is_identifier_character(character))
        .filter(|word| defined.contains(*word))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn sources_naming(root: &Path, sources: &[String], symbol: &str) -> Vec<String> {
    sources
        .iter()
        .filter(|relative| {
            read_text(root, Path::new(relative)).is_ok_and(|source| {
                source
                    .split(|character: char| !is_identifier_character(character))
                    .any(|word| word == symbol)
            })
        })
        .cloned()
        .collect()
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

pub(crate) fn distinguishable_symbol(name: &str) -> bool {
    name.len() >= 4
        && name.starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
        && (name.contains('_')
            || name
                .chars()
                .skip(1)
                .any(|character| character.is_ascii_uppercase()))
}
