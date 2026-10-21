//! Colour, its packed layout, its naming table and its two encodings, from §11.
//!
//! The packed layout is alpha in the top eight bits, then red, then green, then
//! blue. That layout is a wire contract: it reaches saved data and network
//! payloads through the two encodings below, so it is arithmetic rather than a
//! presentation detail and it lives here rather than in [`crate::presentation`].
//!
//! The two encodings deliberately disagree in shape. The document form is a text
//! spelling parsed permissively; the network form is a leading discriminator
//! byte selecting one of three bodies. A peer decodes by the discriminator, so
//! all three arms are reproduced exactly.
//!
//! Animation is a pure function of the frame it is drawn in. The original runs
//! it on a background timer that mutates shared state the drawing thread reads
//! without synchronisation, and §12 declines that timer outright.

use std::collections::BTreeMap;

use num_traits::ToPrimitive;

/// A red, green, blue and alpha quartet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Colour {
    /// The red channel.
    pub red: u8,
    /// The green channel.
    pub green: u8,
    /// The blue channel.
    pub blue: u8,
    /// The alpha channel, where zero is fully transparent.
    pub alpha: u8,
}

impl Colour {
    /// The fully transparent white the entry treats as its default.
    pub const DEFAULT: Self = Self {
        red: 0xFF,
        green: 0xFF,
        blue: 0xFF,
        alpha: 0x00,
    };

    /// Builds an opaque colour.
    #[must_use]
    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 0xFF,
        }
    }

    /// Builds a colour with an explicit alpha.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Returns the packed form: alpha, then red, then green, then blue.
    #[must_use]
    pub const fn packed(self) -> u32 {
        u32::from_be_bytes([self.alpha, self.red, self.green, self.blue])
    }

    /// Reads a colour out of the packed form.
    #[must_use]
    pub const fn from_packed(packed: u32) -> Self {
        let [alpha, red, green, blue] = packed.to_be_bytes();
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Returns whether this is the default colour's exact quartet.
    #[must_use]
    pub fn is_default_quartet(self) -> bool {
        self == Self::DEFAULT
    }

    /// Returns this colour with its hue turned by `degrees`.
    ///
    /// Saturation, value and alpha are all kept. This is the whole of what an
    /// animated colour does, expressed as a function of an angle so that a
    /// caller can supply the angle the frame implies.
    #[must_use]
    pub fn with_hue_turned(self, degrees: f32) -> Self {
        let (hue, saturation, value) = self.to_hsv();
        let turned = (hue + degrees).rem_euclid(360.0);
        Self::from_hsv(turned, saturation, value, self.alpha)
    }

    fn to_hsv(self) -> (f32, f32, f32) {
        let red = f32::from(self.red) / 255.0;
        let green = f32::from(self.green) / 255.0;
        let blue = f32::from(self.blue) / 255.0;
        let high = red.max(green).max(blue);
        let low = red.min(green).min(blue);
        let span = high - low;
        let hue = if span <= f32::EPSILON {
            0.0
        } else if (high - red).abs() <= f32::EPSILON {
            60.0 * ((green - blue) / span)
        } else if (high - green).abs() <= f32::EPSILON {
            60.0 * (((blue - red) / span) + 2.0)
        } else {
            60.0 * (((red - green) / span) + 4.0)
        };
        let saturation = if high <= f32::EPSILON {
            0.0
        } else {
            span / high
        };
        (hue.rem_euclid(360.0), saturation, high)
    }

    fn from_hsv(hue: f32, saturation: f32, value: f32, alpha: u8) -> Self {
        let chroma = value * saturation;
        let sector = hue.rem_euclid(360.0) / 60.0;
        let second = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
        // Wrapped finite sectors fit u32; a non-finite hue keeps the original zero sector.
        let (red, green, blue) = match sector.trunc().to_u32().unwrap_or(0) {
            0 => (chroma, second, 0.0),
            1 => (second, chroma, 0.0),
            2 => (0.0, chroma, second),
            3 => (0.0, second, chroma),
            4 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };
        let base = value - chroma;
        Self {
            red: channel(red + base),
            green: channel(green + base),
            blue: channel(blue + base),
            alpha,
        }
    }
}

