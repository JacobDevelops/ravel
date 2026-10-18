#![forbid(unsafe_code)]

//! A library provider fixture exercising mod-facing declarations.
//! It ships no library and performs no launch-set resolution or loading.

use ravel_sdk::provision::Provided;
use ravel_sdk::{LifecycleState, Mod, ModError, Version, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "provision";

/// The library this mod supplies to other mods.
pub const LIBRARY: &str = "shared_maths";

/// The version it supplies that library at.
pub const VERSION: &str = "2.1.0";

/// A mod declaring one shared library for host-side provision resolution.
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

    /// Returns the shared-library declaration supplied by this mod.
    #[must_use]
    pub fn provided() -> Provided {
        Provided::new(LIBRARY, Version::new(2, 1, 0))
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
    use ravel_sdk::{Declared, Version};

    use super::{LIBRARY, MOD_ID, ProvisionMod, VERSION};

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(ProvisionMod::declared_id().as_str(), MOD_ID);
    }

    #[test]
    fn what_this_mod_declares_is_what_it_provides() {
        let declared = ProvisionMod::provided();
        assert_eq!(declared.library(), LIBRARY);
        assert_eq!(
            declared.version(),
            &Version::parse(VERSION).expect("a valid version")
        );
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(ProvisionMod::new());
