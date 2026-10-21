//! Refreshes manifest digests using the package verifier's hashing and path rules.
//!
//! Replaces only digest byte spans, preserving formatting and key order. A
//! string-aware JSON scanner distinguishes keys from text inside descriptions.

use std::fmt;
use std::fs;
use std::path::Path;

use ravel_core::Sha256Digest;
use ravel_package::{IntegrityError, PackageManifest, verify_provenance, verify_source_file};

/// The file inside a package directory that declares it.
const MANIFEST_FILE: &str = "package.json";

/// The manifest keys that name a file whose digest sits beside them.
const OWNER_KEYS: [&str; 2] = ["path", "record"];

/// The manifest key holding a digest.
const DIGEST_KEY: &str = "sha256";

/// One declared file whose digest was not the digest of its bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleDigest {
    /// The package-relative path the manifest declares.
    pub path: String,
    /// The digest the manifest recorded.
    pub declared: Sha256Digest,
    /// The digest the file actually has.
    pub actual: Sha256Digest,
}

impl fmt::Display for StaleDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` hashes {} but the manifest records {}",
            self.path, self.actual, self.declared
        )
    }
}

/// Reports stale digests without modifying the package.
pub fn stale_digests(directory: &Path) -> Result<Vec<StaleDigest>, Vec<String>> {
    digest_update(directory).map(|update| update.stale)
}

/// Refreshes stale digests, returning the entries that were rewritten.
///
/// Nothing is written unless every declared file and replacement span resolves.
pub fn refresh_digests(directory: &Path) -> Result<Vec<StaleDigest>, Vec<String>> {
    let update = digest_update(directory)?;
    if update.stale.is_empty() {
        return Ok(update.stale);
    }
    let rewritten = rewrite(&update.document, update.replacements);
    write_manifest(directory, &rewritten)?;
    Ok(update.stale)
}

struct DigestUpdate {
    document: String,
    stale: Vec<StaleDigest>,
    replacements: Vec<(Span, String)>,
}

fn digest_update(directory: &Path) -> Result<DigestUpdate, Vec<String>> {
    let manifest_path = directory.join(MANIFEST_FILE);
    let document = fs::read_to_string(&manifest_path)
        .map_err(|error| vec![format!("`{MANIFEST_FILE}` could not be read: {error}")])?;
    let manifest = PackageManifest::parse(&document).map_err(|error| {
        vec![format!(
            "`{MANIFEST_FILE}` is not a package manifest: {error}"
        )]
    })?;

    let spans = digest_spans(&document)?;
    let mut stale = Vec::new();
    let mut replacements = Vec::new();
    let mut refusals = Vec::new();

    for file in &manifest.source.files {
        let declared = file.sha256;
        let path = file.path.as_str().to_owned();
        match verify_source_file(directory, file) {
            Ok(()) => {}
            Err(IntegrityError::Mismatch { actual, .. }) => {
                record(
                    &mut stale,
                    &mut replacements,
                    &spans,
                    path,
                    declared,
                    actual,
                );
            }
            Err(other) => refusals.push(other.to_string()),
        }
    }

    let provenance = &manifest.provenance;
    let path = provenance.record.as_str().to_owned();
    match verify_provenance(directory, provenance) {
        Ok(()) => {}
        Err(IntegrityError::Mismatch { actual, .. }) => record(
            &mut stale,
            &mut replacements,
            &spans,
            path,
            provenance.sha256,
            actual,
        ),
        Err(other) => refusals.push(other.to_string()),
    }

    if !refusals.is_empty() {
        return Err(refusals);
    }
    Ok(DigestUpdate {
        document,
        stale,
        replacements,
    })
}

fn write_manifest(directory: &Path, rewritten: &str) -> Result<(), Vec<String>> {
    // A manifest this tool produced that the loader would refuse is worse than
    // the stale one it replaced, so the rewrite is parsed and verified before it
    // reaches the author's file rather than after.
    let reparsed = PackageManifest::parse(rewritten).map_err(|error| {
        vec![format!(
            "refreshing the digests would have produced a manifest that no longer parses: {error}"
        )]
    })?;
    ravel_package::verify_package(directory, &reparsed).map_err(|error| {
        vec![format!(
            "refreshing the digests would have produced a manifest that still does not verify: \
             {error}"
        )]
    })?;
    fs::write(directory.join(MANIFEST_FILE), rewritten)
        .map_err(|error| vec![format!("`{MANIFEST_FILE}` could not be written: {error}")])?;
    Ok(())
}

fn record(
    stale: &mut Vec<StaleDigest>,
    replacements: &mut Vec<(Span, String)>,
    spans: &[(String, Span)],
    path: String,
    declared: Sha256Digest,
    actual: Sha256Digest,
) {
    if let Some((_, span)) = spans.iter().find(|(owner, _)| *owner == path) {
        replacements.push((*span, actual.to_string()));
    }
    stale.push(StaleDigest {
        path,
        declared,
        actual,
    });
}

/// Half-open byte range of a string value's contents, quotes excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Span {
    start: usize,
    end: usize,
}

fn rewrite(document: &str, mut replacements: Vec<(Span, String)>) -> String {
    // Descending, so an earlier span's offsets are still the offsets of the text
    // it has not yet been spliced into.
    replacements.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut rewritten = document.to_owned();
    for (span, digest) in replacements {
        rewritten.replace_range(span.start..span.end, &digest);
    }
    rewritten
}

/// Pairs each declared file with the span of the digest recorded beside it.
///
/// Every source-file entry states a `path` and a `sha256`, and the provenance
/// entry states a `record` and a `sha256`, so the *n*-th file named in the
/// document owns the *n*-th digest in it however the two objects order their own
/// keys and whichever of `source` and `provenance` the author wrote first.
fn digest_spans(document: &str) -> Result<Vec<(String, Span)>, Vec<String>> {
    let mut owners = Vec::new();
    let mut digests = Vec::new();
    for (key, span) in string_entries(document) {
        if OWNER_KEYS.contains(&key.as_str()) {
            owners.push(document[span.start..span.end].to_owned());
        } else if key == DIGEST_KEY {
            digests.push(span);
        }
    }

    if owners.len() != digests.len() {
        return Err(vec![format!(
            "the manifest names {} file{} and records {} digest{}, so this tool cannot tell which \
             digest belongs to which file",
            owners.len(),
            plural(owners.len()),
            digests.len(),
            plural(digests.len()),
        )]);
    }
    let mut sorted = owners.clone();
    sorted.sort();
    sorted.dedup();
    if sorted.len() != owners.len() {
        return Err(vec![
            "the manifest declares one path more than once, so this tool cannot tell which digest \
             belongs to which declaration"
                .to_owned(),
        ]);
    }
    Ok(owners.into_iter().zip(digests).collect())
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// Every `"key": "value"` pair in the document, as the key and the value's span.
///
/// A hand-written scan rather than a parse, because the spans are the point: a
/// parsed value has forgotten where in the file it came from, and coming back
/// with the author's own formatting intact is the whole reason this rewrites
/// text at all. Everything it has to get right is about string boundaries — an
/// escaped quote inside a string, and a brace or a colon that is only a
/// character in one.
fn string_entries(document: &str) -> Vec<(String, Span)> {
    let bytes = document.as_bytes();
    let mut strings: Vec<(Span, bool)> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut cursor = start;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\\' => cursor += 2,
                b'"' => break,
                _ => cursor += 1,
            }
        }
        if cursor >= bytes.len() {
            break;
        }
        let followed_by_colon = bytes[cursor + 1..]
            .iter()
            .find(|byte| !byte.is_ascii_whitespace())
            == Some(&b':');
        strings.push((Span { start, end: cursor }, followed_by_colon));
        index = cursor + 1;
    }

    let mut entries = Vec::new();
    for window in strings.windows(2) {
        let (key_span, is_key) = window[0];
        let (value_span, value_is_key) = window[1];
        if is_key && !value_is_key {
            entries.push((
                document[key_span.start..key_span.end].to_owned(),
                value_span,
            ));
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::{MANIFEST_FILE, refresh_digests, stale_digests, string_entries};
    use ravel_core::Sha256Digest;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// A package whose digests are all correct, written into a fresh directory.
    struct Package {
        directory: PathBuf,
    }

    impl Package {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir()
                .join(format!("ravel-xtask-digests-{name}-{}", std::process::id()));
            match fs::remove_dir_all(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("remove stale temporary package: {error}"),
            }
            fs::create_dir_all(directory.join("src")).expect("a writable temporary directory");
            let package = Self { directory };
            package.write("src/lib.rs", "pub fn load() {}\n");
            package.write("provenance.json", "{}\n");
            package.write(MANIFEST_FILE, &manifest(ZEROES, ZEROES));
            package
        }

        fn write(&self, path: &str, contents: &str) {
            fs::write(self.directory.join(path), contents).expect("a writable file");
        }

        fn manifest_text(&self) -> String {
            fs::read_to_string(self.directory.join(MANIFEST_FILE)).expect("a readable manifest")
        }

        fn path(&self) -> &Path {
            &self.directory
        }
    }

    impl Drop for Package {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_dir_all(&self.directory)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("cannot remove temporary package: {error}");
            }
        }
    }

    #[test]
    fn temporary_package_cleanup_tolerates_an_already_removed_directory() {
        let package = Package::new("already-removed");
        fs::remove_dir_all(package.path()).expect("temporary package should be removable");
        drop(package);
    }

    const ZEROES: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn manifest(source: &str, provenance: &str) -> String {
        format!(
            r#"{{
  "schemaVersion": 1,
  "id": "example",
  "version": "0.1.0",
  "description": "a path is a word that appears in prose too",
  "compatibility": {{ "ravel": ">=0.0.0, <1.0.0", "minecraft": ["26.1.2"] }},
  "sides": ["client"],
  "authors": [{{ "name": "Example" }}],
  "license": "MIT OR Apache-2.0",
  "provenance": {{ "record": "provenance.json", "sha256": "{provenance}" }},
  "source": {{
    "repository": "https://example.invalid/example",
    "files": [
      {{ "sha256": "{source}", "path": "src/lib.rs" }}
    ]
  }}
}}
"#
        )
    }

    #[test]
    fn a_key_named_in_prose_is_not_mistaken_for_a_key() {
        let entries = string_entries(&manifest(ZEROES, ZEROES));
        let paths = entries.iter().filter(|(key, _)| key == "path").count();
        assert_eq!(paths, 1, "`description` says `path` and declares nothing");
    }

    #[test]
    fn a_stale_manifest_is_rewritten_and_then_reports_current() {
        let package = Package::new("rewritten");
        let stale = refresh_digests(package.path()).expect("the package is readable");
        assert!(!stale.is_empty(), "a manifest of zeroes is stale");
        assert_eq!(
            stale
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            ["src/lib.rs", "provenance.json"],
            "reported in declaration order, source files then the provenance record"
        );
        assert!(stale.iter().all(|entry| {
            entry.declared == Sha256Digest::from_bytes([0; 32]) && entry.actual != entry.declared
        }));

        let rewritten = package.manifest_text();
        assert!(!rewritten.contains(ZEROES), "{rewritten}");
        assert!(
            rewritten.contains("a path is a word that appears in prose too"),
            "nothing but the digests is touched"
        );
        assert_eq!(
            refresh_digests(package.path()).expect("still readable"),
            Vec::new(),
            "a second run has nothing to do and says so"
        );
    }

    #[test]
    fn check_reports_the_same_staleness_and_writes_nothing() {
        let package = Package::new("checked");
        let before = package.manifest_text();
        let stale = stale_digests(package.path()).expect("the package is readable");
        assert_ne!(stale, Vec::new());
        assert_eq!(package.manifest_text(), before, "`--check` writes nothing");
        assert_eq!(
            refresh_digests(package.path()).expect("the stale package should refresh"),
            stale
        );
        assert_eq!(
            stale_digests(package.path()).expect("the package should read"),
            Vec::new()
        );
    }

    #[test]
    fn a_declared_file_the_package_does_not_contain_refuses_and_writes_nothing() {
        let package = Package::new("missing");
        let before = package.manifest_text();
        fs::remove_file(package.path().join("src/lib.rs")).expect("the file was written");
        let refusals = refresh_digests(package.path()).expect_err("a declared file is gone");
        assert_eq!(
            refusals,
            ["the package declares `lib.rs` but does not contain it"]
        );
        assert_eq!(package.manifest_text(), before);
    }

    #[test]
    fn a_manifest_that_does_not_parse_is_refused_rather_than_rewritten() {
        let package = Package::new("unparsable");
        package.write(MANIFEST_FILE, "{ \"schemaVersion\": 1 }\n");
        let refusals = refresh_digests(package.path()).expect_err("it is not a manifest");
        assert_eq!(refusals.len(), 1);
        assert!(refusals[0].starts_with("`package.json` is not a package manifest:"));
    }

    #[test]
    fn an_absent_manifest_is_refused() {
        let directory = std::env::temp_dir().join("ravel-xtask-digests-absent");
        let refusals = stale_digests(&directory).expect_err("there is no manifest");
        assert_eq!(refusals.len(), 1);
        assert!(refusals[0].starts_with("`package.json` could not be read:"));
    }

    #[test]
    fn the_digest_a_rewrite_writes_is_the_digest_the_loader_computes() {
        let package = Package::new("agrees");
        refresh_digests(package.path()).expect("the package is readable");
        let document = package.manifest_text();
        let manifest = ravel_package::PackageManifest::parse(&document).expect("it parses");
        ravel_package::verify_package(package.path(), &manifest)
            .expect("what this tool wrote is what the loader verifies");
    }
}
