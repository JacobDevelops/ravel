use std::fs;
use std::path::PathBuf;

use crate::hooks::{HookCounts, hook_counts, hook_inventory};
use crate::inventory::{
    GUIDE_DIRECTORY, HOOK_DOCUMENT, NETWORK_SOURCE, documented_versions, hook_document_violations,
    verify_guide_enumerations, verify_hook_inventory_document,
};
use crate::test_support::{adapter_document, hook_inventories, temporary_root, version_directory};
use crate::version_boundary::tracked_versions;

fn hook_entry(kind: &str, descriptor: &str) -> String {
    format!(
        "\n[[hook]]\nid = \"an.id\"\nclass = \"net.minecraft.Thing\"\nmember = \"member\"\n\
         descriptor = \"{descriptor}\"\nkind = \"{kind}\"\ncapability = \"Do something.\"\n"
    )
}

#[test]
fn a_hook_inventory_is_counted_by_kind_and_by_handle_parameter() {
    let root = temporary_root("inventory-counts");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join("minecraft/versions/26.1.2/adapter.toml"),
        adapter_document("26.1.2", "live", None).replace("/hooks.toml", "/reviewed-hooks.toml"),
    )
    .expect("adapter should be writable");
    fs::write(
        root.join("minecraft/versions/26.1.2/reviewed-hooks.toml"),
        format!(
            "schemaVersion = 1\nminecraft = '26.1.2'\n{}{}{}{}",
            hook_entry("static-method", "(Ljava/lang/String;Z)Ljava/lang/String;"),
            hook_entry(
                "instance-method",
                "([Ljava/lang/String;[JLjava/util/List;D)V"
            ),
            hook_entry("instance-method", "(I)Z"),
            hook_entry("instance-field", "Z"),
        ),
    )
    .expect("hook inventory should be writable");
    let inventory = hook_inventory(&root, "26.1.2").expect("hook inventory should parse");
    assert_eq!(
        inventory.path,
        "minecraft/versions/26.1.2/reviewed-hooks.toml"
    );
    let counts = hook_counts(&inventory).expect("the inventory should count");

    assert_eq!(counts.entries, 4);
    assert_eq!(counts.methods, 3);
    assert_eq!(counts.fields, 1);
    assert_eq!(counts.handles, 3);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn the_hook_document_fails_on_every_count_that_no_longer_matches_the_inventory() {
    let counts = HookCounts {
        entries: 37,
        methods: 33,
        fields: 4,
        handles: 27,
    };
    let current = "`hooks.toml` therefore grew from 6 entries to 37 — 33 methods and 4 fields,\n\
                   carrying 27 handle parameters between them.";
    let stale = "`hooks.toml` therefore grew from 6 entries to 35 — 31 methods and 4 fields,\n\
                 carrying 23 handle parameters between them.";

    let inventory = "minecraft/versions/26.1.2/hooks.toml";

    assert_eq!(
        hook_document_violations(current, inventory, &counts),
        Vec::<String>::new()
    );
    assert_eq!(
        hook_document_violations(stale, inventory, &counts),
        [
            format!("{HOOK_DOCUMENT} states 35 hook entries where {inventory} declares 37"),
            format!("{HOOK_DOCUMENT} states 31 method entries where {inventory} declares 33"),
            format!("{HOOK_DOCUMENT} states 23 handle parameters where {inventory} declares 27"),
        ]
    );
}

#[test]
fn a_hook_document_stating_no_count_at_all_is_not_read_as_agreement() {
    let counts = HookCounts {
        entries: 37,
        methods: 33,
        fields: 4,
        handles: 27,
    };

    let violations = hook_document_violations(
        "The inventory grew.",
        "minecraft/versions/26.1.2/hooks.toml",
        &counts,
    );

    assert_eq!(violations.len(), 4, "{violations:?}");
    assert!(
        violations
            .iter()
            .all(|violation| violation.contains("no longer writes a count against")),
        "{violations:?}"
    );
}

#[test]
fn a_documents_subject_version_comes_from_the_directories_it_links_into() {
    let document = "Pinned in [adapter.toml](../../minecraft/versions/26.1.2/adapter.toml)\n\
                    beside minecraft/versions/26.1.2/hooks.toml, while minecraft/versions/ \
                    holds the rest.";

    let named = documented_versions(document);

    assert_eq!(named.into_iter().collect::<Vec<_>>(), ["26.1.2"]);
}

