//! Differential conformance between Rust validators and schema patterns run by check-jsonschema.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ravel_core::ModId;
use ravel_package::{PackageManifest, RelativePath, Sha256Digest};
use serde_json::{Value, json};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn schema_path() -> PathBuf {
    repository_root().join("schemas/package.schema.json")
}

fn schema() -> Value {
    let document =
        fs::read_to_string(schema_path()).expect("the package schema should be readable");
    serde_json::from_str(&document).expect("the package schema should be JSON")
}

fn defined_pattern(name: &str) -> String {
    schema()["$defs"][name]["pattern"]
        .as_str()
        .expect("the definition should declare a pattern")
        .to_owned()
}

fn scratch(label: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("ravel-conformance-{label}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale directory should be removable");
    }
    fs::create_dir_all(&root).expect("the directory should be creatable");
    root
}

fn validates(schema: &Value, instance: &Value, label: &str) -> bool {
    let root = scratch(label);
    let schema_file = root.join("schema.json");
    let instance_file = root.join("instance.json");
    fs::write(&schema_file, schema.to_string()).expect("the schema should be writable");
    fs::write(&instance_file, instance.to_string()).expect("the instance should be writable");

    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(&schema_file)
        .arg(&instance_file)
        .output()
        .expect("check-jsonschema should be on PATH inside the devenv shell");
    fs::remove_dir_all(&root).expect("cleanup");
    output.status.success()
}

fn validates_against_package_schema(document: &str, label: &str) -> bool {
    let root = scratch(label);
    let path = root.join("manifest.json");
    fs::write(&path, document).expect("the manifest should be writable");

    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(schema_path())
        .arg(&path)
        .output()
        .expect("check-jsonschema should be on PATH inside the devenv shell");
    fs::remove_dir_all(&root).expect("cleanup");
    output.status.success()
}

// Batch both acceptance directions into two external checks.
fn assert_agrees(
    pattern: &str,
    candidates: &[String],
    accepts: impl Fn(&str) -> bool,
    label: &str,
) {
    let (accepted, rejected): (Vec<&String>, Vec<&String>) =
        candidates.iter().partition(|candidate| accepts(candidate));
    assert!(!accepted.is_empty(), "{label}: nothing was accepted");
    assert!(!rejected.is_empty(), "{label}: nothing was rejected");

    let matcher = json!({ "type": "string", "pattern": pattern });
    assert!(
        validates(
            &json!({ "type": "array", "items": matcher }),
            &json!(accepted),
            &format!("{label}-accepted"),
        ),
        "{label}: the schema refuses a candidate the Rust type accepts"
    );
    assert!(
        validates(
            &json!({ "type": "array", "items": { "not": matcher } }),
            &json!(rejected),
            &format!("{label}-rejected"),
        ),
        "{label}: the schema accepts a candidate the Rust type refuses"
    );
}

fn candidates_over(alphabet: &[char]) -> Vec<String> {
    let mut candidates = vec![String::new()];
    let mut frontier = vec![String::new()];
    for _ in 0_u8..3 {
        let mut next = Vec::new();
        for prefix in &frontier {
            for character in alphabet {
                let mut candidate = prefix.clone();
                candidate.push(*character);
                next.push(candidate);
            }
        }
        candidates.extend(next.iter().cloned());
        frontier = next;
    }
    candidates
}

fn manifest_document(path: &str, side: &str) -> String {
    let document = json!({
        "schemaVersion": 1,
        "id": "example_mod",
        "version": "1.4.0",
        "displayName": "Example Mod",
        "compatibility": { "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0", "26.0.1"] },
        "sides": [side],
        "dependencies": [{
            "id": "example_library",
            "relationship": "required",
            "version": "^1.0.0",
            "sides": ["client"],
            "order": "after"
        }],
        "source": {
            "repository": "https://example.invalid/example_mod",
            "files": [{ "path": path, "sha256": DIGEST }]
        },
        "resources": ["resources/lang/en.json"],
        "savedDataVersions": [1],
        "authors": [{ "name": "Example", "url": "https://example.invalid/example" }],
        "license": "MIT OR Apache-2.0",
        "provenance": { "record": "provenance.json", "sha256": DIGEST }
    });
    document.to_string()
}

fn valid_document() -> String {
    manifest_document("src/lib.rs", "client")
}

#[test]
fn the_mod_id_grammar_agrees_with_the_schema_pattern_under_a_real_engine() {
    let candidates = candidates_over(&['a', 'z', '9', '-', '_', '.', 'A', '/']);
    assert_eq!(candidates.len(), 1 + 8 + 64 + 512);
    assert_agrees(
        &defined_pattern("modId"),
        &candidates,
        |candidate| candidate.parse::<ModId>().is_ok(),
        "mod-id",
    );
}

