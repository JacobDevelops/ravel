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

use std::fmt;

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, SettingKey,
};
use ravel_sdk::{JsonValue, json};

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

/// One colour role: a component, the part of it painted, and the state it is in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Role {
    /// The widget the colour belongs to.
    pub component: &'static str,
    /// The part of that widget the colour paints.
    pub channel: &'static str,
    /// The state the widget is in.
    pub state: &'static str,
    /// The shipped value, as a full thirty-two-bit colour.
    pub default: u32,
}

impl Role {
    /// Returns the setting key this role is written under.
    #[must_use]
    pub fn key(&self) -> String {
        format!("colour.{}.{}.{}", self.component, self.channel, self.state)
    }
}

/// How many colour roles the client document declares.
pub const COLOUR_COUNT: usize = 70;

/// The seventy colour roles, in the order the document declares them.
///
/// The arrangement is seven components, each painted in four channels across
/// the states that component actually has: three for the interactive widgets,
/// two for the ones with no hover, and three channels for the bar. The values
/// are this package's own and carry a full alpha byte on purpose, so that the
/// low-twenty-four-bit storage rule below has something to drop.
pub static COLOURS: [Role; COLOUR_COUNT] = build_roles();

const CHANNELS: [&str; 4] = ["border", "background", "gradient", "text"];

const fn build_roles() -> [Role; COLOUR_COUNT] {
    const COMPONENTS: [(&str, &[&str], usize); 7] = [
        ("button", &["normal", "disabled", "hovering"], 4),
        ("toggle", &["normal", "disabled", "selected"], 4),
        ("slider", &["normal", "disabled"], 4),
        ("slider_knob", &["normal", "hovering", "dragging"], 4),
        ("text_field", &["normal", "disabled", "selected"], 4),
        ("list", &["normal", "selected"], 4),
        ("energy_bar", &["normal", "disabled"], 3),
    ];

    let mut roles = [Role {
        component: "button",
        channel: "border",
        state: "normal",
        default: 0,
    }; COLOUR_COUNT];

    let mut written = 0_usize;
    let mut colour_number = 1_u32;
    let mut component = 0_usize;
    while component < COMPONENTS.len() {
        let (name, states, channels) = COMPONENTS[component];
        let mut channel = 0_usize;
        while channel < channels {
            let mut state = 0_usize;
            while state < states.len() {
                roles[written] = Role {
                    component: name,
                    channel: CHANNELS[channel],
                    state: states[state],
                    default: 0xFF00_0000 | (colour_number * 0x0003_0507),
                };
                written += 1;
                colour_number += 1;
                state += 1;
            }
            channel += 1;
        }
        component += 1;
    }
    roles
}

/// Builds the configuration declaration this package packages.
///
/// # Errors
///
/// Returns whatever [`ConfigurationDeclaration::parse`] refuses, which for a
/// document this function builds from its own tables is a packaging failure.
pub fn declaration() -> Result<ConfigurationDeclaration, DeclarationError> {
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

    let document = json!({
        "schemaVersion": 1,
        "owner": crate::MOD_ID,
        "version": 1,
        "settings": JsonValue::Array(settings),
    });
    ConfigurationDeclaration::parse(&document.to_string())
}

/// Returns the stored form of a colour: the low twenty-four bits, in hex.
///
/// The alpha byte is dropped on the way out, so a pack author reading the file
/// never sees one and cannot infer that writing one would do anything.
#[must_use]
pub fn stored_form(colour: u32) -> String {
    format!("{:06x}", colour & 0x00FF_FFFF)
}

/// Reads a stored colour back, forced fully opaque.
///
/// # Errors
///
/// Returns [`ColourRefused`] when the text is not six hexadecimal digits. The
/// original raises here rather than falling back to the default, and this keeps
/// the refusal: a colour document is a trust boundary and a silent default
/// would hide a pack author's typo behind a screen that merely looks wrong.
pub fn parse_colour(text: &str) -> Result<u32, ColourRefused> {
    if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ColourRefused {
            text: text.to_owned(),
        });
    }
    let value = u32::from_str_radix(text, 16).map_err(|_| ColourRefused {
        text: text.to_owned(),
    })?;
    Ok(0xFF00_0000 | value)
}

