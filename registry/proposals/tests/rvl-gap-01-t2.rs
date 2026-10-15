// RVL-GAP-01, generation-determinism.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: the bytes a declaration generates do not depend on the order
// the entries were declared in, and do not change between two runs of the same
// build. The case declares the same three entries twice in opposite orders and
// requires one answer, because a generated tree that differs by declaration
// order is a tree no later step can compare.

use ravel_sdk::{ModId, server_data::ServerDataDeclaration};

fn declare(order: [&str; 3]) -> Vec<(String, Vec<u8>)> {
    let owner = ModId::new("sample").expect("a well formed owner id");
    let mut declared = ServerDataDeclaration::new(owner);

    for name in order {
        declared
            .crafting(name, br#"{"result": "sample:plate"}"#)
            .expect("each name is claimed once");
    }

    declared
        .generate()
        .expect("a taken declaration generates")
        .files()
        .map(|file| (file.path().to_string(), file.bytes().to_vec()))
        .collect()
}

#[test]
fn generated_bytes_do_not_depend_on_declaration_order() {
    let forward = declare(["first", "second", "third"]);
    let backward = declare(["third", "second", "first"]);

    assert_eq!(forward, backward);
}

#[test]
fn generated_bytes_do_not_change_between_runs() {
    let once = declare(["first", "second", "third"]);
    let twice = declare(["first", "second", "third"]);

    assert_eq!(once, twice);
}
