//! The four settings of §3, and the one document they live in.
//!
//! The original declares two documents and populates one. §3 asks a counterpart
//! to declare one, because an empty second document is a file a pack author can
//! find, edit and be ignored by, so that is what is declared here.
//!
//! All four are read on the server. The original reads two of them once, when
//! the machine's block entity is built, and two of them on every use; editing
//! the buffer capacity or the accept rate therefore leaves a loaded machine
//! unchanged until it is reloaded. §3 says a counterpart that resolves all four
//! uniformly differs in one direction only, by being more responsive, and asks
//! for the divergence to be recorded rather than for the split to be
//! reproduced. [`Settings`] resolves all four together.

use ravel_sdk::ModError;
use ravel_sdk::config::{
    AccessError, ConfigurationDeclaration, DeclarationError, LayeredConfiguration, SettingSide,
};

/// The key the ceiling on completed upgrade steps is written under.
pub const UPGRADE_CEILING: &str = "infusing.upgrade_ceiling";
/// The key the price of one tick of infusing is written under.
pub const ENERGY_PER_TICK: &str = "infusing.energy_per_tick";
/// The key the machine's buffer capacity is written under.
pub const ENERGY_CAPACITY: &str = "infusing.energy_capacity";
/// The key the machine's accept rate is written under.
pub const ENERGY_ACCEPTED: &str = "infusing.energy_accepted_per_tick";

