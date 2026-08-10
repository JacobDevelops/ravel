#![forbid(unsafe_code)]

//! Library provision test mod.
//!
//! The mod for [`ravel_sdk::provision`]. It declares that it provides one shared
//! library at one version, and resolves that declaration against another mod's
//! requirement, which is what `ROADMAP.md`'s general SDK gap row asks an original
//! mod to exercise.
//!
//! It ships no library and loads nothing. A library two mods build on is an
//! ordinary Cargo dependency under ADG-2; what a declaration decides is which
//! *mod* supplies it to the set and at what version, and nothing in a running
//! game reads one yet. The SDK module says so to the author in the same words.

use ravel_sdk::provision::{Provided, Provision, ProvisionRefused, Resolved};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, VersionRange, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "provision";

/// The library this mod supplies to other mods.
pub const LIBRARY: &str = "shared_maths";

/// The version it supplies that library at.
pub const VERSION: &str = "2.1.0";

/// A mod that provides one library and resolves it against a consumer.
///
/// # Examples
///
/// ```
/// use ravel_sdk::{Declared, ModId, VersionRange};
/// use ravel_test_mod_provision::{LIBRARY, ProvisionMod, VERSION};
///
/// let consumer: ModId = "consumer_mod".parse()?;
/// let resolved = ProvisionMod::supplying(&consumer, VersionRange::parse("^2")?)
///     .expect("this mod's version answers the range");
///
/// assert_eq!(resolved.provider_of(LIBRARY), Some(ProvisionMod::declared_id()));
/// assert_eq!(resolved.version_of(LIBRARY), Some(VERSION));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProvisionMod {
    entered: Vec<LifecycleState>,
}

impl ProvisionMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the states this mod was taken through, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Declares what this mod provides into `provision`.
    pub fn provide(provision: &mut Provision) {
        provision.provides(Self::declared_id(), Provided::new(LIBRARY, VERSION));
    }

    /// Resolves this mod's library against `consumer` needing `wanted`.
    ///
    /// # Errors
    ///
    /// Returns [`ProvisionRefused::Unsatisfied`] when [`VERSION`] is outside
    /// `wanted`, naming the consumer. Every other refusal needs a second claim
    /// on the library, which one mod cannot make alone.
    pub fn supplying(consumer: &ModId, wanted: VersionRange) -> Result<Resolved, ProvisionRefused> {
        let mut provision = Provision::default();
        Self::provide(&mut provision);
        provision.requires(consumer, LIBRARY, wanted);
        provision.resolve()
    }

    /// Resolves this mod's library against `rival` claiming to supply it too.
    ///
    /// # Errors
    ///
    /// Returns [`ProvisionRefused::Contested`] naming both mods, because a
    /// launch takes one provider per library.
    pub fn beside(rival: &ModId, version: &str) -> Result<Resolved, ProvisionRefused> {
        let mut provision = Provision::default();
        Self::provide(&mut provision);
        provision.provides(rival, Provided::new(LIBRARY, version));
        provision.resolve()
    }
}

#[declare_mod(id = "provision")]
impl Mod for ProvisionMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::provision::{Provided, Provision, ProvisionRefused};
    use ravel_sdk::{Declared, ModId, VersionRange};

    use super::{LIBRARY, MOD_ID, ProvisionMod, VERSION};

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    fn range(grammar: &str) -> VersionRange {
        VersionRange::parse(grammar).expect("a well formed range")
    }

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(ProvisionMod::declared_id().as_str(), MOD_ID);
    }

    /// The version is read back through the range that accepted it rather than
    /// compared to the constant this mod also declares, so the assertion is
    /// about what the resolution decided.
    #[test]
    fn a_consumer_binds_to_this_mod_at_a_version_inside_the_range_it_asked_for() {
        let consumer = id("consumer_mod");
        let wanted = range(">=2, <3");

        let resolved = ProvisionMod::supplying(&consumer, wanted.clone())
            .expect("this mod's version answers the range");

        let provider = resolved
            .provider_of(LIBRARY)
            .expect("the library resolved to a provider");
        assert_eq!(provider, ProvisionMod::declared_id());

        let bound = resolved
            .version_of(LIBRARY)
            .expect("the provider resolved at a version")
            .parse()
            .expect("the resolved version is a version");
        assert!(wanted.matches(&bound), "{bound}");
    }

    #[test]
    fn a_consumer_that_needs_a_version_this_mod_does_not_supply_is_refused_by_name() {
        let consumer = id("consumer_mod");

        let refusal = ProvisionMod::supplying(&consumer, range("^1"))
            .expect_err("this mod supplies a major version that range excludes");

        assert_eq!(refusal.library(), LIBRARY);
        assert_eq!(refusal.unsatisfied(), [consumer]);
        assert!(!refusal.has_no_provider());
    }

    #[test]
    fn a_second_mod_claiming_this_library_is_refused_naming_both() {
        let rival = id("rival_mod");

        let refusal =
            ProvisionMod::beside(&rival, VERSION).expect_err("one launch takes one provider");

        assert_eq!(
            refusal.providers(),
            [ProvisionMod::declared_id().clone(), rival]
        );
        assert!(matches!(refusal, ProvisionRefused::Contested { .. }));
    }

    #[test]
    fn this_mod_provides_nothing_it_did_not_declare() {
        let mut provision = Provision::default();
        ProvisionMod::provide(&mut provision);

        assert_eq!(
            provision.providers_of(LIBRARY),
            [ProvisionMod::declared_id()]
        );
        assert!(
            provision
                .providers_of("a_library_nobody_declares")
                .is_empty()
        );
    }

    #[test]
    fn a_requirement_on_a_library_this_mod_does_not_supply_is_refused() {
        let consumer = id("consumer_mod");

        let mut provision = Provision::default();
        ProvisionMod::provide(&mut provision);
        provision.requires(&consumer, "a_library_nobody_declares", range("^1"));

        let refusal = provision
            .resolve()
            .expect_err("a library nothing provides is refused before anything runs");

        assert_eq!(refusal.library(), "a_library_nobody_declares");
        assert!(refusal.has_no_provider());
    }

    #[test]
    fn what_this_mod_declares_is_what_it_provides() {
        let declared = Provided::new(LIBRARY, VERSION);
        assert_eq!(declared.library(), LIBRARY);
        assert_eq!(declared.version(), VERSION);
    }
}
