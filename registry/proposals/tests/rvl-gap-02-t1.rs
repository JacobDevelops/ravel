// RVL-GAP-02, round-trip.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: the set of world-shaping mods in force when a world is made is
// written into that world and read back equal. This is the half that makes the
// refusal in the sibling case possible at all, so it is stated on its own: a
// binding nothing can read back is a binding nothing can enforce.

use ravel_sdk::{ModId, world_shaping::{ShapingRecord, WorldShaping}};

#[test]
fn the_shaping_set_written_at_creation_is_read_back_equal() {
    let first = ModId::new("sample").expect("a well formed owner id");
    let second = ModId::new("probe").expect("a well formed owner id");

    let mut shaping = WorldShaping::default();
    shaping.declare(first.clone());
    shaping.declare(second.clone());

    let written = ShapingRecord::at_creation(&shaping);
    let read = ShapingRecord::read(&written.bytes()).expect("what was written reads back");

    assert_eq!(read.mods(), [first, second]);
}

#[test]
fn a_record_the_capability_did_not_write_is_refused() {
    let refusal = ShapingRecord::read(b"not a shaping record")
        .expect_err("bytes from nowhere are refused rather than guessed at");

    assert!(refusal.is_unreadable());
}
