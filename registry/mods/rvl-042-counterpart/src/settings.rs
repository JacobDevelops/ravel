//! What a player and a pack may change, and what the values in force mean.
//!
//! The original declares two documents under one grouping, one client-scoped and
//! one common-scoped, and neither is synchronised to a joining client. Ravel
//! resolves one declaration per mod with a side on each setting, so the two
//! documents are one declaration here and the client/common split is carried by
//! the side rather than by the file. Nothing here is sent over the network,
//! which is what preserves the original's arrangement.
//!
//! The preference order is inert in the original: nothing reads it. It is live
//! here, which is the declared behaviour change [`crate::selection`] describes.

use ravel_sdk::Side;
use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, SettingSide, ValueError,
};

use crate::selection::Preference;

/// The key the tag listing in a tooltip is turned on and off under.
pub const TAG_TOOLTIP: &str = "tooltip.tags";
/// The key the entry-data listing in a tooltip is turned on and off under.
pub const DATA_TOOLTIP: &str = "tooltip.entry_data";
/// The key the namespace preference order is written under.
pub const PREFERENCE: &str = "selection.preference";
/// The key the self-heal behaviour is turned on and off under.
pub const SELF_HEAL: &str = "selection.self_heal";

/// The declaration this counterpart packages.
///
/// The preference order's shipped default is empty. The original's default names
/// five namespaces belonging to other projects; naming them here would carry
/// their identity into this package, and an empty order leaves every namespace
/// unlisted, which the selection rule orders by identifier.
pub const DOCUMENT: &str = r#"{
  "schemaVersion": 1,
  "owner": "rvl_042_counterpart",
  "version": 1,
  "settings": [
    {
      "key": "tooltip.tags",
      "title": "List an entry's tags",
      "description": "Adds every tag an entry belongs to under an advanced tooltip.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "client",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "tooltip.entry_data",
      "title": "List an entry's data",
      "description": "Adds every entry-level data value under an advanced tooltip.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "client",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "selection.preference",
      "title": "Namespace preference order",
      "description": "Namespaces, most preferred first and comma separated, used to pick the entry a tag stands for.",
      "valueSchema": { "type": "string" },
      "default": "",
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "selection.self_heal",
      "title": "Re-choose entries that no longer exist",
      "description": "Drops a recorded entry the registry no longer holds and chooses again.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    }
  ]
}"#;

/// Reads the packaged declaration.
///
/// # Errors
///
/// Returns whatever [`ConfigurationDeclaration::parse`] refuses, which for the
/// packaged document is a compile-time constant and so a packaging failure
/// rather than a runtime one.
pub fn declaration() -> Result<ConfigurationDeclaration, DeclarationError> {
    ConfigurationDeclaration::parse(DOCUMENT)
}

/// The values in force for one launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Whether a tooltip lists the tags an entry belongs to.
    pub tag_tooltip: bool,
    /// Whether a tooltip lists an entry's own data values.
    pub data_tooltip: bool,
    /// The namespace preference order the selection rule honours.
    pub preference: Preference,
    /// Whether a recorded entry the registry no longer holds is chosen again.
    pub self_heal: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let resolved = LayeredConfiguration::resolve(
            declaration().expect("the packaged declaration is valid"),
            None,
            None,
        )
        .expect("the shipped defaults resolve");
        Self::read(&resolved).expect("declared settings have matching types")
    }
}

impl Settings {
    /// Reads the values in force out of a resolved configuration.
    ///
    /// Returns a typed access error if a key is missing or has the wrong kind.
    pub fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            tag_tooltip: resolved.boolean(TAG_TOOLTIP)?,
            data_tooltip: resolved.boolean(DATA_TOOLTIP)?,
            preference: Preference::parse(resolved.text(PREFERENCE)?),
            self_heal: resolved.boolean(SELF_HEAL)?,
        })
    }
}

