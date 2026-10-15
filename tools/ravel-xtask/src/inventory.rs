//! Documentation and generated-inventory consistency.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::evidence::accounting::{Side, stated_counts};
use crate::evidence::findings::relative_label;
use crate::java_shape::{descriptor_handles, parse_hook_inventory};
use crate::tracked::collect_files;
use crate::version_boundary::{HookInventory, VERSION_DATA};
use crate::{EXAMPLE_NAMES, TEST_MOD_NAMES};

const HOOK_DOCUMENT: &str = "docs/architecture/m3-hook-targets.md";

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
    for hook in parse_hook_inventory(inventory)?.hooks {
        let (Some(kind), Some(descriptor)) = (hook.kind.as_deref(), hook.descriptor.as_deref())
        else {
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

pub(crate) fn verify_inventory(
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
