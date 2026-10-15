use std::collections::BTreeMap;
use std::fmt;

use ravel_config::ValueDocument;
use ravel_core::{ContentId, ModId, RelativePath, Version};

use crate::progression::Progression;

/// One file the pack itself writes.
///
/// A list rather than a map, because "the pack supplies this path twice" has to be
/// *representable* for [`crate::validate`] to be able to refuse it. A map would
/// make the second supply overwrite the first silently, which is the defect and not
/// the check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuppliedFile {
    /// Where the file belongs in the written pack.
    pub path: RelativePath,
    /// Its exact content.
    pub bytes: Vec<u8>,
}

/// One path the pack's own content refers to, and what refers to it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceReference {
    /// The path something expects to be able to read.
    pub path: RelativePath,
    /// The pack's own thing that refers to it.
    pub referrer: ContentId,
}

/// Everything one modpack declares.
///
/// `SPEC.md` §4.3 lists what a pack author does, and this is that list as one
/// value: it selects mods, pins their exact versions, configures them
/// declaratively, refers to and supplies resources, and declares a progression.
/// What it does not do is decide whether any of that is consistent — that is
/// [`crate::validate`]'s single job, and keeping the two apart is what lets a pack
/// be half-written in an editor and still be a value.
///
/// A pin is an exact [`Version`] and not a range. A range is what a *mod* declares
/// about its dependencies, because a mod does not know what it will be installed
/// beside; a pack is the thing that decided, so a pack that stated a range would be
/// deferring the one decision it exists to make, and two builds of it could ship
/// different code.
///
/// # Examples
///
/// ```
/// use ravel_core::{ModId, Version};
/// use ravel_pack::{PackDeclaration, PackError};
///
/// let mut pack = PackDeclaration::new(
///     "example_pack".parse::<ModId>().expect("a legal mod id"),
///     Version::new(0, 2, 1),
/// );
/// pack.pin(
///     "example_mod".parse::<ModId>().expect("a legal mod id"),
///     Version::new(1, 4, 0),
/// )?;
///
/// // The same mod twice is a pin the pack cannot mean two ways.
/// assert!(matches!(
///     pack.pin(
///         "example_mod".parse::<ModId>().expect("a legal mod id"),
///         Version::new(1, 5, 0),
///     ),
///     Err(PackError::PinnedTwice { .. })
/// ));
/// # Ok::<(), PackError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackDeclaration {
    id: ModId,
    version: Version,
    members: BTreeMap<ModId, Version>,
    settings: BTreeMap<ModId, ValueDocument>,
    supplied: Vec<SuppliedFile>,
    references: Vec<ResourceReference>,
    progression: Progression,
}

impl PackDeclaration {
    /// Starts an empty pack called `id` at `version`.
    #[must_use]
    pub fn new(id: ModId, version: Version) -> Self {
        Self {
            id,
            version,
            members: BTreeMap::new(),
            settings: BTreeMap::new(),
            supplied: Vec::new(),
            references: Vec::new(),
            progression: Progression::default(),
        }
    }

    /// Returns the pack's own namespace.
    #[must_use]
    pub const fn id(&self) -> &ModId {
        &self.id
    }