fn channel(value: f32) -> u8 {
    let scaled = (value * 255.0).round();
    if scaled.is_nan() {
        return 0;
    }
    if scaled <= 0.0 {
        0
    } else if scaled >= 255.0 {
        255
    } else {
        scaled
            .to_u8()
            .expect("rounded channel lies between zero and 255")
    }
}

/// A colour that turns its own hue by a fixed step.
///
/// **Declared change.** The original mutates one shared instance from a
/// ten-thread timer pool that nothing ever shuts down, and the drawing thread
/// reads that instance with no synchronisation. This is the same visible
/// behaviour with neither the pool nor the race: the value is a function of the
/// frame it is asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animated {
    base: Colour,
    step_degrees: f32,
}

impl Animated {
    /// The step the shipped animated colour turns by, per frame.
    pub const STEP_DEGREES: f32 = 1.0;

    /// Builds an animated colour turning by a step each frame.
    #[must_use]
    pub const fn new(base: Colour, step_degrees: f32) -> Self {
        Self { base, step_degrees }
    }

    /// Builds the shipped animated colour over a base.
    #[must_use]
    pub const fn shipped(base: Colour) -> Self {
        Self::new(base, Self::STEP_DEGREES)
    }

    /// Returns the colour this animation shows on `frame`.
    ///
    /// The frame count is the caller's, so two readers on the same frame see the
    /// same colour and a value never changes between two reads within a frame.
    #[must_use]
    pub fn at(self, frame: u64) -> Colour {
        let turns = f32::from(u16::try_from(frame % 360).expect("a turn is below 360 degrees"));
        self.base.with_hue_turned(self.step_degrees * turns)
    }

    /// Returns the colour the animation turns around.
    #[must_use]
    pub const fn base(self) -> Colour {
        self.base
    }
}

/// What one entry in the naming table holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Named {
    /// A plain colour under a name.
    Plain(Colour),
    /// An animated colour under a name.
    Animated(Animated),
}

impl Named {
    /// Returns the colour this entry shows on `frame`.
    #[must_use]
    pub fn at(self, frame: u64) -> Colour {
        match self {
            Self::Plain(colour) => colour,
            Self::Animated(animated) => animated.at(frame),
        }
    }
}

/// The process-wide naming table, keyed by the lowercased name.
///
/// Both registration paths are first-registration-wins and they say so
/// differently, which is the original's shape and is kept: registering a plain
/// colour reports whether it landed, and registering an animated one hands back
/// whatever is already under the name.
#[derive(Clone, Debug, Default)]
pub struct Names {
    entries: BTreeMap<String, Named>,
}

/// The most names the table will hold.
///
/// A naming table is written from documents a pack author supplies, so it is
/// bounded rather than trusted. The ceiling is far above any plausible palette
/// and the upgrade path is a larger constant.
pub const MAX_NAMES: usize = 4096;

impl Names {
    /// Builds an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a plain colour, keeping any entry already under the name.
    ///
    /// Returns what the registration did: [`Placed::Landed`] when the name was
    /// free, [`Placed::Existing`] with what is under it otherwise, and
    /// [`Placed::Full`] at [`MAX_NAMES`]. The last two are different failures
    /// and a caller that cannot tell them apart drops a palette past the ceiling
    /// without knowing it did.
    pub fn register(&mut self, name: &str, colour: Colour) -> Placed {
        self.place(name, Named::Plain(colour))
    }

    /// Registers an animated colour, returning what is under the name afterwards.
    ///
    /// An existing entry is returned rather than replaced, so this is
    /// first-registration-wins by a second mechanism.
    pub fn register_animated(&mut self, name: &str, animated: Animated) -> Option<Named> {
        match self.place(name, Named::Animated(animated)) {
            Placed::Landed => Some(Named::Animated(animated)),
            Placed::Existing(entry) => Some(entry),
            Placed::Full => None,
        }
    }

