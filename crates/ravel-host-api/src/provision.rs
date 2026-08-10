use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ModId, Version, VersionRange};

/// A library one mod supplies for others to build on, at a version.
///
/// The version is kept as the author wrote it and read when the set is
/// resolved, not when the declaration is made. That is the same choice
/// [`crate::Points`] and `ravel_data::StageDeclaration` make about a name
/// nothing publishes: a declaration records what a mod said, and every refusal
/// about the set is decided in one place, so no author has to handle an error
/// at the point of stating a fact.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Provided {
    library: String,
    version: String,
}

impl Provided {
    /// Declares `library` supplied at `version`.
    pub fn new(library: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            library: library.into(),
            version: version.into(),
        }
    }

    /// Returns the library's name.
    #[must_use]
    pub fn library(&self) -> &str {
        &self.library
    }

    /// Returns the version as it was declared.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Which mod supplies each shared library, and what the mods using it need.
///
/// [`crate::ServiceDeclaration`] orders named calls between two mods that are
/// both installed. This is the other question: which mod *supplies* a library
/// several mods build on, and at what version. A launch takes one provider per
/// library, and a set that cannot produce one — two providers, a version no
/// requirement accepts, a requirement nothing provides — is refused by
/// [`Provision::resolve`] before any mod runs, naming the library and the mods
/// on both sides of the disagreement.
///
/// # What a shared library is here, and what it is not
///
/// It is a declaration, not a shipped artefact. `docs/decisions/adg-2-mod-packaging-and-abi.md`
/// distributes a mod as source the player's machine compiles, so a library two
/// mods both build on is an ordinary Cargo dependency that Cargo resolves once
/// for the build — there is no second copy of it to load, and nothing here
/// bundles, fetches, or unpacks anything. What Cargo cannot see is the *mod
/// set*: that two installed mods each claim to be the one supplying `shared_maths`
/// to the others, or that one of them needs a version the supplier does not
/// offer. Those are facts about a launch, and this is where a launch decides
/// them.
///
/// # Examples
///
/// ```
/// use ravel_core::{ModId, VersionRange};
/// use ravel_host_api::{Provided, Provision};
///
/// let provider: ModId = "maths_mod".parse()?;
/// let consumer: ModId = "shop_mod".parse()?;
///
/// let mut provision = Provision::default();
/// provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
/// provision.requires(
///     &consumer,
///     "shared_maths",
///     VersionRange::parse("^2").expect("a well formed range"),
/// );
///
/// let resolved = provision.resolve().expect("one provider answers the requirement");
/// assert_eq!(resolved.provider_of("shared_maths"), Some(&provider));
/// assert_eq!(resolved.version_of("shared_maths"), Some("2.1.0"));
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Provision {
    provided: BTreeMap<String, Vec<(ModId, String)>>,
    required: BTreeMap<String, Vec<(ModId, VersionRange)>>,
}

impl Provision {
    /// Declares that `owner` supplies a library.
    ///
    /// Declaring the same library at the same version twice declares it once.
    /// Declaring it at two versions, or two mods declaring it at all, is the
    /// contest [`Provision::resolve`] refuses.
    pub fn provides(&mut self, owner: &ModId, provided: Provided) {
        let claim = (owner.clone(), provided.version);
        let claims = self.provided.entry(provided.library).or_default();
        if !claims.contains(&claim) {
            claims.push(claim);
        }
    }

    /// Declares that `owner` needs `library` within `wanted`.
    ///
    /// The same requirement stated twice is one requirement. Two ranges from one
    /// mod are two requirements, and both have to be satisfied by the one
    /// version a launch resolves.
    pub fn requires(&mut self, owner: &ModId, library: &str, wanted: VersionRange) {
        let requirement = (owner.clone(), wanted);
        let requirements = self.required.entry(library.to_owned()).or_default();
        if !requirements.contains(&requirement) {
            requirements.push(requirement);
        }
    }

    /// Returns the mods that declared they supply `library`, in declaration
    /// order.
    #[must_use]
    pub fn providers_of(&self, library: &str) -> Vec<&ModId> {
        self.provided
            .get(library)
            .map(|claims| claims.iter().map(|(owner, _)| owner).collect())
            .unwrap_or_default()
    }

