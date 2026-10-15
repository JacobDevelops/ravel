use ravel_core::{ModId, Side};
use ravel_package::PackageManifest;

use super::{ResolutionError, resolve};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn package(id: &str, version: &str, sides: &str, dependencies: &str) -> PackageManifest {
    let document = format!(
        r#"{{
          "schemaVersion": 1,
          "id": "{id}",
          "version": "{version}",
          "compatibility": {{ "ravel": "*", "minecraft": ["26.0.0"] }},
          "sides": {sides},
          "dependencies": {dependencies},
          "authors": [{{ "name": "Example" }}],
          "license": "MIT",
          "source": {{ "repository": "https://example.invalid/{id}", "files": [{{ "path": "src/lib.rs", "sha256": "{DIGEST}" }}] }},
          "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
        }}"#
    );
    PackageManifest::parse(&document).expect("the fixture manifest should parse")
}

fn client(id: &str, dependencies: &str) -> PackageManifest {
    package(id, "1.0.0", r#"["client"]"#, dependencies)
}

fn ids(plan: &[ModId]) -> Vec<String> {
    plan.iter().map(ToString::to_string).collect()
}

fn order(packages: &[PackageManifest], side: Side) -> Vec<String> {
    ids(resolve(packages, side)
        .expect("the graph should resolve")
        .order())
}

fn failure(packages: &[PackageManifest], side: Side) -> ResolutionError {
    resolve(packages, side).expect_err("the graph should fail closed")
}

fn id(text: &str) -> ModId {
    text.parse().expect("a valid mod id")
}

fn permutations(packages: &[PackageManifest]) -> Vec<Vec<PackageManifest>> {
    if packages.is_empty() {
        return vec![Vec::new()];
    }
    let mut orderings = Vec::new();
    for (index, package) in packages.iter().enumerate() {
        let mut rest = packages.to_vec();
        rest.remove(index);
        for mut ordering in permutations(&rest) {
            ordering.insert(0, package.clone());
            orderings.push(ordering);
        }
    }
    orderings
}

#[test]
fn a_package_set_that_declares_nothing_loads_in_identity_order() {
    let packages = [client("example_mod", "[]"), client("aardvark", "[]")];
    assert_eq!(order(&packages, Side::Client), ["aardvark", "example_mod"]);
}

#[test]
fn an_empty_package_set_resolves_to_an_empty_plan() {
    let plan = resolve(&[], Side::Client).expect("nothing to resolve");
    assert_eq!(plan.order(), []);
    assert_eq!(plan.discouraged(), []);
    assert_eq!(plan.side(), Side::Client);
}

#[test]
fn a_required_dependency_loads_before_the_mod_that_requires_it() {
    let packages = [
        client(
            "aardvark",
            r#"[{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]"#,
        ),
        client("example_library", "[]"),
    ];
    assert_eq!(
        order(&packages, Side::Client),
        ["example_library", "aardvark"],
        "the requirement must beat identity order"
    );
}

#[test]
fn a_required_dependency_that_is_not_installed_fails_the_graph_closed() {
    let packages = [client(
        "example_mod",
        r#"[{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]"#,
    )];
    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::MissingDependency {
            dependant: id("example_mod"),
            dependency: id("example_library"),
            required: "^1.0.0".parse().expect("a valid range"),
        }
    );
}

#[test]
fn a_required_dependency_outside_the_declared_range_is_refused() {
    let packages = [
        client(
            "example_mod",
            r#"[{ "id": "example_library", "relationship": "required", "version": "^2.0.0" }]"#,
        ),
        client("example_library", "[]"),
    ];
    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::UnsatisfiedVersion {
            dependant: id("example_mod"),
            dependency: id("example_library"),
            required: "^2.0.0".parse().expect("a valid range"),
            installed: "1.0.0".parse().expect("a valid version"),
        }
    );
}

#[test]
fn every_comparator_of_the_range_grammar_decides_a_requirement() {
    for (range, version, resolves) in [
        (">=1.2, <2.0", "1.4.0", true),
        (">=1.2, <2.0", "2.0.0", false),
        ("~1.2.3", "1.2.9", true),
        ("~1.2.3", "1.3.0", false),
        ("=1.0.0", "1.0.0", true),
        ("=1.0.0", "1.0.1", false),
        ("*", "9.9.9", true),
    ] {
        let packages = [
            client(
                "example_mod",
                &format!(
                    r#"[{{ "id": "example_library", "relationship": "required",
                         "version": "{range}" }}]"#
                ),
            ),
            package("example_library", version, r#"["client"]"#, "[]"),
        ];
        assert_eq!(
            resolve(&packages, Side::Client).is_ok(),
            resolves,
            "{range} vs {version}"
        );
    }
}

