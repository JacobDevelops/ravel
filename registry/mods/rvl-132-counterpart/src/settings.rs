//! Section three's settings document.
//!
//! Section three counts six properties. Two of them describe outbound network
//! behaviour — an anonymous startup ping and an update check that fetches a
//! document whose address is assembled from the running game version, both on by
//! default — and section three states that a counterpart reproduces neither.
//! That is a design decision rather than a rights one: Ravel does not ship code
//! that contacts a third-party service by default.
//!
//! So four of the six are here, and one further setting is added: section 11.1
//! requires the brewing closure's cap to be a setting rather than a constant.
//!
//! The third-party list is re-read on every settings load and reload, and the
//! comparator of section 6.4 is rebuilt from it each time, so the setting takes
//! effect without a restart. [`Settings::comparator`] is that rebuild, and it is
//! the only way to get a comparator, so a comparator can never outlive the list
//! it resolved.

use core::fmt;

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, ValueError,
};

use crate::data::{Comparator, MAX_IGNORED};
use crate::recipes::Closure;

/// The key an invalid recipe's treatment is switched under.
pub const ABORT_ON_INVALID_RECIPE: &str = "recipes.abort_on_invalid";

/// The key a compatibility bundle's failure treatment is switched under.
pub const ABORT_ON_BUNDLE_FAILURE: &str = "compatibility.abort_on_failure";

/// The key the ignored component kinds are listed under.
pub const IGNORED_COMPONENTS: &str = "matching.ignored_component_kinds";

/// The key unequal-data logging is switched under.
pub const LOG_UNEQUAL_DATA: &str = "diagnostics.log_unequal_data";

/// The key the brewing closure's cap is set under.
pub const CLOSURE_CAP: &str = "recipes.brewing_closure_cap";

/// The five settings this package declares, in declaration order.
pub const KEYS: [&str; 5] = [
    ABORT_ON_INVALID_RECIPE,
    ABORT_ON_BUNDLE_FAILURE,
    IGNORED_COMPONENTS,
    LOG_UNEQUAL_DATA,
    CLOSURE_CAP,
];

/// The two of section three's six this package does not reproduce.
///
/// Both describe outbound network behaviour and both are on by default upstream.
pub const DECLINED: [&str; 2] = ["startup analytics ping", "remote version check"];

/// The most characters the ignored-component list may carry.
pub const MAX_IGNORED_TEXT: usize = 4_096;

