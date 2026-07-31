//! Contract tests for `ravel-resolve` against the spec it implements.
//!
//! The unit tests inside the crate check each resolution rule on its own.
//! These check the two things only an out-of-crate test can: that a pack-shaped
//! package set walks from manifest documents to one load order through the
//! public API alone, and that every `SPEC.md` §9.2 dependency outcome comes
//! back distinctly enough for the surface that reports it to tell them apart.

use std::collections::BTreeSet;

use ravel_core::{FailureKind, LifecycleState, ModId, Side};
use ravel_package::{PackageFailure, PackageManifest};
use ravel_resolve::{ResolutionError, resolve};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn package(id: &str, version: &str, sides: &str, dependencies: &str) -> PackageManifest {
    let document = format!(
        r#"{{
  "schemaVersion": 1,
  "id": "{id}",
  "version": "{version}",
  "compatibility": {{ "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0"] }},
  "sides": {sides},
  "dependencies": {dependencies},
  "targets": [
    {{
      "platform": "linux",
      "architecture": "x86_64",
      "artifacts": [
        {{ "path": "artifacts/mod.bin", "role": "module", "sha256": "{DIGEST}" }}
      ]
    }}
  ],
  "authors": [{{ "name": "Example" }}],
  "license": "MIT OR Apache-2.0",
  "source": "https://example.invalid/{id}",
  "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
}}"#
    );
    PackageManifest::parse(&document).expect("the fixture manifest should parse")
}

fn everywhere(id: &str, version: &str, dependencies: &str) -> PackageManifest {
    package(
        id,
        version,
        r#"["client", "integrated-server", "dedicated-server"]"#,
        dependencies,
    )
}

fn names(plan: &[ModId]) -> Vec<String> {
    plan.iter().map(ToString::to_string).collect()
}

/// A pack shaped like a small real one: a library everything sits on, two mods
/// that need it, a client-only renderer, an optional integration between two
/// mods, and a discouraged pair that still loads.
fn pack() -> Vec<PackageManifest> {
    vec![
        everywhere(
            "example_machines",
            "3.1.0",
            r#"[
                { "id": "example_core", "relationship": "required", "version": "^1.0.0" },
                { "id": "example_power", "relationship": "optional", "version": ">=2.0, <3.0" }
            ]"#,
        ),
        everywhere("example_core", "1.2.0", "[]"),
        everywhere(
            "example_power",
            "2.4.0",
            r#"[{ "id": "example_core", "relationship": "required", "version": "^1.0.0" }]"#,
        ),
        package(
            "example_shaders",
            "0.9.0",
            r#"["client"]"#,
            r#"[{
                "id": "example_core",
                "relationship": "required",
                "version": "^1.0.0",
                "sides": ["client"]
            }]"#,
        ),
        everywhere(
            "example_tweaks",
            "1.0.0",
            r#"[{ "id": "example_machines", "relationship": "discouraged", "version": "*" }]"#,
        ),
    ]
}

#[test]
fn a_pack_walks_from_manifest_documents_to_one_load_order() {
    let packages = pack();
    let plan = resolve(&packages, Side::Client).expect("the pack should resolve");

    assert_eq!(
        names(plan.order()),
        [
            "example_core",
            "example_power",
            "example_machines",
            "example_shaders",
            "example_tweaks",
        ],
        "every mod loads after everything it declared a dependency on"
    );
    assert_eq!(plan.side(), Side::Client);
    assert_eq!(plan.discouraged().len(), 1);
    assert_eq!(
        plan.discouraged()[0].diagnostic().text,
        "`example_tweaks` discourages `example_machines`, and `example_machines` 3.1.0 is \
         installed"
    );
}

#[test]
fn the_order_of_a_pack_does_not_depend_on_the_order_it_was_discovered_in() {
    let packages = pack();
    let expected = resolve(&packages, Side::Client).expect("the pack should resolve");

    let mut rotated = packages.clone();
    for _ in 0..packages.len() {
        rotated.rotate_left(1);
        assert_eq!(
            resolve(&rotated, Side::Client).expect("the pack should resolve"),
            expected
        );

        let mut reversed = rotated.clone();
        reversed.reverse();
        assert_eq!(
            resolve(&reversed, Side::Client).expect("the pack should resolve"),
            expected
        );
    }
}

