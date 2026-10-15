use std::path::Path;

use crate::json::redacted_path;

#[test]
fn gate_output_never_names_an_absolute_path() {
    let absolute = std::env::current_dir()
        .expect("the current directory must be available")
        .join("evidence/m0/manifest.json");

    assert_eq!(redacted_path(&absolute), "evidence/m0/manifest.json");
    assert_eq!(
        redacted_path(Path::new("evidence/m0/manifest.json")),
        "evidence/m0/manifest.json"
    );
    assert_eq!(
        redacted_path(Path::new("/somewhere/else/manifest.json")),
        "manifest.json"
    );
}
