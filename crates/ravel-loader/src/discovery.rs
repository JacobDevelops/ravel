use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_package::{IntegrityError, ManifestError, PackageManifest, verify_package};

use ravel_core::paths::{INSTANCE_DIRECTORY, PACKAGES_DIRECTORY};

/// The manifest filename within each installed package.
pub const MANIFEST_FILE: &str = "package.json";

/// Validates installed packages without running mod code.
///
/// An absent packages directory is an empty set; any unusable entry refuses the instance.
///
/// # Errors
///
/// Returns a [`DiscoveryRefused`] for unreadable, invalid, escaping, or duplicate packages.
pub fn discover(game_directory: &Path) -> Result<Discovered, DiscoveryRefused> {
    let packages = game_directory
        .join(INSTANCE_DIRECTORY)
        .join(PACKAGES_DIRECTORY);
    if !packages.exists() {
        return Ok(Discovered::default());
    }

    let label = format!("{INSTANCE_DIRECTORY}/{PACKAGES_DIRECTORY}");
    let root = canonical(&packages, &label)?;
    let mut found = Vec::new();
    for (name, path) in entries(&root, &label)? {
        let directory = name.to_string_lossy().into_owned();
        found.push(package(&root, &path, directory)?);
    }

    found.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
    if let Some(window) = found
        .windows(2)
        .find(|pair| pair[0].manifest.id == pair[1].manifest.id)
    {
        return Err(DiscoveryRefused::Duplicate {
            mod_id: window[1].manifest.id.clone(),
            directory: window[1].directory.clone(),
        });
    }

    Ok(Discovered { packages: found })
}

// Sort names so the first refusal is independent of filesystem enumeration order.
fn entries(root: &Path, label: &str) -> Result<Vec<(OsString, PathBuf)>, DiscoveryRefused> {
    let reader = fs::read_dir(root).map_err(|error| refuse_io(label, error))?;
    let mut entries = Vec::new();
    for entry in reader {
        let entry = entry.map_err(|error| refuse_io(label, error))?;
        entries.push((entry.file_name(), entry.path()));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

fn package(
    root: &Path,
    path: &Path,
    directory: String,
) -> Result<DiscoveredPackage, DiscoveryRefused> {
    if !path.is_dir() {
        return Err(DiscoveryRefused::NotAPackage { directory });
    }
    let resolved = canonical(path, &directory)?;
    if resolved == root || !resolved.starts_with(root) {
        return Err(DiscoveryRefused::OutsideInstance { directory });
    }

    let document = match fs::read_to_string(resolved.join(MANIFEST_FILE)) {
        Ok(document) => document,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(DiscoveryRefused::MissingManifest { directory });
        }
        Err(error) => return Err(refuse_io(&directory, error)),
    };
    let manifest =
        PackageManifest::parse(&document).map_err(|source| DiscoveryRefused::Manifest {
            directory: directory.clone(),
            source,
        })?;
    verify_package(&resolved, &manifest).map_err(|source| DiscoveryRefused::Integrity {
        directory: directory.clone(),
        source,
    })?;

    Ok(DiscoveredPackage {
        root: resolved,
        manifest,
        directory,
    })
}

fn canonical(path: &Path, directory: &str) -> Result<PathBuf, DiscoveryRefused> {
    fs::canonicalize(path).map_err(|error| refuse_io(directory, error))
}

fn refuse_io(directory: &str, error: std::io::Error) -> DiscoveryRefused {
    DiscoveryRefused::Unreadable {
        directory: directory.to_owned(),
        source: error,
    }
}

/// Every package an instance holds, validated, in mod-id order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Discovered {
    packages: Vec<DiscoveredPackage>,
}

impl Discovered {
    /// Returns every discovered package, in mod-id order.
    #[must_use]
    pub fn packages(&self) -> &[DiscoveredPackage] {
        &self.packages
    }

    /// Returns the manifests for resolution and admission.
    #[must_use]
    pub fn manifests(&self) -> Vec<PackageManifest> {
        self.packages
            .iter()
            .map(|package| package.manifest.clone())
            .collect()
    }
}

