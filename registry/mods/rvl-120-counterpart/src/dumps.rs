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

use ravel_sdk::{JsonValue, ModError, json};

/// Where the registration census is written.
pub const CENSUS_PATH: &str = "generated/census.json";
/// Where the network contracts are written.
pub const NETWORK_PATH: &str = "generated/network.txt";
/// Where the settings table is written.
pub const SETTINGS_PATH: &str = "generated/settings.txt";

/// Every generated document, as a package-relative path and its bytes.
///
/// # Errors
///
/// Returns a refusal when a packaged declaration or content path is invalid.
pub fn documents() -> Result<Vec<(&'static str, String)>, ModError> {
    Ok(vec![
        (CENSUS_PATH, census()?),
        (NETWORK_PATH, network()?),
        (SETTINGS_PATH, settings()?),
    ])
}

/// Renders what this package registers, from [`crate::census`].
///
/// The claimed entries are read back out of the declaration rather than from
/// the constant behind it, so the document says what the registration barrier
/// would actually receive.
///
/// # Errors
///
/// Returns a refusal when the declared recipe path is not a legal content path.
pub fn census() -> Result<String, ModError> {
    let claimed: Vec<JsonValue> = crate::census::claimed()?
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
    Ok(format!("{document}\n"))
}

/// Renders the packet contracts of [`crate::network`] as a tab-separated table.
///
/// The rows are in the order [`crate::network::PAYLOADS`] declares them, which
/// is the order the contracts are built in.
///
/// # Errors
///
/// Returns a refusal when a packaged payload name is not a legal content path.
pub fn network() -> Result<String, ModError> {
    let mut rendered = String::from("contract\tdirection\tversion\n");
    for contract in
        crate::network::contracts().map_err(|error| ModError::refused(error.to_string()))?
    {
        writeln!(
            rendered,
            "{}\t{}\t{}",
            contract.id(),
            contract.direction().as_str(),
            contract.version()
        )
        .expect("writing to a string never fails");
    }
    Ok(rendered)
}

/// Renders the declared settings of [`crate::settings`] as a tab-separated table.
///
/// The rows are in declaration order — the three server settings, then the
/// seventy colour roles — and each default is the one the declaration carries,
/// written in its JSON form so that a colour's six-digit string is told apart
/// from a boolean and from the ceiling's number. A colour appears in the stored
/// form, with its alpha byte already dropped.
///
/// # Errors
///
/// Returns a refusal when the declaration this package builds from its own tables
/// is one the configuration schema refuses.
pub fn settings() -> Result<String, ModError> {
    let declaration =
        crate::settings::declaration().map_err(|error| ModError::refused(error.to_string()))?;
    let mut rendered = String::from("key\tside\tdefault\n");
    for setting in declaration.settings() {
        writeln!(
            rendered,
            "{}\t{}\t{}",
            setting.key.as_str(),
            setting.side.as_str(),
            setting.default
        )
        .expect("writing to a string never fails");
    }
    Ok(rendered)
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
        let rendered = documents().expect("the packaged declarations are valid");
        assert!(
            !rendered.is_empty(),
            "the package ships generated documents"
        );

        for (path, document) in rendered {
            assert!(
                document.ends_with('\n'),
                "{path} ends with a trailing newline"
            );
            let target = root.join(path);
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
