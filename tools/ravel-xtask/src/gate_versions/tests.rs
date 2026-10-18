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
        fs::write(
            directory.join("adapter.toml"),
            format!("minecraft = '{version}'\n"),
        )
        .expect("adapter should be writable");
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

    assert_eq!(verify_gate_versions(&root).violations, Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_skip_line_is_an_honest_outcome_not_a_violation() {
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
    for version in ["1.21.1", "1.21.2"] {
        fs::write(
            root.join(format!("{VERSION_DATA}/{version}/adapter.toml")),
            format!(
                "minecraft = '{version}'\n[inputs.clientMappings]\npath = '{version}.mapping'\n"
            ),
        )
        .expect("adapter should be writable");
    }

    let report = verify_gate_versions(&root);
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    assert_eq!(report.notices.len(), 2, "{:?}", report.notices);
    assert!(
        report
            .notices
            .iter()
            .all(|notice| notice.starts_with("SKIPPED:"))
    );
    for version in ["1.21.1", "1.21.2"] {
        fs::write(root.join(format!("{version}.mapping")), "acquired mapping")
            .expect("mapping should be writable");
    }
    let present = verify_gate_versions(&root);
    assert_eq!(present.violations.len(), 2, "{:?}", present.violations);
    assert!(present.notices.is_empty(), "{:?}", present.notices);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_line_about_a_longer_version_does_not_answer_for_a_shorter_one() {
    let root = gate_versions_root(
        "gate-versions-prefix",
        &["1.21.1", "1.21.11"],
        &format!("ran: Minecraft 1.21.11 sections={EVERY_SECTION}\n"),
    );

    let violations = verify_gate_versions(&root).violations;

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

    let violations = verify_gate_versions(&root).violations;

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

    let violations = verify_gate_versions(&root).violations;

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

    let violations = verify_gate_versions(&root).violations;

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

#[test]
fn generation_only_cannot_claim_adapter_or_harness_coverage() {
    let root = gate_versions_root(
        "gate-generation-only",
        &["1.12.2", "26.1.2"],
        &format!(
            "ran: Minecraft 1.12.2 sections={EVERY_SECTION}\n\
             ran: Minecraft 26.1.2 sections={EVERY_SECTION}\n"
        ),
    );
    fs::write(
        root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml")),
        "minecraft = '1.12.2'\neligibility = 'generation-only'\n",
    )
    .expect("adapter should be writable");
    let violations = verify_gate_versions(&root).violations;
    assert_eq!(violations.len(), 4, "{violations:?}");
    assert!(
        violations
            .iter()
            .all(|violation| violation.contains("1.12.2")),
        "{violations:?}"
    );
    fs::write(
        root.join("target/ravel-jvm/tested-versions.txt"),
        "built: Minecraft 1.12.2; adapter tests not yet run\ntested: Minecraft 26.1.2\n",
    )
    .expect("build record should be writable");
    fs::write(
        root.join("target/ravel-jvm/harness-versions.txt"),
        format!(
            "NOT-YET-RUN: Minecraft 1.12.2; generation-only\n\
             ran: Minecraft 26.1.2 sections={EVERY_SECTION}\n"
        ),
    )
    .expect("harness record should be writable");
    let report = verify_gate_versions(&root);
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    assert_eq!(report.notices.len(), 14);
    assert!(
        report
            .notices
            .iter()
            .all(|notice| notice.contains("not yet met"))
    );
    fs::write(
        root.join("target/ravel-jvm/tested-versions.txt"),
        "built: Minecraft 1.12.2\ntested: Minecraft 1.12.2\ntested: Minecraft 26.1.2\n",
    )
    .expect("build record should be writable");
    fs::write(
        root.join("target/ravel-jvm/harness-versions.txt"),
        format!(
            "NOT-YET-RUN: Minecraft 1.12.2\nran: Minecraft 1.12.2 sections={EVERY_SECTION}\n\
             ran: Minecraft 26.1.2 sections={EVERY_SECTION}\n"
        ),
    )
    .expect("harness record should be writable");
    let violations = verify_gate_versions(&root).violations;
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert!(
        violations
            .iter()
            .all(|violation| violation.contains("falsely claims"))
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn modern_versions_require_tests_and_harness_runs_not_only_builds() {
    let root = gate_versions_root(
        "modern-not-run",
        &["26.1.2"],
        "NOT-YET-RUN: Minecraft 26.1.2\n",
    );
    fs::write(
        root.join("target/ravel-jvm/tested-versions.txt"),
        "built: Minecraft 26.1.2\n",
    )
    .expect("build record should be writable");
    let violations = verify_gate_versions(&root).violations;
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("tested obligation not yet met")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("ran obligation not yet met")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_generation_only_tree_does_not_claim_live_coverage() {
    let root = gate_versions_root("no-live-versions", &["1.12.2"], "");
    fs::write(
        root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml")),
        "minecraft = '1.12.2'\neligibility = 'generation-only'\n",
    )
    .expect("adapter should be writable");
    fs::write(
        root.join("target/ravel-jvm/tested-versions.txt"),
        "built: Minecraft 1.12.2\n",
    )
    .expect("build record should be writable");
    fs::write(
        root.join("target/ravel-jvm/harness-versions.txt"),
        "NOT-YET-RUN: Minecraft 1.12.2\n",
    )
    .expect("harness record should be writable");
    let report = verify_gate_versions(&root);
    assert_eq!(report.violations.len(), 3, "{:?}", report.violations);
    assert!(
        report
            .violations
            .iter()
            .all(|violation| violation.contains("no live-eligible")),
        "{:?}",
        report.violations
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn skips_for_every_version_and_record_do_not_claim_live_coverage() {
    let root = gate_versions_root("gate-all-skipped", &["1.12.2", "26.1.2"], "");
    fs::write(
        root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml")),
        "minecraft = '1.12.2'\neligibility = 'generation-only'\n\
         [inputs.clientMappings]\npath = 'legacy.mapping'\n",
    )
    .expect("adapter should be writable");
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml")),
        "minecraft = '26.1.2'\n[inputs.clientMappings]\npath = 'modern.mapping'\n",
    )
    .expect("adapter should be writable");
    let skipped = "SKIPPED: Minecraft 1.12.2; mapping input is absent\n\
                   SKIPPED: Minecraft 26.1.2; mapping input is absent\n";
    for record in super::GATE_RECORDS {
        fs::write(root.join(record.path), skipped).expect("skip record should be writable");
    }

    let report = verify_gate_versions(&root);
    assert_eq!(report.violations.len(), 3, "{:?}", report.violations);
    assert_eq!(
        report
            .notices
            .iter()
            .filter(|notice| notice.starts_with("SKIPPED:"))
            .count(),
        6
    );
    assert_eq!(
        report
            .violations
            .iter()
            .filter(|violation| violation.contains("no live-eligible"))
            .count(),
        3
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn available_versions_cannot_use_forged_skips() {
    let root = gate_versions_root(
        "gate-forged-skip",
        &["26.1.2"],
        "SKIPPED: Minecraft 26.1.2; fixture is absent\n",
    );
    let report = verify_gate_versions(&root);
    assert!(
        report
            .violations
            .iter()
            .any(|violation| { violation.contains("Minecraft 26.1.2 ran obligation not yet met") }),
        "{:?}",
        report.violations
    );
    assert!(report.notices.is_empty(), "{:?}", report.notices);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
