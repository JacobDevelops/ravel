#![forbid(unsafe_code)]

//! A block, item, recipe, convention tags, translations, and supplied assets.
//!
//! Registration and generated data use [`ContentMod::declaration`] so their
//! identifiers stay aligned.

use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind, RegistryKey};
use ravel_sdk::events::GameEvent;
use ravel_sdk::generate::GeneratedData;
use ravel_sdk::resources::RelativePath;
use ravel_sdk::server_data::ServerDataKind;
use ravel_sdk::tags::TagId;
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, declare_mod};

/// The path this mod's block, item and recipe are all named after.
const LANTERN: &str = "copper_lantern";

// Convention tags may name the shared `c` namespace without claiming it.
const CONVENTION: &str = "c";

/// Supplied block-model bytes, exposed for launch assertions.
pub const MODEL: &[u8] = b"{\n  \"parent\": \"minecraft:block/lantern\"\n}\n";

/// Supplied recipe bytes targeting the example's Minecraft version.
pub const RECIPE: &[u8] = br##"{
  "type": "minecraft:crafting_shaped",
  "category": "misc",
  "key": {
    "#": {
      "item": "minecraft:torch"
    },
    "X": {
      "item": "minecraft:copper_ingot"
    }
  },
  "pattern": [
    "XXX",
    "X#X",
    "XXX"
  ],
  "result": {
    "count": 1,
    "id": "content:copper_lantern"
  }
}
"##;

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
/// // The registry entries, and a block and an item may share the name.
/// let declaration = content.declaration()?;
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
/// // The generated files. Two the mod supplies because their shape is a
/// // version's to decide — the block model and the recipe — and eight Ravel
/// // derives: a tag per registry, a language file per locale, and the
/// // blockstates, item model, item definition and loot table a block needs to
/// // render, be held and drop itself.
/// assert_eq!(
///     content.generated()?.paths().collect::<Vec<_>>(),
///     [
///         "assets/content/blockstates/copper_lantern.json",
///         "assets/content/items/copper_lantern.json",
///         "assets/content/lang/en_us.json",
///         "assets/content/lang/fr_fr.json",
///         "assets/content/models/block/copper_lantern.json",
///         "assets/content/models/item/copper_lantern.json",
///         "data/c/tags/block/lanterns.json",
///         "data/c/tags/item/lanterns.json",
///         "data/content/loot_table/blocks/copper_lantern.json",
///         "data/content/recipe/crafting/copper_lantern.json",
///     ]
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
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
    /// # Errors
    ///
    /// Returns an error if the namespace, path, or registry kind is invalid.
    pub fn lanterns(kind: ContentKind) -> Result<TagId, ModError> {
        let namespace: ModId = CONVENTION.parse().map_err(ModError::from_display)?;
        TagId::new(
            kind,
            ContentId::new(namespace, "lanterns").map_err(ModError::from_display)?,
        )
        .map_err(ModError::from_display)
    }

    /// Returns the path the block model is supplied at.
    ///
    /// # Errors
    ///
    /// Returns an error if the derived package path is invalid.
    pub fn model_path() -> Result<RelativePath, ModError> {
        RelativePath::try_from(format!(
            "assets/{}/models/block/{LANTERN}.json",
            Self::declared_id()
        ))
        .map_err(ModError::from_display)
    }

    /// Declares the content used by registration and generated data.
    ///
    /// # Errors
    ///
    /// Returns a refusal if the packaged declarations conflict or are invalid.
    pub fn declaration(&self) -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());

        let block = declaration
            .entry(ContentKind::Block, LANTERN)
            .map_err(ModError::from_display)?;
        let item = declaration
            .entry(ContentKind::Item, LANTERN)
            .map_err(ModError::from_display)?;
        let recipe = format!("crafting/{LANTERN}");
        declaration
            .entry(ContentKind::Recipe, &recipe)
            .map_err(ModError::from_display)?;

        // Block and item tags belong to distinct registries.
        declaration
            .tag(&Self::lanterns(ContentKind::Block)?, &block)
            .map_err(ModError::from_display)?;
        declaration
            .tag(&Self::lanterns(ContentKind::Item)?, &item)
            .map_err(ModError::from_display)?;

        for (locale, name) in [("en_us", "Copper Lantern"), ("fr_fr", "Lanterne de cuivre")] {
            for prefix in ["block", "item"] {
                declaration
                    .translation(
                        locale,
                        &format!("{prefix}.{}.{LANTERN}", Self::declared_id()),
                        name,
                    )
                    .map_err(ModError::from_display)?;
            }
        }

        declaration
            .resource(Self::model_path()?, MODEL.to_vec())
            .map_err(ModError::from_display)?;
        declaration
            .server_entry(ServerDataKind::Crafting, &recipe, RECIPE.to_vec())
            .map_err(ModError::from_display)?;

        Ok(declaration)
    }

    /// Generates a deterministic pack from this mod's content declaration.
    ///
    /// # Errors
    ///
    /// Returns a refusal if the packaged declarations conflict or are invalid.
    pub fn generated(&self) -> Result<GeneratedData, ModError> {
        Ok(self.declaration()?.generate())
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

    /// Supplies the content claimed by default registration, including its model.
    ///
    /// The model this mod supplies is bytes no version of Ravel generates, so
    /// this is the only way they reach the game at all.
    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        self.declaration().map(std::borrow::Cow::Owned).map(Some)
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
///
/// # Errors
///
/// Returns a refusal if the packaged declarations conflict or are invalid.
pub fn declared_entries() -> Result<Vec<RegistryKey>, ModError> {
    Ok(ContentMod::new().declaration()?.entries().to_vec())
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::{
        ContentDeclarationError, ContentKind, Registrar, RegistrationRefused, RegistryKey,
    };
    use ravel_sdk::events::GameEvent;
    use ravel_sdk::generate::GeneratedFile;
    use ravel_sdk::resources::RelativePath;
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

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
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

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
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
        for key in declared_entries().expect("valid declaration") {
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
        let declaration = content.declaration().expect("valid declaration");
        assert_eq!(
            declaration,
            content.declaration().expect("valid declaration")
        );
        assert_eq!(
            declaration,
            ContentMod::new().declaration().expect("valid declaration")
        );
    }

    #[test]
    fn registration_claims_exactly_the_declared_entries_in_declaration_order() {
        let mut content = ContentMod::new();
        let mut registrar = Accepting::default();
        content.register(&mut registrar).expect("nothing conflicts");
        assert_eq!(
            registrar.claimed(),
            declared_entries().expect("valid declaration")
        );
    }

    #[test]
    fn a_contested_entry_fails_the_mod_and_names_the_owner() {
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
            assert!(
                matches!(&error, ModError::Refused { detail, .. }
                    if detail.contains(owner.as_str()) && detail.contains(kind.as_str())),
                "{error}"
            );

            // Registration stopped at the refusal rather than claiming past it.
            let refused_at = declared_entries()
                .expect("valid declaration")
                .iter()
                .position(|key| key.kind() == kind)
                .expect("every kind is declared");
            assert_eq!(registrar.claimed().len(), refused_at);
        }
    }

    #[test]
    fn a_block_cannot_join_the_item_tag() {
        let mut declaration = ContentMod::new().declaration().expect("valid declaration");
        let block = declaration
            .entries()
            .iter()
            .find(|key| key.kind() == ContentKind::Block)
            .expect("a block is declared")
            .clone();

        let error = declaration
            .tag(
                &ContentMod::lanterns(ContentKind::Item).expect("valid tag"),
                &block,
            )
            .expect_err("an item tag holds items");
        assert!(
            matches!(error, ContentDeclarationError::TagKind { .. }),
            "{error}"
        );
    }

    #[test]
    fn the_supplied_model_is_carried_into_the_generated_pack_unchanged() {
        let generated = ContentMod::new().generated().expect("valid declaration");
        assert_eq!(
            generated
                .file(&ContentMod::model_path().expect("valid model path"))
                .map(GeneratedFile::bytes),
            Some(MODEL)
        );
    }

    #[test]
    fn the_generated_tag_and_language_files_are_the_exact_documents_pinned_here() {
        let generated = ContentMod::new().generated().expect("valid declaration");

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
            let tag = ContentMod::lanterns(kind).expect("valid tag");
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
        let first = content.generated().expect("declared data generates");
        let second = content.generated().expect("declared data generates");

        assert_eq!(first, second);
        for (left, right) in first.files().iter().zip(second.files()) {
            assert_eq!(left.path(), right.path());
            assert_eq!(left.bytes(), right.bytes());
        }
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
        assert!(
            content
                .saved_data_declaration()
                .expect("valid declaration")
                .is_none()
        );
        assert!(
            content
                .save_saved_data()
                .expect("a stateless mod produces no envelope")
                .is_none()
        );
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(ContentMod::default());
