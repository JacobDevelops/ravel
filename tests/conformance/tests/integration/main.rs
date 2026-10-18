#![forbid(unsafe_code)]

//! Cross-crate conformance tests for public Ravel contracts.

mod config_saves_and_services;
mod configuration;
mod content_declarations;
mod example_package;
mod guide_snippets;
mod mod_capabilities;
mod native_names;
mod noop_pipeline;
mod optional_services;
mod pack_closure;
mod pack_round_trip;
mod package_schema;
mod pre_world_gate;
mod profiling_report;
mod registry_lifecycle;
mod saved_data;
mod sdk_boundary;
mod sdk_gaps;
mod sides_and_packets;

fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn assert_publishable(stored: &str) {
    let lowered = stored.to_lowercase();
    for forbidden in [
        "/home/",
        "/users/",
        "c:\\users\\",
        "/.minecraft",
        "tests/fixtures/local/",
        "evidence/private/",
    ]
    .into_iter()
    .chain(ravel_core::CREDENTIAL_WORDS)
    {
        assert!(
            !lowered.contains(forbidden),
            "the record contains `{forbidden}`"
        );
    }
    assert!(
        !ravel_core::contains_credential(stored),
        "the record contains a credential"
    );
    for directory in [repository_root(), std::env::temp_dir()] {
        assert!(
            !stored.contains(directory.to_string_lossy().as_ref()),
            "the record names a local directory"
        );
    }
}

#[test]
fn publication_checks_reject_local_paths_and_production_credential_shapes() {
    assert_publishable("{\"result\":\"passed\"}");
    let private = [
        "/home/example/game".to_owned(),
        "C:\\Users\\example\\game".to_owned(),
        "tests/fixtures/local/game".to_owned(),
        "evidence/private/run.json".to_owned(),
        format!("{}example", concat!("gh", "p_")),
        format!("{}example", concat!("xox", "b-")),
        format!("{}=example", concat!("api", "_key")),
        repository_root().join("private.json").display().to_string(),
        std::env::temp_dir()
            .join("private.json")
            .display()
            .to_string(),
    ];
    for stored in private {
        assert!(
            std::panic::catch_unwind(|| assert_publishable(&stored)).is_err(),
            "publication accepted private data"
        );
    }
}
