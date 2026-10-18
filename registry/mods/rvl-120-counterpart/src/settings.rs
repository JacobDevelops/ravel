//! The two configuration documents of §3, and the colour rules inside them.
//!
//! The original declares two documents under one grouping each: a server one of
//! three settings and a client one of seventy colours. Ravel resolves one
//! declaration per mod with a side on every setting, so the two documents are
//! one declaration here and the split is carried by [`Document`] rather than by
//! a file. Neither half is synchronised to a joining client, which preserves the
//! original's arrangement.
//!
//! The three server settings are declared `common` rather than
//! `dedicated-server`, which is a declared change. Ravel has no "any server"
//! side and a setting declared `dedicated-server` applies to that side alone, so
//! on the integrated server this package also supports the three would appear on
//! no screen and could not be edited: a single-player player could never turn
//! the ownership record off or move the infusion ceiling.
//!
//! Three rules about the colours are behaviour rather than presentation, and all
//! three are kept: a colour is stored as a hexadecimal **string** holding the
//! low twenty-four bits of the value, reading one forces it fully opaque, and a
//! value that is not hexadecimal is refused rather than falling back to the
//! default.

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, ValueError,
};
use ravel_sdk::{JsonValue, json};

mod colour;

pub use colour::{COLOUR_COUNT, COLOURS, ColourRefused, Colours, Role, parse_colour, stored_form};

/// The key debug logging is turned on and off under.
pub const DEBUG_LOGGING: &str = "server.debug_logging";
/// The key the ownership record is turned on and off under.
pub const RECORD_OWNER: &str = "server.record_owner";
/// The key the single-machine upgrade ceiling is written under.
pub const INFUSION_CEILING: &str = "server.infusion_ceiling";

/// The shipped default for the upgrade ceiling.
pub const INFUSION_CEILING_DEFAULT: i64 = 256;

/// Which of the two documents a load or a reload delivered.
///
/// The original tests this on the load path and does **not** test it on the
/// reload path, so reloading the server document on a dedicated server re-reads
/// seventy client values that have no loaded backing. This counterpart tests it
/// on both paths, which is the declared change `NOTES.md` records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Document {
    /// The seventy colours.
    Client,
    /// The three server settings.
    Server,
}

impl Document {
    /// Returns the document a side resolves.
    ///
    /// This is what makes the scoping below run in production rather than only
    /// under test: an instance built for a server side never re-resolves the
    /// colours, because a server has no loaded backing for them.
    #[must_use]
    pub const fn of(side: ravel_sdk::Side) -> Self {
        match side {
            ravel_sdk::Side::Client => Self::Client,
            ravel_sdk::Side::IntegratedServer | ravel_sdk::Side::DedicatedServer => Self::Server,
        }
    }
}

/// Builds the configuration declaration this package packages.
///
/// # Errors
///
/// Returns whatever [`ConfigurationDeclaration::parse`] refuses, which for a
/// document this function builds from its own tables is a packaging failure.
pub fn declaration() -> Result<ConfigurationDeclaration, DeclarationError> {
    ConfigurationDeclaration::parse(&document().to_string())
}

fn document() -> JsonValue {
    let mut settings = vec![
        json!({
            "key": DEBUG_LOGGING,
            "title": "Log what the library is doing",
            "description": "Writes the library's own diagnostics to the log.",
            "valueSchema": { "type": "boolean" },
            "default": false,
            "side": "common",
            "restartRequired": false,
            "packOverride": "allowed"
        }),
        json!({
            "key": RECORD_OWNER,
            "title": "Record who placed a block",
            "description": "Stores the placing player's name and identity on every block built on this library, which is what gives the security channel meaning.",
            "valueSchema": { "type": "boolean" },
            "default": true,
            "side": "common",
            "restartRequired": false,
            "packOverride": "allowed"
        }),
        json!({
            "key": INFUSION_CEILING,
            "title": "Upgrade material a machine may absorb",
            "description": "The most of one upgrade material a single machine may take.",
            "valueSchema": { "type": "integer", "minimum": 1.0, "maximum": 2_147_483_647.0 },
            "default": INFUSION_CEILING_DEFAULT,
            "side": "common",
            "restartRequired": false,
            "packOverride": "allowed"
        }),
    ];

    for role in &COLOURS {
        settings.push(json!({
            "key": role.key(),
            "title": format!("{} {} colour, {}", role.component, role.channel, role.state),
            "description": "A widget colour, written as six hexadecimal digits.",
            "valueSchema": { "type": "string", "minLength": 6, "maxLength": 6 },
            "default": stored_form(role.default),
            "side": "client",
            "restartRequired": false,
            "packOverride": "allowed"
        }));
    }

    json!({
        "schemaVersion": 1,
        "owner": crate::MOD_ID,
        "version": 1,
        "settings": JsonValue::Array(settings),
    })
}

