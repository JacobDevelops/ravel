use std::fs;
use std::path::PathBuf;

use crate::evidence::manifest::MILESTONE_MANIFESTS;
use crate::inventory::{
    CampaignInventory, GUIDE_DIRECTORY, HOOK_DOCUMENT, HookCounts, NETWORK_SOURCE,
    ROUND_COUNT_EXEMPT, campaign_document_violations, documented_versions, hook_counts,
    hook_document_violations, numbers_before, round_count_restatements, round_count_subjects,
    verify_campaign_inventory, verify_guide_enumerations, verify_hook_inventory_document,
    verify_manifest_percentages, verify_void_limits,
};
use crate::test_support::{temporary_root, version_directory};
use crate::version_boundary::hook_inventories;

fn hook_entry(kind: &str, descriptor: &str) -> String {
    format!(
        "\n[[hook]]\nid = \"an.id\"\nclass = \"net.minecraft.Thing\"\nmember = \"member\"\n\
         descriptor = \"{descriptor}\"\nkind = \"{kind}\"\ncapability = \"Do something.\"\n"
    )
}

#[test]
fn a_hook_inventory_is_counted_by_kind_and_by_handle_parameter() {
    let inventory = format!(
        "schemaVersion = 1\n{}{}{}{}",
        hook_entry("static-method", "(Ljava/lang/String;Z)Ljava/lang/String;"),
        hook_entry(
            "instance-method",
            "([Ljava/lang/String;[JLjava/util/List;D)V"
        ),
        hook_entry("instance-method", "(I)Z"),
        hook_entry("instance-field", "Z"),
    );

    let counts = hook_counts(&inventory).expect("the inventory should count");

    assert_eq!(counts.entries, 4);
    assert_eq!(counts.methods, 3);
    assert_eq!(counts.fields, 1);
    assert_eq!(counts.handles, 3);
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
fn a_campaign_document_stating_the_wrong_inventory_anywhere_is_reported() {
    let derived = CampaignInventory {
        classes: 15,
        targets: 33,
        prior: 31,
        launches: 3,
    };

    let violations = campaign_document_violations(
        "Rewriting all 33 callable targets, over the same 15 classes in the reviewed \
         inventory. The count moved from 31 to 33, over three surface launches. A later \
         launch at 34 callable targets over 16 classes in the reviewed inventory.",
        &derived,
    );

    assert_eq!(
        violations,
        [
            "evidence/m3/README.md states 34 callable targets where \
             evidence/m3/linux-process-matrix.json records 33"
                .to_owned(),
            "evidence/m3/README.md states 16 classes in the reviewed inventory where \
             evidence/m3/linux-process-matrix.json records 15"
                .to_owned(),
        ]
    );
}

#[test]
fn a_campaign_figure_split_across_a_wrapped_line_is_still_checked() {
    let derived = CampaignInventory {
        classes: 15,
        targets: 33,
        prior: 31,
        launches: 3,
    };

    let violations = campaign_document_violations(
        "The count moved from 31 to 33 callable targets over the same 16 classes in the\n  \
         reviewed inventory, across three surface launches.",
        &derived,
    );

    assert_eq!(
        violations,
        [
            "evidence/m3/README.md states 16 classes in the reviewed inventory where \
             evidence/m3/linux-process-matrix.json records 15"
                .to_owned()
        ]
    );
}

#[test]
fn a_campaign_document_stating_no_count_at_all_is_not_read_as_agreement() {
    let derived = CampaignInventory {
        classes: 15,
        targets: 33,
        prior: 31,
        launches: 3,
    };

    let violations = campaign_document_violations("The campaigns are recorded.", &derived);

    assert_eq!(violations.len(), 4, "{violations:?}");
    assert!(
        violations
            .iter()
            .all(|violation| violation.contains("no longer writes a count against")),
        "{violations:?}"
    );
}

#[test]
fn a_count_is_read_against_its_noun_however_it_was_spelled_or_qualified() {
    assert_eq!(
        numbers_before("six independent rounds have run", &["rounds"]),
        [6]
    );
    assert_eq!(
        numbers_before("the five tracked independent M3-W8 rounds", &["rounds"]),
        [5]
    );
    assert_eq!(numbers_before("5 rounds are tracked", &["rounds"]), [5]);
    assert_eq!(
        numbers_before("over nine world lifecycles", &["world", "lifecycles"]),
        [9]
    );

    assert_eq!(
        numbers_before("the fourth round of review", &["round"]),
        Vec::<usize>::new()
    );
    assert_eq!(
        numbers_before("every round returned FAIL", &["rounds"]),
        Vec::<usize>::new()
    );
}

#[test]
fn a_count_spelled_above_twenty_is_read_up_to_ninety_nine() {
    assert_eq!(numbers_before("forty M3 review rounds", &["rounds"]), [40]);
    assert_eq!(
        numbers_before("twenty-one M3 review rounds", &["rounds"]),
        [21]
    );
    assert_eq!(numbers_before("ninety-nine rounds", &["rounds"]), [99]);
    assert_eq!(numbers_before("thirty rounds", &["rounds"]), [30]);

    assert_eq!(
        numbers_before("a hundred rounds", &["rounds"]),
        Vec::<usize>::new()
    );
    assert_eq!(
        numbers_before("thirty-thousand-entry rounds", &["rounds"]),
        Vec::<usize>::new()
    );
    assert_eq!(
        numbers_before("twenty-eleven rounds", &["rounds"]),
        Vec::<usize>::new()
    );
    assert_eq!(numbers_before("twenty one rounds", &["rounds"]), [1]);
}

#[test]
fn a_round_count_is_caught_whichever_noun_it_chose_and_whichever_side_it_sat() {
    for restatement in [
        "Five independent reviews have run against this milestone.",
        "The independent rounds recorded here number five in total.",
        "There have been five review iterations.",
        "Five independent rounds have run.",
        "There are 5 verdicts recorded below.",
    ] {
        assert_eq!(
            round_count_restatements(restatement)
                .into_iter()
                .map(|(count, _)| count)
                .collect::<Vec<_>>(),
            [5],
            "{restatement}"
        );
    }

    for innocent in [
        "the profiling overhead rounds to `0.05` and not one digit better",
        "Round 5 returned FAIL with four blocking findings.",
        "every round so far has been an independent one",
    ] {
        assert_eq!(round_count_restatements(innocent), Vec::new(), "{innocent}");
    }
}

#[test]
fn no_tracked_manifest_writes_a_percentage_into_its_prose() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_manifest_percentages(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_void_budget_limits_are_the_ones_the_roadmap_declares() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_void_limits(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
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

        verify_hook_inventory_document(&root, &hook_inventories(&root), &mut violations);

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

    verify_hook_inventory_document(&root, &hook_inventories(&root), &mut violations);

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
fn the_tracked_campaign_document_agrees_with_the_tracked_launch_record() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_campaign_inventory(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_tracked_hook_document_agrees_with_the_tracked_inventory() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_hook_inventory_document(&root, &hook_inventories(&root), &mut violations);

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

#[test]
fn the_round_count_rule_sweeps_every_milestone_it_is_not_exempt_from() {
    let subjects = round_count_subjects();

    assert_eq!(
        subjects.len(),
        MILESTONE_MANIFESTS.len() - ROUND_COUNT_EXEMPT.len()
    );
    assert!(
        subjects
            .iter()
            .any(|(record, _)| record == "evidence/m5/review.md"),
        "M5 must be swept: {subjects:?}"
    );
    assert!(
        !subjects
            .iter()
            .any(|(record, _)| record == "evidence/m0/review.md"),
        "M0 is exempt: {subjects:?}"
    );
}