    /// Decides one provider per library for this launch.
    ///
    /// A library nothing requires still resolves. A provider nobody needs is not
    /// a mistake — it is a mod that supplies something for others and happens to
    /// be installed alone — and refusing it would make installing a library mod
    /// first an error.
    ///
    /// # Errors
    ///
    /// Returns [`ProvisionRefused::NoProvider`] when a required library is
    /// supplied by nothing, [`ProvisionRefused::Contested`] when more than one
    /// claim supplies one library, [`ProvisionRefused::Unsatisfied`] when the
    /// provided version is outside some mod's declared range, and
    /// [`ProvisionRefused::UnreadableVersion`] when a declared version is not a
    /// version at all. All four are facts about the set, which is why they are
    /// decided here rather than when a call is made.
    pub fn resolve(&self) -> Result<Resolved, ProvisionRefused> {
        let libraries: BTreeSet<&String> =
            self.provided.keys().chain(self.required.keys()).collect();
        let mut chosen = BTreeMap::new();

        for library in libraries {
            let claims = self.provided.get(library).map_or(&[][..], Vec::as_slice);
            let requirements = self.required.get(library).map_or(&[][..], Vec::as_slice);
            let requiring = |chosen: Option<&Version>| -> Vec<ModId> {
                requirements
                    .iter()
                    .filter(|(_, wanted)| chosen.is_none_or(|version| !wanted.matches(version)))
                    .map(|(owner, _)| owner.clone())
                    .collect()
            };

            let (provider, version) = match claims {
                [] => {
                    return Err(ProvisionRefused::NoProvider {
                        library: library.clone(),
                        unsatisfied: requiring(None),
                    });
                }
                [single] => single,
                contested => {
                    return Err(ProvisionRefused::Contested {
                        library: library.clone(),
                        providers: contested.iter().map(|(owner, _)| owner.clone()).collect(),
                    });
                }
            };

            let parsed =
                Version::parse(version).map_err(|_| ProvisionRefused::UnreadableVersion {
                    library: library.clone(),
                    provider: provider.clone(),
                    version: version.clone(),
                })?;

            let unsatisfied = requiring(Some(&parsed));
            if !unsatisfied.is_empty() {
                return Err(ProvisionRefused::Unsatisfied {
                    library: library.clone(),
                    provider: provider.clone(),
                    version: version.clone(),
                    unsatisfied,
                });
            }

            chosen.insert(library.clone(), (provider.clone(), version.clone()));
        }

        Ok(Resolved { chosen })
    }
}

/// The one provider each library resolved to for this launch.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Resolved {
    chosen: BTreeMap<String, (ModId, String)>,
}

impl Resolved {
    /// Returns the mod supplying `library`, if any mod supplies it.
    #[must_use]
    pub fn provider_of(&self, library: &str) -> Option<&ModId> {
        self.chosen.get(library).map(|(provider, _)| provider)
    }

    /// Returns the version `library` resolved to, as its provider declared it.
    #[must_use]
    pub fn version_of(&self, library: &str) -> Option<&str> {
        self.chosen
            .get(library)
            .map(|(_, version)| version.as_str())
    }

    /// Returns every resolved library, in name order.
    pub fn libraries(&self) -> impl Iterator<Item = &str> + '_ {
        self.chosen.keys().map(String::as_str)
    }
}

/// Why a set of mods has no single answer for a library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvisionRefused {
    /// A required library is supplied by nothing in the set.
    NoProvider {
        /// The library nothing supplies.
        library: String,
        /// The mods that need it, in declaration order.
        unsatisfied: Vec<ModId>,
    },
    /// More than one claim supplies one library.
    Contested {
        /// The library claimed twice.
        library: String,
        /// The mods claiming it, in declaration order. A mod appears twice when
        /// it declared one library at two versions.
        providers: Vec<ModId>,
    },
    /// The supplied version is outside some mod's declared range.
    Unsatisfied {
        /// The library the mods disagree about.
        library: String,
        /// The mod supplying it.
        provider: ModId,
        /// The version it supplies.
        version: String,
        /// The mods whose range that version is outside, in declaration order.
        unsatisfied: Vec<ModId>,
    },
    /// A declared version is not a version.
    UnreadableVersion {
        /// The library it was declared for.
        library: String,
        /// The mod that declared it.
        provider: ModId,
        /// What was written where a version belongs.
        version: String,
    },
}

