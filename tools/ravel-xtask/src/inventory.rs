//! Documentation and generated-inventory consistency.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::TEST_MOD_NAMES;
use crate::evidence::findings::relative_label;
use crate::evidence::manifest::MILESTONE_MANIFESTS;
use crate::java_shape::{descriptor_handles, quoted_value};
use crate::json::jq_value;
use crate::tracked::collect_files;
use crate::version_boundary::{HookInventory, VERSION_DATA};

const BUDGET_RECORDS: &[&str] = &["evidence/m2/loader-overhead.json"];

pub(crate) const VOID_LIMITS: &[(&str, &str)] = &[
    (
        "build.packCompile",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; the build cost of source distribution is evidence/m3/pack-compile.json",
    ),
    (
        "build.relink",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; nothing relinks a pack under the selected mechanism",
    ),
    (
        "build.binaryGrowthPerMod",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; per-mod binary growth describes an artefact the mechanism does not produce",
    ),
];

pub(crate) const EXAMPLE_NAMES: &[&str] = &[
    "content-mod",
    "guide-snippets",
    "minimal-mod",
    "stateful-mod",
];

const HOOK_DOCUMENT: &str = "docs/architecture/m3-hook-targets.md";

const CAMPAIGN_MATRIX: &str = "evidence/m3/linux-process-matrix.json";

const CAMPAIGN_DOCUMENT: &str = "evidence/m3/README.md";

const ROUND_COUNT_EXEMPT: [&str; 2] = ["M0", "M1"];

const PLAN_DOCUMENT: &str = "ROADMAP.md";

const PERSISTENCE_HARNESS: &str = "jvm/harness/run-harness.sh";

pub(crate) struct HookCounts {
    pub(crate) entries: usize,
    methods: usize,
    fields: usize,
    handles: usize,
}

pub(crate) fn verify_hook_inventory_document(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    let document = match fs::read_to_string(root.join(HOOK_DOCUMENT)) {
        Ok(text) => text,
        Err(error) => {
            violations.push(format!("cannot read {HOOK_DOCUMENT}: {error}"));
            return;
        }
    };

    let named = documented_versions(&document);
    let mut naming = named.iter();
    let (Some(version), None) = (naming.next(), naming.next()) else {
        let found = named.iter().cloned().collect::<Vec<_>>().join(", ");
        violations.push(format!(
            "{HOOK_DOCUMENT} links into {} version directories ({found}); it states one \
             inventory's counts, so the inventory it is held to has to be the one it names",
            named.len()
        ));
        return;
    };

    let Some(inventory) = inventories
        .iter()
        .find(|inventory| &inventory.version == version)
    else {
        violations.push(format!(
            "{HOOK_DOCUMENT} states the counts of {VERSION_DATA}/{version}, which is not a \
             tracked version whose inventory can be read"
        ));
        return;
    };

    let counts = match hook_counts(&inventory.text) {
        Ok(counts) => counts,
        Err(error) => {
            violations.push(format!("cannot count {}: {error}", inventory.path));
            return;
        }
    };
    violations.extend(hook_document_violations(
        &document,
        &inventory.path,
        &counts,
    ));
}

fn documented_versions(document: &str) -> BTreeSet<String> {
    let prefix = format!("{VERSION_DATA}/");
    let mut named = BTreeSet::new();
    for (start, _) in document.match_indices(prefix.as_str()) {
        let rest = &document[start + prefix.len()..];
        let bytes = rest.as_bytes();
        let mut end = 0;
        while end < bytes.len() && bytes[end] != b'/' && !bytes[end].is_ascii_whitespace() {
            end += 1;
        }
        if end > 0 && bytes.get(end) == Some(&b'/') {
            named.insert(rest[..end].to_owned());
        }
    }
    named
}