/// Whether any shipped setting would have to travel to a joining client.
///
/// A setting a side resolves for itself never travels. One declared on a server
/// side alone would have to, because a client that reads it holds no value of
/// its own; every setting here is declared client or common, so none does.
///
/// That nothing travels is deliberate. Selection decides what a recipe produces,
/// so two sides that resolved a tag differently would disagree; what makes that
/// safe is that a result travels already resolved, so the serialising side is
/// authoritative and the other takes the answer rather than choosing again. It
/// matters more here than in the original, where the settings could not produce
/// a difference because neither side read them.
#[must_use]
pub fn travels_to_clients() -> bool {
    declaration().is_ok_and(|declaration| {
        declaration.settings().iter().any(|setting| {
            matches!(
                setting.side,
                SettingSide::DedicatedServer | SettingSide::IntegratedServer
            )
        })
    })
}

/// Whether a setting is one a given side resolves at all.
///
/// The answer is read off the shipped declaration rather than restated, so a
/// side edited in [`DOCUMENT`] moves this with it.
#[must_use]
pub fn resolved_on(key: &str, side: Side) -> bool {
    let Ok(key) = key.parse() else {
        return false;
    };
    declaration().is_ok_and(|declaration| {
        declaration
            .setting(&key)
            .is_some_and(|setting| setting.side.applies_to(side))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        let changed = DOCUMENT.replace(TAG_TOOLTIP, "tooltip.renamed");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the changed declaration resolves");
        assert!(Settings::read(&resolved).is_err());
    }

    #[test]
    fn typed_reads_apply_tooltip_switches_and_borrowed_preference_text() {
        let document = ravel_sdk::config::ValueDocument::parse(
            &ravel_sdk::json!({
                "schemaVersion": 1,
                "owner": crate::MOD_ID,
                "values": {
                    TAG_TOOLTIP: false,
                    DATA_TOOLTIP: true,
                    PREFERENCE: " other, host, other ",
                    SELF_HEAL: false,
                },
            })
            .to_string(),
        )
        .expect("a legal override document");
        let resolved = LayeredConfiguration::resolve(
            declaration().expect("the packaged declaration parses"),
            Some(&document),
            None,
        )
        .expect("the override values resolve");
        let settings = Settings::read(&resolved).expect("declared settings have matching types");
        assert!(!settings.tag_tooltip);
        assert!(settings.data_tooltip);
        assert!(!settings.self_heal);
        assert_eq!(settings.preference.namespaces(), ["other", "host"]);
    }

    #[test]
    fn the_packaged_document_parses_and_declares_four_settings() {
        let declaration = declaration().expect("the packaged document parses");
        for key in [TAG_TOOLTIP, DATA_TOOLTIP, PREFERENCE, SELF_HEAL] {
            let parsed = key.parse().expect("every declared key is legal");
            assert!(
                declaration.setting(&parsed).is_some(),
                "the document declares {key}"
            );
        }
    }

    #[test]
    fn the_shipped_defaults_are_one_tooltip_on_one_off_and_self_heal_on() {
        let shipped = Settings::default();
        assert!(shipped.tag_tooltip);
        assert!(!shipped.data_tooltip);
        assert!(shipped.self_heal);
        assert_eq!(shipped.preference.namespaces(), Vec::<String>::new());
    }

    #[test]
    fn the_two_tooltips_are_client_settings_and_the_two_selection_settings_are_common() {
        assert!(resolved_on(TAG_TOOLTIP, Side::Client));
        assert!(!resolved_on(TAG_TOOLTIP, Side::DedicatedServer));
        assert!(resolved_on(PREFERENCE, Side::DedicatedServer));
        assert!(resolved_on(SELF_HEAL, Side::Client));
        assert!(!resolved_on("nothing.declares.this", Side::Client));
    }

    #[test]
    fn every_shipped_setting_is_one_the_reading_side_resolves_for_itself() {
        let declaration = declaration().expect("the packaged document parses");
        for setting in declaration.settings() {
            assert!(
                matches!(setting.side, SettingSide::Client | SettingSide::Common),
                "{} is declared on a side a client cannot resolve for itself",
                setting.key
            );
        }
        assert!(
            !travels_to_clients(),
            "a result travels already resolved, so no setting has to"
        );
    }
}