    fn place(&mut self, name: &str, entry: Named) -> Placed {
        let key = name.to_lowercase();
        if let Some(existing) = self.entries.get(&key) {
            return Placed::Existing(*existing);
        }
        if self.entries.len() >= MAX_NAMES {
            return Placed::Full;
        }
        self.entries.insert(key, entry);
        Placed::Landed
    }

    /// Returns the entry under a name, matched case-insensitively.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Named> {
        self.entries.get(&name.to_lowercase()).copied()
    }

    /// Returns how many names are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What one registration did.
///
/// A registration that did not land says which of the two reasons it was: a
/// caller told only "not you" would read a full table as a name it had already
/// registered, and would drop the rest of its palette silently.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placed {
    /// The name was free and now holds this entry.
    Landed,
    /// The name already held an entry, which is returned rather than replaced.
    Existing(Named),
    /// The table is at [`MAX_NAMES`] and took nothing.
    Full,
}

/// One colour value, as the two encodings see it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// The default colour, which the network form spells as a bare
    /// discriminator and no body at all.
    Default,
    /// A colour held under a name, which the network form spells as its name so
    /// the receiver resolves it in its own table and animates independently.
    Named(String),
    /// A colour spelled outright, which the network form spells as the packed
    /// integer.
    Packed(Colour),
}

/// The discriminator byte the network form leads with.
pub mod discriminator {
    /// The default colour, followed by nothing.
    pub const DEFAULT: u8 = 0;
    /// A named colour, followed by a text spelling.
    pub const NAMED: u8 = 1;
    /// Any other colour, followed by the packed integer.
    pub const PACKED: u8 = 2;
}

/// The most bytes a name may occupy on the wire.
pub const MAX_NAME_BYTES: usize = 128;

/// Why a colour did not decode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColourError {
    /// The buffer ended before the value did.
    Truncated,
    /// The leading byte named no arm.
    UnknownDiscriminator(u8),
    /// A name's bytes are not text.
    NotText,
    /// A name is longer than [`MAX_NAME_BYTES`].
    NameTooLong(usize),
    /// A document spelling matched no accepted form.
    Unspellable(String),
}

impl std::fmt::Display for ColourError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(formatter, "the buffer ended inside a colour"),
            Self::UnknownDiscriminator(byte) => {
                write!(formatter, "a colour led with the unknown arm {byte}")
            }
            Self::NotText => write!(formatter, "a colour's name is not text"),
            Self::NameTooLong(len) => {
                write!(
                    formatter,
                    "a colour's name is {len} bytes, over the ceiling"
                )
            }
            Self::Unspellable(text) => write!(formatter, "`{text}` spells no colour"),
        }
    }
}

impl std::error::Error for ColourError {}

impl Value {
    /// Writes the network form: a discriminator and one of three bodies.
    ///
    /// # Errors
    ///
    /// Returns [`ColourError::NameTooLong`] for a name past [`MAX_NAME_BYTES`],
    /// which is the ceiling the reader refuses at. Writing it anyway would
    /// encode a name a legal registration produced into a packet the peer at
    /// the other end refuses.
    pub fn to_wire(&self) -> Result<Vec<u8>, ColourError> {
        let mut bytes = Vec::new();
        match self {
            Self::Default => bytes.push(discriminator::DEFAULT),
            Self::Named(name) => {
                let text = name.as_bytes();
                if text.len() > MAX_NAME_BYTES {
                    return Err(ColourError::NameTooLong(text.len()));
                }
                bytes.push(discriminator::NAMED);
                let len = u32::try_from(text.len()).expect("a packet list shorter than 2^32");
                bytes.extend_from_slice(&len.to_be_bytes());
                bytes.extend_from_slice(text);
            }
            Self::Packed(colour) => {
                bytes.push(discriminator::PACKED);
                bytes.extend_from_slice(&colour.packed().to_be_bytes());
            }
        }
        Ok(bytes)
    }