#[test]
fn a_hook_document_naming_no_single_version_is_refused() {
    for (label, links, expected) in [
        ("document-none", "It links nowhere.\n", "links into 0"),
        (
            "document-many",
            "See minecraft/versions/26.1.2/hooks.toml and \
             minecraft/versions/99.0.0-synthetic/hooks.toml.\n",
            "links into 2",
        ),
    ] {
        let root = temporary_root(label);
        version_directory(&root, "26.1.2", &[]);
        let document = root.join(HOOK_DOCUMENT);
        fs::create_dir_all(
            document
                .parent()
                .expect("the document sits inside a directory"),
        )
        .expect("document directory should be creatable");
        fs::write(&document, links).expect("document should be writable");
        let mut violations = Vec::new();

        verify_hook_inventory_document(
            &root,
            &hook_inventories(
                &root,
                &tracked_versions(&root).expect("tracked versions should be readable"),
            )
            .expect("hook inventories should be readable"),
            &mut violations,
        );

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains(expected), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn a_hook_document_naming_an_untracked_version_is_refused() {
    let root = temporary_root("document-untracked");
    version_directory(&root, "26.1.2", &[]);
    let document = root.join(HOOK_DOCUMENT);
    fs::create_dir_all(
        document
            .parent()
            .expect("the document sits inside a directory"),
    )
    .expect("document directory should be creatable");
    fs::write(
        &document,
        "Counted from minecraft/versions/99.0.0-synthetic/hooks.toml.\n",
    )
    .expect("document should be writable");
    let mut violations = Vec::new();

    verify_hook_inventory_document(
        &root,
        &hook_inventories(
            &root,
            &tracked_versions(&root).expect("tracked versions should be readable"),
        )
        .expect("hook inventories should be readable"),
        &mut violations,
    );

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains(
            "states the counts of minecraft/versions/99.0.0-synthetic, which is not a tracked \
             version"
        ),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn the_tracked_hook_document_agrees_with_the_tracked_inventory() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_hook_inventory_document(
        &root,
        &hook_inventories(
            &root,
            &tracked_versions(&root).expect("tracked versions should be readable"),
        )
        .expect("hook inventories should be readable"),
        &mut violations,
    );

    assert_eq!(violations, Vec::<String>::new());
}

fn guide_enumeration_root(label: &str, source: &str, guide: &str) -> PathBuf {
    let root = temporary_root(label);
    fs::create_dir_all(root.join("crates/ravel-network/src"))
        .expect("directory should be creatable");
    fs::write(root.join(NETWORK_SOURCE), source).expect("source should be writable");
    fs::create_dir_all(root.join(GUIDE_DIRECTORY)).expect("directory should be creatable");
    fs::write(root.join(GUIDE_DIRECTORY).join("running.md"), guide)
        .expect("guide should be writable");
    root
}

fn network_source(variants: &[&str]) -> String {
    let mut source = String::from("pub enum WireError {\n");
    for variant in variants {
        source.push_str("    /// A refusal.\n    ");
        source.push_str(variant);
        source.push_str(" {\n        /// A field.\n        Needed: usize,\n    },\n");
    }
    source.push_str("}\n\npub enum DeliveryRefused {\n    /// A refusal.\n    WrongSide,\n}\n");
    source
}

#[test]
fn a_guide_enumerating_every_variant_the_code_declares_is_accepted() {
    let root = guide_enumeration_root(
        "guide-enumeration-agrees",
        &network_source(&["Truncated", "NotUtf8"]),
        "`WireError` is the whole of those refusals: `WireError::Truncated` and \
         `WireError::NotUtf8`.\n\nSeparately, `DeliveryRefused::WrongSide`.\n",
    );

    let mut violations = Vec::new();
    verify_guide_enumerations(&root, &mut violations);

    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_guide_enumeration_that_has_drifted_from_the_code_is_reported() {
    let root = guide_enumeration_root(
        "guide-enumeration-drifted",
        &network_source(&["Truncated", "NotUtf8"]),
        "`WireError` is the whole of those refusals: `WireError::Truncated` and \
         `WireError::NotABool`.\n",
    );

    let mut violations = Vec::new();
    verify_guide_enumerations(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "docs/guide names WireError::NotABool and crates/ravel-network/src/lib.rs declares \
             no such variant; an author told to match on a name that does not exist finds out \
             at the compiler rather than here",
            "docs/guide/running.md's enumeration of WireError omits WireError::NotUtf8, which \
             crates/ravel-network/src/lib.rs declares; a variant left out of a list an author \
             reads as complete is the one their match arm will not have, and naming it on \
             another page does not complete this one",
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_variant_named_on_another_page_does_not_complete_the_enumeration() {
    let root = guide_enumeration_root(
        "guide-enumeration-elsewhere",
        &network_source(&["Truncated", "NotUtf8"]),
        "`WireError` is the whole of those refusals: `WireError::Truncated`.\n",
    );
    fs::write(
        root.join(GUIDE_DIRECTORY).join("limits.md"),
        "In passing, `WireError::NotUtf8` is also refused.\n",
    )
    .expect("guide should be writable");

    let mut violations = Vec::new();
    verify_guide_enumerations(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "docs/guide/running.md's enumeration of WireError omits WireError::NotUtf8, which \
          crates/ravel-network/src/lib.rs declares; a variant left out of a list an author \
          reads as complete is the one their match arm will not have, and naming it on \
          another page does not complete this one",
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_guide_that_drops_its_completeness_claim_is_reported() {
    let root = guide_enumeration_root(
        "guide-enumeration-claimless",
        &network_source(&["Truncated", "NotUtf8"]),
        "`WireError::Truncated` and `WireError::NotUtf8` are refusals.\n",
    );

    let mut violations = Vec::new();
    verify_guide_enumerations(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "docs/guide/running.md no longer holds the passage saying WireError \"is the whole \
          of those refusals\", so no list an author reads as complete is held to \
          crates/ravel-network/src/lib.rs; the claim is what this check exists for and \
          dropping it while keeping the list is not a way out of it",
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_guide_naming_part_of_an_enum_it_does_not_enumerate_is_accepted() {
    let root = guide_enumeration_root(
        "guide-enumeration-partial",
        &network_source(&["Truncated"]),
        "`WireError` is the whole of those refusals: `WireError::Truncated`, and nothing \
         about `DeliveryRefused` at all.\n",
    );

    let mut violations = Vec::new();
    verify_guide_enumerations(&root, &mut violations);

    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