/// One installed package: where it was found and what it declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredPackage {
    root: PathBuf,
    manifest: PackageManifest,
    directory: String,
}

impl DiscoveredPackage {
    /// Returns the package directory, canonicalized.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns what the package declares about itself.
    #[must_use]
    pub const fn manifest(&self) -> &PackageManifest {
        &self.manifest
    }

    /// Returns the installed directory name, without its identifying parent path.
    #[must_use]
    pub fn directory(&self) -> &str {
        &self.directory
    }
}

/// An installed-package refusal, naming directories rather than identifying paths.
#[derive(Debug)]
pub enum DiscoveryRefused {
    /// A directory or a manifest exists and could not be read.
    Unreadable {
        /// The package directory's name, or the packages directory itself.
        directory: String,
        /// What the operating system reported.
        source: std::io::Error,
    },
    /// An entry of the packages directory is not a directory.
    NotAPackage {
        /// The entry's name.
        directory: String,
    },
    /// A package directory resolves outside the packages directory.
    OutsideInstance {
        /// The entry's name.
        directory: String,
    },
    /// A package directory carries no manifest.
    MissingManifest {
        /// The package directory's name.
        directory: String,
    },
    /// A manifest is not a document the package schema accepts.
    Manifest {
        /// The package directory's name.
        directory: String,
        /// What `ravel-package` refused.
        source: ManifestError,
    },
    /// A declared file is not the file the manifest describes.
    Integrity {
        /// The package directory's name.
        directory: String,
        /// What `ravel-package` refused.
        source: IntegrityError,
    },
    /// Two package directories declare one identity.
    Duplicate {
        /// The identity both declare.
        mod_id: ModId,
        /// The second directory declaring it, in mod-id order.
        directory: String,
    },
}

impl DiscoveryRefused {
    /// Returns the package directory this refusal is about.
    #[must_use]
    pub fn directory(&self) -> &str {
        match self {
            Self::Unreadable { directory, .. }
            | Self::NotAPackage { directory }
            | Self::OutsideInstance { directory }
            | Self::MissingManifest { directory }
            | Self::Manifest { directory, .. }
            | Self::Integrity { directory, .. }
            | Self::Duplicate { directory, .. } => directory,
        }
    }
}

impl fmt::Display for DiscoveryRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { directory, source } => {
                write!(formatter, "`{directory}` could not be read: {source}")
            }
            Self::NotAPackage { directory } => write!(
                formatter,
                "`{directory}` is installed beside the packages and is not one"
            ),
            Self::OutsideInstance { directory } => write!(
                formatter,
                "`{directory}` resolves outside the installed packages"
            ),
            Self::MissingManifest { directory } => write!(
                formatter,
                "`{directory}` carries no `{MANIFEST_FILE}`, so nothing says what it is"
            ),
            Self::Manifest { directory, source } => {
                write!(
                    formatter,
                    "`{directory}` declares no legal package: {source}"
                )
            }
            Self::Integrity { directory, source } => {
                write!(formatter, "`{directory}` is not intact: {source}")
            }
            Self::Duplicate { mod_id, directory } => write!(
                formatter,
                "`{directory}` declares `{mod_id}`, which another installed package already \
                 declares"
            ),
        }
    }
}