    /// Reads the network form.
    ///
    /// # Errors
    ///
    /// Returns [`ColourError`] for an unknown arm, a buffer that ends inside the
    /// value, a name that is not text, or a name past [`MAX_NAME_BYTES`].
    pub fn from_wire(bytes: &[u8]) -> Result<(Self, usize), ColourError> {
        let (&head, rest) = bytes.split_first().ok_or(ColourError::Truncated)?;
        match head {
            discriminator::DEFAULT => Ok((Self::Default, 1)),
            discriminator::NAMED => {
                if rest.len() < 4 {
                    return Err(ColourError::Truncated);
                }
                let (prefix, tail) = rest.split_at(4);
                let announced = u32::from_be_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]);
                let len = usize::try_from(announced).map_err(|_| ColourError::Truncated)?;
                if len > MAX_NAME_BYTES {
                    return Err(ColourError::NameTooLong(len));
                }
                if tail.len() < len {
                    return Err(ColourError::Truncated);
                }
                let name = std::str::from_utf8(&tail[..len])
                    .map_err(|_| ColourError::NotText)?
                    .to_owned();
                Ok((Self::Named(name), 5 + len))
            }
            discriminator::PACKED => {
                if rest.len() < 4 {
                    return Err(ColourError::Truncated);
                }
                let packed = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
                Ok((Self::Packed(Colour::from_packed(packed)), 5))
            }
            other => Err(ColourError::UnknownDiscriminator(other)),
        }
    }

    /// Writes the document form: one text spelling.
    #[must_use]
    pub fn to_document(&self) -> String {
        match self {
            Self::Default => "default".to_owned(),
            Self::Named(name) => name.clone(),
            Self::Packed(colour) => format!("#{:08X}", colour.packed()),
        }
    }

    /// Reads the document form, permissively.
    ///
    /// The accepted spellings are stated here rather than guessed: the word
    /// `default`; a `#` or `0x` prefixed hexadecimal of six or eight digits; a
    /// bare hexadecimal of six or eight digits; a decimal integer inside the
    /// unsigned thirty-two bit range; and anything else as a name. A six-digit
    /// spelling is opaque, because a document that named three channels meant a
    /// colour a reader can see.
    ///
    /// # Errors
    ///
    /// Returns [`ColourError::Unspellable`] for an empty spelling and for one
    /// whose length or characters match no form above.
    pub fn from_document(text: &str) -> Result<Self, ColourError> {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_NAME_BYTES {
            return Err(ColourError::Unspellable(text.to_owned()));
        }
        if trimmed.eq_ignore_ascii_case("default") {
            return Ok(Self::Default);
        }
        let digits = trimmed
            .strip_prefix('#')
            .or_else(|| trimmed.strip_prefix("0x"))
            .or_else(|| trimmed.strip_prefix("0X"));
        let explicit = digits.is_some();
        let body = digits.unwrap_or(trimmed);
        if (body.len() == 6 || body.len() == 8) && body.bytes().all(|b| b.is_ascii_hexdigit()) {
            let value =
                u32::from_str_radix(body, 16).expect("six or eight hexadecimal digits fit u32");
            let colour = if body.len() == 6 {
                Colour::from_packed(value | 0xFF00_0000)
            } else {
                Colour::from_packed(value)
            };
            return Ok(Self::Packed(colour));
        }
        if explicit {
            return Err(ColourError::Unspellable(text.to_owned()));
        }
        if trimmed.bytes().all(|b| b.is_ascii_digit())
            && let Ok(value) = trimmed.parse::<u32>()
        {
            return Ok(Self::Packed(Colour::from_packed(value)));
        }
        Ok(Self::Named(trimmed.to_owned()))
    }

    /// Resolves this value against a naming table on a frame.
    ///
    /// A name nothing holds resolves to the default, which is what a receiver
    /// that lacks the sender's palette shows.
    #[must_use]
    pub fn resolve(&self, names: &Names, frame: u64) -> Colour {
        match self {
            Self::Default => Colour::DEFAULT,
            Self::Named(name) => names
                .get(name)
                .map_or(Colour::DEFAULT, |entry| entry.at(frame)),
            Self::Packed(colour) => *colour,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Animated, Colour, ColourError, MAX_NAME_BYTES, Names, Placed, Value, discriminator,
    };

    #[test]
    fn channel_rounding_keeps_nonfinite_and_half_byte_edges() {
        assert_eq!(super::channel(f32::NAN), 0);
        assert_eq!(super::channel(f32::INFINITY), 255);
        assert_eq!(super::channel(f32::NEG_INFINITY), 0);
        assert_eq!(super::channel(0.5), 128);
    }

    #[test]
    fn the_packed_layout_is_alpha_red_green_blue_from_the_top() {
        let colour = Colour::new(0x12, 0x34, 0x56, 0x78);
        assert_eq!(colour.packed(), 0x7812_3456);
        assert_eq!(Colour::from_packed(0x7812_3456), colour);
    }

    #[test]
    fn every_quartet_round_trips_through_the_packed_form() {
        for value in [0x0000_0000, 0xFFFF_FFFF, 0x8000_0001, 0x00FF_00FF] {
            assert_eq!(Colour::from_packed(value).packed(), value);
        }
    }

    #[test]
    fn a_plain_registration_does_not_replace_what_is_under_the_name() {
        let mut names = Names::new();
        assert_eq!(
            names.register("Accent", Colour::opaque(1, 2, 3)),
            Placed::Landed
        );
        assert_eq!(
            names.register("accent", Colour::opaque(4, 5, 6)),
            Placed::Existing(super::Named::Plain(Colour::opaque(1, 2, 3))),
            "a name already held says so, and says what holds it"
        );
        assert_eq!(
            names.get("ACCENT"),
            Some(super::Named::Plain(Colour::opaque(1, 2, 3)))
        );
    }

    #[test]
    fn an_animated_registration_hands_back_the_existing_entry_rather_than_replacing_it() {
        let mut names = Names::new();
        let first = Animated::shipped(Colour::opaque(10, 20, 30));
        let second = Animated::shipped(Colour::opaque(40, 50, 60));
        assert_eq!(
            names.register_animated("cycle", first),
            Some(super::Named::Animated(first))
        );
        assert_eq!(
            names.register_animated("cycle", second),
            Some(super::Named::Animated(first)),
            "the second call reports the first, not itself"
        );
    }

    #[test]
    fn the_naming_table_is_bounded() {
        let mut names = Names::new();
        for index in 0..super::MAX_NAMES {
            assert_eq!(
                names.register(&format!("n{index}"), Colour::DEFAULT),
                Placed::Landed
            );
        }
        assert_eq!(names.len(), super::MAX_NAMES);
        assert_eq!(
            names.register("one_too_many", Colour::DEFAULT),
            Placed::Full,
            "a full table is a different answer from a name already taken"
        );
        assert!(names.get("one_too_many").is_none());
        assert!(
            names
                .register_animated("also_too_many", Animated::shipped(Colour::DEFAULT))
                .is_none()
        );
    }

    #[test]
    fn an_animated_colour_is_a_function_of_the_frame_and_never_of_a_timer() {
        let animated = Animated::new(Colour::opaque(0xFF, 0x00, 0x00), 60.0);
        assert_eq!(
            animated.at(7),
            animated.at(7),
            "two reads on one frame agree"
        );
        assert_ne!(animated.at(0), animated.at(1));
        assert_eq!(animated.at(0), Colour::opaque(0xFF, 0x00, 0x00));
        assert_eq!(
            animated.at(6),
            animated.at(0),
            "six sixty-degree steps return"
        );
    }

    #[test]
    fn turning_a_hue_keeps_the_alpha() {
        let colour = Colour::new(0x20, 0x80, 0xC0, 0x40);
        assert_eq!(colour.with_hue_turned(90.0).alpha, 0x40);
        assert_eq!(colour.with_hue_turned(360.0), colour);
    }

    #[test]
    fn a_grey_has_no_hue_to_turn() {
        let grey = Colour::opaque(0x40, 0x40, 0x40);
        assert_eq!(grey.with_hue_turned(123.0), grey);
    }

    #[test]
    fn the_three_network_arms_lead_with_their_own_discriminator() {
        assert_eq!(
            Value::Default
                .to_wire()
                .expect("a short name is inside the ceiling"),
            vec![discriminator::DEFAULT]
        );
        assert_eq!(
            Value::Named("a".to_owned())
                .to_wire()
                .expect("a short name is inside the ceiling")[0],
            discriminator::NAMED
        );
        assert_eq!(
            Value::Packed(Colour::DEFAULT)
                .to_wire()
                .expect("a short name is inside the ceiling")[0],
            discriminator::PACKED
        );
    }

    #[test]
    fn the_default_arm_carries_no_body_at_all() {
        assert_eq!(
            Value::Default
                .to_wire()
                .expect("a short name is inside the ceiling")
                .len(),
            1
        );
        let (value, read) = Value::from_wire(&[discriminator::DEFAULT]).expect("one byte is whole");
        assert_eq!(value, Value::Default);
        assert_eq!(read, 1);
    }

    #[test]
    fn every_arm_round_trips_and_reports_what_it_read() {
        for value in [
            Value::Default,
            Value::Named("accent".to_owned()),
            Value::Packed(Colour::new(1, 2, 3, 4)),
        ] {
            let bytes = value.to_wire().expect("a short name is inside the ceiling");
            let (read, used) = Value::from_wire(&bytes).expect("its own bytes read back");
            assert_eq!(read, value);
            assert_eq!(used, bytes.len());
        }
    }

    #[test]
    fn a_value_followed_by_more_reads_only_its_own_bytes() {
        let mut bytes = Value::Packed(Colour::new(9, 8, 7, 6))
            .to_wire()
            .expect("a short name is inside the ceiling");
        bytes.extend_from_slice(b"and more");
        let (value, used) = Value::from_wire(&bytes).expect("the value is whole");
        assert_eq!(value, Value::Packed(Colour::new(9, 8, 7, 6)));
        assert_eq!(used, 5);
    }

    #[test]
    fn a_colour_is_refused_five_ways_off_the_wire() {
        assert_eq!(Value::from_wire(&[]), Err(ColourError::Truncated));
        assert_eq!(
            Value::from_wire(&[9]),
            Err(ColourError::UnknownDiscriminator(9))
        );
        assert_eq!(
            Value::from_wire(&[discriminator::PACKED, 1, 2]),
            Err(ColourError::Truncated)
        );
        let mut lying = vec![discriminator::NAMED];
        lying.extend_from_slice(&9999u32.to_be_bytes());
        assert_eq!(
            Value::from_wire(&lying),
            Err(ColourError::NameTooLong(9999))
        );
        let mut not_text = vec![discriminator::NAMED];
        not_text.extend_from_slice(&2u32.to_be_bytes());
        not_text.extend_from_slice(&[0xFF, 0xFE]);
        assert_eq!(Value::from_wire(&not_text), Err(ColourError::NotText));
    }

    #[test]
    fn a_lying_length_prefix_is_refused_before_anything_is_read() {
        let mut bytes = vec![discriminator::NAMED];
        bytes.extend_from_slice(&40u32.to_be_bytes());
        bytes.extend_from_slice(b"short");
        assert_eq!(Value::from_wire(&bytes), Err(ColourError::Truncated));
    }

    #[test]
    fn the_document_form_accepts_the_stated_spellings() {
        assert_eq!(Value::from_document("default"), Ok(Value::Default));
        assert_eq!(Value::from_document("  DEFAULT "), Ok(Value::Default));
        assert_eq!(
            Value::from_document("#112233"),
            Ok(Value::Packed(Colour::opaque(0x11, 0x22, 0x33)))
        );
        assert_eq!(
            Value::from_document("0x40112233"),
            Ok(Value::Packed(Colour::new(0x11, 0x22, 0x33, 0x40)))
        );
        assert_eq!(
            Value::from_document("112233"),
            Ok(Value::Packed(Colour::opaque(0x11, 0x22, 0x33)))
        );
        assert_eq!(
            Value::from_document("255"),
            Ok(Value::Packed(Colour::from_packed(255)))
        );
        assert_eq!(
            Value::from_document("accent"),
            Ok(Value::Named("accent".to_owned()))
        );
    }

    #[test]
    fn a_six_digit_spelling_is_opaque_rather_than_transparent() {
        let Ok(Value::Packed(colour)) = Value::from_document("#000000") else {
            panic!("six digits spell a colour");
        };
        assert_eq!(colour.alpha, 0xFF);
    }

    #[test]
    fn a_document_spelling_is_refused_three_ways() {
        assert_eq!(
            Value::from_document("   "),
            Err(ColourError::Unspellable("   ".to_owned()))
        );
        assert_eq!(
            Value::from_document("#12345"),
            Err(ColourError::Unspellable("#12345".to_owned()))
        );
        let long = "n".repeat(MAX_NAME_BYTES + 1);
        assert_eq!(
            Value::from_document(&long),
            Err(ColourError::Unspellable(long))
        );
    }

    #[test]
    fn the_two_encodings_disagree_in_shape_and_both_are_kept() {
        let value = Value::Named("accent".to_owned());
        assert_eq!(value.to_document(), "accent");
        assert_eq!(
            value.to_wire().expect("a short name is inside the ceiling")[0],
            discriminator::NAMED
        );
        let packed = Value::Packed(Colour::new(1, 2, 3, 4));
        assert_eq!(packed.to_document(), "#04010203");
        assert_eq!(Value::from_document(&packed.to_document()), Ok(packed));
    }

    #[test]
    fn a_named_colour_travels_as_its_name_so_the_receiver_animates_on_its_own() {
        let mut here = Names::new();
        let mut there = Names::new();
        here.register_animated("cycle", Animated::new(Colour::opaque(0xFF, 0, 0), 60.0));
        there.register_animated("cycle", Animated::new(Colour::opaque(0, 0xFF, 0), 60.0));
        let value = Value::Named("cycle".to_owned());
        let bytes = value.to_wire().expect("a short name is inside the ceiling");
        let (received, _) = Value::from_wire(&bytes).expect("a name reads back");
        assert_eq!(received, value);
        assert_ne!(
            value.resolve(&here, 3),
            received.resolve(&there, 3),
            "the two ends animate from their own tables"
        );
    }

    #[test]
    fn a_name_past_the_readers_ceiling_is_refused_by_the_writer_too() {
        let legal = "n".repeat(MAX_NAME_BYTES);
        let mut names = Names::new();
        assert_eq!(names.register(&legal, Colour::DEFAULT), Placed::Landed);
        assert!(
            Value::Named(legal).to_wire().is_ok(),
            "the ceiling itself still travels"
        );

        let long = "n".repeat(MAX_NAME_BYTES + 1);
        assert_eq!(
            Value::Named(long.clone()).to_wire(),
            Err(ColourError::NameTooLong(long.len())),
            "a packet the peer refuses is not written in the first place"
        );
    }

    #[test]
    fn an_unheld_name_resolves_to_the_default_rather_than_refusing() {
        let names = Names::new();
        assert_eq!(
            Value::Named("absent".to_owned()).resolve(&names, 0),
            Colour::DEFAULT
        );
        assert_eq!(names.len(), 0);
    }
}
