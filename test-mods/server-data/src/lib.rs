#![forbid(unsafe_code)]

//! Declares one entry of each server-resolved data kind through `ravel-sdk`.

use ravel_sdk::content::{ContentDeclaration, ContentDeclarationError};
use ravel_sdk::server_data::ServerDataKind;
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "server_data";

/// What this mod declares, one entry per kind.
///
/// One name shared by two kinds on purpose. That is the case a barrier keyed on
/// the name alone gets wrong, and the one the derived path keeps apart, so the
/// mod that proves server data works is the one that exercises it.
const ENTRIES: [(ServerDataKind, &str, &[u8]); 4] = [
    (
        ServerDataKind::Crafting,
        "plate_from_ingot",
        br#"{"result": "server_data:plate"}"#,
    ),
    (
        ServerDataKind::Loot,
        "plate_from_ingot",
        br#"{"drops": "server_data:plate"}"#,
    ),
    (
        ServerDataKind::Progression,
        "first_plate",
        br#"{"awarded_for": "server_data:plate"}"#,
    ),
    (
        ServerDataKind::Procedure,
        "grant_plate",
        b"give @s server_data:plate\n",
    ),
];

/// A mod whose whole content is server-resolved.
///
/// # Examples
///
/// ```
/// use ravel_test_mod_server_data::ServerDataMod;
///
/// let declared = ServerDataMod::declaration().expect("the declaration is well formed");
/// let generated = declared.generate();
/// assert_eq!(generated.files().len(), 4);
///
/// // The paths are derived, so this mod never spells one.
/// let paths: Vec<String> = generated.files().iter().map(|file| file.path().to_string()).collect();
/// assert!(paths.contains(&"data/server_data/recipe/plate_from_ingot.json".to_owned()));
/// assert!(paths.contains(&"data/server_data/function/grant_plate.mcfunction".to_owned()));
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServerDataMod {
    entered: Vec<LifecycleState>,
}

impl ServerDataMod {
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

    /// Builds this mod's server-data declaration.
    ///
    /// # Errors
    ///
    /// Returns [`ContentDeclarationError`] if any entry is refused, which would mean two
    /// of `ENTRIES` name one file.
    pub fn declaration() -> Result<ContentDeclaration, ContentDeclarationError> {
        let mut declared = ContentDeclaration::new(Self::declared_id().clone());
        for (kind, name, bytes) in ENTRIES {
            declared.server_entry(kind, name, bytes)?;
        }
        Ok(declared)
    }
}

#[declare_mod(id = "server_data")]
impl Mod for ServerDataMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        Self::declaration()
            .map(|declaration| Some(std::borrow::Cow::Owned(declaration)))
            .map_err(ModError::from_display)
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::server_data::ServerDataKind;
    use ravel_sdk::{Declared, LifecycleState, Mod};

    use super::{MOD_ID, ServerDataMod};

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(ServerDataMod::declared_id().as_str(), MOD_ID);
    }

    #[test]
    fn one_name_under_two_kinds_is_two_entries_at_two_paths() {
        let declared = ServerDataMod::declaration().expect("the declaration is well formed");
        let generated = declared.generate();
        let paths: Vec<&str> = generated
            .paths()
            .filter(|path| path.ends_with("/plate_from_ingot.json"))
            .collect();

        assert_eq!(paths.len(), 2);
        assert_ne!(paths[0], paths[1]);
    }

    #[test]
    fn every_kind_this_mod_declares_is_generated_once() {
        let declared = ServerDataMod::declaration().expect("the declaration is well formed");
        let generated = declared.generate();

        for kind in ServerDataKind::ALL {
            let prefix = format!("data/server_data/{}/", kind.directory());
            let count = generated
                .paths()
                .filter(|path| path.starts_with(&prefix))
                .count();
            assert_eq!(count, 1, "{kind}");
        }
        assert_eq!(generated.files().len(), 4);
    }

    #[test]
    fn the_mod_records_every_state_the_loader_takes_it_through() {
        let mut mod_under_test = ServerDataMod::new();
        for state in LifecycleState::ALL {
            mod_under_test
                .enter(state)
                .expect("the mod accepts every step");
        }
        assert_eq!(mod_under_test.entered(), LifecycleState::ALL);
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(ServerDataMod::new());
