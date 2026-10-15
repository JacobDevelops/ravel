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

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, SettingSide,
};

/// The key the ceiling on completed upgrade steps is written under.
pub const UPGRADE_CEILING: &str = "infusing.upgrade_ceiling";
/// The key the price of one tick of infusing is written under.
pub const ENERGY_PER_TICK: &str = "infusing.energy_per_tick";
/// The key the machine's buffer capacity is written under.
pub const ENERGY_CAPACITY: &str = "infusing.energy_capacity";
/// The key the machine's accept rate is written under.
pub const ENERGY_ACCEPTED: &str = "infusing.energy_accepted_per_tick";

/// The shipped default ceiling on completed upgrade steps.
pub const DEFAULT_UPGRADE_CEILING: i32 = 256;
/// The shipped default price of one tick of infusing.
pub const DEFAULT_ENERGY_PER_TICK: i32 = 600;
/// The shipped default buffer capacity.
pub const DEFAULT_ENERGY_CAPACITY: i32 = 60_000;
/// The shipped default accept rate.
pub const DEFAULT_ENERGY_ACCEPTED: i32 = 600;

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

impl Default for Settings {
    fn default() -> Self {
        Self {
            upgrade_ceiling: DEFAULT_UPGRADE_CEILING,
            energy_per_tick: DEFAULT_ENERGY_PER_TICK,
            energy_capacity: DEFAULT_ENERGY_CAPACITY,
            energy_accepted: DEFAULT_ENERGY_ACCEPTED,
        }
    }
}

impl Settings {
    /// Reads the four values off a resolved configuration.
    ///
    /// A value the declaration would have refused never arrives, so a key that
    /// is absent here is a key this build did not declare and the shipped
    /// default stands. All four are read together, which is the divergence §3
    /// asks for rather than the original's split between values read once and
    /// values read on every use. The ceiling is floored at one whatever arrives, because
    /// a ceiling of zero would make every upgradable item unacceptable and the
    /// declared range already excludes it.
    #[must_use]
    pub fn read(resolved: &LayeredConfiguration) -> Self {
        Self::read_reporting(resolved).0
    }

    /// Reads the four values and says which of them were not read.
    ///
    /// A key that is absent is the shipped default and is not reported: this
    /// build declares four settings, so an absent key is a value nobody wrote.
    /// A key that is *present* and unreadable is reported, because that is the
    /// one failure this function exists to catch, and folding it into the
    /// shipped default is how a pack that wrote a value out of range runs on a
    /// number nobody chose and nothing says so.
    ///
    /// The declaration validates first, so this is defence in depth. It is kept
    /// because the layer it defends against is the one where a value arrives
    /// having passed nothing.
    #[must_use]
    pub fn read_reporting(resolved: &LayeredConfiguration) -> (Self, Vec<Unread>) {
        let mut unread = Vec::new();
        let mut whole = |name: &'static str, fallback: i32| -> i32 {
            let Ok(key) = name.parse() else {
                unread.push(Unread {
                    setting: name,
                    why: Why::NotAKey,
                });
                return fallback;
            };
            let (value, refused) = read_whole(name, resolved.value(&key), fallback);
            if let Some(refused) = refused {
                unread.push(refused);
            }
            value
        };
        let settings = Self {
            upgrade_ceiling: whole(UPGRADE_CEILING, DEFAULT_UPGRADE_CEILING).max(1),
            energy_per_tick: whole(ENERGY_PER_TICK, DEFAULT_ENERGY_PER_TICK).max(0),
            energy_capacity: whole(ENERGY_CAPACITY, DEFAULT_ENERGY_CAPACITY).max(0),
            energy_accepted: whole(ENERGY_ACCEPTED, DEFAULT_ENERGY_ACCEPTED).max(0),
        };
        (settings, unread)
    }
}

/// Reads one written value, saying what was wrong with it where something was.
///
/// Separate from [`Settings::read_reporting`] because it is the half that can be
/// driven: the declaration refuses a bad value before a resolved configuration
/// exists, so no resolved configuration can be built carrying one. That is what
/// makes this defence in depth, and it is also why the layer below it is held to
/// its own assertions here rather than through a resolve that cannot produce the
/// input.
#[must_use]
pub fn read_whole(
    setting: &'static str,
    written: Option<&ravel_sdk::JsonValue>,
    fallback: i32,
) -> (i32, Option<Unread>) {
    // An absent key is a value nobody wrote, which is the shipped default and
    // not a failure. This build declares four settings, so absence here means a
    // pack left one alone.
    let Some(written) = written else {
        return (fallback, None);
    };
    let Some(whole) = written.as_i64() else {
        return (
            fallback,
            Some(Unread {
                setting,
                why: Why::NotAWholeNumber,
            }),
        );
    };
    match i32::try_from(whole) {
        Ok(value) => (value, None),
        Err(_) => (
            fallback,
            Some(Unread {
                setting,
                why: Why::OutOfRange,
            }),
        ),
    }
}

