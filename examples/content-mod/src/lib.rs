#![forbid(unsafe_code)]

//! A Ravel mod that adds content.
//!
//! `examples/minimal-mod` claims one command, which is the smallest thing a mod
//! can do. This one adds a block, the item that places it, the recipe that makes
//! it, the tags that let other mods' recipes find it, its name in two languages,
//! and the model file Ravel does not generate — which is the smallest thing a
//! *content* mod does, and it touches every part of M4-W2's surface.
//!
//! Everything is declared once, in [`ContentMod::declaration`]. The registration
//! window claims the entries from that declaration and
//! [`ContentMod::generated`] produces the pack from the same one, so the block
//! this mod registers and the block its tag names cannot drift apart.
//!
//! `Cargo.toml` beside this file lists one dependency, `ravel-sdk`, which is a
//! mod author's whole manifest. Nothing here names a loader type, a Minecraft
//! version, or any Java at all.

use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind, Registrar, RegistryKey};
use ravel_sdk::events::GameEvent;
use ravel_sdk::generate::GeneratedData;
use ravel_sdk::resources::RelativePath;
use ravel_sdk::tags::TagId;
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, declare_mod};

/// The path this mod's block, item and recipe are all named after.
const LANTERN: &str = "copper_lantern";

/// The namespace mods share on purpose.
///
/// A tag in `c` is how one mod's recipe accepts another mod's ingot. Naming a
/// tag there is not a claim on the namespace — a tag has no owner — which is the
/// one place a mod legitimately writes outside its own name.
const CONVENTION: &str = "c";

/// The bytes of the block model, which Ravel does not generate.
///
/// A model's shape is a Minecraft version's to decide, so the SDK generates none
/// and this mod supplies it. It travels in the same generated pack as the derived
/// files, which is what stops a supplied path and a generated path disagreeing.
const MODEL: &[u8] = b"{\n  \"parent\": \"minecraft:block/lantern\"\n}\n";

/// A mod that adds a copper lantern.
///
/// # Examples
///
/// ```
/// use ravel_example_content_mod::ContentMod;
/// use ravel_sdk::{Declared, Mod};
///
/// let content = ContentMod::new();
/// assert_eq!(content.id().as_str(), "content");
///
/// // Three registry entries, and a block and an item may share the name.
/// let declaration = content.declaration();
/// assert_eq!(
///     declaration
///         .entries()
///         .iter()
///         .map(ToString::to_string)
///         .collect::<Vec<_>>(),
///     [
///         "block:content:copper_lantern",
///         "item:content:copper_lantern",
///         "recipe:content:crafting/copper_lantern",
///     ]
/// );
///
/// // Four files: two tags, one language file per locale, and the model.
/// assert_eq!(
///     content.generated().paths().collect::<Vec<_>>(),
///     [
///         "assets/content/lang/en_us.json",
///         "assets/content/lang/fr_fr.json",
///         "assets/content/models/block/copper_lantern.json",
///         "data/c/tags/block/lanterns.json",
///         "data/c/tags/item/lanterns.json",
///     ]
/// );
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentMod {
    reloads: u64,
}