#[test]
fn a_required_dependency_that_loads_on_another_side_is_a_side_mismatch() {
    let packages = [
        package(
            "example_mod",
            "1.0.0",
            r#"["client", "dedicated-server"]"#,
            r#"[{ "id": "example_library", "relationship": "required", "version": "*" }]"#,
        ),
        client("example_library", "[]"),
    ];

    assert_eq!(
        order(&packages, Side::Client),
        ["example_library", "example_mod"]
    );
    assert_eq!(
        failure(&packages, Side::DedicatedServer),
        ResolutionError::SideMismatch {
            dependant: id("example_mod"),
            dependency: id("example_library"),
            side: Side::DedicatedServer,
        },
        "an installed mod that does not load here is not a missing mod"
    );
}

#[test]
fn a_requirement_narrowed_to_another_side_does_not_apply_on_this_one() {
    let packages = [package(
        "example_mod",
        "1.0.0",
        r#"["client", "dedicated-server"]"#,
        r#"[{
            "id": "example_library",
            "relationship": "required",
            "version": "*",
            "sides": ["client"]
        }]"#,
    )];

    assert_eq!(order(&packages, Side::DedicatedServer), ["example_mod"]);
    assert!(
        matches!(
            failure(&packages, Side::Client),
            ResolutionError::MissingDependency { .. }
        ),
        "the same declaration still binds on the side it names"
    );
}

#[test]
fn a_mod_that_does_not_declare_this_side_is_absent_from_the_plan() {
    let packages = [
        client("example_mod", "[]"),
        package(
            "example_server_mod",
            "1.0.0",
            r#"["dedicated-server"]"#,
            "[]",
        ),
    ];
    assert_eq!(order(&packages, Side::Client), ["example_mod"]);
    assert_eq!(
        order(&packages, Side::DedicatedServer),
        ["example_server_mod"]
    );
    assert_eq!(
        order(&packages, Side::IntegratedServer),
        [] as [std::string::String; 0]
    );
}

#[test]
fn an_optional_dependency_is_absent_without_failing_and_orders_when_present() {
    let optional = r#"[{ "id": "aardvark", "relationship": "optional", "version": "^1.0.0" }]"#;
    let alone = [client("example_mod", optional)];
    assert_eq!(order(&alone, Side::Client), ["example_mod"]);

    let together = [client("example_mod", optional), client("aardvark", "[]")];
    assert_eq!(order(&together, Side::Client), ["aardvark", "example_mod"]);
}

#[test]
fn an_optional_dependency_present_outside_its_range_is_refused() {
    let packages = [
        client(
            "example_mod",
            r#"[{ "id": "example_library", "relationship": "optional", "version": "^2.0.0" }]"#,
        ),
        client("example_library", "[]"),
    ];
    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::UnsatisfiedVersion {
            dependant: id("example_mod"),
            dependency: id("example_library"),
            required: "^2.0.0".parse().expect("a valid range"),
            installed: "1.0.0".parse().expect("a valid version"),
        }
    );
}

#[test]
fn an_optional_dependency_loading_on_another_side_costs_nothing() {
    let packages = [
        package(
            "example_mod",
            "1.0.0",
            r#"["client", "dedicated-server"]"#,
            r#"[{ "id": "example_library", "relationship": "optional", "version": "^2.0.0" }]"#,
        ),
        client("example_library", "[]"),
    ];
    assert_eq!(order(&packages, Side::DedicatedServer), ["example_mod"]);
}

#[test]
fn two_incompatible_mods_loading_together_fail_the_graph_closed() {
    let packages = [
        client(
            "example_mod",
            r#"[{ "id": "example_rival", "relationship": "incompatible", "version": "*" }]"#,
        ),
        client("example_rival", "[]"),
    ];
    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::IncompatiblePair {
            declaring: id("example_mod"),
            rejected: id("example_rival"),
            refused: "*".parse().expect("a valid range"),
            installed: "1.0.0".parse().expect("a valid version"),
        }
    );
}

