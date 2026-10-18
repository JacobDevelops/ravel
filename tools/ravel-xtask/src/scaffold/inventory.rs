//! Documentation and generated-inventory consistency.

use std::path::Path;
use std::{collections::BTreeSet, fs};

use crate::hooks::{HookCounts, HookInventory, hook_counts};
use crate::text::{NumberAt, read_text, stated_counts, verify_stated_counts};
use crate::tracked::collect_files;
use crate::version_boundary::VERSION_DATA;

const HOOK_DOCUMENT: &str = "docs/architecture/m3-hook-targets.md";

pub(crate) fn verify_hook_inventory_document(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    let document = match read_text(root, Path::new(HOOK_DOCUMENT)) {
        Ok(text) => text,
        Err(error) => {
            violations.push(error);
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
        // Admission already reports unreadable inputs for tracked versions.
        if root.join(VERSION_DATA).join(version).is_dir() {
            return;
        }
        violations.push(format!(
            "{HOOK_DOCUMENT} states the counts of {VERSION_DATA}/{version}, which is not a \
             tracked version whose inventory can be read"
        ));
        return;
    };

    let counts = match hook_counts(inventory) {
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

fn hook_document_violations(document: &str, inventory: &str, counts: &HookCounts) -> Vec<String> {
    let mut violations = Vec::new();
    for (phrase, side, expected, subject) in [
        (
            " entries to ",
            NumberAt::AfterPhrase,
            counts.entries,
            "hook entries",
        ),
        (
            " methods and ",
            NumberAt::BeforePhrase,
            counts.methods,
            "method entries",
        ),
        (
            " fields",
            NumberAt::BeforePhrase,
            counts.fields,
            "field entries",
        ),
        (
            " handle parameters",
            NumberAt::BeforePhrase,
            counts.handles,
            "handle parameters",
        ),
    ] {
        let stated = stated_counts(document, phrase, side);
        verify_stated_counts(
            &stated,
            expected,
            || {
                format!(
                    "{HOOK_DOCUMENT} no longer writes a count against `{phrase}`, so nothing in it is \
                 held to {inventory}'s {expected} {subject}"
                )
            },
            |count| {
                format!(
                    "{HOOK_DOCUMENT} states {count} {subject} where {inventory} declares \
                 {expected}"
                )
            },
            &mut violations,
        );
    }
    violations
}

const GUIDE_DIRECTORY: &str = "docs/guide";

const NETWORK_SOURCE: &str = "crates/ravel-network/src/lib.rs";

const ENUMERATED_IN_FULL: &str = "WireError";

const NAMED_IN_PART: &str = "DeliveryRefused";

const ENUMERATING_DOCUMENT: &str = "docs/guide/running.md";

const COMPLETENESS_CLAIM: &str = "is the whole of those refusals";

pub(crate) fn verify_guide_enumerations(root: &Path, violations: &mut Vec<String>) {
    let source = match read_text(root, Path::new(NETWORK_SOURCE)) {
        Ok(source) => source,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let mut guide = String::new();
    let mut documents = Vec::new();
    if let Err(error) = collect_files(root, &root.join(GUIDE_DIRECTORY), &mut documents) {
        violations.push(error);
    }
    documents.sort();
    for path in &documents {
        match read_text(root, path) {
            Ok(document) => guide.push_str(&document),
            Err(error) => violations.push(error),
        }
    }

    let passage = match read_text(root, Path::new(ENUMERATING_DOCUMENT)) {
        Ok(document) => enumerating_passage(&document).map(str::to_owned),
        Err(error) => {
            violations.push(error);
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

#[cfg(test)]
mod tests;