impl ContentMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub const fn new() -> Self {
        Self { reloads: 0 }
    }

    /// Returns how many times the game has rebuilt its resources.
    #[must_use]
    pub const fn reloads(&self) -> u64 {
        self.reloads
    }

    /// Returns the tag of `kind` called `lanterns` in the shared namespace.
    ///
    /// # Panics
    ///
    /// Panics for a kind nothing groups. Both callers pass a taggable kind, and
    /// a test covers both.
    #[must_use]
    pub fn lanterns(kind: ContentKind) -> TagId {
        let namespace: ModId = CONVENTION.parse().expect("`c` is a legal mod id");
        TagId::new(
            kind,
            ContentId::new(namespace, "lanterns").expect("`lanterns` is a legal content path"),
        )
        .expect("blocks and items are taggable")
    }

    /// Returns the path the block model is supplied at.
    ///
    /// # Panics
    ///
    /// Panics if the derived path stops being a legal package path, which would
    /// mean this mod's own identity can no longer name a file.
    #[must_use]
    pub fn model_path() -> RelativePath {
        RelativePath::try_from(format!(
            "assets/{}/models/block/{LANTERN}.json",
            Self::declared_id()
        ))
        .expect("a mod id and a literal cannot spell an illegal path")
    }

    /// Declares everything this mod contributes.
    ///
    /// The declaration is built rather than stored, which is what makes it the
    /// single statement of what this mod adds: the registration window and the
    /// generator both call this, so neither can be right while the other is
    /// stale.
    ///
    /// # Panics
    ///
    /// Panics if any declaration here is refused. Every one is a constant in
    /// this file, so a refusal means this mod's own content is contradictory —
    /// which is the point of the checks and is exactly what should stop a build
    /// rather than reach a player.
    #[must_use]
    pub fn declaration(&self) -> ContentDeclaration {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());

        let block = declaration
            .entry(ContentKind::Block, LANTERN)
            .expect("the block is declared once");
        let item = declaration
            .entry(ContentKind::Item, LANTERN)
            .expect("an item is a different registry from a block");
        declaration
            .entry(ContentKind::Recipe, &format!("crafting/{LANTERN}"))
            .expect("the recipe is declared once");

        // The block and the item go in the tag for their own registry. One tag
        // for both would be the mistake `TagId` refuses.
        declaration
            .tag(&Self::lanterns(ContentKind::Block), &block)
            .expect("a block joins a block tag");
        declaration
            .tag(&Self::lanterns(ContentKind::Item), &item)
            .expect("an item joins an item tag");

        for (locale, name) in [("en_us", "Copper Lantern"), ("fr_fr", "Lanterne de cuivre")] {
            for prefix in ["block", "item"] {
                declaration
                    .translation(
                        locale,
                        &format!("{prefix}.{}.{LANTERN}", Self::declared_id()),
                        name,
                    )
                    .expect("each key is named once per locale");
            }
        }

        declaration
            .resource(Self::model_path(), MODEL.to_vec())
            .expect("the model is supplied once, and nothing generates that path");

        declaration
    }

    /// Produces the pack this mod ships.
    ///
    /// Calling this twice produces the same bytes, because the declaration is
    /// the same and generation reads nothing else.
    #[must_use]
    pub fn generated(&self) -> GeneratedData {
        self.declaration().generate()
    }
}

#[declare_mod(id = "content")]
impl Mod for ContentMod {
    /// Accepts every step. Nothing this mod does needs setting up in a phase.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    /// Claims every entry the declaration holds.
    ///
    /// A conflict is fatal, which is what [`ravel_sdk::content::claim_all`]
    /// does: a content mod whose block another mod already owns cannot load as
    /// half of itself.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] naming the contested entry and its owner.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        ravel_sdk::content::claim_all(&self.declaration(), registrar)
    }

    /// Counts resource reloads and ignores everything else.
    ///
    /// # Errors
    ///
    /// Never returns an error. A reload is something a player did, and failing
    /// the set for it would put a mod graph's fail-closed path in the way of a
    /// keypress.
    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        if matches!(event, GameEvent::ResourcesReloaded) {
            self.reloads = self.reloads.saturating_add(1);
        }
        Ok(())
    }
}

