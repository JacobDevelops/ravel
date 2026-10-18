use std::fs;
use std::path::Path;

use super::{numbers_after, numbers_before, read_text, redacted_path, relative_label};
use crate::test_support::temporary_root;

#[test]
fn stated_count_comparison_reports_absence_and_each_mismatch() {
    for (stated, expected) in [
        (&[][..], vec!["absent".to_owned()]),
        (&[3][..], vec![]),
        (
            &[2, 3, 4, 2][..],
            vec![
                "differs: 2".to_owned(),
                "differs: 4".to_owned(),
                "differs: 2".to_owned(),
            ],
        ),
    ] {
        let mut violations = Vec::new();
        super::verify_stated_counts(
            stated,
            3,
            || "absent".to_owned(),
            |count| format!("differs: {count}"),
            &mut violations,
        );
        assert_eq!(violations, expected);
    }
}

#[test]
fn gate_output_never_names_an_absolute_path() {
    let root = Path::new("/repository");
    let absolute = root.join("evidence/m0/manifest.json");

    assert_eq!(redacted_path(root, &absolute), "evidence/m0/manifest.json");
    assert_eq!(
        redacted_path(root, Path::new("evidence/m0/manifest.json")),
        "evidence/m0/manifest.json"
    );
    assert_eq!(
        redacted_path(root, Path::new("/somewhere/else/manifest.json")),
        "manifest.json"
    );
}
#[test]
fn text_loading_uses_the_supplied_root_and_keeps_relative_error_labels() {
    let root = temporary_root("text-rooted");
    let relative = Path::new("nested/record.txt");
    fs::create_dir_all(root.join("nested")).expect("directory should be creatable");
    fs::write(root.join(relative), "recorded").expect("record should be writable");
    assert_eq!(
        read_text(&root, relative).expect("record should read"),
        "recorded"
    );
    assert_eq!(
        read_text(&root, &root.join(relative)).expect("absolute record should read"),
        "recorded"
    );
    assert_eq!(
        relative_label(&root, &root.join(relative)),
        "nested/record.txt"
    );
    let error =
        read_text(&root, Path::new("nested/missing.txt")).expect_err("missing record must fail");
    assert!(
        error.starts_with("cannot read nested/missing.txt:"),
        "{error}"
    );
    assert!(!error.contains(&root.display().to_string()), "{error}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn text_loading_accepts_paths_already_prefixed_by_a_relative_root() {
    let root = std::path::PathBuf::from(format!(
        "target/xtask-relative-text-root-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("relative root should be creatable");
    let relative = Path::new("record.txt");
    let prefixed = root.join(relative);
    fs::write(&prefixed, "recorded").expect("record should be writable");
    assert_eq!(
        read_text(&root, relative).expect("relative record should read"),
        "recorded"
    );
    assert_eq!(
        read_text(&root, &prefixed).expect("prefixed record should read"),
        "recorded"
    );
    assert_eq!(relative_label(&root, &prefixed), "record.txt");
    let absolute = std::env::current_dir()
        .expect("current directory")
        .join(&prefixed);
    assert_eq!(
        read_text(&root, &absolute).expect("absolute record should read"),
        "recorded"
    );
    let error = read_text(&root, &root.join("missing.txt")).expect_err("missing record must fail");
    assert!(error.starts_with("cannot read missing.txt:"), "{error}");
    fs::remove_dir_all(root).expect("relative root should be removable");
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
fn a_following_count_starts_immediately_after_its_noun() {
    assert_eq!(
        numbers_after(
            "Blocking findings: 3 across two rounds.",
            &["blocking", "findings"]
        ),
        [3]
    );
    assert_eq!(numbers_after("rounds: 5", &["rounds"]), [5]);
    assert_eq!(
        numbers_after("rounds: one two three four 5", &["rounds"]),
        [1]
    );
    assert_eq!(
        numbers_after("rounds: no count is stated 5", &["rounds"]),
        Vec::<usize>::new()
    );
}

#[test]
fn a_count_before_a_noun_excludes_unrelated_following_numbers() {
    let summary = "raised four blocking findings, three of which were sentences";
    assert_eq!(numbers_before(summary, &["blocking", "findings"]), [4]);
    assert_eq!(
        numbers_after(summary, &["blocking", "findings"]),
        Vec::<usize>::new()
    );
    assert_eq!(
        numbers_after("it reviews 12 crates", &["reviews"]),
        Vec::<usize>::new()
    );
    assert_eq!(
        numbers_after(
            "four blocking findings were addressed, and after another complete review \
             Blocking findings: 3",
            &["blocking", "findings"]
        ),
        [3]
    );
}

#[test]
fn required_files_have_one_repository_relative_absence_policy() {
    let root = temporary_root("required-file");
    assert_eq!(
        super::require_file(&root, "nested/required.txt"),
        Err("missing required file: nested/required.txt".to_owned())
    );
    fs::create_dir_all(root.join("nested/required.txt")).expect("directory should exist");
    assert!(super::require_file(&root, "nested/required.txt").is_err());
    let file = root.join("nested/present.txt");
    fs::write(&file, "present").expect("required file should write");
    assert_eq!(super::require_file(&root, "nested/present.txt"), Ok(file));
    fs::remove_dir_all(root).expect("temporary root should remove");
}