/// A missing or invalid server setting.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsRefused {
    /// Typed configuration access failed.
    Value(ValueError),
    /// The infusion ceiling must be between one and `i32::MAX`.
    InfusionCeiling,
}

impl core::fmt::Display for SettingsRefused {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Value(error) => error.fmt(formatter),
            Self::InfusionCeiling => {
                write!(formatter, "{INFUSION_CEILING} must be in 1..={}", i32::MAX)
            }
        }
    }
}

impl std::error::Error for SettingsRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Value(error) => Some(error),
            Self::InfusionCeiling => None,
        }
    }
}

impl From<ValueError> for SettingsRefused {
    fn from(error: ValueError) -> Self {
        Self::Value(error)
    }
}

/// The three server values in force for one launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Whether the library writes its own diagnostics.
    pub debug_logging: bool,
    /// Whether a placed block records who placed it.
    pub record_owner: bool,
    /// The most of one upgrade material a machine may absorb.
    pub infusion_ceiling: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            debug_logging: false,
            record_owner: true,
            infusion_ceiling: INFUSION_CEILING_DEFAULT,
        }
    }
}

impl Settings {
    /// Reads the three server values out of a resolved configuration.
    ///
    /// A value outside the supported positive `i32` range is refused rather than
    /// defaulted. Missing keys and mismatched kinds are also returned.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsRefused`] for missing keys, wrong kinds or an unsupported ceiling.
    pub fn read(resolved: &LayeredConfiguration) -> Result<Self, SettingsRefused> {
        let infusion_ceiling = i32::try_from(resolved.integer(INFUSION_CEILING)?)
            .map_err(|_| SettingsRefused::InfusionCeiling)?;
        if infusion_ceiling < 1 {
            return Err(SettingsRefused::InfusionCeiling);
        }
        Ok(Self {
            debug_logging: resolved.boolean(DEBUG_LOGGING)?,
            record_owner: resolved.boolean(RECORD_OWNER)?,
            infusion_ceiling: i64::from(infusion_ceiling),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::Side;
    use ravel_sdk::config::ValueDocument;

    fn resolved(values: &str) -> LayeredConfiguration {
        let declaration = declaration().expect("the packaged declaration is well formed");
        let document = format!(
            r#"{{ "schemaVersion": 1, "owner": "{}", "values": {values} }}"#,
            crate::MOD_ID
        );
        let player = ValueDocument::parse(&document).expect("a legal value document");
        LayeredConfiguration::resolve(declaration, None, Some(&player)).expect("the layers resolve")
    }

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        let changed = document()
            .to_string()
            .replace(DEBUG_LOGGING, "server.renamed");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the changed declaration resolves");
        assert_eq!(
            Settings::read(&resolved),
            Err(SettingsRefused::Value(ValueError::Unknown {
                key: DEBUG_LOGGING.to_owned()
            }))
        );
    }

    #[test]
    fn a_missing_colour_is_refused_without_changing_any_colours() {
        let changed = document()
            .to_string()
            .replace(&COLOURS[1].key(), "client.renamed");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let written = stored_form(COLOURS[0].default ^ 0x00FF_FFFF);
        let values = json!({
            "schemaVersion": 1,
            "owner": crate::MOD_ID,
            "values": { (COLOURS[0].key()): written }
        });
        let player = ValueDocument::parse(&values.to_string()).expect("a legal colour override");
        let resolved = LayeredConfiguration::resolve(declared, None, Some(&player))
            .expect("the changed declaration resolves");
        assert_eq!(resolved.text(&COLOURS[0].key()), Ok(written.as_str()));
        let mut colours = Colours::shipped();
        let before = colours.clone();
        assert!(matches!(
            colours.resolve(Document::Client, &resolved),
            Err(ColourRefused::Value(_))
        ));
        assert_eq!(colours, before);
    }

    #[test]
    fn a_wrongly_typed_setting_or_colour_is_refused() {
        for name in [DEBUG_LOGGING.to_owned(), COLOURS[0].key()] {
            let mut changed = document();
            let settings = changed["settings"]
                .as_array_mut()
                .expect("a settings array");
            let setting = settings
                .iter_mut()
                .find(|setting| setting["key"] == name)
                .expect("the key is declared");
            setting["valueSchema"] = json!({ "type": "integer" });
            setting["default"] = json!(1);
            let declared =
                ConfigurationDeclaration::parse(&changed.to_string()).expect("a legal declaration");
            let resolved = LayeredConfiguration::resolve(declared, None, None)
                .expect("the changed declaration resolves");
            if name == DEBUG_LOGGING {
                assert!(matches!(
                    Settings::read(&resolved),
                    Err(SettingsRefused::Value(ValueError::AccessKind { .. }))
                ));
            } else {
                assert!(matches!(
                    Colours::shipped().resolve(Document::Client, &resolved),
                    Err(ColourRefused::Value(_))
                ));
            }
        }
    }

    #[test]
    fn the_declaration_carries_three_server_settings_and_seventy_colours() {
        let declaration = declaration().expect("the packaged declaration is well formed");
        for key in [DEBUG_LOGGING, RECORD_OWNER, INFUSION_CEILING] {
            assert!(declaration.setting(key).is_some(), "{key} is declared");
        }
        for role in &COLOURS {
            assert!(declaration.setting(&role.key()).is_some());
        }
    }

    #[test]
    fn the_server_settings_default_to_off_on_and_two_hundred_and_fifty_six() {
        let settings = Settings::default();
        assert!(!settings.debug_logging);
        assert!(settings.record_owner);
        assert_eq!(settings.infusion_ceiling, 256);
    }

    #[test]
    fn the_three_server_values_are_read_out_of_a_resolved_document() {
        let resolved = resolved(
            r#"{
                "server.debug_logging": true,
                "server.record_owner": false,
                "server.infusion_ceiling": 64
            }"#,
        );
        let settings = Settings::read(&resolved).expect("declared settings have matching types");
        assert!(settings.debug_logging);
        assert!(!settings.record_owner);
        assert_eq!(settings.infusion_ceiling, 64);
    }

    #[test]
    fn a_ceiling_outside_the_supported_range_is_refused() {
        for value in [-1, 0, i64::from(i32::MAX) + 1] {
            let mut changed = document();
            let settings = changed["settings"]
                .as_array_mut()
                .expect("a settings array");
            let ceiling = settings
                .iter_mut()
                .find(|setting| setting["key"] == INFUSION_CEILING)
                .expect("the ceiling is declared");
            ceiling["valueSchema"] = json!({ "type": "integer" });
            ceiling["default"] = json!(value);
            let declared = ConfigurationDeclaration::parse(&changed.to_string())
                .expect("an unconstrained integer declaration");
            let resolved = LayeredConfiguration::resolve(declared, None, None)
                .expect("the out-of-range integer resolves");
            assert_eq!(
                Settings::read(&resolved),
                Err(SettingsRefused::InfusionCeiling)
            );
        }
    }

    #[test]
    fn the_shipped_colours_are_seventy_and_every_one_is_opaque() {
        let colours = Colours::shipped();
        assert_eq!(colours.values().len(), COLOUR_COUNT);
        assert!(colours.values().iter().all(|value| value >> 24 == 0xFF));
        assert_eq!(colours.colour(COLOUR_COUNT), None);
    }

    #[test]
    fn a_server_document_does_not_re_resolve_the_client_colours() {
        let resolved = resolved("{}");
        let mut colours = Colours::shipped();
        let before = colours.values().to_vec();

        assert_eq!(colours.resolve(Document::Server, &resolved), Ok(false));
        assert_eq!(colours.values(), before.as_slice());

        assert_eq!(colours.resolve(Document::Client, &resolved), Ok(true));
    }

    #[test]
    fn a_client_document_holding_a_written_colour_takes_it_forced_opaque() {
        let key = COLOURS[0].key();
        let resolved = resolved(&format!(r#"{{ "{key}": "010203" }}"#));
        let mut colours = Colours::shipped();
        assert_eq!(colours.resolve(Document::Client, &resolved), Ok(true));
        assert_eq!(colours.colour(0), Some(0xFF01_0203));
    }

    #[test]
    fn a_colour_the_declaration_let_through_is_still_refused_at_the_resolution() {
        // Six characters, so the value schema admits it and the refusal can
        // only happen where §3 puts it: in configuration resolution.
        let key = COLOURS[0].key();
        let resolved = resolved(&format!(r#"{{ "{key}": "zzzzzz" }}"#));
        let mut colours = Colours::shipped();
        let before = colours.values().to_vec();

        assert_eq!(
            colours.resolve(Document::Client, &resolved),
            Err(ColourRefused::NotHexadecimal {
                text: "zzzzzz".to_owned()
            })
        );
        assert_eq!(
            colours.values(),
            before.as_slice(),
            "a refused document leaves every colour as it was"
        );
    }

    #[test]
    fn the_document_a_side_resolves_is_the_client_one_only_on_the_client() {
        assert_eq!(Document::of(Side::Client), Document::Client);
        assert_eq!(Document::of(Side::IntegratedServer), Document::Server);
        assert_eq!(Document::of(Side::DedicatedServer), Document::Server);
    }

    #[test]
    fn the_three_server_settings_are_reachable_on_every_side_a_server_runs_on() {
        use ravel_sdk::config::SettingSide;

        let declaration = declaration().expect("the packaged declaration is well formed");
        for key in [DEBUG_LOGGING, RECORD_OWNER, INFUSION_CEILING] {
            let setting = declaration.setting(key).expect("the setting is declared");
            assert_eq!(
                setting.side,
                SettingSide::Common,
                "`{key}` is unreachable on an integrated server if it names one server side"
            );
            for side in [Side::IntegratedServer, Side::DedicatedServer] {
                assert!(setting.side.applies_to(side), "`{key}` applies on {side:?}");
            }
        }
    }
}