#[test]
fn an_incompatibility_that_names_a_range_only_fires_inside_it() {
    let declaration =
        r#"[{ "id": "example_rival", "relationship": "incompatible", "version": "<1.0.0" }]"#;
    let outgrown = [
        client("example_mod", declaration),
        package("example_rival", "1.0.0", r#"["client"]"#, "[]"),
    ];
    assert_eq!(
        order(&outgrown, Side::Client),
        ["example_mod", "example_rival"]
    );

    let inside = [
        client("example_mod", declaration),
        package("example_rival", "0.9.0", r#"["client"]"#, "[]"),
    ];
    assert!(matches!(
        failure(&inside, Side::Client),
        ResolutionError::IncompatiblePair { .. }
    ));
}

#[test]
fn an_incompatible_mod_that_is_not_loading_here_is_not_a_conflict() {
    let packages = [
        package(
            "example_mod",
            "1.0.0",
            r#"["client", "dedicated-server"]"#,
            r#"[{ "id": "example_rival", "relationship": "incompatible", "version": "*" }]"#,
        ),
        client("example_rival", "[]"),
    ];
    assert_eq!(order(&packages, Side::DedicatedServer), ["example_mod"]);
    assert!(matches!(
        failure(&packages, Side::Client),
        ResolutionError::IncompatiblePair { .. }
    ));
}

#[test]
fn a_discouraged_combination_is_reported_and_still_loads() {
    let packages = [
        client(
            "example_mod",
            r#"[{ "id": "example_rival", "relationship": "discouraged", "version": "*" }]"#,
        ),
        client("example_rival", "[]"),
    ];

    let plan = resolve(&packages, Side::Client).expect("discouragement does not fail");
    assert_eq!(ids(plan.order()), ["example_mod", "example_rival"]);
    assert_eq!(plan.discouraged().len(), 1);
    assert_eq!(plan.discouraged()[0].declaring, id("example_mod"));
    assert_eq!(plan.discouraged()[0].subject, id("example_rival"));
    assert_eq!(
        plan.discouraged()[0].installed,
        "1.0.0".parse().expect("a valid version")
    );
}

#[test]
fn a_discouraged_mod_that_is_absent_is_not_reported() {
    let packages = [client(
        "example_mod",
        r#"[{ "id": "example_rival", "relationship": "discouraged", "version": "*" }]"#,
    )];
    let plan = resolve(&packages, Side::Client).expect("nothing to discourage");
    assert_eq!(plan.discouraged(), []);
}

#[test]
fn a_discouraged_declaration_orders_only_when_it_asks_to() {
    let unordered = [
        client(
            "zebra",
            r#"[{ "id": "aardvark", "relationship": "discouraged", "version": "*" }]"#,
        ),
        client("aardvark", "[]"),
    ];
    assert_eq!(order(&unordered, Side::Client), ["aardvark", "zebra"]);

    let ordered = [
        client(
            "zebra",
            r#"[{
                "id": "aardvark",
                "relationship": "discouraged",
                "version": "*",
                "order": "before"
            }]"#,
        ),
        client("aardvark", "[]"),
    ];
    assert_eq!(
        order(&ordered, Side::Client),
        ["zebra", "aardvark"],
        "a declared order beats identity order"
    );
}

#[test]
fn a_dependency_declaring_order_before_puts_the_declaring_mod_first() {
    let packages = [
        client(
            "zebra",
            r#"[{
                "id": "aardvark",
                "relationship": "required",
                "version": "*",
                "order": "before"
            }]"#,
        ),
        client("aardvark", "[]"),
    ];
    assert_eq!(order(&packages, Side::Client), ["zebra", "aardvark"]);
}

#[test]
fn a_dependency_cycle_names_every_mod_in_it() {
    let packages = [
        client(
            "example_mod",
            r#"[{ "id": "example_library", "relationship": "required", "version": "*" }]"#,
        ),
        client(
            "example_library",
            r#"[{ "id": "example_mod", "relationship": "required", "version": "*" }]"#,
        ),
    ];

    let error = failure(&packages, Side::Client);
    assert_eq!(
        error,
        ResolutionError::Cycle {
            members: vec![id("example_library"), id("example_mod")],
        }
    );
    assert_eq!(
        error.to_string(),
        "the dependency cycle `example_library` -> `example_mod` -> `example_library` \
         cannot be ordered"
    );
}

#[test]
fn a_longer_cycle_is_reported_in_load_order_from_its_smallest_member() {
    let requires =
        |id: &str| format!(r#"[{{ "id": "{id}", "relationship": "required", "version": "*" }}]"#);
    let packages = [
        client("beetle", &requires("aardvark")),
        client("crane", &requires("beetle")),
        client("aardvark", &requires("crane")),
        client("dormouse", "[]"),
    ];

    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::Cycle {
            members: vec![id("aardvark"), id("beetle"), id("crane")],
        }
    );
}

#[test]
fn a_mod_that_depends_on_itself_is_a_cycle_of_one() {
    let packages = [client(
        "example_mod",
        r#"[{ "id": "example_mod", "relationship": "required", "version": "*" }]"#,
    )];
    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::Cycle {
            members: vec![id("example_mod")],
        }
    );
}