/// Returns the entries this mod declares, for a caller that has no instance.
///
/// A test or a pack validator wants the content without driving a lifecycle.
#[must_use]
pub fn declared_entries() -> Vec<RegistryKey> {
    ContentMod::new().declaration().entries().to_vec()
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::{
        ContentDeclaration, ContentKind, DeclarationError, Registrar, RegistrationRefused,
        RegistryKey,
    };
    use ravel_sdk::events::GameEvent;
    use ravel_sdk::generate::GeneratedFile;
    use ravel_sdk::resources::{RelativePath, ResourceSet};
    use ravel_sdk::tags::TagSet;
    use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId};

    use super::{ContentMod, LANTERN, MODEL, declared_entries};

    /// A registrar that accepts every claim, standing in for the loader's table.
    #[derive(Default)]
    struct Accepting {
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Accepting {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }
    }

    /// A registrar that refuses claims of one kind as already owned.
    struct Contested {
        kind: ContentKind,
        owner: ModId,
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Contested {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            if key.kind() == self.kind {
                return Err(RegistrationRefused::Conflict {
                    key,
                    owner: self.owner.clone(),
                });
            }
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }
    }

    #[test]
    fn the_mod_reports_the_identity_it_declared() {
        let expected: ModId = "content".parse().expect("`content` is a legal mod id");
        assert_eq!(ContentMod::declared_id(), &expected);
        assert_eq!(ContentMod::new().id(), &expected);
        assert_eq!(ContentMod::new(), ContentMod::default());
        assert_eq!(ContentMod::new().reloads(), 0);
    }

    #[test]
    fn every_lifecycle_step_is_accepted() {
        let mut content = ContentMod::new();
        for state in LifecycleState::ALL {
            content.enter(state).expect("nothing is refused");
        }
    }

    #[test]
    fn every_declared_entry_is_named_in_this_mods_own_namespace() {
        for key in declared_entries() {
            assert_eq!(key.id().namespace(), ContentMod::declared_id());
            assert!(key.id().path().contains(LANTERN), "{key}");
            // The canonical spelling is what the barrier stores, so it has to
            // survive a round trip.
            assert_eq!(
                key.to_string().parse::<RegistryKey>().expect("a legal key"),
                key
            );
        }
    }

    #[test]
    fn the_declaration_is_the_same_statement_every_time_it_is_built() {
        let content = ContentMod::new();
        assert_eq!(content.declaration(), content.declaration());
        assert_eq!(content.declaration(), ContentMod::new().declaration());
    }

    #[test]
    fn registration_claims_exactly_the_declared_entries_in_declaration_order() {
        let mut content = ContentMod::new();
        let mut registrar = Accepting::default();
        content.register(&mut registrar).expect("nothing conflicts");
        assert_eq!(registrar.claimed(), declared_entries());
    }

    #[test]
    fn a_contested_entry_fails_the_mod_and_names_the_owner_and_the_phase() {
        for kind in [ContentKind::Block, ContentKind::Item, ContentKind::Recipe] {
            let owner: ModId = "other".parse().expect("a legal mod id");
            let mut registrar = Contested {
                kind,
                owner: owner.clone(),
                claimed: Vec::new(),
            };

            let error = ContentMod::new()
                .register(&mut registrar)
                .expect_err("one kind is taken");
            assert_eq!(error.mod_id(), ContentMod::declared_id());
            assert_eq!(error.state(), LifecycleState::Constructed);
            assert!(
                matches!(&error, ModError::Refused { detail, .. }
                    if detail.contains(owner.as_str()) && detail.contains(kind.as_str())),
                "{error}"
            );

            // Registration stopped at the refusal rather than claiming past it.
            let refused_at = declared_entries()
                .iter()
                .position(|key| key.kind() == kind)
                .expect("every kind is declared");
            assert_eq!(registrar.claimed().len(), refused_at);
        }
    }

    #[test]
    fn the_block_and_the_item_join_the_tag_for_their_own_registry() {
        let content = ContentMod::new();
        let declaration = content.declaration();
        let set = TagSet::merged([&declaration]);

        for kind in [ContentKind::Block, ContentKind::Item] {
            let tag = ContentMod::lanterns(kind);
            let members = set.members(&tag);
            assert_eq!(members.len(), 1, "{tag}");
            assert_eq!(members[0].kind(), kind);
            assert_eq!(set.contributors(&tag), [ContentMod::declared_id().clone()]);
        }
    }

    #[test]
    fn a_second_mod_joins_the_same_shared_tag_rather_than_conflicting_with_this_one() {
        // The convention namespace exists for this. Two mods in `c:lanterns` is
        // the case a tag is for, and it is not a conflict anywhere.
        let content = ContentMod::new().declaration();
        let tag = ContentMod::lanterns(ContentKind::Item);

        let mut other = ContentDeclaration::new("other".parse().expect("a legal mod id"));
        let theirs = other
            .entry(ContentKind::Item, "tin_lantern")
            .expect("a legal entry");
        other.tag(&tag, &theirs).expect("an item joins an item tag");

        let set = TagSet::merged([&content, &other]);
        assert_eq!(set.members(&tag).len(), 2);
        assert_eq!(set.contributors(&tag).len(), 2);
    }

    #[test]
    fn a_block_cannot_join_the_item_tag() {
        let mut declaration = ContentMod::new().declaration();
        let block = declaration
            .entries()
            .iter()
            .find(|key| key.kind() == ContentKind::Block)
            .expect("a block is declared")
            .clone();

        let error = declaration
            .tag(&ContentMod::lanterns(ContentKind::Item), &block)
            .expect_err("an item tag holds items");
        assert!(matches!(error, DeclarationError::TagKind { .. }), "{error}");
    }

    #[test]
    fn the_supplied_model_is_carried_into_the_generated_pack_unchanged() {
        let generated = ContentMod::new().generated();
        assert_eq!(
            generated
                .file(&ContentMod::model_path())
                .map(GeneratedFile::bytes),
            Some(MODEL)
        );
    }

    #[test]
    fn the_generated_tag_and_language_files_are_the_exact_documents_pinned_here() {
        let generated = ContentMod::new().generated();

        for (kind, expected) in [
            (
                ContentKind::Block,
                "{\n  \"replace\": false,\n  \"values\": [\n    \"content:copper_lantern\"\n  ]\n}\n",
            ),
            (
                ContentKind::Item,
                "{\n  \"replace\": false,\n  \"values\": [\n    \"content:copper_lantern\"\n  ]\n}\n",
            ),
        ] {
            let tag = ContentMod::lanterns(kind);
            let file = generated
                .file(&tag.generated_path())
                .expect("the tag is declared");
            assert_eq!(
                std::str::from_utf8(file.bytes()).expect("generated JSON is UTF-8"),
                expected,
                "{tag}"
            );
        }

        let english = RelativePath::try_from("assets/content/lang/en_us.json".to_owned())
            .expect("a legal path");
        assert_eq!(
            std::str::from_utf8(
                generated
                    .file(&english)
                    .expect("English is declared")
                    .bytes()
            )
            .expect("generated JSON is UTF-8"),
            "{\n  \"block.content.copper_lantern\": \"Copper Lantern\",\n  \
             \"item.content.copper_lantern\": \"Copper Lantern\"\n}\n"
        );
    }

    #[test]
    fn generating_the_pack_twice_produces_the_same_bytes() {
        let content = ContentMod::new();
        let first = content.generated();
        let second = content.generated();

        assert_eq!(first, second);
        for (left, right) in first.files().iter().zip(second.files()) {
            assert_eq!(left.path(), right.path());
            assert_eq!(left.bytes(), right.bytes());
        }
    }

    #[test]
    fn this_mods_language_file_loses_to_a_mod_that_loads_after_it() {
        // A resource is not a claim. A resource pack later in load order taking
        // `en_us.json` is ordinary, and the mod it shadowed is named.
        let english = RelativePath::try_from("assets/content/lang/en_us.json".to_owned())
            .expect("a legal path");
        let later: ModId = "translation_pack".parse().expect("a legal mod id");

        let set = ResourceSet::layered([
            (ContentMod::declared_id(), [&english]),
            (&later, [&english]),
        ]);

        assert_eq!(set.supplier(&english), Some(&later));
        assert_eq!(
            set.overrides()[0].shadowed,
            [ContentMod::declared_id().clone()]
        );
    }

    #[test]
    fn only_a_resource_reload_is_counted_and_no_event_fails_the_set() {
        let mut content = ContentMod::new();
        for event in GameEvent::ALL {
            content.observe(event).expect("nothing is refused");
        }
        assert_eq!(
            content.reloads(),
            1,
            "exactly one of `GameEvent::ALL` is a reload"
        );
    }

    #[test]
    fn a_mod_that_owns_no_world_data_declares_none_and_saves_none() {
        let content = ContentMod::new();
        assert!(content.saved_data_declaration().is_none());
        assert!(
            content
                .save_saved_data()
                .expect("a stateless mod produces no envelope")
                .is_none()
        );
    }
}
