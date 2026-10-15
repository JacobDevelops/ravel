// RVL-GAP-05, conflict-refused.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: two mods that each want to replace an outcome at one point are
// a refusal at the barrier, not a race decided by load order. Refusing and
// proceeding compose, because either answer is still the game's own; replacing
// does not, because two replacements are two different games.

use ravel_sdk::{ModId, interception::{Outcome, Points}};

#[test]
fn two_replacements_at_one_point_are_refused_at_the_barrier() {
    let first = ModId::new("sample").expect("a well formed owner id");
    let second = ModId::new("probe").expect("a well formed owner id");

    let mut points = Points::default();
    points.replace_at(&first, "block_is_broken", |_| Outcome::Proceed);
    points.replace_at(&second, "block_is_broken", |_| Outcome::Proceed);

    let refusal = points
        .admit()
        .expect_err("two replacements at one point are a conflict");

    assert_eq!(refusal.point(), "block_is_broken");
    assert_eq!(refusal.claimants(), [first, second]);
}

#[test]
fn a_replacement_beside_plain_answers_is_admitted() {
    let replacing = ModId::new("sample").expect("a well formed owner id");
    let answering = ModId::new("probe").expect("a well formed owner id");

    let mut points = Points::default();
    points.replace_at(&replacing, "block_is_broken", |_| Outcome::Proceed);
    points.answer_at(&answering, "block_is_broken", |_| Outcome::Refuse);

    points.admit().expect("one replacement beside answers is admitted");
}
