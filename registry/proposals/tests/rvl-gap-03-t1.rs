// RVL-GAP-03, declaration-accepted.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: an audio entry and the event that plays it are one declaration,
// so the bytes and what reaches for them cannot drift apart. The case declares
// an entry, asks the declaration for the event, and requires the event to name
// the entry rather than a path a caller had to know.

use ravel_sdk::{ModId, audio::AudioDeclaration};

#[test]
fn a_declared_audio_entry_yields_an_event_that_names_it() {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let mut declared = AudioDeclaration::new(owner.clone());

    let entry = declared
        .entry("door_opens", b"the supplied bytes")
        .expect("a first claim on an unclaimed name is taken");
    let event = declared
        .event("door_opens")
        .expect("an event is declared for an entry that exists");

    assert_eq!(event.entry(), entry.name());
    assert_eq!(entry.owner(), &owner);
}

#[test]
fn an_event_for_no_declared_entry_is_refused() {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let mut declared = AudioDeclaration::new(owner);

    let refusal = declared
        .event("door_opens")
        .expect_err("an event with nothing to play is refused at declaration");

    assert_eq!(refusal.unknown_entry(), "door_opens");
}