    /// Returns the pack's own version.
    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.version
    }

    /// Selects `member` at exactly `version`.
    ///
    /// # Errors
    ///
    /// Returns [`PackError::PinnedTwice`] when the pack already pinned the mod, and
    /// [`PackError::PinsItself`] when a pack pins its own namespace — a pack is not
    /// one of its own members, and a pack that pinned itself would ask
    /// [`crate::validate`] for a manifest that does not exist.
    pub fn pin(&mut self, member: ModId, version: Version) -> Result<(), PackError> {
        if member == self.id {
            return Err(PackError::PinsItself { pack: member });
        }
        if let Some(pinned) = self.members.get(&member) {
            return Err(PackError::PinnedTwice {
                member,
                pinned: pinned.clone(),
            });
        }
        self.members.insert(member, version);
        Ok(())
    }

    /// Returns every pinned member and its version, in identity order.
    pub fn members(&self) -> impl Iterator<Item = (&ModId, &Version)> + '_ {
        self.members.iter()
    }

    /// Returns the version the pack pinned `member` at, if it pinned it.
    #[must_use]
    pub fn pinned(&self, member: &ModId) -> Option<&Version> {
        self.members.get(member)
    }

    /// States the pack's overrides for one member's settings.
    ///
    /// The pack layer of the three configuration layers, so the document is a
    /// [`ValueDocument`] exactly as a player's file is and the same schema check
    /// applies to both. Which values a pack is allowed to state at all is the
    /// setting's `packOverride`, which `ravel-config` decides when the layers are
    /// resolved rather than something restated here.
    ///
    /// # Errors
    ///
    /// Returns [`PackError::SettingsTwice`] when the pack already stated overrides
    /// for the mod, and [`PackError::SettingsOwner`] when the document belongs to a
    /// different mod than the one it is filed under — the same wiring mistake
    /// `LayeredConfiguration` refuses, refused here so a pack cannot even be built
    /// holding it.
    pub fn configure(&mut self, member: ModId, document: ValueDocument) -> Result<(), PackError> {
        if document.owner != member {
            return Err(PackError::SettingsOwner {
                filed_under: member,
                owner: document.owner,
            });
        }
        if self.settings.contains_key(&member) {
            return Err(PackError::SettingsTwice { member });
        }
        self.settings.insert(member, document);
        Ok(())
    }

    /// Returns every member the pack states overrides for, in identity order.
    pub fn settings(&self) -> impl Iterator<Item = (&ModId, &ValueDocument)> + '_ {
        self.settings.iter()
    }

    /// Supplies `bytes` as the content of `path` in the written pack.
    ///
    /// Recorded rather than refused when the path repeats. [`crate::validate`] is
    /// where a repeated path is refused, because "the pack writes one file per
    /// path" is one of the four closure properties M4-W5's gate names and a check
    /// spread across two layers is a check with two answers.
    pub fn supply(&mut self, path: RelativePath, bytes: Vec<u8>) {
        self.supplied.push(SuppliedFile { path, bytes });
    }

    /// Returns every file the pack writes itself, in the order it was supplied.
    #[must_use]
    pub fn supplied(&self) -> &[SuppliedFile] {
        &self.supplied
    }

    /// Records that `referrer` expects to be able to read `path`.
    ///
    /// This is what makes resource closure checkable at all. A recipe naming a
    /// texture, or a progression step naming an icon, is a reference nothing in the
    /// file itself distinguishes from a string; saying so here is what lets
    /// [`crate::validate`] refuse a pack that ships a name with no file behind it.
    pub fn refer_to(&mut self, path: RelativePath, referrer: ContentId) {
        self.references.push(ResourceReference { path, referrer });
    }

    /// Returns every reference the pack made, in the order it made them.
    #[must_use]
    pub fn references(&self) -> &[ResourceReference] {
        &self.references
    }

    /// Returns the pack's progression, to read or to add a step to.
    #[must_use]
    pub const fn progression(&self) -> &Progression {
        &self.progression
    }

    /// Returns the pack's progression for editing.
    pub const fn progression_mut(&mut self) -> &mut Progression {
        &mut self.progression
    }
}

/// Why a pack could not declare what it was asked to.
///
/// Every variant is something one pack can get wrong on its own, with no member's
/// manifest and no other pack involved — the same division
/// [`ravel_data::DeclarationError`] draws. Everything that needs the members is
/// [`crate::ClosureError`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackError {
    /// The pack pinned its own namespace as a member.
    PinsItself {
        /// The pack's identity.
        pack: ModId,
    },
    /// The pack already pinned this member.
    PinnedTwice {
        /// The member pinned twice.
        member: ModId,
        /// The version it is already pinned at.
        pinned: Version,
    },
    /// The pack already stated overrides for this member.
    SettingsTwice {
        /// The member whose settings were stated twice.
        member: ModId,
    },
    /// The override document belongs to a different mod than it was filed under.
    SettingsOwner {
        /// The member the pack filed it under.
        filed_under: ModId,
        /// The member the document itself names.
        owner: ModId,
    },
}

impl fmt::Display for PackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PinsItself { pack } => write!(
                formatter,
                "`{pack}` is the pack, so it is not one of its own members"
            ),
            Self::PinnedTwice { member, pinned } => write!(
                formatter,
                "`{member}` is already pinned at `{pinned}`, and a pin is one version"
            ),
            Self::SettingsTwice { member } => write!(
                formatter,
                "this pack already states `{member}`'s settings, and one mod has one pack layer"
            ),
            Self::SettingsOwner { filed_under, owner } => write!(
                formatter,
                "the document filed under `{filed_under}` says it belongs to `{owner}`"
            ),
        }
    }
}

impl std::error::Error for PackError {}

#[cfg(test)]
mod tests {
    use ravel_config::ValueDocument;
    use ravel_core::{ContentId, ModId, RelativePath, Version};

    use super::{PackDeclaration, PackError};

    fn id(text: &str) -> ModId {
        text.parse().expect("a legal mod id")
    }