/// One setting that was present and could not be read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unread {
    /// Which setting.
    pub setting: &'static str,
    /// What was wrong with it.
    pub why: Why,
}

/// Why a present setting was not read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Why {
    /// The name this build spells is not a legal configuration key, which for a
    /// constant is a packaging failure.
    NotAKey,
    /// The value is not a whole number.
    NotAWholeNumber,
    /// The value is a whole number wider than the setting's declared range.
    OutOfRange,
}

impl std::fmt::Display for Unread {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let why = match self.why {
            Why::NotAKey => "is not a legal configuration key",
            Why::NotAWholeNumber => "was written as something other than a whole number",
            Why::OutOfRange => "was written outside the range the setting declares",
        };
        write!(
            formatter,
            "the setting {} {why}, so the shipped default is in force",
            self.setting
        )
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
        .settings
        .iter()
        .map(|setting| setting.side)
        .collect())
}

#[cfg(test)]
mod tests {
    use ravel_sdk::config::ValueDocument;
    use ravel_sdk::json;

    #[test]
    fn a_written_value_that_cannot_be_read_is_reported_rather_than_silently_defaulted() {
        assert_eq!(
            read_whole(ENERGY_CAPACITY, None, DEFAULT_ENERGY_CAPACITY),
            (DEFAULT_ENERGY_CAPACITY, None),
            "an absent key is a value nobody wrote"
        );
        assert_eq!(
            read_whole(
                ENERGY_CAPACITY,
                Some(&json!("600")),
                DEFAULT_ENERGY_CAPACITY
            ),
            (
                DEFAULT_ENERGY_CAPACITY,
                Some(Unread {
                    setting: ENERGY_CAPACITY,
                    why: Why::NotAWholeNumber,
                })
            ),
            "a pack that wrote text where a number belongs is told rather than run at the default"
        );
        assert_eq!(
            read_whole(
                ENERGY_CAPACITY,
                Some(&json!(i64::from(i32::MAX) + 1)),
                DEFAULT_ENERGY_CAPACITY
            ),
            (
                DEFAULT_ENERGY_CAPACITY,
                Some(Unread {
                    setting: ENERGY_CAPACITY,
                    why: Why::OutOfRange,
                })
            )
        );
        assert_eq!(
            read_whole(ENERGY_CAPACITY, Some(&json!(42)), DEFAULT_ENERGY_CAPACITY),
            (42, None)
        );
    }

    #[test]
    fn the_declaration_refuses_an_unreadable_value_before_a_resolved_one_exists() {
        let declaration = declaration().expect("the shipped document is legal");
        let written = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": crate::MOD_ID,
                "values": { ENERGY_CAPACITY: "600" },
            })
            .to_string(),
        )
        .expect("the override document is well formed");

        assert!(
            LayeredConfiguration::resolve(declaration, Some(&written), None).is_err(),
            "the declaration is the layer that refuses, and the reporting read is behind it"
        );
    }

    #[test]
    fn a_configuration_that_resolves_cleanly_reports_nothing_unread() {
        let declaration = declaration().expect("the shipped document is legal");
        let resolved = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the shipped defaults resolve");
        let (settings, unread) = Settings::read_reporting(&resolved);
        assert_eq!(settings, Settings::default());
        assert!(
            unread.is_empty(),
            "an absent key is a value nobody wrote, which is not a failure"
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
    fn an_unresolved_launch_runs_on_the_four_shipped_defaults() {
        let resolved = LayeredConfiguration::resolve(declaration().expect("it parses"), None, None)
            .expect("the defaults resolve");
        assert_eq!(Settings::read(&resolved), Settings::default());
        assert_eq!(Settings::default().upgrade_ceiling, 256);
        assert_eq!(Settings::default().energy_per_tick, 600);
        assert_eq!(Settings::default().energy_capacity, 60_000);
        assert_eq!(Settings::default().energy_accepted, 600);
    }

    #[test]
    fn the_ceiling_is_floored_at_one_even_when_nothing_resolved_it() {
        let resolved = LayeredConfiguration::resolve(declaration().expect("it parses"), None, None)
            .expect("the defaults resolve");
        let settings = Settings::read(&resolved);
        assert!(settings.upgrade_ceiling >= 1);
    }

    #[test]
    fn every_declared_key_is_one_the_reader_asks_for() {
        let declaration = declaration().expect("the packaged document parses");
        let declared: Vec<String> = declaration
            .settings
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
