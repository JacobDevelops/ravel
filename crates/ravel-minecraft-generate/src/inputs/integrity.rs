//! Verifies tracked input bytes and rewrites lock pins without losing TOML prose.
//! Diagnostic paths remain reduced to file names at this shared I/O seam.

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{GenerateError, Input, Inputs};

/// Refuses an input whose bytes no longer match the tracked hash.
pub(crate) fn verify_input(root: &Path, input: &Input) -> Result<(), GenerateError> {
    // Omit paths from recorded evidence to avoid exposing machine-local details.
    let name = file_name(&input.path);
    if input.sha256.is_none() && input.bytes.is_none() {
        return Err(GenerateError::Input(format!(
            "{name} pins neither a hash nor a length, so nothing about it is verified"
        )));
    }
    let bytes = fs::read(root.join(&input.path))
        .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
    if let Some(expected) = &input.sha256 {
        let actual = sha256(&bytes);
        if &actual != expected {
            return Err(GenerateError::Drift(format!(
                "{name} hashes {actual} but the adapter inputs pin {expected}"
            )));
        }
    }
    if let Some(expected) = input.bytes
        && u64::try_from(bytes.len()).expect("input length fits in u64") != expected
    {
        return Err(GenerateError::Drift(format!(
            "{name} is {} bytes but the adapter inputs pin {expected}",
            bytes.len()
        )));
    }
    Ok(())
}

/// Rewrites what the adapter inputs pin for the two generated locks.
pub(crate) fn repin_locks(
    root: &Path,
    directory: &Path,
    inputs: &Inputs,
) -> Result<(), GenerateError> {
    let path = directory.join("adapter.toml");
    let mut document = fs::read_to_string(&path)
        .map_err(|error| GenerateError::Input(format!("cannot read adapter.toml: {error}")))?;
    for (table, input) in [
        ("[inputs.mappingsLock]", &inputs.mappings_lock),
        ("[inputs.stubsLock]", &inputs.stubs_lock),
    ] {
        let name = file_name(&input.path);
        let bytes = fs::read(root.join(&input.path))
            .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
        let length = u64::try_from(bytes.len()).expect("lock length fits in u64");
        document = replace_pin(&document, table, &sha256(&bytes), length)?;
    }
    fs::write(&path, document)
        .map_err(|error| GenerateError::Input(format!("cannot write adapter.toml: {error}")))
}

/// Replaces one table's hash and length without discarding TOML comments.
pub(crate) fn replace_pin(
    document: &str,
    table: &str,
    sha256: &str,
    bytes: u64,
) -> Result<String, GenerateError> {
    let mut rewritten = String::with_capacity(document.len());
    let mut inside = false;
    let mut pinned_hash = false;
    let mut pinned_length = false;
    for line in document.lines() {
        if line.starts_with('[') {
            inside = line.trim() == table;
        }
        if inside && line.starts_with("sha256 = ") {
            rewritten.push_str("sha256 = \"");
            rewritten.push_str(sha256);
            rewritten.push('"');
            pinned_hash = true;
        } else if inside && line.starts_with("bytes = ") {
            rewritten.push_str("bytes = ");
            rewritten.push_str(&bytes.to_string());
            pinned_length = true;
        } else {
            rewritten.push_str(line);
        }
        rewritten.push('\n');
    }
    if !pinned_hash || !pinned_length {
        return Err(GenerateError::Input(format!(
            "the adapter inputs have no {table} hash and length to rewrite"
        )));
    }
    Ok(rewritten)
}

/// The last path segment, which is all a recorded message may say about a file.
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Hex-encoded SHA-256 of some bytes.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        text.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::super::{Adapter, Input};
    use super::super::{parse, tracked_versions};
    use super::{file_name, sha256, verify_input};
    use crate::test_fixtures::repository_root;
    #[test]
    fn an_input_that_no_longer_hashes_to_its_pin_is_refused() {
        let root = repository_root();
        let adapter: Adapter = parse(
            &root
                .join("minecraft/versions")
                .join(&tracked_versions(root)[0])
                .join("adapter.toml"),
        )
        .expect("the tracked adapter inputs should parse");

        let drifted = Input {
            path: adapter.inputs.hook_inventory.path,
            sha256: Some("0".repeat(64)),
            bytes: None,
            upstream: None,
        };

        let error = verify_input(root, &drifted).expect_err("a drifted hash must be refused");
        let message = error.to_string();

        assert!(message.contains("hooks.toml"), "{message}");
        assert!(!message.contains('/'), "a message must not name a path");
    }

    #[test]
    fn an_input_of_the_wrong_length_is_refused_even_when_no_hash_is_pinned() {
        let root = repository_root();
        let wrong_length = Input {
            path: "minecraft/README.md".to_owned(),
            sha256: None,
            bytes: Some(1),
            upstream: None,
        };
        let missing = Input {
            path: "minecraft/absent.toml".to_owned(),
            sha256: None,
            bytes: Some(1),
            upstream: None,
        };

        assert!(
            verify_input(root, &wrong_length)
                .expect_err("a wrong length must be refused")
                .to_string()
                .contains("bytes but the adapter inputs pin")
        );
        assert!(
            verify_input(root, &missing)
                .expect_err("a missing input must be refused")
                .to_string()
                .contains("cannot read absent.toml")
        );
    }

    #[test]
    fn an_input_with_nothing_to_verify_against_is_refused_rather_than_passed() {
        let unpinned = Input {
            path: "minecraft/README.md".to_owned(),
            sha256: None,
            bytes: None,
            upstream: None,
        };

        let error = verify_input(repository_root(), &unpinned)
            .expect_err("an unverifiable input must be refused");

        assert!(
            error
                .to_string()
                .contains("pins neither a hash nor a length"),
            "{error}"
        );
    }

    #[test]
    fn repinning_rewrites_one_tables_pin_and_leaves_the_prose_alone() {
        let document = "\
# why the seeds are pinned
[inputs.stubSeeds]
path = \"stubs.toml\"
sha256 = \"aa\"
bytes = 1

# why the lock is pinned
[inputs.mappingsLock]
path = \"mappings.lock\"
sha256 = \"bb\"
bytes = 2
";

        let rewritten = super::replace_pin(document, "[inputs.mappingsLock]", "cc", 3)
            .expect("the table is there to rewrite");

        assert!(
            rewritten.contains("sha256 = \"cc\"\nbytes = 3\n"),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("sha256 = \"aa\"\nbytes = 1\n"),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("# why the lock is pinned"),
            "{rewritten}"
        );
    }

    #[test]
    fn repinning_a_table_that_pins_nothing_is_refused_rather_than_appended() {
        let error = super::replace_pin(
            "[inputs.stubsLock]\npath = \"a\"\n",
            "[inputs.stubsLock]",
            "cc",
            3,
        )
        .expect_err("there is no pin to rewrite");

        assert!(
            error.to_string().contains("hash and length to rewrite"),
            "{error}"
        );
    }

    #[test]
    fn a_message_names_a_file_and_never_the_directory_it_sits_in() {
        assert_eq!(file_name("a/b/client.jar"), "client.jar");
        assert_eq!(file_name("client.jar"), "client.jar");
    }

    #[test]
    fn hashing_matches_the_digest_the_adapter_inputs_pin() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"ravel"),
            sha256(b"ravel"),
            "hashing is a function of the bytes alone"
        );
        assert_ne!(sha256(b"ravel"), sha256(b"Ravel"));
    }
}
