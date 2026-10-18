//! What this package registers, from §4.
//!
//! The census is small and two of its rows are the kind a parity test finds and
//! a reader does not expect.
//!
//! The original ships one block-state document naming a model for a multi-part
//! block that is **not registered** at the reviewed revision: both the block and
//! its model loader are present and switched off, so the document is shipped,
//! loadable and unreachable. It is not shipped here.
//!
//! The original's language file holds three keys, one of which is a comment
//! marker rather than a translation. There are **two** real keys and a
//! language-parity run should expect two.

use ravel_sdk::content::{ContentDeclaration, ContentDeclarationError, ContentKind, RegistryKey};
use ravel_sdk::{Declared, ModError};

/// The one crafting recipe kind this package claims.
pub const RECIPE_KIND: &str = "shaped_transferring_chosen_data";

/// The two per-owner data attachments, by the role each plays.
///
/// These are role names this package chose. The identifiers the original
/// registers them under are upstream names the specification does not carry, so
/// nothing here is save-compatible with it and nothing pretends to be.
pub const ATTACHMENTS: [&str; 2] = ["player_display_preferences", "block_common_data"];

/// The seven item data component kinds, by the role each plays.
///
/// All seven are persistent and network-synchronised. They are the vocabulary
/// dependents store on their own items, which is why a registry-parity run
/// measures them.
pub const COMPONENTS: [&str; 7] = [
    "block_common_data",
    "upgrade_infusion_level",
    "stored_energy",
    "inventory",
    "fluid_content",
    "security_setting",
    "module_payload",
];

/// The two real translatable keys: the binding's category and the binding.
pub const TRANSLATIONS: [&str; 2] = [
    "key.category.rvl_120_counterpart",
    "key.rvl_120_counterpart.open_manual",
];

/// The one interface texture this package supplies.
pub const TEXTURE: &str = "textures/gui/widgets.png";

/// How many block-state documents are shipped.
///
/// One is counted in the original's census and none is shipped, because the one
/// it counts names a model for a block nothing registers.
pub const BLOCK_STATE_DOCUMENTS: usize = 0;

/// How many recipe, tag, loot and world-generation documents are shipped.
///
/// Zero of all four kinds, which is why the parity tests of this package's
/// dependents for those kinds can only ever be about their own content.
pub const DATA_DOCUMENTS: usize = 0;

/// Builds what this package claims at the registration barrier.
///
/// One recipe kind is a registry entry Ravel has a kind for. The attachments and
/// the component kinds are not: Ravel publishes no attachment registry and no
/// item-data-component registry at the reviewed revision, so those nine names
/// are carried as data and `NOTES.md` records them as declarable only.
///
/// # Errors
///
/// Returns [`ContentDeclarationError`] when a declared path is not a legal content
/// path, which for these constants is a packaging failure.
pub fn declaration() -> Result<ContentDeclaration, ContentDeclarationError> {
    let mut declaration = ContentDeclaration::new(crate::Entry::declared_id().clone());
    declaration.entry(ContentKind::Recipe, RECIPE_KIND)?;
    Ok(declaration)
}

/// Returns the one key this package claims.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when the declared path is not a legal content
/// path.
pub fn claimed() -> Result<Vec<RegistryKey>, ModError> {
    let declaration = declaration().map_err(ModError::from_display)?;
    Ok(declaration.entries().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_claims_exactly_one_recipe_kind_and_no_other_registry_entry() {
        let claimed = claimed().expect("the declared path is legal");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].kind(), ContentKind::Recipe);
        assert_eq!(claimed[0].id().path(), RECIPE_KIND);
        assert_eq!(
            claimed[0].id().to_string(),
            format!("{}:{RECIPE_KIND}", crate::MOD_ID)
        );
    }

    #[test]
    fn the_census_counts_two_attachments_and_seven_component_kinds() {
        assert_eq!(ATTACHMENTS.len(), 2);
        assert_eq!(COMPONENTS.len(), 7);

        let mut named: Vec<&str> = COMPONENTS.to_vec();
        named.sort_unstable();
        named.dedup();
        assert_eq!(named.len(), COMPONENTS.len());
    }

    #[test]
    fn the_common_block_data_is_the_one_role_registered_twice() {
        assert!(ATTACHMENTS.contains(&"block_common_data"));
        assert!(COMPONENTS.contains(&"block_common_data"));
    }

    #[test]
    fn the_counted_documents_are_the_ones_the_declaration_actually_claims() {
        // The two counts are held against the declaration rather than against
        // their own literals: raising either without shipping the document it
        // counts is what this has to notice.
        let claimed = claimed().expect("the declared path is legal");
        assert_eq!(
            claimed.len(),
            1 + BLOCK_STATE_DOCUMENTS + DATA_DOCUMENTS,
            "the one recipe kind is everything this package claims"
        );
        assert!(
            claimed.iter().all(|key| key.kind() == ContentKind::Recipe),
            "no block-state, tag, loot or world-generation document is claimed"
        );
    }

    #[test]
    fn the_language_file_holds_two_real_keys_and_not_three() {
        assert_eq!(
            TRANSLATIONS,
            [
                format!("key.category.{}", crate::MOD_ID),
                format!("key.{}.{}", crate::MOD_ID, crate::KEY_BINDING),
            ],
            "the two real keys are the binding's category and the binding itself"
        );
        assert!(TRANSLATIONS.iter().all(|key| !key.starts_with('#')));
    }

    #[test]
    fn the_package_ships_one_texture_and_it_is_an_interface_texture() {
        assert_eq!(
            std::path::Path::new(TEXTURE).extension(),
            Some(std::ffi::OsStr::new("png"))
        );
        assert!(TEXTURE.starts_with("textures/gui/"));
    }
}
