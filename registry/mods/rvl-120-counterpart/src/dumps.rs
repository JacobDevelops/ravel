//! The documents this package renders out of its own tables.
//!
//! Nothing here decides anything. Each function asks a module of this crate for
//! what it already builds — the registration census of §4, the thirteen packet
//! contracts of §9, the seventy-three declared settings of §3 — and renders it
//! in a form a reader or a parity run can read without a Rust toolchain.
//!
//! Every document is rendered from the crate's own functions rather than
//! transcribed, so a table that changes changes the document, and the test
//! beside these functions fails until the tree on disk is regenerated. Each
//! document ends with a trailing newline and each is deterministic: the two
//! tabular ones are written in the order their source table declares, which is
//! the order the wire and the configuration screen use.

use std::fmt::Write as _;

use ravel_sdk::{JsonValue, json};

/// Every generated document, as a package-relative path and its bytes.
///
/// # Panics
///
/// Panics when a name this package packages is not a legal content path, which
/// is a packaging failure rather than a runtime one.
#[must_use]
pub fn documents() -> Vec<(String, String)> {
    vec![
        ("generated/census.json".to_owned(), census()),
        ("generated/network.txt".to_owned(), network()),
        ("generated/settings.txt".to_owned(), settings()),
    ]
}

/// Renders what this package registers, from [`crate::census`].
///
/// The claimed entries are read back out of the declaration rather than from
/// the constant behind it, so the document says what the registration barrier
/// would actually receive.
///
/// # Panics
///
/// Panics when the declared recipe path is not a legal content path.
#[must_use]
pub fn census() -> String {
    let claimed: Vec<JsonValue> = crate::census::claimed()
        .expect("a packaged content path is legal")
        .iter()
        .map(|key| {
            json!({
                "kind": key.kind().as_str(),
                "id": key.id().to_string(),
            })
        })
        .collect();

    let document = json!({
        "owner": crate::MOD_ID,
        "claimed": claimed,
        "attachments": crate::census::ATTACHMENTS,
        "components": crate::census::COMPONENTS,
        "translations": crate::census::TRANSLATIONS,
        "texture": crate::census::TEXTURE,
        "blockStateDocuments": crate::census::BLOCK_STATE_DOCUMENTS,
        "dataDocuments": crate::census::DATA_DOCUMENTS,
        "integrations": crate::INTEGRATIONS,
    });
    format!("{document}\n")
}

/// Renders the packet contracts of [`crate::network`] as a tab-separated table.
///
/// The rows are in the order [`crate::network::PAYLOADS`] declares them, which
/// is the order the contracts are built in.
///
/// # Panics
///
/// Panics when a packaged payload name is not a legal content path.
#[must_use]
pub fn network() -> String {
    let mut rendered = String::from("contract\tdirection\tversion\n");
    for contract in crate::network::contracts().expect("a packaged payload name is a legal path") {
        writeln!(
            rendered,
            "{}\t{}\t{}",
            contract.id(),
            contract.direction().as_str(),
            contract.version()
        )
        .expect("writing to a string never fails");
    }
    rendered
}

/// Renders the declared settings of [`crate::settings`] as a tab-separated table.
///
/// The rows are in declaration order — the three server settings, then the
/// seventy colour roles — and each default is the one the declaration carries,
/// written in its JSON form so that a colour's six-digit string is told apart
/// from a boolean and from the ceiling's number. A colour appears in the stored
/// form, with its alpha byte already dropped.
///
/// # Panics
///
/// Panics when the declaration this package builds from its own tables is one
/// the configuration schema refuses.
#[must_use]
pub fn settings() -> String {
    let declaration = crate::settings::declaration().expect("a packaged declaration is legal");
    let mut rendered = String::from("key\tside\tdefault\n");
    for setting in &declaration.settings {
        writeln!(
            rendered,
            "{}\t{}\t{}",
            setting.key.as_str(),
            setting.side.name(),
            setting.default
        )
        .expect("writing to a string never fails");
    }
    rendered
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Rebuilds every document and holds the tree on disk to it byte for byte.
    ///
    /// Setting `RAVEL_REGENERATE_GENERATED` writes the documents instead, which
    /// is how the tree is produced; with the variable unset this is the check
    /// that it has not drifted from the tables it was rendered from.
    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        let rendered = documents();
        assert!(
            !rendered.is_empty(),
            "the package ships generated documents"
        );

        for (path, document) in rendered {
            assert!(
                document.ends_with('\n'),
                "{path} ends with a trailing newline"
            );
            let target = root.join(&path);
            if regenerating {
                let directory = target.parent().expect("a document has a parent directory");
                std::fs::create_dir_all(directory).expect("the directory can be created");
                std::fs::write(&target, document.as_bytes()).expect("the document can be written");
            } else {
                let on_disk = std::fs::read_to_string(&target)
                    .unwrap_or_else(|error| panic!("{path} could not be read: {error}"));
                assert_eq!(
                    on_disk, document,
                    "{path} has drifted from what the crate renders"
                );
            }
        }
    }
}
