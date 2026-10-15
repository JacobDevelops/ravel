// RVL-GAP-04, absent-dependency-refused.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: a stage name nothing publishes is refused when it is declared,
// not ignored when the drawing happens. A contribution to a stage that does not
// exist is the mistake a mod author makes most cheaply and finds most expensively,
// so the capability owes an answer at declaration time.

use ravel_sdk::{ModId, render_stages::StageDeclaration};

#[test]
fn a_contribution_to_an_unpublished_stage_is_refused() {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let mut stages = StageDeclaration::default();

    let refusal = stages
        .contribute_checked(&owner, "a_stage_nobody_publishes")
        .expect_err("a stage nothing publishes cannot be contributed to");

    assert_eq!(refusal.unknown_stage(), "a_stage_nobody_publishes");
}

#[test]
fn an_ordering_against_a_mod_that_contributes_nothing_is_refused() {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let absent = ModId::new("probe").expect("a well formed owner id");

    let mut stages = StageDeclaration::default();
    stages.contribute(&owner, "after_terrain");
    stages.after(&owner, &absent);

    let refusal = stages
        .resolve()
        .expect_err("an order against nothing is refused rather than dropped");

    assert_eq!(refusal.unknown_contributors(), [absent]);
}