/// A colour setting held something that is not a colour.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColourRefused {
    /// What the document held.
    pub text: String,
}

impl fmt::Display for ColourRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` is not six hexadecimal digits, so no colour was resolved",
            self.text
        )
    }
}

impl std::error::Error for ColourRefused {}

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
    /// A value outside the declared bounds keeps the shipped default rather than
    /// being clamped into range. A resolution is not a trusted input, and a
    /// ceiling of zero would refuse every infusion silently.
    #[must_use]
    pub fn read(resolved: &LayeredConfiguration) -> Self {
        let mut settings = Self::default();
        if let Some(value) = boolean(resolved, DEBUG_LOGGING) {
            settings.debug_logging = value;
        }
        if let Some(value) = boolean(resolved, RECORD_OWNER) {
            settings.record_owner = value;
        }
        if let Some(value) = integer(resolved, INFUSION_CEILING)
            && (1..=i64::from(i32::MAX)).contains(&value)
        {
            settings.infusion_ceiling = value;
        }
        settings
    }
}

/// The seventy colours in force, in the order [`COLOURS`] declares them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Colours {
    values: Vec<u32>,
}

impl Default for Colours {
    fn default() -> Self {
        Self::shipped()
    }
}

impl Colours {
    /// The colours as shipped, every one fully opaque.
    #[must_use]
    pub fn shipped() -> Self {
        Self {
            values: COLOURS
                .iter()
                .map(|role| 0xFF00_0000 | (role.default & 0x00FF_FFFF))
                .collect(),
        }
    }

    /// Re-resolves the colours, but only when the client document arrived.
    ///
    /// This is the divergence of §3. Scoping the re-resolve to the client
    /// document on the reload path as well as the load path is what keeps a
    /// dedicated server, where the client document is declared and never
    /// populated, from reading seventy values that have no backing.
    ///
    /// # Errors
    ///
    /// Returns [`ColourRefused`] for the first colour whose text is not
    /// hexadecimal, naming the text rather than the key so the refusal says what
    /// was wrong with the document.
    pub fn resolve(
        &mut self,
        document: Document,
        resolved: &LayeredConfiguration,
    ) -> Result<bool, ColourRefused> {
        if document != Document::Client {
            return Ok(false);
        }
        let mut read = Vec::with_capacity(COLOUR_COUNT);
        for role in &COLOURS {
            match text(resolved, &role.key()) {
                Some(written) => read.push(parse_colour(&written)?),
                None => read.push(0xFF00_0000 | (role.default & 0x00FF_FFFF)),
            }
        }
        self.values = read;
        Ok(true)
    }

    /// Returns the colour in force for one role.
    #[must_use]
    pub fn colour(&self, index: usize) -> Option<u32> {
        self.values.get(index).copied()
    }

    /// Returns every colour in force, in declaration order.
    #[must_use]
    pub fn values(&self) -> &[u32] {
        &self.values
    }
}

fn key_of(key: &str) -> Option<SettingKey> {
    key.parse().ok()
}

fn boolean(resolved: &LayeredConfiguration, key: &str) -> Option<bool> {
    resolved.value(&key_of(key)?)?.as_bool()
}

fn integer(resolved: &LayeredConfiguration, key: &str) -> Option<i64> {
    resolved.value(&key_of(key)?)?.as_i64()
}

fn text(resolved: &LayeredConfiguration, key: &str) -> Option<String> {
    resolved
        .value(&key_of(key)?)?
        .as_str()
        .map(std::borrow::ToOwned::to_owned)
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
    fn the_declaration_carries_three_server_settings_and_seventy_colours() {
        let declaration = declaration().expect("the packaged declaration is well formed");
        for key in [DEBUG_LOGGING, RECORD_OWNER, INFUSION_CEILING] {
            let parsed: SettingKey = key.parse().expect("a legal setting key");
            assert!(declaration.setting(&parsed).is_some(), "{key} is declared");
        }
        for role in &COLOURS {
            let parsed: SettingKey = role.key().parse().expect("a legal setting key");
            assert!(declaration.setting(&parsed).is_some());
        }
        assert_eq!(COLOURS.len(), 70);
    }

    #[test]
    fn every_colour_role_is_named_once_and_belongs_to_one_of_seven_components() {
        let mut keys: Vec<String> = COLOURS.iter().map(Role::key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), COLOUR_COUNT);

        let mut components: Vec<&str> = COLOURS.iter().map(|role| role.component).collect();
        components.sort_unstable();
        components.dedup();
        assert_eq!(components.len(), 7);
    }

    #[test]
    fn the_stored_form_drops_the_alpha_byte_on_the_way_out() {
        assert_eq!(stored_form(0xFF12_3456), "123456");
        assert_eq!(stored_form(0x0012_3456), "123456");
        assert_eq!(stored_form(0), "000000");
        assert_eq!(stored_form(u32::MAX), "ffffff");
    }

    #[test]
    fn a_parsed_colour_is_forced_fully_opaque_whatever_was_written() {
        assert_eq!(parse_colour("123456"), Ok(0xFF12_3456));
        assert_eq!(parse_colour("000000"), Ok(0xFF00_0000));
        assert_eq!(parse_colour("ffffff"), Ok(0xFFFF_FFFF));
        assert_eq!(
            parse_colour("ABCDEF"),
            Ok(0xFFAB_CDEF),
            "upper case digits are digits"
        );
    }

    #[test]
    fn no_pack_author_can_make_one_of_these_colours_translucent() {
        for written in ["000000", "112233", "ffffff"] {
            let parsed = parse_colour(written).expect("six hexadecimal digits");
            assert_eq!(parsed >> 24, 0xFF);
        }
    }

    #[test]
    fn a_colour_that_is_not_hexadecimal_is_refused_rather_than_defaulted() {
        for written in ["", "12345", "1234567", "12345g", "#123456", " 12345"] {
            assert!(
                parse_colour(written).is_err(),
                "`{written}` is not a colour and must not resolve to one"
            );
        }
    }

    #[test]
    fn every_shipped_colour_round_trips_through_its_stored_form() {
        for role in &COLOURS {
            let stored = stored_form(role.default);
            assert_eq!(
                parse_colour(&stored).expect("a shipped default is hexadecimal"),
                0xFF00_0000 | (role.default & 0x00FF_FFFF)
            );
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
        let settings = Settings::read(&resolved);
        assert!(settings.debug_logging);
        assert!(!settings.record_owner);
        assert_eq!(settings.infusion_ceiling, 64);
    }

    #[test]
    fn a_ceiling_outside_its_declared_bounds_keeps_the_shipped_default() {
        // The declaration itself refuses these, so what reaches `read` is the
        // default; the bound here is the second one, held against a resolution
        // that is not trusted to have applied the first.
        let resolved = resolved(r#"{ "server.infusion_ceiling": 0 }"#);
        assert_eq!(Settings::read(&resolved).infusion_ceiling, 256);
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
            Err(ColourRefused {
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
        assert_eq!(Document::of(ravel_sdk::Side::Client), Document::Client);
        assert_eq!(
            Document::of(ravel_sdk::Side::IntegratedServer),
            Document::Server
        );
        assert_eq!(
            Document::of(ravel_sdk::Side::DedicatedServer),
            Document::Server
        );
    }

    #[test]
    fn the_three_server_settings_are_reachable_on_every_side_a_server_runs_on() {
        use ravel_sdk::config::SettingSide;

        let declaration = declaration().expect("the packaged declaration is well formed");
        for key in [DEBUG_LOGGING, RECORD_OWNER, INFUSION_CEILING] {
            let parsed: SettingKey = key.parse().expect("a legal setting key");
            let setting = declaration
                .setting(&parsed)
                .expect("the setting is declared");
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
