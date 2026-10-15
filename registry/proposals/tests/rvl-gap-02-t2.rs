// RVL-GAP-02, absent-dependency-refused.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: a world made while a shaping mod was in force is refused on a
// later launch that no longer supplies that mod, and the refusal names what is
// missing. The point is when it is refused. A world that opens and then differs
// silently is worse than one that does not open, so the case requires the answer
// before any world data is read.

use ravel_sdk::{ModId, world_shaping::{ShapingRecord, WorldShaping}};

#[test]
fn a_world_is_refused_when_a_shaping_mod_is_gone() {
    let present = ModId::new("sample").expect("a well formed owner id");
    let departed = ModId::new("probe").expect("a well formed owner id");

    let mut at_creation = WorldShaping::default();
    at_creation.declare(present.clone());
    at_creation.declare(departed.clone());
    let written = ShapingRecord::at_creation(&at_creation);

    let mut now = WorldShaping::default();
    now.declare(present);

    let refusal = written
        .admit(&now)
        .expect_err("a world is refused when what shaped it is gone");

    assert_eq!(refusal.missing(), [departed]);
}

#[test]
fn a_shaping_mod_added_after_creation_is_refused_too() {
    let original = ModId::new("sample").expect("a well formed owner id");
    let arrival = ModId::new("probe").expect("a well formed owner id");

    let mut at_creation = WorldShaping::default();
    at_creation.declare(original.clone());
    let written = ShapingRecord::at_creation(&at_creation);

    let mut now = WorldShaping::default();
    now.declare(original);
    now.declare(arrival.clone());

    let refusal = written
        .admit(&now)
        .expect_err("a world is refused when something new would shape it");

    assert_eq!(refusal.arrived(), [arrival]);
}
