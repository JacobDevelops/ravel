#![forbid(unsafe_code)]

//! Registry and content test mod.

use std::borrow::Cow;
use std::sync::LazyLock;

use ravel_sdk::content::{
    BlockBehaviour, ContentBehaviour, ContentDeclaration, ContentId, ContentKind, ItemBehaviour,
    Registrar, RegistryKey,
};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "registries";

/// The paths this mod claims, one per [`ContentKind`].
///
/// A block and an item deliberately share the path `copper`. That is the case
/// a barrier keyed on the name alone gets wrong, and the one `ravel-core`'s
/// [`RegistryKey`] exists to keep apart, so the mod that proves registration
/// works is the one that exercises it.
const CLAIMS: [(ContentKind, &str); 3] = [
    (ContentKind::Block, "copper"),
    (ContentKind::Item, "copper"),
    (ContentKind::Recipe, "smelting/copper"),
];

// At least one real admitted legacy block must reach ID 512: its packed state
// ID is 8192, beyond the unmodified thirteen-bit global palette.
const PALETTE_BLOCKS: usize = 257;

static CONTENT: LazyLock<Result<ContentDeclaration, ModError>> = LazyLock::new(|| {
    let mut declaration = ContentDeclaration::new(RegistriesMod::declared_id().clone());
    let path = "assets/registries/recipes/probe.json"
        .to_owned()
        .try_into()
        .map_err(ModError::from_display)?;
    declaration
        .resource(
            path,
            include_bytes!("../assets/registries/recipes/probe.json").to_vec(),
        )
        .map_err(ModError::from_display)?;
    Ok(declaration)
});

/// How this mod's content behaves, or nothing for a kind with no value.
///
/// Deliberately not the defaults. A test mod that described what it would have
/// got anyway proves that a description arrived only by proving nothing: the
/// registered value would look identical if every field were dropped in transit.
const fn behaviour(kind: ContentKind) -> Option<ContentBehaviour> {
    match kind {
        ContentKind::Block => Some(ContentBehaviour::Block(BlockBehaviour {
            hardness: 50.0,
            explosion_resistance: 1200.0,
            requires_correct_tool: true,
            random_ticks: false,
        })),
        ContentKind::Item => Some(ContentBehaviour::Item(ItemBehaviour {
            stack_size: 16,
            fire_resistant: true,
        })),
        ContentKind::Recipe | ContentKind::Command => None,
    }
}

/// A mod that registers palette-proof blocks, one item, and one recipe.
///
/// `ROADMAP.md`'s M3-W1 row asks for content registration to be shown on real
/// test mods and for duplicate or conflicting claims to fail. This mod is the
/// success half: it claims identifiers in its own namespace and records
/// what the loader accepted. Refusal coverage lives in this crate's
/// `a_refused_claim_stops_registration_and_names_the_identifier` test and the
/// registration-barrier tests in `ravel-loader`.
///
/// # Examples
///
/// ```
/// use ravel_sdk::content::ContentKind;
/// use ravel_test_mod_registries::RegistriesMod;
///
/// let registries = RegistriesMod::new();
/// assert_eq!(registries.claims().len(), 260);
///
/// // The block and the item share a name and are still two claims.
/// let block = registries.key(ContentKind::Block, "copper");
/// let item = registries.key(ContentKind::Item, "copper");
/// assert_ne!(block, item);
/// assert_eq!(block.id().to_string(), "registries:copper");
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistriesMod {
    registered: Vec<RegistryKey>,
}

impl RegistriesMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registered: Vec::new(),
        }
    }

    /// Returns the keys this mod claims, in claim order.
    #[must_use]
    pub fn claims(&self) -> Vec<RegistryKey> {
        CLAIMS
            .into_iter()
            .map(|(kind, path)| self.key(kind, path))
            .chain(
                (0..PALETTE_BLOCKS)
                    .map(|index| self.key(ContentKind::Block, &format!("palette/{index:03}"))),
            )
            .collect()
    }

    /// Builds one key of `kind` at `path` inside this mod's own namespace.
    ///
    /// # Panics
    ///
    /// Panics if `path` is not a legal [`ContentId`] path. Callers use `CLAIMS`
    /// or generated palette paths, and a test asserts that every claim parses.
    #[must_use]
    pub fn key(&self, kind: ContentKind, path: &str) -> RegistryKey {
        RegistryKey::new(
            kind,
            ContentId::new(Self::declared_id().clone(), path).expect("a legal content path"),
        )
    }

    /// Returns the keys the loader accepted, in the order it accepted them.
    #[must_use]
    pub fn registered(&self) -> &[RegistryKey] {
        &self.registered
    }
}

impl Default for RegistriesMod {
    fn default() -> Self {
        Self::new()
    }
}