impl std::error::Error for DiscoveryRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            Self::NotAPackage { .. }
            | Self::OutsideInstance { .. }
            | Self::MissingManifest { .. }
            | Self::Duplicate { .. } => None,
            Self::Manifest { source, .. } => Some(source),
            Self::Integrity { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for DiscoveryRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Discovered
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Unreadable { .. } => true,
            Self::Manifest { source, .. } => source.quotes_untrusted_text(),
            Self::Integrity { source, .. } => source.quotes_untrusted_text(),
            Self::NotAPackage { .. }
            | Self::OutsideInstance { .. }
            | Self::MissingManifest { .. }
            | Self::Duplicate { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    use ravel_core::{LifecycleState, Side, StructuredFailure};
    use ravel_package::{IntegrityError, PackageManifest};
    use ravel_resolve::resolve;

    use super::{Discovered, DiscoveryRefused, MANIFEST_FILE, PACKAGES_DIRECTORY, discover};
    use crate::admission::admitted;

    const SOURCE_BYTES: &[u8] = b"pub fn load() {}\n";
    const RECORD_BYTES: &[u8] = br#"{"origin":"example"}"#;

    // Fixture digests computed independently with `sha256sum`.
    const SOURCE_DIGEST: &str = "d43fb0c057531a1578825aa660a5cc1b415a10ae8eb1b42994ca7ceda27d6706";
    const RECORD_DIGEST: &str = "5cf5e6735c38fa43af69f9fe2210ca13d9fd75cc98cd9650ee969c0498a28ab0";
    const OTHER_DIGEST: &str = "a126bbfe4662a8d151f8e62afbb278a4a273e01fe0a88fc1545b25d017f19fb3";

    const GAME: &str = "test-version";

    static NEXT: AtomicU32 = AtomicU32::new(0);

    #[test]
    fn a_packages_directory_error_names_the_instance_relative_root() {
        let root = instance("packages-file");
        let path = packages(&root);
        fs::remove_dir(&path).expect("the empty packages directory is removable");
        fs::write(&path, "not a directory").expect("a file replaces the directory");
        let error = discover(&root).expect_err("a packages file cannot be listed");
        let label = format!("{}/{}", super::INSTANCE_DIRECTORY, PACKAGES_DIRECTORY);
        assert!(error.to_string().contains(&label), "{error}");
    }

    fn instance(label: &str) -> PathBuf {
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ravel-discovery-{label}-{}-{unique}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("a stale instance should be removable");
        }
        fs::create_dir_all(
            root.join(super::INSTANCE_DIRECTORY)
                .join(PACKAGES_DIRECTORY),
        )
        .expect("an instance is creatable");
        root
    }

    fn packages(instance: &Path) -> PathBuf {
        instance
            .join(super::INSTANCE_DIRECTORY)
            .join(PACKAGES_DIRECTORY)
    }

    fn manifest_document(id: &str, sides: &[&str], source_digest: &str) -> String {
        let sides = sides
            .iter()
            .map(|side| format!("\"{side}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{id}",
              "version": "1.0.0",
              "compatibility": {{ "ravel": ">=0.0.0", "minecraft": ["{GAME}"] }},
              "sides": [{sides}],
              "authors": [{{ "name": "Ravel" }}],
              "license": "MIT",
              "source": {{
                "repository": "https://example.invalid/{id}",
                "files": [{{ "path": "src/lib.rs", "sha256": "{source_digest}" }}]
              }},
              "provenance": {{
                "record": "provenance.json",
                "sha256": "{RECORD_DIGEST}"
              }}
            }}"#
        )
    }

    fn install(instance: &Path, directory: &str, id: &str, sides: &[&str]) -> PathBuf {
        install_document(
            instance,
            directory,
            &manifest_document(id, sides, SOURCE_DIGEST),
        )
    }

    fn install_document(instance: &Path, directory: &str, document: &str) -> PathBuf {
        let root = packages(instance).join(directory);
        fs::create_dir_all(root.join("src")).expect("a package directory is creatable");
        fs::write(root.join(MANIFEST_FILE), document).expect("manifest written");
        fs::write(root.join("src/lib.rs"), SOURCE_BYTES).expect("source written");
        fs::write(root.join("provenance.json"), RECORD_BYTES).expect("record written");
        root
    }

    fn ids(discovered: &Discovered) -> Vec<String> {
        discovered
            .packages()
            .iter()
            .map(|package| package.manifest().id.as_str().to_owned())
            .collect()
    }

    #[test]
    fn a_package_installed_in_an_instance_reaches_the_lifecycle_entry_shape() {
        let instance = instance("admitted");
        install(&instance, "example", "example_mod", &["client"]);

        let discovered = discover(&instance).expect("an intact package");
        assert_eq!(ids(&discovered), ["example_mod"]);
        assert_eq!(discovered.packages()[0].directory(), "example");
        assert!(
            discovered.packages()[0]
                .root()
                .join(MANIFEST_FILE)
                .is_file()
        );

        let manifests = discovered.manifests();
        let plan = resolve(&manifests, Side::Client).expect("one package orders");
        let entries = admitted(
            &plan,
            &manifests,
            &"1.0.0".parse().expect("a legal version"),
            GAME,
        )
        .expect("a compatible set");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mod_id.as_str(), "example_mod");
        assert!(entries[0].sides.loads_on(Side::Client));
        assert!(
            !entries[0].sides.loads_on(Side::DedicatedServer),
            "the declaration was widened on the way in"
        );
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn discovery_answers_in_mod_id_order_whatever_the_directories_are_called() {
        let instance = instance("order");
        install(&instance, "zzz", "a_mod", &["client"]);
        install(&instance, "aaa", "b_mod", &["client"]);

        let discovered = discover(&instance).expect("two intact packages");
        assert_eq!(ids(&discovered), ["a_mod", "b_mod"]);
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn an_instance_that_has_installed_nothing_discovers_nothing() {
        let root =
            std::env::temp_dir().join(format!("ravel-discovery-bare-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("a stale instance is removable");
        }
        fs::create_dir_all(&root).expect("an instance is creatable");

        let discovered = discover(&root).expect("no packages is not a failure");
        assert_eq!(discovered.packages(), []);
        assert_eq!(discovered.manifests(), Vec::<PackageManifest>::new());
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_manifest_the_schema_refuses_refuses_the_instance() {
        let instance = instance("schema");
        install_document(
            &instance,
            "example",
            &manifest_document("example_mod", &["client"], SOURCE_DIGEST)
                .replace("\"license\"", "\"licence\""),
        );

        let refused = discover(&instance).expect_err("the schema refuses the document");
        assert!(
            matches!(refused, DiscoveryRefused::Manifest { .. }),
            "{refused:?}"
        );
        assert_eq!(refused.directory(), "example");
        assert!(std::error::Error::source(&refused).is_some());
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn a_package_whose_source_is_not_what_it_declares_refuses_the_instance() {
        let instance = instance("integrity");
        install_document(
            &instance,
            "example",
            &manifest_document("example_mod", &["client"], OTHER_DIGEST),
        );

        let refused = discover(&instance).expect_err("the source is not the declared source");
        let DiscoveryRefused::Integrity { directory, source } = &refused else {
            panic!("the refusal should be about integrity: {refused:?}");
        };
        assert_eq!(directory, "example");
        assert!(
            matches!(source, IntegrityError::Mismatch { .. }),
            "{source}"
        );
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn an_unreadable_source_retains_the_integrity_and_io_source_chain() {
        let instance = instance("integrity-io-source");
        let root = install(&instance, "example", "example_mod", &["client"]);
        fs::remove_file(root.join("src/lib.rs")).expect("source removed");
        fs::create_dir(root.join("src/lib.rs")).expect("a directory replaces the source");

        let refused = discover(&instance).expect_err("a directory is not source");
        assert_eq!(refused.directory(), "example");
        let integrity = std::error::Error::source(&refused)
            .expect("discovery retains the integrity refusal")
            .downcast_ref::<IntegrityError>()
            .expect("the first source is an integrity error");
        assert_eq!(integrity.file(), "lib.rs");
        let source = std::error::Error::source(integrity)
            .expect("integrity retains the filesystem failure")
            .downcast_ref::<std::io::Error>()
            .expect("the second source is the original IO error");
        assert_eq!(source.kind(), std::io::ErrorKind::IsADirectory);
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn a_package_missing_a_declared_source_file_refuses_the_instance() {
        let instance = instance("absent-source");
        let root = install(&instance, "example", "example_mod", &["client"]);
        fs::remove_file(root.join("src/lib.rs")).expect("source removed");

        let refused = discover(&instance).expect_err("a declared file is absent");
        assert!(
            matches!(
                refused,
                DiscoveryRefused::Integrity {
                    source: IntegrityError::Missing { .. },
                    ..
                }
            ),
            "{refused:?}"
        );
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn a_directory_carrying_no_manifest_is_refused_rather_than_skipped() {
        let instance = instance("no-manifest");
        install(&instance, "example", "example_mod", &["client"]);
        fs::create_dir_all(packages(&instance).join("half-copied"))
            .expect("a second directory is creatable");

        let refused = discover(&instance).expect_err("nothing says what that directory is");
        assert!(
            matches!(&refused, DiscoveryRefused::MissingManifest { directory } if directory == "half-copied"),
            "{refused:?}"
        );
        assert!(refused.to_string().contains(MANIFEST_FILE));
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn a_file_installed_beside_the_packages_is_refused_rather_than_skipped() {
        let instance = instance("stray-file");
        install(&instance, "example", "example_mod", &["client"]);
        fs::write(packages(&instance).join("pack.jar"), b"an archive")
            .expect("a stray file is writable");

        let refused = discover(&instance).expect_err("a file is not a package");
        assert!(
            matches!(&refused, DiscoveryRefused::NotAPackage { directory } if directory == "pack.jar"),
            "{refused:?}"
        );
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn two_directories_declaring_one_identity_refuse_the_instance() {
        let instance = instance("duplicate");
        install(&instance, "example", "example_mod", &["client"]);
        install(&instance, "example-copy", "example_mod", &["client"]);

        let refused = discover(&instance).expect_err("one identity is installed twice");
        let DiscoveryRefused::Duplicate { mod_id, directory } = &refused else {
            panic!("the refusal should name the duplicate: {refused:?}");
        };
        assert_eq!(mod_id.as_str(), "example_mod");
        assert!(
            ["example", "example-copy"].contains(&directory.as_str()),
            "{directory}"
        );
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn a_package_directory_symlinked_out_of_the_instance_is_refused() {
        let outside = instance("escape-target");
        let instance = instance("escape");
        install(&outside, "example", "example_mod", &["client"]);
        symlink(
            packages(&outside).join("example"),
            packages(&instance).join("borrowed"),
        )
        .expect("a directory symlink is creatable");

        let refused = discover(&instance).expect_err("the package is not in this instance");
        assert!(
            matches!(&refused, DiscoveryRefused::OutsideInstance { directory } if directory == "borrowed"),
            "{refused:?}"
        );
        fs::remove_dir_all(&instance).expect("cleanup");
        fs::remove_dir_all(&outside).expect("cleanup");
    }

    #[test]
    fn a_manifest_that_cannot_be_read_is_distinct_from_an_absent_one() {
        let instance = instance("unreadable");
        let root = install(&instance, "example", "example_mod", &["client"]);
        fs::remove_file(root.join(MANIFEST_FILE)).expect("manifest removed");
        fs::create_dir(root.join(MANIFEST_FILE)).expect("a directory in the manifest's place");

        let refused = discover(&instance).expect_err("a directory is not a manifest");
        assert!(
            matches!(refused, DiscoveryRefused::Unreadable { .. }),
            "{refused:?}"
        );
        assert_eq!(refused.directory(), "example");
        assert!(refused.quotes_untrusted_text());
        let source = std::error::Error::source(&refused)
            .expect("the filesystem error is retained")
            .downcast_ref::<std::io::Error>()
            .expect("the source is the original IO error");
        assert_eq!(source.kind(), std::io::ErrorKind::IsADirectory);
        fs::remove_dir_all(&instance).expect("cleanup");
    }

    #[test]
    fn a_refusal_never_names_where_the_player_keeps_their_game() {
        let instance = instance("redaction");
        install(&instance, "example", "example_mod", &["client"]);
        fs::write(packages(&instance).join("pack.jar"), b"an archive").expect("a stray file");

        let refused = discover(&instance).expect_err("a file is not a package");
        let rendered = refused.to_string();
        assert!(!rendered.contains('/'), "{rendered}");
        assert!(
            !rendered.contains(&instance.display().to_string()),
            "{rendered}"
        );
        assert_eq!(refused.failure_kind(), None);
        assert_eq!(refused.lifecycle_state(), LifecycleState::Discovered);
        assert!(!refused.quotes_untrusted_text());
        fs::remove_dir_all(&instance).expect("cleanup");
    }
}
