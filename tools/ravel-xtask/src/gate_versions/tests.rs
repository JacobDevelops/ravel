use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use crate::gate_versions::verify_gate_versions;
use crate::test_support::temporary_root;
use crate::version_boundary::VERSION_DATA;

const EVERY_SECTION: &str = "cluster,content,hierarchy,initialization,load,members,outward,\
                             persistence,refusal,screens,settings,side,transformation";

fn gate_versions_root(label: &str, versions: &[&str], harness: &str) -> PathBuf {
    let root = temporary_root(label);
    for version in versions {
        let directory = root.join(format!("{VERSION_DATA}/{version}"));
        fs::create_dir_all(&directory).expect("version directory should be creatable");
    }
    let lines = |verb: &str| {
        let mut record = String::new();
        for version in versions {
            writeln!(record, "{verb}: Minecraft {version}").expect("a String never fails");
        }
        record
    };
    let target = root.join("target/ravel-jvm");
    fs::create_dir_all(&target).expect("record directory should be creatable");
    fs::write(target.join("tested-versions.txt"), lines("tested"))
        .expect("test record should be writable");
    fs::write(target.join("compiled-versions.txt"), lines("compiled"))
        .expect("compile record should be writable");
    fs::write(target.join("harness-versions.txt"), harness)
        .expect("harness record should be writable");
    root
}

#[test]
fn a_version_that_ran_every_section_raises_nothing() {
    let root = gate_versions_root(
        "gate-versions-clean",
        &["26.1.2"],
        &format!("ran: Minecraft 26.1.2 sections={EVERY_SECTION}\n"),
    );

    assert_eq!(verify_gate_versions(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_skip_line_names_the_version_it_skipped() {
    let root = gate_versions_root(
        "gate-versions-skip",
        &["1.21.1", "1.21.2", "26.1.2"],
        &format!(
            "SKIPPED: the harness was NOT run for Minecraft 1.21.1; \
             tests/fixtures/local/tooling/minecraft-1.21.1/client.txt is absent\n\
             SKIPPED: Minecraft 1.21.2 was NOT run by the harness; \
             tests/fixtures/local/tooling/minecraft-1.21.2/client.txt is absent\n\
             ran: Minecraft 26.1.2 sections={EVERY_SECTION}\n"
        ),
    );

    assert_eq!(verify_gate_versions(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_line_about_a_longer_version_does_not_answer_for_a_shorter_one() {
    let root = gate_versions_root(
        "gate-versions-prefix",
        &["1.21.1", "1.21.11"],
        &format!("ran: Minecraft 1.21.11 sections={EVERY_SECTION}\n"),
    );

    let violations = verify_gate_versions(&root);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("names no outcome for Minecraft 1.21.1,")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_harness_line_naming_no_section_is_a_violation() {
    let root = gate_versions_root(
        "gate-versions-empty",
        &["26.1.2"],
        "ran: Minecraft 26.1.2 sections=\n",
    );

    let violations = verify_gate_versions(&root);

    assert!(
        violations
            .iter()
            .any(|violation| violation.ends_with("so what it covered is unrecorded")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_section_the_harness_does_not_have_is_a_violation_in_the_record() {
    let root = gate_versions_root(
        "gate-versions-unknown-record",
        &["26.1.2"],
        &format!("ran: Minecraft 26.1.2 sections={EVERY_SECTION},screenz\n"),
    );

    let violations = verify_gate_versions(&root);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("ran a screenz section")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_absent_section_is_a_violation() {
    let root = gate_versions_root(
        "gate-versions-absent",
        &["26.1.2"],
        &format!(
            "ran: Minecraft 26.1.2 sections={}\n",
            EVERY_SECTION.replace("persistence,", "")
        ),
    );

    let violations = verify_gate_versions(&root);

    assert!(
        violations.iter().any(|violation| {
            violation.contains("did not run the persistence section")
                && violation.contains("runs every section")
        }),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