#[declare_mod(id = "registries")]
impl Mod for RegistriesMod {
    fn content_declaration(&self) -> Result<Option<Cow<'_, ContentDeclaration>>, ModError> {
        CONTENT
            .as_ref()
            .map(|declaration| Some(Cow::Borrowed(declaration)))
            .map_err(Clone::clone)
    }

    /// Accepts every step and does nothing with it.
    ///
    /// # Errors
    ///
    /// Never returns an error. Everything this mod does happens in
    /// [`Mod::register`].
    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    /// Claims the base identifiers and the high-ID palette-proof blocks.
    ///
    /// A refusal ends registration rather than skipping the identifier. Content
    /// this mod could not claim is content it cannot then use, so carrying on
    /// would leave a mod running against a registry it does not own, and
    /// `SPEC.md` §9.2 wants the conflict reported rather than absorbed.
    ///
    /// # Errors
    ///
    /// Returns a [`ModError`] naming the identifier the loader would
    /// not give it and why.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        for key in self.claims() {
            // Described where there is something to describe, and claimed plainly
            // where there is not. A recipe names no value a registry builds, so
            // there is nothing about it to say.
            let claimed = match behaviour(key.kind()) {
                Some(behaviour) => registrar.claim_describing(key.clone(), behaviour),
                None => registrar.claim(key.clone()),
            };
            claimed.map_err(ModError::from_display)?;
            self.registered.push(key);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::{ContentKind, Registrar, RegistrationRefused, RegistryKey};
    use ravel_sdk::{Mod, ModId};

    use super::{CLAIMS, MOD_ID, PALETTE_BLOCKS, RegistriesMod};

    /// A registrar that accepts everything, so a test sees what was asked for.
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

    /// A registrar that refuses the `kind` it was built with.
    struct Refusing {
        kind: ContentKind,
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Refusing {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            if key.kind() == self.kind {
                return Err(RegistrationRefused::Conflict {
                    key,
                    owner: "other_mod".parse().expect("a legal mod id"),
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
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let registries = RegistriesMod::new();
        assert_eq!(
            registries.id(),
            &MOD_ID.parse::<ModId>().expect("a legal mod id")
        );
        assert_eq!(RegistriesMod::default(), registries);
        assert_eq!(registries.registered(), []);
    }

    #[test]
    fn every_declared_claim_is_a_legal_key_in_this_mods_own_namespace() {
        let registries = RegistriesMod::new();
        let claims = registries.claims();

        assert_eq!(claims.len(), CLAIMS.len() + PALETTE_BLOCKS);
        for key in &claims {
            assert_eq!(key.id().namespace().as_str(), MOD_ID);
            // The canonical string form is what the barrier stores, so it has
            // to survive a round trip.
            assert_eq!(
                key.to_string().parse::<RegistryKey>().expect("a legal key"),
                *key
            );
        }
    }

    #[test]
    fn a_block_and_an_item_sharing_a_path_are_two_distinct_claims() {
        let registries = RegistriesMod::new();
        let block = registries.key(ContentKind::Block, "copper");
        let item = registries.key(ContentKind::Item, "copper");

        assert_ne!(block, item);
        assert_eq!(block.id(), item.id());
    }

    #[test]
    fn an_accepted_registration_records_exactly_what_was_claimed_in_order() {
        let mut registries = RegistriesMod::new();
        let mut registrar = Accepting::default();

        registries
            .register(&mut registrar)
            .expect("nothing refuses this mod");

        let expected = RegistriesMod::new().claims();
        assert_eq!(registrar.claimed(), expected);
        assert_eq!(registries.registered(), expected);
    }

    #[test]
    fn a_refused_claim_stops_registration_and_names_the_identifier() {
        // The kinds this mod claims, not every kind that exists. A command is a
        // `ContentKind` this mod deliberately does not register, and iterating
        // the whole enum would assert that a refusal this mod never provokes
        // stops a registration it never starts.
        for (kind, _) in CLAIMS {
            let mut registries = RegistriesMod::new();
            let mut registrar = Refusing {
                kind,
                claimed: Vec::new(),
            };

            let error = registries
                .register(&mut registrar)
                .expect_err("one kind is refused");
            let text = error.to_string();
            assert!(text.contains("other_mod"), "{text}");
            assert!(text.contains(kind.as_str()), "{text}");

            // Nothing after the refused claim was attempted, and the mod
            // records only what the loader actually accepted.
            let refused_at = RegistriesMod::new()
                .claims()
                .iter()
                .position(|key| key.kind() == kind)
                .expect("every kind is claimed");
            assert_eq!(registries.registered().len(), refused_at);
            assert_eq!(registrar.claimed().len(), refused_at);
        }
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(RegistriesMod::new());
