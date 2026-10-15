// RVL-GAP-05, ordering.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: several mods answering at one published point are asked in a
// declared order, every one of them is asked while the outcome still stands, and
// the first refusal ends the round. The last clause is the whole design: a point
// whose answers are collected and then reconciled has no meaning for the mod
// that refused.

use ravel_sdk::{ModId, interception::{Outcome, Points}};

#[test]
fn mods_at_one_point_are_asked_in_declared_order() {
    let early = ModId::new("sample").expect("a well formed owner id");
    let late = ModId::new("probe").expect("a well formed owner id");

    let mut points = Points::default();
    points.answer_at(&early, "block_is_broken", |_| Outcome::Proceed);
    points.answer_at(&late, "block_is_broken", |_| Outcome::Proceed);
    points.after(&late, &early);

    let round = points.ask("block_is_broken").expect("a published point is asked");

    assert_eq!(round.asked(), [early, late]);
    assert_eq!(round.outcome(), Outcome::Proceed);
}

#[test]
fn the_first_refusal_ends_the_round() {
    let early = ModId::new("sample").expect("a well formed owner id");
    let late = ModId::new("probe").expect("a well formed owner id");

    let mut points = Points::default();
    points.answer_at(&early, "block_is_broken", |_| Outcome::Refuse);
    points.answer_at(&late, "block_is_broken", |_| Outcome::Proceed);
    points.after(&late, &early);

    let round = points.ask("block_is_broken").expect("a published point is asked");

    assert_eq!(round.asked(), [early]);
    assert_eq!(round.outcome(), Outcome::Refuse);
}
