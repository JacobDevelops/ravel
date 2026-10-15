// RVL-GAP-06, conflict-refused.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: a set with no version that satisfies everyone is refused, and
// so is a requirement nothing provides. Both are refused before anything runs.
// The alternative is what the counted evidence shows the entries doing today,
// which is each shipping a copy and hoping the one that wins is theirs.

use ravel_sdk::{ModId, VersionRange, provision::{Provided, Provision}};

#[test]
fn two_requirements_no_version_satisfies_are_refused() {
    let provider = ModId::new("sample").expect("a well formed owner id");
    let older = ModId::new("probe").expect("a well formed owner id");
    let newer = ModId::new("witness").expect("a well formed owner id");

    let mut provision = Provision::default();
    provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
    provision.requires(&older, "shared_maths", VersionRange::parse("^1").expect("a range"));
    provision.requires(&newer, "shared_maths", VersionRange::parse("^2").expect("a range"));

    let refusal = provision
        .resolve()
        .expect_err("no single version answers both, so the set is refused");

    assert_eq!(refusal.library(), "shared_maths");
    assert_eq!(refusal.unsatisfied(), [older]);
}

#[test]
fn a_requirement_nothing_provides_is_refused() {
    let consumer = ModId::new("probe").expect("a well formed owner id");

    let mut provision = Provision::default();
    provision.requires(&consumer, "shared_maths", VersionRange::parse("^2").expect("a range"));

    let refusal = provision
        .resolve()
        .expect_err("a library nothing provides is refused before anything runs");

    assert_eq!(refusal.library(), "shared_maths");
    assert!(refusal.has_no_provider());
}