impl ProvisionRefused {
    /// Returns the library this refusal is about.
    #[must_use]
    pub fn library(&self) -> &str {
        match self {
            Self::NoProvider { library, .. }
            | Self::Contested { library, .. }
            | Self::Unsatisfied { library, .. }
            | Self::UnreadableVersion { library, .. } => library,
        }
    }

    /// Returns whether the library is supplied by nothing in the set.
    #[must_use]
    pub const fn has_no_provider(&self) -> bool {
        matches!(self, Self::NoProvider { .. })
    }

    /// Returns the mods whose requirement this refusal leaves unanswered.
    ///
    /// Empty for a refusal that is not about a requirement: a contested library
    /// has no chosen version to hold a range against, and a version nothing can
    /// read is refused before any range is tried.
    #[must_use]
    pub fn unsatisfied(&self) -> &[ModId] {
        match self {
            Self::NoProvider { unsatisfied, .. } | Self::Unsatisfied { unsatisfied, .. } => {
                unsatisfied
            }
            Self::Contested { .. } | Self::UnreadableVersion { .. } => &[],
        }
    }

    /// Returns the mods claiming to supply one library, in declaration order.
    ///
    /// Empty unless the claim is contested, which is the refusal that has more
    /// than one claimant to name.
    #[must_use]
    pub fn providers(&self) -> &[ModId] {
        match self {
            Self::Contested { providers, .. } => providers,
            Self::NoProvider { .. } | Self::Unsatisfied { .. } | Self::UnreadableVersion { .. } => {
                &[]
            }
        }
    }
}

impl fmt::Display for ProvisionRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = |mods: &[ModId]| {
            mods.iter()
                .map(ModId::as_str)
                .collect::<Vec<&str>>()
                .join("`, `")
        };
        match self {
            Self::NoProvider {
                library,
                unsatisfied,
            } => write!(
                formatter,
                "`{}` need `{library}`, which no installed mod provides",
                names(unsatisfied)
            ),
            Self::Contested { library, providers } => write!(
                formatter,
                "`{}` each provide `{library}`, and one launch takes one provider",
                names(providers)
            ),
            Self::Unsatisfied {
                library,
                provider,
                version,
                unsatisfied,
            } => write!(
                formatter,
                "`{provider}` provides `{library}` at `{version}`, which is outside the range `{}` \
                 declared",
                names(unsatisfied)
            ),
            Self::UnreadableVersion {
                library,
                provider,
                version,
            } => write!(
                formatter,
                "`{provider}` provides `{library}` at `{version}`, which is not a version"
            ),
        }
    }
}

impl std::error::Error for ProvisionRefused {}

#[cfg(test)]
mod tests {
    use ravel_core::{ModId, VersionRange};

    use super::{Provided, Provision, ProvisionRefused};

    fn id(name: &str) -> ModId {
        name.parse().expect("a well formed owner id")
    }

    fn range(grammar: &str) -> VersionRange {
        VersionRange::parse(grammar).expect("a well formed range")
    }

    #[test]
    fn one_provider_answers_one_requirement_and_is_named() {
        let provider = id("sample");
        let consumer = id("probe");

        let mut provision = Provision::default();
        provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
        provision.requires(&consumer, "shared_maths", range("^2"));

        let resolved = provision.resolve().expect("one provider resolves");

        assert_eq!(resolved.provider_of("shared_maths"), Some(&provider));
        assert_eq!(resolved.version_of("shared_maths"), Some("2.1.0"));
        assert_eq!(
            resolved.libraries().collect::<Vec<&str>>(),
            ["shared_maths"]
        );
    }

    #[test]
    fn a_library_nothing_declares_resolved_to_nobody() {
        let resolved = Provision::default()
            .resolve()
            .expect("an empty set resolves");

        assert_eq!(resolved.provider_of("shared_maths"), None);
        assert_eq!(resolved.version_of("shared_maths"), None);
    }

