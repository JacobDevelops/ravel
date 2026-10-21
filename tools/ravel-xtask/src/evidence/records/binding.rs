//! Tracked artifact digests and the review verdicts those artifacts state.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ravel_core::Sha256Digest;
use sha2::{Digest, Sha256};

use super::Review;
use crate::evidence::model::Verdict;
use crate::text::{read_text, redacted_path};

pub(super) fn verify_review(
    root: &Path,
    path: &Path,
    review: &Review<'_>,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let Some(artifact) = &review.record else {
        return;
    };
    let label = redacted_path(root, path);
    let Some(directory) = path.parent() else {
        return;
    };
    if !names_a_path(artifact.name) {
        violations.push(format!(
            "{label} records a review record by title rather than by path, so its digest \
             binds nothing obtainable: {}",
            artifact.name
        ));
        return;
    }
    let prior_violations = violations.len();
    verify_tracked_digest(
        &TrackedDigest {
            label: &label,
            kind: "review record",
            root,
            tracked,
            manifest_path: path,
            directory,
            name: artifact.name,
            recorded: artifact.sha256,
        },
        violations,
    );
    if violations.len() != prior_violations {
        return;
    }
    verify_recorded_verdict(
        root,
        &label,
        review.result,
        &directory.join(artifact.name),
        violations,
    );
}

fn verify_recorded_verdict(
    root: &Path,
    label: &str,
    claimed: Option<Verdict>,
    record: &Path,
    violations: &mut Vec<String>,
) {
    let Some(claimed) = claimed else {
        violations.push(format!(
            "{label} binds a review record but records no review.result for it to agree with"
        ));
        return;
    };
    let text = match read_text(root, record) {
        Ok(text) => text,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    match text.lines().find_map(stated_verdict) {
        Some(stated) if stated == claimed => {}
        Some(stated) => violations.push(format!(
            "{label} records review.result {claimed} while the review record it binds states {stated}"
        )),
        None => violations.push(format!(
            "{label} binds a review record that states no verdict, so nothing checks that \
             review.result {claimed} is what the review concluded"
        )),
    }
}

fn stated_verdict(line: &str) -> Option<Verdict> {
    let line = line.trim().trim_start_matches(['#', '*', ' ']);
    let value = line
        .strip_prefix("Result:")
        .or_else(|| line.strip_prefix("Result**:"))?;
    let value = value.trim().trim_matches(['*', ' ']).to_ascii_lowercase();
    match value.as_str() {
        "pass" => Some(Verdict::Pass),
        "fail" => Some(Verdict::Fail),
        "not-required" => Some(Verdict::NotRequired),
        _ => None,
    }
}

pub(super) fn names_a_path(name: &str) -> bool {
    Path::new(name).extension().is_some()
}

pub(super) struct TrackedDigest<'a> {
    pub(super) label: &'a str,
    pub(super) kind: &'a str,
    pub(super) root: &'a Path,
    pub(super) tracked: &'a BTreeSet<String>,
    pub(super) manifest_path: &'a Path,
    pub(super) directory: &'a Path,
    pub(super) name: &'a str,
    pub(super) recorded: Sha256Digest,
}

pub(super) fn verify_tracked_digest(record: &TrackedDigest<'_>, violations: &mut Vec<String>) {
    let TrackedDigest {
        label,
        kind,
        root,
        tracked,
        manifest_path,
        directory,
        name,
        recorded,
    } = *record;
    if name.is_empty() || name.starts_with('/') || name.split('/').any(|part| part == "..") {
        violations.push(format!(
            "{label} records a {kind} outside its own milestone: {name}"
        ));
        return;
    }
    let path = directory.join(name);
    if path == manifest_path {
        violations.push(format!("{label} records its own digest, which cannot bind"));
        return;
    }
    let relative = redacted_path(root, &path);
    if !tracked.contains(&relative) {
        violations.push(format!(
            "{label} records a {kind} that the repository does not track, so its digest binds \
             nothing a reader can obtain: {name}"
        ));
        return;
    }
    match file_digest(&path) {
        Ok(actual) if actual == recorded => {}
        Ok(actual) => violations.push(format!(
            "{label} records {name} as {recorded} but it hashes {actual}"
        )),
        Err(error) => violations.push(format!("{label} records {name}: {error}")),
    }
}

fn file_digest(path: &Path) -> Result<Sha256Digest, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read the recorded file: {error}"))?;
    Ok(Sha256Digest::from_bytes(Sha256::digest(&bytes).into()))
}
