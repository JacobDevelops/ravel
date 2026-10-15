// RVL-GAP-01, declaration-accepted.
//
// Original test case, written against the proposed Ravel capability. It runs
// nowhere yet: the module it names does not exist, and the point of the case is
// to say what would have to be true of it before it does.
//
// What it holds: a server-resolved entry is declared beside the registry keys a
// mod already declares, and one generation step writes it. The path the entry
// occupies is the declaration's answer, not the test case's guess, so the case
// asks the declaration where the entry went rather than asserting a path.

use ravel_sdk::{ModId, server_data::ServerDataDeclaration};

#[test]
fn a_declared_server_entry_reaches_generation() {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let mut declared = ServerDataDeclaration::new(owner.clone());

    let entry = declared
        .crafting("plate_from_ingot", br#"{"result": "sample:plate"}"#)
        .expect("a first claim on an unclaimed name is taken");

    assert_eq!(entry.owner(), &owner);

    let generated = declared.generate().expect("a taken declaration generates");
    assert_eq!(generated.files().count(), 1);
    assert!(generated.contains(entry.path()));
}

#[test]
fn generation_of_nothing_is_empty_rather_than_refused() {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let declared = ServerDataDeclaration::new(owner);

    let generated = declared.generate().expect("an empty declaration generates");
    assert_eq!(generated.files().count(), 0);
}