#[test]
fn the_package_path_grammar_agrees_with_the_schema_pattern_under_a_real_engine() {
    let mut candidates = candidates_over(&['a', '/', '.', '\\', '\n']);
    candidates.extend(
        [
            "artifacts/linux-x86_64/mod.bin",
            "nested/../../escape",
            "..\\..\\escape",
            "\\\\server\\share",
            "a\rb",
            "a\u{2028}b",
            "a\u{2029}b",
            "trailing\n",
            "a\tb",
            "a b",
            "resources/lang/en.json",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    assert_agrees(
        &defined_pattern("relativePath"),
        &candidates,
        |candidate| RelativePath::try_from(candidate.to_owned()).is_ok(),
        "relative-path",
    );
}

#[test]
fn the_digest_grammar_agrees_with_the_schema_pattern_under_a_real_engine() {
    let candidates: Vec<String> = [
        DIGEST.to_owned(),
        DIGEST.to_uppercase(),
        "0".repeat(64),
        "f".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
        String::new(),
        format!("g{}", &DIGEST[1..]),
        format!(" {}", &DIGEST[1..]),
        format!("{}\u{e9}", &DIGEST[..62]),
    ]
    .into_iter()
    .collect();
    assert_agrees(
        &defined_pattern("sha256"),
        &candidates,
        |candidate| Sha256Digest::try_from(candidate.to_owned()).is_ok(),
        "sha256",
    );
}

#[test]
fn a_manifest_ravel_package_accepts_is_one_the_schema_validates() {
    let document = valid_document();
    assert!(PackageManifest::parse(&document).is_ok());
    assert!(validates_against_package_schema(&document, "accepted"));
}

#[test]
fn a_manifest_ravel_package_rendered_is_one_the_schema_validates() {
    let manifest = PackageManifest::parse(&valid_document()).expect("the fixture should parse");
    assert!(validates_against_package_schema(
        &manifest.to_json(),
        "rendered"
    ));
}

#[test]
fn a_document_ravel_package_refuses_is_one_the_schema_also_refuses() {
    let valid = valid_document();
    let base: Value = serde_json::from_str(&valid).expect("the fixture is JSON");
    let mut broken = Vec::new();

    let mut document = base.clone();
    document["signature"] = json!("x");
    broken.push(("unknown-field", document));

    let mut document = base.clone();
    document["id"] = json!("Example_Mod");
    broken.push(("bad-id", document));

    let mut document = base.clone();
    document["version"] = json!("1.4");
    broken.push(("bad-version", document));

    let mut document = base.clone();
    document["sides"] = json!([]);
    broken.push(("empty-sides", document));

    let mut document = base.clone();
    document["targets"] = json!([]);
    broken.push(("old-targets", document));

    let mut document = base.clone();
    document["source"]["files"] = json!([]);
    broken.push(("empty-source-files", document));

    let mut document = base.clone();
    document["source"]["files"][0]["size"] = json!(1);
    broken.push(("unknown-source-field", document));

    let mut document = base.clone();
    document["source"]["files"][0]["sha256"] = json!(DIGEST.to_uppercase());
    broken.push(("bad-digest", document));

    for (label, path) in [
        ("absolute-path", "/src/lib.rs"),
        ("traversal-path", "../src/lib.rs"),
        ("backslash-path", "..\\..\\src\\lib.rs"),
        ("line-break-path", "src/\nlib.rs"),
    ] {
        let mut document = base.clone();
        document["source"]["files"][0]["path"] = json!(path);
        broken.push((label, document));
    }

    for (label, document) in broken {
        let document = document.to_string();
        assert_ne!(document, valid, "{label} should have changed the document");
        assert!(
            PackageManifest::parse(&document).is_err(),
            "{label}: ravel-package accepted it"
        );
        assert!(
            !validates_against_package_schema(&document, label),
            "{label}: the schema accepted it"
        );
    }
}

#[test]
fn the_schema_still_declares_what_ravel_package_implements() {
    let schema = schema();

    assert_eq!(schema["$id"], "urn:ravel:schema:package:1");
    assert_eq!(schema["properties"]["schemaVersion"]["const"], 1);
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["required"],
        json!([
            "schemaVersion",
            "id",
            "version",
            "compatibility",
            "sides",
            "authors",
            "license",
            "source",
            "provenance"
        ])
    );
    assert_eq!(
        schema["properties"]["source"]["required"],
        json!(["repository", "files"])
    );
    assert_eq!(
        schema["properties"]["source"]["properties"]["files"]["minItems"],
        1
    );
    assert_eq!(
        schema["$defs"]["sourceFile"]["required"],
        json!(["path", "sha256"])
    );
    assert_eq!(
        schema["$defs"]["dependency"]["properties"]["relationship"]["enum"],
        json!(["required", "optional", "incompatible", "discouraged"])
    );
    assert_eq!(
        schema["$defs"]["dependency"]["properties"]["order"]["enum"],
        json!(["before", "after"])
    );
    assert_eq!(
        schema["properties"]["sides"]["items"]["enum"],
        json!(["client", "integrated-server", "dedicated-server"])
    );
}

#[test]
fn every_object_the_schema_defines_refuses_additional_properties() {
    let mut checked = 0_usize;
    let mut objects = vec![schema()];
    while let Some(node) = objects.pop() {
        if node["type"] == "object" {
            assert_eq!(node["additionalProperties"], false, "{node}");
            checked += 1;
        }
        for child in node
            .as_object()
            .into_iter()
            .flat_map(serde_json::Map::values)
        {
            objects.push(child.clone());
        }
    }
    assert!(
        checked >= 6,
        "the schema should define at least six objects"
    );
}
