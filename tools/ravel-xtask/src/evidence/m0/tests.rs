use serde_json::{Value, json};

use super::m0_platform_matches;

fn platform() -> Value {
    json!({
        "platform": "linux",
        "architecture": "x86_64",
        "native": true,
        "hardwareProfile": "linux-reference-v1",
        "versions": {
            "minecraft": "26.1.2",
            "neoforge": "26.1.2.78",
            "prism": "11.0.3",
            "fixture": "ATM11 0.2.1",
            "java": "25.0.1",
            "rust": "1.95.0",
            "nix": "2.33.3",
            "devenv": "2.1.2"
        },
        "settings": {"renderDistance": 12},
        "commands": [
            {"command": "devenv test --no-tui"},
            {"command": "nix flake check --accept-flake-config"}
        ],
        "baselines": [{"workload": "vanilla"}, {"workload": "official-atm11"}]
    })
}

#[test]
fn complete_native_linux_evidence_matches() {
    assert!(m0_platform_matches(&platform()));
}

#[test]
fn every_missing_or_null_platform_field_is_refused() {
    for pointer in [
        "/platform",
        "/architecture",
        "/native",
        "/hardwareProfile",
        "/versions/minecraft",
        "/versions/devenv",
        "/settings",
        "/commands",
        "/baselines",
    ] {
        let mut nulled = platform();
        *nulled.pointer_mut(pointer).expect("pointer should exist") = Value::Null;
        assert!(!m0_platform_matches(&nulled), "null {pointer} was accepted");

        let mut missing = platform();
        let (parent, field) = pointer
            .rsplit_once('/')
            .expect("pointer should have a parent");
        let parent = if parent.is_empty() {
            &mut missing
        } else {
            missing.pointer_mut(parent).expect("parent should exist")
        };
        parent
            .as_object_mut()
            .expect("parent should be an object")
            .remove(field);
        assert!(
            !m0_platform_matches(&missing),
            "missing {pointer} was accepted"
        );
    }
}

#[test]
fn empty_settings_and_a_missing_baseline_are_refused() {
    let mut empty = platform();
    empty["settings"] = json!({});
    assert!(!m0_platform_matches(&empty));

    let mut one_baseline = platform();
    one_baseline["baselines"] = json!([{"workload": "vanilla"}]);
    assert!(!m0_platform_matches(&one_baseline));

    let mut string_native = platform();
    string_native["native"] = json!("true");
    assert!(!m0_platform_matches(&string_native));
}
