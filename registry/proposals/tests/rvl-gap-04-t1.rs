// RVL-GAP-04, ordering.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: two mods contributing to one named drawing stage are entered in
// the order their declarations imply, and a pair that each want to follow the
// other is refused rather than resolved by whichever happened to load first.

use ravel_sdk::{ModId, render_stages::StageDeclaration};

#[test]
fn contributors_to_one_stage_are_entered_in_declared_order() {
    let early = ModId::new("sample").expect("a well formed owner id");
    let late = ModId::new("probe").expect("a well formed owner id");

    let mut stages = StageDeclaration::default();
    stages.contribute(&early, "after_terrain");
    stages.contribute(&late, "after_terrain");
    stages.after(&late, &early);

    let order = stages.resolve().expect("a stage without a cycle resolves");

    assert_eq!(order.at("after_terrain"), [early, late]);
}

#[test]
fn a_cycle_between_contributors_is_refused() {
    let first = ModId::new("sample").expect("a well formed owner id");
    let second = ModId::new("probe").expect("a well formed owner id");

    let mut stages = StageDeclaration::default();
    stages.contribute(&first, "after_terrain");
    stages.contribute(&second, "after_terrain");
    stages.after(&first, &second);
    stages.after(&second, &first);

    let refusal = stages
        .resolve()
        .expect_err("a cycle has no order and is not given one");

    assert_eq!(refusal.cycle(), [first, second]);
}