/// The packaged settings document.
///
/// The three settings section three calls server-scoped are declared `common`
/// rather than `dedicated-server`. Ravel's `dedicated-server` side is that side
/// alone, and this package declares `integrated-server` as a supported side, so
/// declaring them the way section three's server document reads would put them
/// on no screen in single-player: a player could never change the matching rule
/// their own world runs under. `common` reaches every side a server runs on and
/// one it does not.
pub const DOCUMENT: &str = r#"{
  "schemaVersion": 1,
  "owner": "rvl_132_counterpart",
  "version": 1,
  "settings": [
    {
      "key": "recipes.abort_on_invalid",
      "title": "Abort loading on an invalid recipe",
      "description": "Aborts loading when an invalid recipe is met while enumerating, rather than skipping it.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "compatibility.abort_on_failure",
      "title": "Abort loading on a compatibility bundle failure",
      "description": "Aborts loading when a compatibility bundle fails, rather than logging the failure and carrying on.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "matching.ignored_component_kinds",
      "title": "Component kinds to ignore when comparing item and fluid data",
      "description": "A comma-separated list of data component kind names. Two items carrying different values of an ignored kind compare equal, so this changes what machines accept.",
      "valueSchema": { "type": "string", "maxLength": 4096 },
      "default": "",
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "diagnostics.log_unequal_data",
      "title": "Log every unequal data comparison",
      "description": "Logs every comparison that answers unequal. A performance hazard: a machine compares data many times per tick.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "client",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "recipes.brewing_closure_cap",
      "title": "Most brewing recipes the closure may derive",
      "description": "The brewing recipe list is derived rather than read, and is unbounded under a badly-behaved package. Past this many the derivation stops and reports a probable cycle.",
      "valueSchema": { "type": "integer", "minimum": 1.0, "maximum": 100000.0 },
      "default": 7500,
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
/// compile-time constant is a packaging failure rather than a runtime one.
pub fn declaration() -> Result<ConfigurationDeclaration, DeclarationError> {
    ConfigurationDeclaration::parse(DOCUMENT)
}

/// What a resolution refuses.
#[derive(Clone, Debug, PartialEq)]
pub enum Refused {
    /// A declared setting is missing or has the wrong kind.
    Value(ValueError),
    /// The brewing closure refuses a cap outside its supported domain.
    Cap(crate::recipes::Refused),
    /// The ignored-component list is longer than [`MAX_IGNORED_TEXT`].
    ListTooLong {
        /// The published ceiling.
        limit: usize,
    },
    /// The ignored-component list names more kinds than [`MAX_IGNORED`].
    TooManyIgnored {
        /// The published ceiling.
        limit: usize,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(error) => error.fmt(formatter),
            Self::Cap(error) => error.fmt(formatter),
            Self::ListTooLong { limit } => {
                write!(
                    formatter,
                    "an ignored-kind list longer than {limit} characters"
                )
            }
            Self::TooManyIgnored { limit } => {
                write!(
                    formatter,
                    "an ignored-kind list naming more than {limit} kinds"
                )
            }
        }
    }
}

impl std::error::Error for Refused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Value(error) => Some(error),
            Self::Cap(error) => Some(error),
            Self::ListTooLong { .. } | Self::TooManyIgnored { .. } => None,
        }
    }
}

impl From<ValueError> for Refused {
    fn from(error: ValueError) -> Self {
        Self::Value(error)
    }
}

/// The values in force for one launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Whether an invalid recipe met while enumerating aborts loading.
    pub abort_on_invalid_recipe: bool,
    /// Whether a failure inside a compatibility bundle aborts loading.
    pub abort_on_bundle_failure: bool,
    /// The component kinds to ignore when comparing data.
    pub ignored: Vec<String>,
    /// Whether every unequal-data comparison is logged.
    pub log_unequal_data: bool,
    closure: Closure,
}

impl Settings {
    /// The values declared by the packaged document, before any overrides.
    ///
    /// # Errors
    /// Returns the packaged declaration, resolution, or typed-value refusal.
    pub fn shipped() -> Result<Self, ravel_sdk::ModError> {
        let declared = declaration().map_err(ravel_sdk::ModError::from_display)?;
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .map_err(ravel_sdk::ModError::from_display)?;
        Self::read(&resolved).map_err(ravel_sdk::ModError::from_display)
    }

    /// Reads every declared setting from the resolved configuration.
    ///
    /// Missing or wrongly typed values, out-of-range closure caps and ignored
    /// lists past either ceiling are refused without partially applying the resolution.
    pub fn read(resolved: &LayeredConfiguration) -> Result<Self, Refused> {
        let abort_on_invalid_recipe = resolved.boolean(ABORT_ON_INVALID_RECIPE)?;
        let abort_on_bundle_failure = resolved.boolean(ABORT_ON_BUNDLE_FAILURE)?;
        let log_unequal_data = resolved.boolean(LOG_UNEQUAL_DATA)?;
        let ignored = ignored_from(resolved.text(IGNORED_COMPONENTS)?)?;
        let cap = usize::try_from(resolved.integer(CLOSURE_CAP)?).map_err(|_| {
            Refused::Cap(crate::recipes::Refused::Cap {
                limit: crate::recipes::MAX_CLOSURE_CAP,
            })
        })?;
        let closure = Closure::new(cap).map_err(Refused::Cap)?;
        Ok(Self {
            abort_on_invalid_recipe,
            abort_on_bundle_failure,
            ignored,
            log_unequal_data,
            closure,
        })
    }

