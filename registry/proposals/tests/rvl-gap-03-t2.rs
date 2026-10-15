// RVL-GAP-03, side-refusal.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: playing is client work, and asking for it on a server is
// refused rather than skipped. Skipping is the failure mode worth writing a case
// against, because a call that quietly does nothing on one side is a mod that
// works in single player and is silent for everybody else.

use ravel_sdk::{ModId, Side, audio::AudioDeclaration};

fn declared() -> AudioDeclaration {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let mut declared = AudioDeclaration::new(owner);
    declared
        .entry("door_opens", b"the supplied bytes")
        .expect("a first claim on an unclaimed name is taken");
    declared
        .event("door_opens")
        .expect("an event is declared for an entry that exists");
    declared
}

#[test]
fn playing_on_a_server_is_refused() {
    let refusal = declared()
        .play("door_opens", Side::Server)
        .expect_err("a server has nothing to play with and says so");

    assert_eq!(refusal.wrong_side(), Side::Server);
}

#[test]
fn declaring_the_entry_on_a_server_is_not_refused() {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let mut declared = AudioDeclaration::new(owner);

    declared
        .entry("door_opens", b"the supplied bytes")
        .expect("a server may own the bytes it will never play");
}