pub(crate) fn hook_counts(inventory: &str) -> Result<HookCounts, String> {
    let mut counts = HookCounts {
        entries: 0,
        methods: 0,
        fields: 0,
        handles: 0,
    };
    for block in inventory.split("[[hook]]").skip(1) {
        let body = block
            .split("\n[")
            .next()
            .expect("splitting a string yields at least one part");
        let mut kind = None;
        let mut descriptor = None;
        for line in body.lines().map(str::trim) {
            if let Some(value) = quoted_value(line, "kind") {
                kind = Some(value);
            } else if let Some(value) = quoted_value(line, "descriptor") {
                descriptor = Some(value);
            }
        }
        let (Some(kind), Some(descriptor)) = (kind, descriptor) else {
            return Err("a `[[hook]]` entry declares no kind or no descriptor".to_owned());
        };

        counts.entries += 1;
        if kind.ends_with("-field") {
            counts.fields += 1;
        } else {
            counts.methods += 1;
            counts.handles += descriptor_handles(descriptor)
                .ok_or_else(|| format!("`{descriptor}` is not a method descriptor"))?;
        }
    }

    if counts.entries == 0 {
        return Err("the inventory declares no `[[hook]]` entry".to_owned());
    }

    Ok(counts)
}

fn hook_document_violations(document: &str, inventory: &str, counts: &HookCounts) -> Vec<String> {
    let mut violations = Vec::new();
    for (phrase, side, expected, subject) in [
        (" entries to ", Side::After, counts.entries, "hook entries"),
        (
            " methods and ",
            Side::Before,
            counts.methods,
            "method entries",
        ),
        (" fields", Side::Before, counts.fields, "field entries"),
        (
            " handle parameters",
            Side::Before,
            counts.handles,
            "handle parameters",
        ),
    ] {
        let stated = stated_counts(document, phrase, side);
        if stated.is_empty() {
            violations.push(format!(
                "{HOOK_DOCUMENT} no longer writes a count against `{phrase}`, so nothing in it is \
                 held to {inventory}'s {expected} {subject}"
            ));
            continue;
        }
        for count in stated {
            if count != expected {
                violations.push(format!(
                    "{HOOK_DOCUMENT} states {count} {subject} where {inventory} declares \
                     {expected}"
                ));
            }
        }
    }
    violations
}

#[derive(Clone, Copy)]
enum Side {
    Before,
    After,
}

