//! Client colour roles, hexadecimal storage, and atomic document resolution.

use std::fmt;

use ravel_sdk::config::{AccessError, LayeredConfiguration};

use super::Document;

/// One colour role: a component, the part of it painted, and the state it is in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
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
        return Err(ColourRefused::NotHexadecimal {
            text: text.to_owned(),
        });
    }
    let value = u32::from_str_radix(text, 16).map_err(|_| ColourRefused::NotHexadecimal {
        text: text.to_owned(),
    })?;
    Ok(0xFF00_0000 | value)
}

/// Why a colour setting could not be resolved.
#[derive(Clone, Debug, PartialEq)]
pub enum ColourRefused {
    /// The setting is absent or has the wrong kind.
    Value(AccessError),
    /// The setting is not six hexadecimal digits.
    NotHexadecimal {
        /// What the document held.
        text: String,
    },
}

impl fmt::Display for ColourRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(source) => source.fmt(formatter),
            Self::NotHexadecimal { text } => write!(
                formatter,
                "`{text}` is not six hexadecimal digits, so no colour was resolved"
            ),
        }
    }
}

impl std::error::Error for ColourRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Value(source) => Some(source),
            Self::NotHexadecimal { .. } => None,
        }
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
    /// Returns [`ColourRefused`] for a missing or wrongly typed setting, or the
    /// first colour whose text is not hexadecimal.
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
            let written = resolved.text(&role.key()).map_err(ColourRefused::Value)?;
            read.push(parse_colour(written)?);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_refusals_preserve_configuration_sources_only() {
        let missing = ColourRefused::Value(AccessError {
            key: COLOURS[0].key(),
            reason: ravel_sdk::config::ValueError::Unknown {
                key: COLOURS[0].key(),
            },
        });
        assert!(std::error::Error::source(&missing).is_some());
        let malformed = parse_colour("zzzzzz").expect_err("not hexadecimal");
        assert!(std::error::Error::source(&malformed).is_none());
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
}
