pub(super) const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

pub(super) fn manifest_document(
    ravel: &str,
    minecraft: &str,
    source_digest: &str,
    record_digest: &str,
) -> String {
    format!(
        r#"{{
          "schemaVersion": 1,
          "id": "example_mod",
          "version": "1.4.0",
          "compatibility": {{ "ravel": "{ravel}", "minecraft": {minecraft} }},
          "sides": ["client"],
          "authors": [{{ "name": "Example" }}],
          "license": "MIT",
          "source": {{
            "repository": "https://example.invalid/example_mod",
            "files": [{{ "path": "src/lib.rs", "sha256": "{source_digest}" }}]
          }},
          "provenance": {{ "record": "provenance.json", "sha256": "{record_digest}" }}
        }}"#
    )
}
