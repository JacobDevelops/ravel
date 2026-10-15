use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_package::{IntegrityError, ManifestError, PackageManifest, verify_package};

/// Where an instance keeps the Ravel packages installed into it.
///
/// Relative to the game directory, and deliberately not `mods`: that directory
/// belongs to the Java loaders' archives, which `tools/ravel-xtask`'s instance
/// scan counts and Ravel never reads. It sits beside the `ravel-pack` directory
/// the selected pack's documents already use.
pub const PACKAGES_DIRECTORY: &str = "ravel/mods";

/// The document each installed package carries, named as `SPEC.md` §3.4's
/// manifest and as every `test-mods/*/package.json` already writes it.
pub const MANIFEST_FILE: &str = "package.json";

/// Finds every package installed into an instance and validates each one.
///
/// This is `SPEC.md` §3.5's "mod discovery from an installed instance" and
/// §14's **Discovered** and **Validated** states: each package directory is
/// located, its manifest parsed against `schemas/package.schema.json`, and its
/// declared source and provenance digests verified, before anything asks
/// whether the set has a load order. No mod code runs here and none can — this
/// crate holds no mod code — so a package that fails any step fails before the
/// state that would let it run.
///
/// The answer is [`Discovered`], whose [`Discovered::manifests`] is exactly what
/// [`ravel_resolve::resolve`] and [`crate::admitted`] take. A discovered package
/// therefore reaches the same [`crate::LifecycleEntry`] a compiled-in one does,
/// by the same two functions, rather than by a second route that could answer
/// differently.
///
/// # What an absent directory means
///
/// An instance with no `ravel/mods` directory has no packages installed, which
/// is an empty set and not a failure: a player who installed nothing gets a
/// game with nothing added. A directory that exists and cannot be read is a
/// refusal, because that is an instance whose packages could not be established
/// either way.
///
/// # Failure closure
///
/// One unusable entry refuses the whole instance. Skipping it would start a
/// game the player believes holds a mod it does not, and `SPEC.md` §9.2 reports
/// what was refused before world access rather than after. Every refusal names
/// the package directory it is about.
///
/// # Errors
///
/// Returns [`DiscoveryRefused::Unreadable`] when the packages directory or an
/// entry inside it cannot be read, [`DiscoveryRefused::NotAPackage`] when an
/// entry is not a directory, [`DiscoveryRefused::OutsideInstance`] when an entry
/// resolves outside the packages directory,
/// [`DiscoveryRefused::MissingManifest`] when a package directory carries no
/// `package.json`, [`DiscoveryRefused::Manifest`] when that document is not one
/// the package schema accepts, [`DiscoveryRefused::Integrity`] when a declared
/// file is not the file the manifest describes, and
/// [`DiscoveryRefused::Duplicate`] when two directories declare one identity.
pub fn discover(game_directory: &Path) -> Result<Discovered, DiscoveryRefused> {
    let packages = game_directory.join(PACKAGES_DIRECTORY);
    if !packages.exists() {
        return Ok(Discovered::default());
    }

    let root = canonical(&packages, PACKAGES_DIRECTORY)?;
    let mut found = Vec::new();
    for (name, path) in entries(&root)? {
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

/// Reads the packages directory into name-ordered entries.
///
/// The order `std::fs::read_dir` yields is the filesystem's, so it is sorted
/// here. Nothing downstream depends on it — the result is ordered by mod id —
/// but the *first* refusal a broken instance reports would otherwise depend on
/// which directory the filesystem happened to hand over first.
fn entries(root: &Path) -> Result<Vec<(OsString, PathBuf)>, DiscoveryRefused> {
    let reader = fs::read_dir(root).map_err(|error| refuse_io(PACKAGES_DIRECTORY, &error))?;
    let mut entries = Vec::new();
    for entry in reader {
        let entry = entry.map_err(|error| refuse_io(PACKAGES_DIRECTORY, &error))?;
        entries.push((entry.file_name(), entry.path()));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

/// Validates one entry of the packages directory into a discovered package.
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
        Err(error) => return Err(refuse_io(&directory, &error)),
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
    fs::canonicalize(path).map_err(|error| refuse_io(directory, &error))
}

fn refuse_io(directory: &str, error: &std::io::Error) -> DiscoveryRefused {
    DiscoveryRefused::Unreadable {
        directory: directory.to_owned(),
        detail: error.to_string(),
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

    /// Returns the manifests, in the form [`ravel_resolve::resolve`] and
    /// [`crate::admitted`] take them.
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
    ///
    /// The declared resources, configuration schemas, and source files a
    /// manifest names are package-relative, so this is what resolves them.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns what the package declares about itself.
    #[must_use]
    pub const fn manifest(&self) -> &PackageManifest {
        &self.manifest
    }

    /// Returns the name of the directory it was installed under.
    ///
    /// The name alone, never the path: a refusal about this package is text a
    /// player reads, and `SPEC.md` §9.1 requires reports to redact
    /// unnecessarily identifying filesystem paths.
    #[must_use]
    pub fn directory(&self) -> &str {
        &self.directory
    }
}

/// Why an instance's installed packages were refused.
///
/// Every variant names a directory rather than a path, for the reason
/// [`ravel_package::IntegrityError`] names a file rather than one: the
/// directory an instance lives in says where a player keeps their game.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryRefused {
    /// A directory or a manifest exists and could not be read.
    Unreadable {
        /// The package directory's name, or the packages directory itself.
        directory: String,
        /// What the operating system reported.
        detail: String,
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
            Self::Unreadable { directory, detail } => {
                write!(formatter, "`{directory}` could not be read: {detail}")
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
            Self::Unreadable { .. }
            | Self::NotAPackage { .. }
            | Self::OutsideInstance { .. }
            | Self::MissingManifest { .. }
            | Self::Duplicate { .. } => None,
            Self::Manifest { source, .. } => Some(source),
            Self::Integrity { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for DiscoveryRefused {
    /// No variant is one of `SPEC.md` §9.2's dependency-graph outcomes. Nothing
    /// here has reached a graph: the set the graph would be built from is what
    /// was refused.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// Discovery is the state before validation, and every refusal here is a
    /// package that never left it.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Discovered
    }

    /// [`Self::Unreadable`] quotes the operating system and the manifest and
    /// integrity refusals quote whatever `ravel-package` says about a document
    /// an author wrote. The rest are sentences written here.
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
    use ravel_package::IntegrityError;
    use ravel_resolve::resolve;

    use super::{Discovered, DiscoveryRefused, MANIFEST_FILE, PACKAGES_DIRECTORY, discover};
    use crate::admitted;

    const SOURCE_BYTES: &[u8] = b"pub fn load() {}\n";
    const RECORD_BYTES: &[u8] = br#"{"origin":"example"}"#;

    /// The digests of the two fixtures above, recorded rather than computed.
    ///
    /// A test that hashes its own fixture with the same library the code under
    /// test hashes it with agrees with that library by construction. These came
    /// from `sha256sum` and are the outside answer.
    const SOURCE_DIGEST: &str = "d43fb0c057531a1578825aa660a5cc1b415a10ae8eb1b42994ca7ceda27d6706";
    const RECORD_DIGEST: &str = "5cf5e6735c38fa43af69f9fe2210ca13d9fd75cc98cd9650ee969c0498a28ab0";
    /// `pub fn other() {}\n`, which no fixture below declares.
    const OTHER_DIGEST: &str = "a126bbfe4662a8d151f8e62afbb278a4a273e01fe0a88fc1545b25d017f19fb3";

    /// The game this fixture's manifests declare, and nothing a real game names.
    const GAME: &str = "test-version";

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A game directory of its own, so tests may run in any order at once.
    fn instance(label: &str) -> PathBuf {
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ravel-discovery-{label}-{}-{unique}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("a stale instance should be removable");
        }
        fs::create_dir_all(root.join(PACKAGES_DIRECTORY)).expect("an instance is creatable");
        root
    }

    fn packages(instance: &Path) -> PathBuf {
        instance.join(PACKAGES_DIRECTORY)
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

    /// Installs a package into an instance the way a player would: a directory
    /// holding a manifest, the source it declares, and its provenance record.
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

    /// The whole point: a package nothing was compiled from becomes an entry.
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

    /// Two packages, so the order the answer comes back in is a property of the
    /// set and not of the directory names it happened to be installed under.
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
        assert_eq!(
            discovered.manifests(),
            [] as [ravel_package::PackageManifest; 0]
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_manifest_the_schema_refuses_refuses_the_instance() {
        let instance = instance("schema");
        // A field the schema does not declare. `deny_unknown_fields` is what
        // makes a typo a rejection rather than a dropped declaration, and this
        // is that rule reached through a package on disk.
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
        // The manifest records a digest for source this package does not carry.
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
        assert_eq!(
            refused,
            DiscoveryRefused::MissingManifest {
                directory: "half-copied".to_owned()
            }
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
        assert_eq!(
            refused,
            DiscoveryRefused::NotAPackage {
                directory: "pack.jar".to_owned()
            }
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
        assert_eq!(
            refused,
            DiscoveryRefused::OutsideInstance {
                directory: "borrowed".to_owned()
            }
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