fn stated_counts(document: &str, phrase: &str, side: Side) -> Vec<usize> {
    let mut counts = Vec::new();
    let mut offset = 0;
    while let Some(found) = document[offset..].find(phrase) {
        let at = offset + found;
        offset = at + phrase.len();
        let digits = match side {
            Side::Before => {
                let head = document[..at].trim_end();
                let start =
                    head.len() - head.chars().rev().take_while(char::is_ascii_digit).count();
                &head[start..]
            }
            Side::After => {
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

struct CampaignInventory {
    classes: usize,
    targets: usize,
    prior: usize,
    launches: usize,
}

pub(crate) fn verify_campaign_inventory(root: &Path, violations: &mut Vec<String>) {
    let matrix = root.join(CAMPAIGN_MATRIX);
    if !matrix.is_file() {
        violations.push(format!("missing required file: {CAMPAIGN_MATRIX}"));
        return;
    }
    let Some(derived) = campaign_inventory(&matrix, violations) else {
        return;
    };

    for (field, expected, subject) in [
        (
            "classesInInventory",
            derived.classes,
            "classes its own target inventory enumerates",
        ),
        (
            "callableTargetsInInventory",
            derived.targets,
            "callable targets its own target inventory enumerates",
        ),
    ] {
        match matrix_count(&matrix, &format!(".rewrite.{field}")) {
            Ok(recorded) if recorded == expected => {}
            Ok(recorded) => violations.push(format!(
                "{CAMPAIGN_MATRIX} records {field} {recorded} against the {expected} {subject}"
            )),
            Err(error) => violations.push(error),
        }
    }

    match fs::read_to_string(root.join(CAMPAIGN_DOCUMENT)) {
        Ok(document) => violations.extend(campaign_document_violations(&document, &derived)),
        Err(error) => violations.push(format!("cannot read {CAMPAIGN_DOCUMENT}: {error}")),
    }
}

fn campaign_inventory(matrix: &Path, violations: &mut Vec<String>) -> Option<CampaignInventory> {
    for (filter, subject) in [
        (
            "[.rewrite.targetInventory[].class]",
            "lists a class more than once",
        ),
        (
            "[.rewrite.targetInventory[].methods[]]",
            "lists a callable target more than once",
        ),
    ] {
        let listed = matrix_count(matrix, &format!("{filter} | length"));
        let distinct = matrix_count(matrix, &format!("{filter} | unique | length"));
        match (listed, distinct) {
            (Ok(listed), Ok(distinct)) if listed == distinct => {}
            (Ok(_), Ok(_)) => {
                violations.push(format!("{CAMPAIGN_MATRIX}'s target inventory {subject}"));
                return None;
            }
            (Err(error), _) | (_, Err(error)) => {
                violations.push(error);
                return None;
            }
        }
    }

    let classes = matrix_count(
        matrix,
        "[.rewrite.targetInventory[].class] | unique | length",
    );
    let targets = matrix_count(matrix, "[.rewrite.targetInventory[].methods[]] | length");
    let prior = matrix_count(matrix, ".matrixNotCarriedForward.priorCallableTargets");
    let launches = matrix_count(matrix, ".surfaceCampaign.launches");
    match (classes, targets, prior, launches) {
        (Ok(classes), Ok(targets), Ok(prior), Ok(launches)) => Some(CampaignInventory {
            classes,
            targets,
            prior,
            launches,
        }),
        (Err(error), _, _, _)
        | (_, Err(error), _, _)
        | (_, _, Err(error), _)
        | (_, _, _, Err(error)) => {
            violations.push(error);
            None
        }
    }
}

fn matrix_count(matrix: &Path, filter: &str) -> Result<usize, String> {
    let value = jq_value(matrix, filter)?;
    value.parse().map_err(|_| {
        format!("{CAMPAIGN_MATRIX} does not record a whole number at `{filter}`, but `{value}`")
    })
}

fn campaign_document_violations(document: &str, derived: &CampaignInventory) -> Vec<String> {
    let mut violations = Vec::new();

    let document = document.split_whitespace().collect::<Vec<_>>().join(" ");
    let document = document.as_str();
    for (phrase, side, expected, subject) in [
        (
            " callable targets",
            Side::Before,
            derived.targets,
            "callable targets",
        ),
        (
            " classes in the reviewed inventory",
            Side::Before,
            derived.classes,
            "classes in the reviewed inventory",
        ),
        (
            "moved from ",
            Side::After,
            derived.prior,
            "callable targets before the count moved",
        ),
    ] {
        let stated = stated_counts(document, phrase, side);
        if stated.is_empty() {
            violations.push(format!(
                "{CAMPAIGN_DOCUMENT} no longer writes a count against `{phrase}`, so nothing in it \
                 is held to {CAMPAIGN_MATRIX}'s {expected} {subject}"
            ));
            continue;
        }
        for count in stated {
            if count != expected {
                violations.push(format!(
                    "{CAMPAIGN_DOCUMENT} states {count} {subject} where {CAMPAIGN_MATRIX} records \
                     {expected}"
                ));
            }
        }
    }

    let stated = numbers_before(document, &["surface", "launches"]);
    if stated.is_empty() {
        violations.push(format!(
            "{CAMPAIGN_DOCUMENT} no longer writes a count against `surface launches`, so nothing \
             in it is held to {CAMPAIGN_MATRIX}'s {} launches in the surface campaign",
            derived.launches
        ));
    }
    for count in stated {
        if count != derived.launches {
            violations.push(format!(
                "{CAMPAIGN_DOCUMENT} states {count} surface launches where {CAMPAIGN_MATRIX} \
                 records {}",
                derived.launches
            ));
        }
    }
    violations
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

fn numbers_before(document: &str, noun: &[&str]) -> Vec<usize> {
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

fn numbers_after(document: &str, noun: &[&str]) -> Vec<usize> {
    let words: Vec<&str> = document.split_whitespace().collect();
    let mut found = Vec::new();
    for (at, window) in words.windows(noun.len()).enumerate() {
        if !window_is_noun(noun, window) {
            continue;
        }
        let from = at + noun.len() + 1;
        let to = (at + noun.len() + COUNT_LOOKBACK).min(words.len());
        if from >= to {
            continue;
        }
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

const ROUND_NOUNS: [&str; 5] = ["rounds", "verdicts", "audits", "reviews", "iterations"];

pub(crate) fn verify_review_round_counts(root: &Path, violations: &mut Vec<String>) {
    for (record, documents) in round_count_subjects() {
        if !root.join(&record).is_file() {
            violations.push(format!("missing required file: {record}"));
        }
        for relative in documents {
            if !root.join(&relative).exists() {
                continue;
            }
            match fs::read_to_string(root.join(&relative)) {
                Ok(document) => {
                    for (count, noun) in round_count_restatements(&document) {
                        violations.push(format!(
                            "{relative} restates that {count} review {noun} exist; \
                             {record} is the one record that counts them"
                        ));
                    }
                }
                Err(error) => violations.push(format!("cannot read {relative}: {error}")),
            }
        }
    }

    match fs::read_to_string(root.join(PLAN_DOCUMENT)) {
        Ok(document) => {
            for (count, noun) in round_count_restatements(&document) {
                violations.push(format!(
                    "{PLAN_DOCUMENT} restates that {count} review {noun} exist; each milestone's \
                     review.md is the one record that counts its own rounds, and a count written \
                     in the plan is falsified by the next round"
                ));
            }
        }
        Err(error) => violations.push(format!("cannot read {PLAN_DOCUMENT}: {error}")),
    }
}

fn round_count_subjects() -> Vec<(String, [String; 2])> {
    MILESTONE_MANIFESTS
        .iter()
        .filter(|(_, milestone)| !ROUND_COUNT_EXEMPT.contains(milestone))
        .map(|(manifest, milestone)| {
            let directory = format!("evidence/{}", milestone.to_ascii_lowercase());
            (
                format!("{directory}/review.md"),
                [format!("{directory}/README.md"), (*manifest).to_owned()],
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

pub(crate) fn verify_review_blocking_counts(root: &Path, violations: &mut Vec<String>) {
    for (record, _) in round_count_subjects() {
        let path = root.join(&record);
        if !path.is_file() {
            continue;
        }

        let Ok(document) = fs::read_to_string(&path) else {
            continue;
        };
        let per_round = round_heading_blocking_counts(&document);
        if per_round.is_empty() {
            continue;
        }

        if per_round.iter().all(|(_, count)| count.is_none()) {
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
            .split(ROUND_HEADING)
            .next()
            .unwrap_or_default()
            .to_owned();

        let mut stated = numbers_before(&summary, &["blocking", "findings"]);
        stated.extend(numbers_after(&summary, &["blocking", "findings"]));
        if stated.is_empty() {
            violations.push(format!(
                "{record} states no tally of the blocking findings its {rounds} round headings \
                 declare; the summary carries that figure so a reader is not left to sum them"
            ));
        }
        for count in stated {
            if count != total {
                violations.push(format!(
                    "{record} states that {count} blocking findings were raised; its own round \
                     headings declare {total} across {rounds} rounds, and the headings are where \
                     that figure is written"
                ));
            }
        }
    }
}

pub(crate) fn verify_manifest_percentages(root: &Path, violations: &mut Vec<String>) {
    for (relative, milestone) in MILESTONE_MANIFESTS {
        match fs::read_to_string(root.join(relative)) {
            Ok(manifest) => {
                for line in manifest.lines().filter(|line| line.contains('%')) {
                    let figure = line
                        .split('%')
                        .next()
                        .map(|head| {
                            head.chars()
                                .rev()
                                .take_while(|character| {
                                    character.is_ascii_digit() || *character == '.'
                                })
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect::<String>()
                        })
                        .filter(|figure| !figure.is_empty())
                        .unwrap_or_else(|| "a percentage".to_owned());
                    violations.push(format!(
                        "{relative} writes {figure}% into {milestone}'s prose; no gate recomputes \
                         a percentage a manifest states, so it belongs in the record that \
                         measured it"
                    ));
                }
            }
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }
}

pub(crate) fn verify_void_limits(root: &Path, violations: &mut Vec<String>) {
    let record = root.join(BUDGET_RECORDS[0]);
    let recorded = match jq_value(&record, "[.budget.limits[].metric] | join(\" \")") {
        Ok(recorded) => recorded,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let recorded: Vec<&str> = recorded.split_whitespace().collect();
    let roadmap = match fs::read_to_string(root.join("ROADMAP.md")) {
        Ok(roadmap) => roadmap,
        Err(error) => {
            violations.push(format!("cannot read ROADMAP.md: {error}"));
            return;
        }
    };
    let Some(declaration) = roadmap
        .split("\n\n")
        .find(|paragraph| paragraph.contains("are void"))
        .map(str::to_owned)
    else {
        violations.push(
            "ROADMAP.md declares no limit void, and evidence/m2/loader-overhead.json's expiry \
             clause voided three of its own"
                .to_owned(),
        );
        return;
    };
    let roadmap = declaration;

    for (metric, why) in VOID_LIMITS {
        if !recorded.contains(metric) {
            violations.push(format!(
                "{} is treated as void and {} carries no limit for it, so the void set describes a \
                 record that is not there",
                metric, BUDGET_RECORDS[0]
            ));
        }
        if !roadmap.contains(metric) {
            violations.push(format!(
                "ROADMAP.md's declaration of which limits are void does not name {metric}, which \
                 is {why}; a limit the gate stops comparing and the roadmap stops declaring is \
                 indistinguishable from one that passed"
            ));
        }
    }
    for metric in recorded {
        if VOID_LIMITS.iter().any(|(void, _)| *void == metric) {
            continue;
        }
        if roadmap.contains(metric) {
            violations.push(format!(
                "ROADMAP.md declares {metric} void where it says which of {}'s limits are, and \
                 that limit still stands",
                BUDGET_RECORDS[0]
            ));
        }
    }
}

pub(crate) fn verify_harness_lifecycles(root: &Path, violations: &mut Vec<String>) {
    let harness = match fs::read_to_string(root.join(PERSISTENCE_HARNESS)) {
        Ok(harness) => harness,
        Err(error) => {
            violations.push(format!("cannot read {PERSISTENCE_HARNESS}: {error}"));
            return;
        }
    };
    let declared = harness
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("persistence_lifecycles_declared="))
        .and_then(|value| value.parse::<usize>().ok());
    let Some(declared) = declared else {
        violations.push(format!(
            "{PERSISTENCE_HARNESS} no longer declares `persistence_lifecycles_declared`, so no \
             record's world-lifecycle count is held to it"
        ));
        return;
    };

    for relative in [CAMPAIGN_DOCUMENT, CAMPAIGN_MATRIX] {
        match fs::read_to_string(root.join(relative)) {
            Ok(document) => {
                let stated = numbers_before(&document, &["world", "lifecycles"]);
                if stated.is_empty() {
                    violations.push(format!(
                        "{relative} no longer writes a count against `world lifecycles`, so \
                         nothing in it is held to {PERSISTENCE_HARNESS}'s {declared}"
                    ));
                }
                for count in stated {
                    if count != declared {
                        violations.push(format!(
                            "{relative} states {count} world lifecycles where \
                             {PERSISTENCE_HARNESS} declares {declared}"
                        ));
                    }
                }
            }
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }
}

const GUIDE_DIRECTORY: &str = "docs/guide";

const NETWORK_SOURCE: &str = "crates/ravel-network/src/lib.rs";

const ENUMERATED_IN_FULL: &str = "WireError";

const NAMED_IN_PART: &str = "DeliveryRefused";

const ENUMERATING_DOCUMENT: &str = "docs/guide/running.md";

const COMPLETENESS_CLAIM: &str = "is the whole of those refusals";

pub(crate) fn verify_guide_enumerations(root: &Path, violations: &mut Vec<String>) {
    let source = match fs::read_to_string(root.join(NETWORK_SOURCE)) {
        Ok(source) => source,
        Err(error) => {
            violations.push(format!("cannot read {NETWORK_SOURCE}: {error}"));
            return;
        }
    };
    let mut guide = String::new();
    let mut documents = Vec::new();
    collect_files(&root.join(GUIDE_DIRECTORY), &mut documents);
    documents.sort();
    for path in &documents {
        match fs::read_to_string(path) {
            Ok(document) => guide.push_str(&document),
            Err(error) => violations.push(format!(
                "cannot read {}: {error}",
                relative_label(root, path)
            )),
        }
    }

    let passage = match fs::read_to_string(root.join(ENUMERATING_DOCUMENT)) {
        Ok(document) => enumerating_passage(&document).map(str::to_owned),
        Err(error) => {
            violations.push(format!("cannot read {ENUMERATING_DOCUMENT}: {error}"));
            return;
        }
    };
    let Some(passage) = passage else {
        violations.push(format!(
            "{ENUMERATING_DOCUMENT} no longer holds the passage saying {ENUMERATED_IN_FULL} \
             \"{COMPLETENESS_CLAIM}\", so no list an author reads as complete is held to \
             {NETWORK_SOURCE}; the claim is what this check exists for and dropping it while \
             keeping the list is not a way out of it"
        ));
        return;
    };

    for name in [ENUMERATED_IN_FULL, NAMED_IN_PART] {
        let declared = declared_variants(&source, name);
        if declared.is_empty() {
            violations.push(format!(
                "{NETWORK_SOURCE} declares no variant for {name}, so the enumeration \
                 {GUIDE_DIRECTORY} states is held to nothing"
            ));
            continue;
        }
        let stated = stated_variants(&guide, name);
        for extra in stated.difference(&declared) {
            violations.push(format!(
                "{GUIDE_DIRECTORY} names {name}::{extra} and {NETWORK_SOURCE} declares no such \
                 variant; an author told to match on a name that does not exist finds out at the \
                 compiler rather than here"
            ));
        }
        if name != ENUMERATED_IN_FULL {
            continue;
        }
        let enumerated = stated_variants(&passage, name);
        for missing in declared.difference(&enumerated) {
            violations.push(format!(
                "{ENUMERATING_DOCUMENT}'s enumeration of {name} omits {name}::{missing}, which \
                 {NETWORK_SOURCE} declares; a variant left out of a list an author reads as \
                 complete is the one their match arm will not have, and naming it on another page \
                 does not complete this one"
            ));
        }
    }
}

fn enumerating_passage(document: &str) -> Option<&str> {
    document
        .split("\n\n")
        .find(|paragraph| paragraph.contains(COMPLETENESS_CLAIM))
}

fn declared_variants(source: &str, name: &str) -> BTreeSet<String> {
    let opening = format!("pub enum {name} {{");
    let mut variants = BTreeSet::new();
    let mut inside = false;
    for line in source.lines() {
        if !inside {
            inside = line.trim() == opening;
            continue;
        }
        if line == "}" {
            break;
        }

        let Some(rest) = line.strip_prefix("    ") else {
            continue;
        };
        if rest.starts_with(char::is_uppercase) {
            let variant: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !variant.is_empty() {
                variants.insert(variant);
            }
        }
    }
    variants
}

fn stated_variants(document: &str, name: &str) -> BTreeSet<String> {
    let prefix = format!("{name}::");
    let mut variants = BTreeSet::new();
    let mut rest = document;
    while let Some(at) = rest.find(&prefix) {
        rest = &rest[at + prefix.len()..];
        let variant: String = rest
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !variant.is_empty() {
            variants.insert(variant);
        }
    }
    variants
}

pub(crate) fn verify_game_type_claims(root: &Path, paths: &[String], violations: &mut Vec<String>) {
    let claims: Vec<String> = ["name", "names", "named", "naming"]
        .into_iter()
        .map(|verb| format!("{verb} no game type"))
        .collect();

    for relative in paths {
        let path = root.join(relative);
        if relative.starts_with("evidence/") || path.is_symlink() {
            continue;
        }
        let text = match fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => {
                violations.push(format!("cannot scan tracked file {relative}: {error}"));
                continue;
            }
        };

        for (index, line) in text.lines().enumerate() {
            for claim in &claims {
                if !line.contains(claim.as_str()) || line.contains(&format!("\"{claim}\"")) {
                    continue;
                }
                violations.push(format!(
                    "{relative}:{} says an artifact `{claim}`. That is false of the always \
                     present agent jar and of generated Hook.java, both of which carry game \
                     class names as strings. State the precise claim instead — the jar \
                     *declares* no game type, so it compiles with no `-cp` and no game \
                     fixture — or put the false form in quotation marks where the point is that \
                     it was false.",
                    index + 1
                ));
            }
        }
    }
}

pub(crate) fn verify_example_inventory(root: &Path, violations: &mut Vec<String>) {
    verify_inventory(root, "examples", EXAMPLE_NAMES, "EXAMPLE_NAMES", violations);
}

pub(crate) fn verify_test_mod_inventory(root: &Path, violations: &mut Vec<String>) {
    verify_inventory(
        root,
        "test-mods",
        TEST_MOD_NAMES,
        "TEST_MOD_NAMES",
        violations,
    );
}

fn verify_inventory(
    root: &Path,
    directory: &str,
    listed: &[&str],
    list: &str,
    violations: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(root.join(directory)) else {
        violations.push(format!("cannot read {directory}"));
        return;
    };

    for entry in entries.flatten() {
        if !entry.path().join("Cargo.toml").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !listed.contains(&name.as_str()) {
            violations.push(format!("{directory}/{name} is not listed in {list}"));
        }
    }
}

#[cfg(test)]
mod tests;