    /// The data comparator these values put in force.
    ///
    /// Section three: the list is re-read on every load and reload and the
    /// comparator is rebuilt from it each time. A rebuilt comparator resolves its
    /// list once and keeps it, which is harmless only because the whole
    /// comparator is replaced, and this is the replacement.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::TooManyIgnored`] for a list past [`MAX_IGNORED`].
    /// [`Settings::read`] guards the list it resolves, but the fields are public
    /// and a caller may assign one, and a comparator that silently ignores
    /// nothing is a matching change with no signal.
    pub fn comparator(&self) -> Result<Comparator, Refused> {
        Comparator::new(self.ignored.clone())
            .map_err(|_| Refused::TooManyIgnored { limit: MAX_IGNORED })
    }

    /// The brewing closure these values put in force.
    #[must_use]
    pub fn closure(&self) -> Closure {
        self.closure
    }
}

/// Reads the ignored-component list out of its comma-separated text.
///
/// # Errors
///
/// Returns [`Refused::ListTooLong`] past [`MAX_IGNORED_TEXT`] characters and
/// [`Refused::TooManyIgnored`] past [`MAX_IGNORED`] names.
pub fn ignored_from(text: &str) -> Result<Vec<String>, Refused> {
    if text.len() > MAX_IGNORED_TEXT {
        return Err(Refused::ListTooLong {
            limit: MAX_IGNORED_TEXT,
        });
    }
    let mut names: Vec<String> = text
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort_unstable();
    names.dedup();
    if names.len() > MAX_IGNORED {
        return Err(Refused::TooManyIgnored { limit: MAX_IGNORED });
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Components, Value};

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        for name in KEYS {
            let changed = DOCUMENT.replace(name, "renamed.key");
            let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
            let resolved = LayeredConfiguration::resolve(declared, None, None)
                .expect("the changed declaration resolves");
            let refused = Settings::read(&resolved).expect_err("the declared key is missing");
            assert!(matches!(refused, Refused::Value(_)));
            assert!(
                std::error::Error::source(&refused)
                    .expect("the typed value failure is retained")
                    .downcast_ref::<ValueError>()
                    .is_some()
            );
        }
    }

    #[test]
    fn the_packaged_declaration_parses_and_names_five_settings() {
        let declared = declaration().expect("a packaged document");
        assert_eq!(declared.settings().len(), 5);
        let keys: Vec<String> = declared
            .settings()
            .iter()
            .map(|setting| setting.key.to_string())
            .collect();
        assert_eq!(keys, KEYS);
    }

    #[test]
    fn the_two_outbound_settings_are_declared_nowhere() {
        // Neither of the two is a key, and no declared key names anything a
        // network switch would be named after, so the document cannot grow one
        // by a rename.
        let declared = declaration().expect("a packaged document");
        let keys: Vec<String> = declared
            .settings()
            .iter()
            .map(|setting| setting.key.to_string())
            .collect();
        for outbound in ["ping", "analytics", "telemetry", "update", "version"] {
            assert!(!keys.iter().any(|key| key.contains(outbound)), "{outbound}");
        }
    }

    #[test]
    fn the_three_server_scoped_settings_are_declared_common() {
        let declared = declaration().expect("a packaged document");
        for key in [
            ABORT_ON_INVALID_RECIPE,
            ABORT_ON_BUNDLE_FAILURE,
            IGNORED_COMPONENTS,
        ] {
            let setting = declared.setting(key).expect("a declared setting");
            assert_eq!(
                setting.side,
                ravel_sdk::config::SettingSide::Common,
                "{key}"
            );
        }
    }

    #[test]
    fn the_diagnostic_setting_is_client_scoped() {
        let declared = declaration().expect("a packaged document");
        let setting = declared
            .setting(LOG_UNEQUAL_DATA)
            .expect("a declared setting");
        assert_eq!(setting.side, ravel_sdk::config::SettingSide::Client);
    }

    #[test]
    fn the_shipped_values_are_the_ones_the_document_declares() {
        let resolved =
            LayeredConfiguration::resolve(declaration().expect("a packaged document"), None, None)
                .expect("the shipped defaults resolve");
        assert_eq!(
            Settings::read(&resolved),
            Ok(Settings::shipped().expect("valid shipped settings"))
        );
    }

    #[test]
    fn an_empty_list_names_nothing() {
        assert_eq!(ignored_from(""), Ok(Vec::new()));
        assert_eq!(ignored_from("  ,  , "), Ok(Vec::new()));
    }

    #[test]
    fn a_list_is_read_trimmed_sorted_and_without_repeats() {
        assert_eq!(
            ignored_from(" wear , tint ,wear"),
            Ok(vec!["tint".to_owned(), "wear".to_owned()])
        );
    }

    #[test]
    fn a_list_past_either_ceiling_is_refused_rather_than_truncated() {
        let long = "a".repeat(MAX_IGNORED_TEXT + 1);
        assert_eq!(
            ignored_from(&long),
            Err(Refused::ListTooLong {
                limit: MAX_IGNORED_TEXT
            })
        );
        let many: Vec<String> = (0..=MAX_IGNORED).map(|index| format!("k{index}")).collect();
        assert_eq!(
            ignored_from(&many.join(",")),
            Err(Refused::TooManyIgnored { limit: MAX_IGNORED })
        );
        let at_ceiling: Vec<String> = (0..MAX_IGNORED).map(|index| format!("k{index}")).collect();
        assert_eq!(
            ignored_from(&at_ceiling.join(",")).map(|names| names.len()),
            Ok(MAX_IGNORED)
        );
    }

    fn wear_data(wear: i64) -> Components {
        Components::from([("wear".to_owned(), Value::Whole(wear))])
    }

    #[test]
    fn the_comparator_is_rebuilt_from_the_values_in_force() {
        let mut settings = Settings::shipped().expect("the packaged settings resolve");
        let sharp = wear_data(0);
        let blunt = wear_data(90);
        assert!(
            !settings
                .comparator()
                .expect("an empty list")
                .equal(&sharp, &blunt)
        );
        settings.ignored = vec!["wear".to_owned()];
        assert!(
            settings
                .comparator()
                .expect("a one-name list")
                .equal(&sharp, &blunt)
        );
    }

    #[test]
    fn an_over_long_ignore_list_refuses_the_comparator_rather_than_ignoring_nothing() {
        // The fields are public and assignable, and a comparator that silently
        // ignores nothing is a matching change with no signal.
        let settings = Settings {
            ignored: (0..=MAX_IGNORED).map(|index| format!("k{index}")).collect(),
            ..Settings::shipped().expect("the packaged settings resolve")
        };
        assert_eq!(
            settings.comparator(),
            Err(Refused::TooManyIgnored { limit: MAX_IGNORED })
        );
    }

    #[test]
    fn the_closure_carries_the_cap_in_force() {
        let changed = DOCUMENT.replace("\"default\": 7500", "\"default\": 40");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the changed defaults resolve");
        assert_eq!(
            Settings::read(&resolved)
                .expect("a valid cap")
                .closure()
                .cap(),
            40
        );
    }

    #[test]
    fn a_widened_declaration_cannot_silently_default_an_out_of_range_cap() {
        for cap in ["-1", "0", "100001", "9223372036854775807"] {
            let changed = DOCUMENT
                .replace(
                    "\"minimum\": 1.0, \"maximum\": 100000.0",
                    "\"minimum\": -1.0",
                )
                .replace("\"default\": 7500", &format!("\"default\": {cap}"));
            let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
            let resolved = LayeredConfiguration::resolve(declared, None, None)
                .expect("the widened declaration resolves");
            assert_eq!(
                Settings::read(&resolved),
                Err(Refused::Cap(crate::recipes::Refused::Cap {
                    limit: crate::recipes::MAX_CLOSURE_CAP
                })),
                "{cap}"
            );
        }
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            Refused::ListTooLong { limit: 8 }
                .to_string()
                .contains("longer than 8")
        );
        assert!(
            Refused::TooManyIgnored { limit: 8 }
                .to_string()
                .contains("more than 8")
        );
    }
}