    fn path(text: &str) -> RelativePath {
        RelativePath::try_from(text.to_owned()).expect("a legal relative path")
    }

    fn pack() -> PackDeclaration {
        PackDeclaration::new(id("example_pack"), Version::new(0, 2, 1))
    }

    fn document(owner: &str) -> ValueDocument {
        ValueDocument::parse(&format!(
            r#"{{ "schemaVersion": 1, "owner": "{owner}", "values": {{ "limit": 3 }} }}"#
        ))
        .expect("a legal document")
    }

    #[test]
    fn an_empty_pack_is_named_and_versioned_and_holds_nothing_else() {
        let pack = pack();
        assert_eq!(pack.id(), &id("example_pack"));
        assert_eq!(pack.version(), &Version::new(0, 2, 1));
        assert_eq!(pack.members().count(), 0);
        assert_eq!(pack.settings().count(), 0);
        assert_eq!(pack.supplied(), []);
        assert_eq!(pack.references(), []);
        assert!(pack.progression().is_empty());
        assert_eq!(pack.pinned(&id("example_mod")), None);
    }

    #[test]
    fn members_are_pinned_at_one_exact_version_each_and_read_back_in_identity_order() {
        let mut pack = pack();
        pack.pin(id("zzz_mod"), Version::new(2, 0, 0))
            .expect("a legal pin");
        pack.pin(id("aaa_mod"), Version::new(1, 4, 0))
            .expect("a legal pin");

        assert_eq!(
            pack.members()
                .map(|(member, version)| (member.as_str(), version.to_string()))
                .collect::<Vec<_>>(),
            [
                ("aaa_mod", "1.4.0".to_owned()),
                ("zzz_mod", "2.0.0".to_owned())
            ]
        );
        assert_eq!(pack.pinned(&id("aaa_mod")), Some(&Version::new(1, 4, 0)));
    }

    #[test]
    fn a_pack_cannot_pin_itself_or_pin_one_member_twice() {
        let mut pack = pack();
        assert_eq!(
            pack.pin(id("example_pack"), Version::new(0, 2, 1)),
            Err(PackError::PinsItself {
                pack: id("example_pack")
            })
        );
        pack.pin(id("example_mod"), Version::new(1, 4, 0))
            .expect("a legal pin");
        let error = pack
            .pin(id("example_mod"), Version::new(1, 5, 0))
            .expect_err("the mod is already pinned");
        assert_eq!(
            error,
            PackError::PinnedTwice {
                member: id("example_mod"),
                pinned: Version::new(1, 4, 0),
            }
        );
        assert!(error.to_string().contains("1.4.0"), "{error}");
        assert_eq!(
            pack.pinned(&id("example_mod")),
            Some(&Version::new(1, 4, 0)),
            "the refusal left the first pin in place"
        );
    }

    #[test]
    fn a_settings_document_is_filed_under_the_mod_it_says_it_belongs_to() {
        let mut pack = pack();
        let error = pack
            .configure(id("example_mod"), document("other_mod"))
            .expect_err("the document names another mod");
        assert_eq!(
            error,
            PackError::SettingsOwner {
                filed_under: id("example_mod"),
                owner: id("other_mod"),
            }
        );
        assert_eq!(pack.settings().count(), 0);

        pack.configure(id("example_mod"), document("example_mod"))
            .expect("a document that names its own mod");
        assert_eq!(
            pack.configure(id("example_mod"), document("example_mod")),
            Err(PackError::SettingsTwice {
                member: id("example_mod")
            })
        );
        assert_eq!(pack.settings().count(), 1);
    }

    #[test]
    fn a_repeated_supply_and_a_reference_are_both_recorded_rather_than_refused() {
        // Recording is what makes the closure check possible: `validate` refuses the
        // repeat, and it can only do so if the declaration kept both.
        let mut pack = pack();
        pack.supply(
            path("data/example_pack/recipe/lantern.json"),
            b"{}".to_vec(),
        );
        pack.supply(
            path("data/example_pack/recipe/lantern.json"),
            b"{\"changed\": true}".to_vec(),
        );
        pack.refer_to(
            path("assets/example_pack/textures/lantern.png"),
            ContentId::new(id("example_pack"), "lantern").expect("a legal content path"),
        );

        assert_eq!(pack.supplied().len(), 2);
        assert_eq!(pack.supplied()[0].bytes, b"{}");
        assert_eq!(pack.references().len(), 1);
        assert_eq!(
            pack.references()[0].path.as_str(),
            "assets/example_pack/textures/lantern.png"
        );
    }
}
