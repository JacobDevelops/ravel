#![forbid(unsafe_code)]

//! Registry and content test mod.

use ravel_core::{ContentId, ContentKind, LifecycleState, ModId, RegistryKey};
use ravel_host_api::{HostError, HostMod, Registrar};

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

/// A mod that registers one block, one item, and one recipe.
///
/// `ROADMAP.md`'s M3-W1 row asks for content registration to be shown on real
/// test mods and for duplicate or conflicting claims to fail. This mod is the
/// success half: it claims three identifiers in its own namespace and records
/// what the loader accepted. The refusal half lives in
/// `ravel-test-mod-failure-cases`, because a mod that both succeeds and fails
/// proves neither cleanly.
///
/// Like `ravel-test-mod-noop`, this is a plain Rust value the loader owns and
/// it names [`HostMod`] directly. How a mod's code arrives in the process is
/// ADG-2's question, and the SDK facade that will hide this trait is M4-W1's.
///
/// # Examples
///
/// ```
/// use ravel_core::ContentKind;
/// use ravel_test_mod_registries::RegistriesMod;
///
/// let registries = RegistriesMod::new();
/// assert_eq!(registries.claims().len(), 3);
///
/// // The block and the item share a name and are still two claims.
/// let block = registries.key(ContentKind::Block, "copper");
/// let item = registries.key(ContentKind::Item, "copper");
/// assert_ne!(block, item);
/// assert_eq!(block.id().to_string(), "registries:copper");
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistriesMod {
    id: ModId,
    registered: Vec<RegistryKey>,
}

impl RegistriesMod {
    /// Builds the mod instance the loader drives.
    ///
    /// # Panics
    ///
    /// Panics if [`MOD_ID`] stops being a legal mod id, which would mean this
    /// mod can no longer be packaged under the identity its manifest declares.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: MOD_ID.parse().expect("`MOD_ID` is a legal mod id"),
            registered: Vec::new(),
        }
    }

    /// Returns the keys this mod claims, in claim order.
    #[must_use]
    pub fn claims(&self) -> Vec<RegistryKey> {
        CLAIMS
            .into_iter()
            .map(|(kind, path)| self.key(kind, path))
            .collect()
    }

    /// Builds one key of `kind` at `path` inside this mod's own namespace.
    ///
    /// # Panics
    ///
    /// Panics if `path` is not a legal [`ContentId`] path. Every caller in this
    /// crate passes a constant from [`CLAIMS`], and a test asserts they parse.
    #[must_use]
    pub fn key(&self, kind: ContentKind, path: &str) -> RegistryKey {
        RegistryKey::new(
            kind,
            ContentId::new(self.id.clone(), path).expect("a legal content path"),
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

impl HostMod for RegistriesMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    /// Accepts every step and does nothing with it.
    ///
    /// # Errors
    ///
    /// Never returns an error. Everything this mod does happens in
    /// [`HostMod::register`].
    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    /// Claims one block, one item, and one recipe.
    ///
    /// A refusal ends registration rather than skipping the identifier. Content
    /// this mod could not claim is content it cannot then use, so carrying on
    /// would leave a mod running against a registry it does not own, and
    /// `SPEC.md` §9.2 wants the conflict reported rather than absorbed.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Refused`] naming the identifier the loader would
    /// not give it and why.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
        for key in self.claims() {
            registrar
                .claim(key.clone())
                .map_err(|refused| HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                    detail: refused.to_string(),
                })?;
            self.registered.push(key);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentKind, ModId, RegistryKey};
    use ravel_host_api::{HostMod, Registrar, RegistrationRefused};

    use super::{CLAIMS, MOD_ID, RegistriesMod};

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
    }

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let registries = RegistriesMod::new();
        assert_eq!(
            registries.id(),
            &MOD_ID.parse::<ModId>().expect("a legal mod id")
        );
        assert_eq!(RegistriesMod::default(), registries);
        assert!(registries.registered().is_empty());
    }

    #[test]
    fn every_declared_claim_is_a_legal_key_in_this_mods_own_namespace() {
        let registries = RegistriesMod::new();
        let claims = registries.claims();

        assert_eq!(claims.len(), CLAIMS.len());
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