/// The declaration this counterpart packages.
///
/// Every setting is read on a server and none is synchronised to a joining
/// client. Ravel's four sides carry no value meaning "either server", and the
/// machine of §7 runs on the integrated server as well as on a standalone one,
/// so the document declares `common` rather than one of the two server names.
/// Nothing here reaches the client either way, because Ravel synchronises no
/// setting at all. The three energy figures accept zero where the ceiling does
/// not, which is the range §3 states for each.
pub const DOCUMENT: &str = r#"{
  "schemaVersion": 1,
  "owner": "rvl_025_counterpart",
  "version": 1,
  "settings": [
    {
      "key": "infusing.upgrade_ceiling",
      "title": "Upgrade steps to full infusion",
      "description": "How many completed steps count as a fully infused machine.",
      "valueSchema": { "type": "integer", "minimum": 1.0, "maximum": 2147483647.0 },
      "default": 256,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "infusing.energy_per_tick",
      "title": "Energy consumed per tick while infusing",
      "description": "The undiscounted price of one tick, before the machine's own infusion is applied.",
      "valueSchema": { "type": "integer", "minimum": 0.0, "maximum": 2147483647.0 },
      "default": 600,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "infusing.energy_capacity",
      "title": "Energy buffer capacity",
      "description": "How much the machine's own buffer holds.",
      "valueSchema": { "type": "integer", "minimum": 0.0, "maximum": 2147483647.0 },
      "default": 60000,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "infusing.energy_accepted_per_tick",
      "title": "Energy accepted per tick",
      "description": "How much the machine's buffer takes in during one tick.",
      "valueSchema": { "type": "integer", "minimum": 0.0, "maximum": 2147483647.0 },
      "default": 600,
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
/// Returns whatever [`ConfigurationDeclaration::parse`] refuses, which for a
/// packaged constant is a packaging failure rather than a runtime one.
pub fn declaration() -> Result<ConfigurationDeclaration, DeclarationError> {
    ConfigurationDeclaration::parse(DOCUMENT)
}

/// The values in force for one launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    /// How many completed steps count as fully infused.
    pub upgrade_ceiling: i32,
    /// The undiscounted price of one tick of infusing.
    pub energy_per_tick: i32,
    /// How much the machine's buffer holds.
    pub energy_capacity: i32,
    /// How much the buffer takes in during one tick.
    pub energy_accepted: i32,
}

impl Settings {
    /// Resolves the packaged defaults without panicking on a packaging failure.
    pub fn shipped() -> Result<Self, ModError> {
        ravel_sdk::config::read_shipped(DOCUMENT, Self::read)
    }

    /// Reads all four validated integer settings together for one launch.
    ///
    /// Returns a typed access error for a missing key, wrong kind or out-of-range integer.
    pub fn read(resolved: &LayeredConfiguration) -> Result<Self, AccessError> {
        Ok(Self {
            upgrade_ceiling: resolved.integer_in(UPGRADE_CEILING)?,
            energy_per_tick: resolved.integer_in::<i32>(ENERGY_PER_TICK)?,
            energy_capacity: resolved.integer_in::<i32>(ENERGY_CAPACITY)?,
            energy_accepted: resolved.integer_in::<i32>(ENERGY_ACCEPTED)?,
        })
    }
}

/// Returns the side every setting in this document is declared on.
///
/// One document, one side. The check exists because §3's whole point about
/// configuration is that nothing here is a client's to resolve, and a setting
/// that drifted to a client side would be one a client screen offered to edit.
///
/// # Errors
///
/// Returns whatever [`declaration`] refuses.
pub fn declared_sides() -> Result<Vec<SettingSide>, DeclarationError> {
    Ok(declaration()?
        .settings()
        .iter()
        .map(|setting| setting.side)
        .collect())
}

#[cfg(test)]
mod tests {
    use ravel_sdk::config::{ValueDocument, ValueError};
    use ravel_sdk::json;

    #[test]
    fn the_declaration_refuses_wrong_types_and_integer_overflow() {
        for value in [json!("600"), json!(i64::from(i32::MAX) + 1)] {
            let written = ValueDocument::parse(
                &json!({
                    "schemaVersion": 1,
                    "owner": crate::MOD_ID,
                    "values": { ENERGY_CAPACITY: value },
                })
                .to_string(),
            )
            .expect("the override document is well formed");
            assert!(
                LayeredConfiguration::resolve(
                    declaration().expect("the shipped document is legal"),
                    Some(&written),
                    None,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn typed_integer_reads_preserve_zero_and_i32_boundary_values() {
        let written = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": crate::MOD_ID,
                "values": {
                    UPGRADE_CEILING: 1,
                    ENERGY_PER_TICK: 0,
                    ENERGY_CAPACITY: i32::MAX,
                    ENERGY_ACCEPTED: i32::MAX,
                },
            })
            .to_string(),
        )
        .expect("the override document is well formed");
        let resolved = LayeredConfiguration::resolve(
            declaration().expect("the shipped document is legal"),
            Some(&written),
            None,
        )
        .expect("the boundary values resolve");
        assert_eq!(
            Settings::read(&resolved).expect("the declared integers read"),
            Settings {
                upgrade_ceiling: 1,
                energy_per_tick: 0,
                energy_capacity: i32::MAX,
                energy_accepted: i32::MAX,
            }
        );
    }

    #[test]
    fn a_configuration_that_resolves_cleanly_reads_the_shipped_defaults() {
        let declaration = declaration().expect("the shipped document is legal");
        let resolved = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the shipped defaults resolve");
        assert_eq!(
            Settings::read(&resolved),
            Ok(Settings::shipped().expect("valid settings"))
        );
    }

    use super::*;

    #[test]
    fn the_document_declares_four_settings_and_every_one_of_them_on_a_server() {
        let sides = declared_sides().expect("the packaged document parses");
        assert_eq!(sides.len(), 4);
        assert!(
            sides.iter().all(|side| *side == SettingSide::Common),
            "one document, one side, and no setting a client resolves on its own"
        );
    }

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        for missing in [
            UPGRADE_CEILING,
            ENERGY_PER_TICK,
            ENERGY_CAPACITY,
            ENERGY_ACCEPTED,
        ] {
            let changed = DOCUMENT.replace(missing, "infusing.renamed");
            let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
            let resolved = LayeredConfiguration::resolve(declared, None, None)
                .expect("the changed declaration resolves");
            assert!(Settings::read(&resolved).is_err());
        }
    }

    #[test]
    fn a_widened_declaration_cannot_silently_default_an_out_of_range_integer() {
        let changed = DOCUMENT
            .replace("2147483647.0", "4294967295.0")
            .replace("\"default\": 60000", "\"default\": 2147483648");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the widened declaration resolves");
        assert_eq!(
            Settings::read(&resolved),
            Err(AccessError {
                key: ENERGY_CAPACITY.to_owned(),
                reason: ValueError::TargetRange { target: "i32" },
            })
        );
        let changed = DOCUMENT
            .replace("\"minimum\": 0.0", "\"minimum\": -4294967295.0")
            .replace("\"default\": 60000", "\"default\": -2147483649");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the widened declaration resolves");
        assert_eq!(
            Settings::read(&resolved),
            Err(AccessError {
                key: ENERGY_CAPACITY.to_owned(),
                reason: ValueError::TargetRange { target: "i32" },
            })
        );
    }

    #[test]
    fn every_declared_key_is_one_the_reader_asks_for() {
        let declaration = declaration().expect("the packaged document parses");
        let declared: Vec<String> = declaration
            .settings()
            .iter()
            .map(|setting| setting.key.to_string())
            .collect();
        for key in [
            UPGRADE_CEILING,
            ENERGY_PER_TICK,
            ENERGY_CAPACITY,
            ENERGY_ACCEPTED,
        ] {
            assert!(
                declared.iter().any(|name| name == key),
                "the reader asks for {key} and the document declares it"
            );
        }
    }
}