    #[test]
    fn a_provider_nobody_needs_is_still_resolved() {
        let provider = id("sample");

        let mut provision = Provision::default();
        provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));

        let resolved = provision
            .resolve()
            .expect("an unused provider is not an error");

        assert_eq!(resolved.provider_of("shared_maths"), Some(&provider));
    }

    #[test]
    fn a_requirement_nothing_provides_is_refused_naming_who_needed_it() {
        let consumer = id("probe");

        let mut provision = Provision::default();
        provision.requires(&consumer, "shared_maths", range("^2"));

        let refusal = provision
            .resolve()
            .expect_err("a library nothing provides is refused before anything runs");

        assert_eq!(refusal.library(), "shared_maths");
        assert!(refusal.has_no_provider());
        assert_eq!(refusal.unsatisfied(), [consumer]);
        assert!(refusal.providers().is_empty());
    }

    #[test]
    fn two_requirements_no_version_satisfies_are_refused_naming_the_one_left_out() {
        let provider = id("sample");
        let older = id("probe");
        let newer = id("witness");

        let mut provision = Provision::default();
        provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
        provision.requires(&older, "shared_maths", range("^1"));
        provision.requires(&newer, "shared_maths", range("^2"));

        let refusal = provision
            .resolve()
            .expect_err("no single version answers both, so the set is refused");

        assert_eq!(refusal.library(), "shared_maths");
        assert_eq!(refusal.unsatisfied(), [older]);
        assert!(!refusal.has_no_provider());
        assert!(refusal.to_string().contains("2.1.0"), "{refusal}");
    }

    #[test]
    fn two_mods_providing_one_library_are_refused_naming_both() {
        let first = id("sample");
        let second = id("probe");

        let mut provision = Provision::default();
        provision.provides(&first, Provided::new("shared_maths", "2.1.0"));
        provision.provides(&second, Provided::new("shared_maths", "2.1.0"));

        let refusal = provision
            .resolve()
            .expect_err("one launch takes one provider");

        assert_eq!(refusal.library(), "shared_maths");
        assert_eq!(refusal.providers(), [first, second]);
        assert!(refusal.unsatisfied().is_empty());
        assert!(matches!(refusal, ProvisionRefused::Contested { .. }));
    }

    #[test]
    fn one_mod_providing_two_versions_of_one_library_is_the_same_contest() {
        let provider = id("sample");

        let mut provision = Provision::default();
        provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
        provision.provides(&provider, Provided::new("shared_maths", "3.0.0"));

        let refusal = provision.resolve().expect_err("one library, one version");

        assert_eq!(refusal.providers(), [provider.clone(), provider]);
    }

    #[test]
    fn declaring_one_thing_twice_declares_it_once() {
        let provider = id("sample");
        let consumer = id("probe");

        let mut provision = Provision::default();
        provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
        provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
        provision.requires(&consumer, "shared_maths", range("^2"));
        provision.requires(&consumer, "shared_maths", range("^2"));

        assert_eq!(provision.providers_of("shared_maths"), [&provider]);
        assert!(provision.providers_of("shared_words").is_empty());
        assert_eq!(
            provision
                .resolve()
                .expect("one claim resolves")
                .provider_of("shared_maths"),
            Some(&provider)
        );
    }

    #[test]
    fn a_version_that_is_not_a_version_is_refused_rather_than_ignored() {
        let provider = id("sample");
        let consumer = id("probe");

        let mut provision = Provision::default();
        provision.provides(&provider, Provided::new("shared_maths", "the latest one"));
        provision.requires(&consumer, "shared_maths", range("*"));

        let refusal = provision
            .resolve()
            .expect_err("a version nothing can read answers no range");

        assert_eq!(refusal.library(), "shared_maths");
        assert!(refusal.unsatisfied().is_empty());
        assert!(refusal.to_string().contains("the latest one"), "{refusal}");
        assert!(matches!(
            refusal,
            ProvisionRefused::UnreadableVersion { .. }
        ));
    }

    #[test]
    fn a_declaration_keeps_the_library_and_version_it_was_written_with() {
        let provided = Provided::new("shared_maths", "2.1.0");
        assert_eq!(provided.library(), "shared_maths");
        assert_eq!(provided.version(), "2.1.0");
    }

    #[test]
    fn one_library_refused_is_refused_whatever_the_others_do() {
        let provider = id("sample");
        let consumer = id("probe");

        let mut provision = Provision::default();
        provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
        provision.requires(&consumer, "shared_maths", range("^2"));
        provision.requires(&consumer, "shared_words", range("^1"));

        let refusal = provision
            .resolve()
            .expect_err("a set is refused for any library in it");

        assert_eq!(refusal.library(), "shared_words");
        assert!(refusal.has_no_provider());
    }
}