#[test]
fn contradictory_ordering_between_two_mods_is_a_cycle() {
    let packages = [
        client(
            "aardvark",
            r#"[{
                "id": "beetle",
                "relationship": "required",
                "version": "*",
                "order": "after"
            }]"#,
        ),
        client(
            "beetle",
            r#"[{
                "id": "aardvark",
                "relationship": "required",
                "version": "*",
                "order": "after"
            }]"#,
        ),
    ];
    assert!(matches!(
        failure(&packages, Side::Client),
        ResolutionError::Cycle { .. }
    ));
}

#[test]
fn two_packages_claiming_one_identity_fail_the_graph_closed() {
    let packages = [
        client("example_mod", "[]"),
        package("example_mod", "2.0.0", r#"["dedicated-server"]"#, "[]"),
    ];
    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::DuplicateIdentity {
            id: id("example_mod"),
        },
        "a collision is an installation fault, not a per-side one"
    );
}

#[test]
fn a_duplicate_is_reported_by_identity_rather_than_by_arrival() {
    let packages = [
        client("zebra", "[]"),
        client("aardvark", "[]"),
        client("zebra", "[]"),
        client("aardvark", "[]"),
    ];
    assert_eq!(
        failure(&packages, Side::Client),
        ResolutionError::DuplicateIdentity { id: id("aardvark") }
    );
}

#[test]
fn the_same_package_set_resolves_identically_from_every_input_order() {
    let packages = [
        client(
            "zebra",
            r#"[{ "id": "beetle", "relationship": "required", "version": "^1.0.0" }]"#,
        ),
        client(
            "beetle",
            r#"[{ "id": "aardvark", "relationship": "optional", "version": "*" }]"#,
        ),
        client("aardvark", "[]"),
        client(
            "crane",
            r#"[{ "id": "aardvark", "relationship": "discouraged", "version": "*" }]"#,
        ),
        client("dormouse", "[]"),
    ];

    let orderings = permutations(&packages);
    assert_eq!(orderings.len(), 120, "every ordering of five packages");

    let expected = resolve(&packages, Side::Client).expect("the graph should resolve");
    for ordering in orderings {
        assert_eq!(
            resolve(&ordering, Side::Client).expect("the graph should resolve"),
            expected,
            "{:?}",
            ids(&ordering
                .iter()
                .map(|package| package.id.clone())
                .collect::<Vec<_>>())
        );
    }
    assert_eq!(
        ids(expected.order()),
        ["aardvark", "beetle", "crane", "dormouse", "zebra"]
    );
}

#[test]
fn a_failure_is_the_same_failure_from_every_input_order() {
    let packages = [
        client(
            "zebra",
            r#"[{ "id": "beetle", "relationship": "required", "version": "^2.0.0" }]"#,
        ),
        client("beetle", "[]"),
        client(
            "aardvark",
            r#"[{ "id": "crane", "relationship": "required", "version": "*" }]"#,
        ),
    ];

    let expected = failure(&packages, Side::Client);
    for ordering in permutations(&packages) {
        assert_eq!(failure(&ordering, Side::Client), expected);
    }
    assert_eq!(
        expected,
        ResolutionError::MissingDependency {
            dependant: id("aardvark"),
            dependency: id("crane"),
            required: "*".parse().expect("a valid range"),
        },
        "the smallest identity reports first, whatever order they arrived in"
    );
}

#[test]
fn a_mod_may_declare_several_relationships_at_once() {
    let packages = [
        client(
            "zebra",
            r#"[
                { "id": "aardvark", "relationship": "required", "version": "^1.0.0" },
                { "id": "beetle", "relationship": "optional", "version": "*" },
                { "id": "crane", "relationship": "discouraged", "version": "*" },
                { "id": "emu", "relationship": "incompatible", "version": "*" }
            ]"#,
        ),
        client("aardvark", "[]"),
        client("beetle", "[]"),
        client("crane", "[]"),
    ];

    let plan = resolve(&packages, Side::Client).expect("the graph should resolve");
    assert_eq!(ids(plan.order()), ["aardvark", "beetle", "crane", "zebra"]);
    assert_eq!(plan.discouraged().len(), 1);
}

#[test]
fn a_relationship_declared_twice_orders_once() {
    let packages = [
        client(
            "zebra",
            r#"[
                { "id": "aardvark", "relationship": "required", "version": "^1.0.0" },
                { "id": "aardvark", "relationship": "optional", "version": "*" }
            ]"#,
        ),
        client("aardvark", "[]"),
    ];
    assert_eq!(order(&packages, Side::Client), ["aardvark", "zebra"]);
}