#[test]
fn a_side_gets_the_plan_its_own_declarations_produce() {
    let packages = pack();
    let client = resolve(&packages, Side::Client).expect("the pack should resolve");
    let server = resolve(&packages, Side::DedicatedServer).expect("the pack should resolve");

    assert!(names(client.order()).contains(&"example_shaders".to_owned()));
    assert!(
        !names(server.order()).contains(&"example_shaders".to_owned()),
        "a client-only mod never loads on a server side"
    );
    assert_ne!(client, server);
    assert_eq!(server.side(), Side::DedicatedServer);
}

#[test]
fn every_dependency_outcome_the_spec_names_is_reported_distinctly() {
    let cases = [
        (
            FailureKind::MissingRequiredDependency,
            vec![everywhere(
                "example_machines",
                "1.0.0",
                r#"[{ "id": "example_core", "relationship": "required", "version": "^1.0.0" }]"#,
            )],
        ),
        (
            FailureKind::IncompatibleVersion,
            vec![
                everywhere(
                    "example_machines",
                    "1.0.0",
                    r#"[{ "id": "example_core", "relationship": "required",
                         "version": "^2.0.0" }]"#,
                ),
                everywhere("example_core", "1.0.0", "[]"),
            ],
        ),
        (
            FailureKind::DependencyCycle,
            vec![
                everywhere(
                    "example_machines",
                    "1.0.0",
                    r#"[{ "id": "example_core", "relationship": "required", "version": "*" }]"#,
                ),
                everywhere(
                    "example_core",
                    "1.0.0",
                    r#"[{ "id": "example_machines", "relationship": "required",
                         "version": "*" }]"#,
                ),
            ],
        ),
        (
            FailureKind::SideMismatch,
            vec![
                everywhere(
                    "example_machines",
                    "1.0.0",
                    r#"[{ "id": "example_core", "relationship": "required", "version": "*" }]"#,
                ),
                package("example_core", "1.0.0", r#"["client"]"#, "[]"),
            ],
        ),
        (
            FailureKind::DuplicateIdentity,
            vec![
                everywhere("example_core", "1.0.0", "[]"),
                everywhere("example_core", "2.0.0", "[]"),
            ],
        ),
    ];

    let mut messages = BTreeSet::new();
    for (expected, packages) in cases {
        let error =
            resolve(&packages, Side::DedicatedServer).expect_err("the graph should fail closed");
        assert_eq!(error.failure_kind(), Some(expected), "{error}");
        assert_eq!(error.lifecycle_state(), LifecycleState::Validated);
        assert!(messages.insert(error.to_string()), "{error} repeats");
    }
    assert_eq!(messages.len(), 5, "each outcome reads differently");
}

#[test]
fn an_incompatible_pair_is_told_apart_from_a_version_that_is_merely_wrong() {
    let packages = [
        everywhere(
            "example_machines",
            "1.0.0",
            r#"[{ "id": "example_core", "relationship": "incompatible", "version": "*" }]"#,
        ),
        everywhere("example_core", "1.0.0", "[]"),
    ];

    let error = resolve(&packages, Side::Client).expect_err("the pair cannot load together");
    assert!(
        matches!(error, ResolutionError::IncompatiblePair { .. }),
        "{error}"
    );
    assert_eq!(
        error.failure_kind(),
        Some(FailureKind::IncompatibleVersion),
        "the spec names one outcome for both, and the variant keeps them apart"
    );
}

#[test]
fn a_broken_required_graph_yields_no_load_order_at_all() {
    let mut packages = pack();
    packages.retain(|package| package.id.as_str() != "example_core");

    let error = resolve(&packages, Side::Client).expect_err("the pack lost its library");
    assert_eq!(
        error.failure_kind(),
        Some(FailureKind::MissingRequiredDependency)
    );
    assert!(
        !error.to_string().contains('/'),
        "a resolution failure names mods, never paths: {error}"
    );
}
