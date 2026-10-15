use std::fs;

use crate::evidence::closure::verify_milestone_figures;
use crate::evidence::m5::M5_FIGURES;
use crate::test_support::{balanced_summary, summary_root};

#[test]
fn a_manifest_figure_must_follow_from_the_record_it_summarises() {
    let root = summary_root("m5-carried-figures", &balanced_summary());
    let manifest = root.join("evidence/m5/manifest.json");
    fs::write(
        &manifest,
        r#"{"environment":{"settings":{"inventory":{"artifacts":11,"planned":10,
            "graphNodes":12,"graphEdges":20,"clusters":3,"sourceProhibited":2,
            "sourceUnresolved":1,"publishable":0,"readingSource":4,
            "dependencyEdges":7,"archiveReads":99,"entryBodyReads":0,
            "digestsBound":1}}}}"#,
    )
    .expect("manifest should be writable");
    let mut violations = Vec::new();

    verify_milestone_figures(&root, &manifest, &M5_FIGURES, &mut violations);

    assert_eq!(
        violations.len(),
        1,
        "violations:\n{}",
        violations.join("\n")
    );
    assert!(
        violations[0].contains("records .environment.settings.inventory.artifacts 11"),
        "violation was: {}",
        violations[0]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
