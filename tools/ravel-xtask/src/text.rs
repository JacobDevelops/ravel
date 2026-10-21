//! Shared text loading, diagnostic paths, and prose counts.

use std::fs;
use std::path::{Path, PathBuf};

/// Resolves a required repository file, keeping missing-file diagnostics repository-relative.
pub(crate) fn require_file(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = root.join(relative);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("missing required file: {relative}"))
    }
}

pub(crate) fn read_text(root: &Path, path: &Path) -> Result<String, String> {
    let rooted;
    let resolved = if path.is_absolute() || path.starts_with(root) {
        path
    } else {
        rooted = root.join(path);
        &rooted
    };
    fs::read_to_string(resolved)
        .map_err(|error| format!("cannot read {}: {error}", redacted_path(root, path)))
}

pub(crate) fn redacted_path(root: &Path, path: &Path) -> String {
    let label = if path.is_relative() && !path.starts_with(root) {
        path.display().to_string()
    } else {
        path.strip_prefix(root).map_or_else(
            |_| {
                path.file_name().map_or_else(
                    || "an unnamed path".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                )
            },
            |relative| relative.display().to_string(),
        )
    };
    label.replace('\\', "/")
}
const CARDINALS: [&str; 21] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

const TENS: [&str; 8] = [
    "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn numeral(word: &str) -> Option<usize> {
    let word = word
        .trim_matches(|character: char| !character.is_alphanumeric())
        .to_ascii_lowercase();
    if !word.is_empty() && word.chars().all(|character| character.is_ascii_digit()) {
        return word.parse().ok();
    }
    if let Some(count) = CARDINALS.iter().position(|spelled| *spelled == word) {
        return Some(count);
    }
    if let Some(tens) = TENS.iter().position(|spelled| *spelled == word) {
        return Some(tens_value(tens));
    }
    let (tens, unit) = word.split_once('-')?;
    let tens = TENS.iter().position(|spelled| *spelled == tens)?;
    let unit = CARDINALS.iter().position(|spelled| *spelled == unit)?;

    if !(1..=9).contains(&unit) {
        return None;
    }
    Some(tens_value(tens) + unit)
}

fn tens_value(at: usize) -> usize {
    (at + 2) * 10
}

const COUNT_LOOKBACK: usize = 4;

pub(crate) fn numbers_before(document: &str, noun: &[&str]) -> Vec<usize> {
    let words: Vec<&str> = document.split_whitespace().collect();
    let mut found = Vec::new();
    for (at, window) in words.windows(noun.len()).enumerate() {
        if !window_is_noun(noun, window) {
            continue;
        }
        let from = at.saturating_sub(COUNT_LOOKBACK);
        if let Some(count) = words[from..at].iter().rev().find_map(|word| numeral(word)) {
            found.push(count);
        }
    }
    found
}

pub(crate) fn numbers_after(document: &str, noun: &[&str]) -> Vec<usize> {
    let words: Vec<&str> = document.split_whitespace().collect();
    let mut found = Vec::new();
    for (at, window) in words.windows(noun.len()).enumerate() {
        if !window_is_noun(noun, window) {
            continue;
        }
        if words[at.saturating_sub(COUNT_LOOKBACK)..at]
            .iter()
            .any(|word| numeral(word).is_some())
        {
            continue;
        }
        let from = at + noun.len();
        if noun.len() == 1
            && !window[0].ends_with(':')
            && words.get(from).is_some_and(|word| numeral(word).is_some())
        {
            continue;
        }
        let to = (at + noun.len() + COUNT_LOOKBACK).min(words.len());
        if let Some(count) = words[from..to].iter().find_map(|word| numeral(word)) {
            found.push(count);
        }
    }
    found
}

fn window_is_noun(noun: &[&str], window: &[&str]) -> bool {
    noun.iter().zip(window).all(|(expected, word)| {
        word.trim_matches(|character: char| !character.is_alphanumeric())
            .eq_ignore_ascii_case(expected)
    })
}

pub(crate) fn verify_stated_counts(
    stated: &[usize],
    expected: usize,
    absent: impl FnOnce() -> String,
    differs: impl Fn(usize) -> String,
    violations: &mut Vec<String>,
) {
    if stated.is_empty() {
        violations.push(absent());
    }
    for &count in stated {
        if count != expected {
            violations.push(differs(count));
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum NumberAt {
    BeforePhrase,
    AfterPhrase,
}

pub(crate) fn stated_counts(document: &str, phrase: &str, number_at: NumberAt) -> Vec<usize> {
    let mut counts = Vec::new();
    let mut offset = 0;
    while let Some(found) = document[offset..].find(phrase) {
        let at = offset + found;
        offset = at + phrase.len();
        let digits = match number_at {
            NumberAt::BeforePhrase => {
                let head = document[..at].trim_end();
                let start =
                    head.len() - head.chars().rev().take_while(char::is_ascii_digit).count();
                &head[start..]
            }
            NumberAt::AfterPhrase => {
                let tail = document[offset..].trim_start();
                let end = tail.chars().take_while(char::is_ascii_digit).count();
                &tail[..end]
            }
        };
        if let Ok(count) = digits.parse() {
            counts.push(count);
        }
    }
    counts
}

#[cfg(test)]
mod tests;
