//! The per-player display record of §5.
//!
//! One record per player, persisted with that player and pushed to that
//! player's client. It is the only thing that tells a joining client where its
//! status icons go, and the dirty flag being **set at construction** is what
//! makes that work: the first server-side tick after the record exists pushes
//! it whether or not anything changed.
//!
//! Two rules here look like bugs and are contracts. The record declares a
//! serialisation form and nothing more, so it does **not** survive a death or a
//! dimension change; and resetting does not restore what construction built,
//! because reset anchors top-left where a fresh record anchors bottom-right.

use std::fmt;

/// The offset both axes default to.
pub const DEFAULT_OFFSET: i32 = -20;

/// The style a fresh record and a reset one both prefer.
pub const DEFAULT_STYLE: &str = "flat_gradient";

/// The styles the widget toolkit publishes, in declaration order.
pub const STYLES: [&str; 3] = ["flat_gradient", "flat", "raised"];

/// Which corner of the screen status icons anchor to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Corner {
    /// The top left corner.
    TopLeft,
    /// The top right corner.
    TopRight,
    /// The bottom left corner.
    BottomLeft,
    /// The bottom right corner.
    BottomRight,
}

impl Corner {
    /// Every corner, in ordinal order, which is the order the wire carries.
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    /// Returns the ordinal the wire carries.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::TopLeft => 0,
            Self::TopRight => 1,
            Self::BottomLeft => 2,
            Self::BottomRight => 3,
        }
    }

    /// Returns the corner an ordinal names, refusing one outside the four.
    #[must_use]
    pub const fn from_ordinal(ordinal: u8) -> Option<Self> {
        match ordinal {
            0 => Some(Self::TopLeft),
            1 => Some(Self::TopRight),
            2 => Some(Self::BottomLeft),
            3 => Some(Self::BottomRight),
            _ => None,
        }
    }
}

/// One player's display preferences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preferences {
    corner: Corner,
    horizontal: i32,
    vertical: i32,
    style: String,
    dirty: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self::new()
    }
}

impl Preferences {
    /// Builds a fresh record, already dirty.
    ///
    /// The flag is set here rather than on the first change, and that is the
    /// whole of how a joining client ever learns these values: nothing else
    /// sends them.
    #[must_use]
    pub fn new() -> Self {
        Self {
            corner: Corner::BottomRight,
            horizontal: DEFAULT_OFFSET,
            vertical: DEFAULT_OFFSET,
            style: DEFAULT_STYLE.to_owned(),
            dirty: true,
        }
    }

    /// Returns the corner status icons anchor to.
    #[must_use]
    pub const fn corner(&self) -> Corner {
        self.corner
    }

    /// Returns the horizontal offset.
    #[must_use]
    pub const fn horizontal(&self) -> i32 {
        self.horizontal
    }

    /// Returns the vertical offset.
    #[must_use]
    pub const fn vertical(&self) -> i32 {
        self.vertical
    }

    /// Returns the preferred widget style.
    #[must_use]
    pub fn style(&self) -> &str {
        &self.style
    }

    /// Returns whether the record has changes the next tick should push.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Sets the anchor corner, refusing only a missing value.
    ///
    /// The enumeration setter refuses nothing else, which is the asymmetry §5
    /// states against the style setter below.
    pub fn set_corner(&mut self, corner: Option<Corner>) -> bool {
        match corner {
            None => false,
            Some(corner) => {
                self.corner = corner;
                self.dirty = true;
                true
            }
        }
    }

    /// Sets both offsets.
    pub const fn set_offsets(&mut self, horizontal: i32, vertical: i32) {
        self.horizontal = horizontal;
        self.vertical = vertical;
        self.dirty = true;
    }

    /// Sets the preferred style by name, refusing an unknown one.
    ///
    /// An unknown name changes nothing and answers false rather than raising,
    /// because this is reached from a client-to-server payload and a name a
    /// client invented is ordinary untrusted input.
    pub fn set_style(&mut self, name: &str) -> bool {
        if !STYLES.contains(&name) {
            return false;
        }
        name.clone_into(&mut self.style);
        self.dirty = true;
        true
    }

    /// Returns the record to its reset state, which is not its fresh state.
    ///
    /// The offsets and the style return to their defaults and the anchor goes
    /// to the **top left**, where a freshly constructed record anchors bottom
    /// right. A counterpart that treated reset as "construct a new one" would
    /// disagree with the original on exactly this field.
    pub fn reset(&mut self) {
        self.corner = Corner::TopLeft;
        self.horizontal = DEFAULT_OFFSET;
        self.vertical = DEFAULT_OFFSET;
        DEFAULT_STYLE.clone_into(&mut self.style);
        self.dirty = true;
    }

    /// Takes the record for sending, clearing the flag.
    ///
    /// A clean record answers nothing, so a tick that has nothing to say sends
    /// nothing. There is no batching and no rate limit beyond one send per
    /// player tick, which is the caller's cadence rather than this record's.
    pub fn take_for_send(&mut self) -> Option<Self> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some(Self {
            dirty: false,
            ..self.clone()
        })
    }

    /// Writes the record in its wire form.
    #[must_use]
    pub fn to_wire(&self) -> Vec<u8> {
        let mut bytes = vec![self.corner.ordinal()];
        bytes.extend_from_slice(&self.horizontal.to_be_bytes());
        bytes.extend_from_slice(&self.vertical.to_be_bytes());
        let style = self.style.as_bytes();
        bytes.extend_from_slice(&crate::arithmetic::signed_len(style.len()).to_be_bytes());
        bytes.extend_from_slice(style);
        bytes
    }

    /// Reads a record back from its wire form.
    ///
    /// A record that arrives is clean: it is what the other side already knows,
    /// so nothing about receiving it asks for it to be sent back.
    ///
    /// # Errors
    ///
    /// Returns [`PreferencesRefused`] for a buffer that ends inside a field,
    /// carries trailing bytes, contains invalid UTF-8, names a corner outside
    /// the four, or names a style this build does not publish.
    pub fn from_wire(bytes: &[u8]) -> Result<Self, PreferencesRefused> {
        let (&ordinal, rest) = bytes.split_first().ok_or(PreferencesRefused::Truncated)?;
        let corner = Corner::from_ordinal(ordinal).ok_or(PreferencesRefused::Corner { ordinal })?;
        if rest.len() < 12 {
            return Err(PreferencesRefused::Truncated);
        }
        let (offsets, rest) = rest.split_at(8);
        let horizontal = i32::from_be_bytes([offsets[0], offsets[1], offsets[2], offsets[3]]);
        let vertical = i32::from_be_bytes([offsets[4], offsets[5], offsets[6], offsets[7]]);
        let (header, body) = rest.split_at(4);
        let announced = i32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let length = usize::try_from(announced).map_err(|_| PreferencesRefused::Truncated)?;
        if length > body.len() {
            return Err(PreferencesRefused::Truncated);
        }
        if length < body.len() {
            return Err(PreferencesRefused::Trailing);
        }
        let name = std::str::from_utf8(body).map_err(|_| PreferencesRefused::NotText)?;
        if !STYLES.contains(&name) {
            return Err(PreferencesRefused::Style {
                name: name.to_owned(),
            });
        }
        Ok(Self {
            corner,
            horizontal,
            vertical,
            style: name.to_owned(),
            dirty: false,
        })
    }
}

/// Why a preference record was not read back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreferencesRefused {
    /// The record is incomplete or announces a negative style length.
    Truncated,
    /// The buffer carries bytes after the complete preference record.
    Trailing,
    /// The style field is not valid UTF-8.
    NotText,
    /// The corner ordinal is outside the four the layout declares.
    Corner {
        /// What the buffer carried.
        ordinal: u8,
    },
    /// The style is not one this build publishes.
    Style {
        /// What the buffer carried.
        name: String,
    },
}

impl fmt::Display for PreferencesRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter
                .write_str("the preference record is incomplete or has a negative style length"),
            Self::Trailing => formatter.write_str("the preference record carries trailing bytes"),
            Self::NotText => formatter.write_str("the preference style is not valid UTF-8"),
            Self::Corner { ordinal } => {
                write!(
                    formatter,
                    "{ordinal} names no corner; the layout declares four"
                )
            }
            Self::Style { name } => {
                write!(formatter, "`{name}` is not a style this build publishes")
            }
        }
    }
}

impl std::error::Error for PreferencesRefused {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_record_anchors_bottom_right_at_minus_twenty_on_both_axes() {
        let fresh = Preferences::new();
        assert_eq!(fresh.corner(), Corner::BottomRight);
        assert_eq!(fresh.horizontal(), -20);
        assert_eq!(fresh.vertical(), -20);
        assert_eq!(fresh.style(), "flat_gradient");
    }

    #[test]
    fn a_fresh_record_is_dirty_so_the_first_tick_always_pushes_it() {
        let mut fresh = Preferences::new();
        assert!(fresh.is_dirty());
        let sent = fresh
            .take_for_send()
            .expect("a fresh record always sends once");
        assert_eq!(sent.corner(), Corner::BottomRight);
        assert!(!fresh.is_dirty());
        assert_eq!(fresh.take_for_send(), None, "a clean record sends nothing");
    }

    #[test]
    fn every_mutation_sets_the_flag_again_and_sending_clears_it() {
        let mut preferences = Preferences::new();
        let _ = preferences.take_for_send();

        preferences.set_offsets(4, 5);
        assert!(preferences.is_dirty());
        let _ = preferences.take_for_send();
        assert!(!preferences.is_dirty());

        assert!(preferences.set_style("raised"));
        assert!(preferences.is_dirty());
        let _ = preferences.take_for_send();

        assert!(preferences.set_corner(Some(Corner::TopRight)));
        assert!(preferences.is_dirty());
    }

    #[test]
    fn a_refused_change_leaves_the_record_alone_and_does_not_dirty_it() {
        let mut preferences = Preferences::new();
        let _ = preferences.take_for_send();

        assert!(!preferences.set_style("nothing_publishes_this"));
        assert_eq!(preferences.style(), "flat_gradient");
        assert!(!preferences.is_dirty());

        assert!(!preferences.set_corner(None));
        assert_eq!(preferences.corner(), Corner::BottomRight);
        assert!(!preferences.is_dirty());
    }

    #[test]
    fn resetting_anchors_top_left_where_construction_anchors_bottom_right() {
        let mut preferences = Preferences::new();
        preferences.set_offsets(100, 100);
        assert!(preferences.set_style("flat"));

        preferences.reset();
        assert_eq!(preferences.corner(), Corner::TopLeft);
        assert_eq!(preferences.horizontal(), -20);
        assert_eq!(preferences.vertical(), -20);
        assert_eq!(preferences.style(), "flat_gradient");

        assert_ne!(
            preferences.corner(),
            Preferences::new().corner(),
            "reset is not construction, and the anchor is where they differ"
        );
        assert!(preferences.is_dirty());
    }

    #[test]
    fn the_style_setter_refuses_a_name_it_does_not_publish_and_takes_every_one_it_does() {
        let mut preferences = Preferences::new();
        for name in STYLES {
            assert!(preferences.set_style(name));
            assert_eq!(preferences.style(), name);
        }
        for name in ["", "FLAT", "flat ", "gradient"] {
            assert!(!preferences.set_style(name), "`{name}` is not published");
        }
    }

    #[test]
    fn the_four_corners_keep_their_ordinal_layout() {
        for (ordinal, corner) in Corner::ALL.into_iter().enumerate() {
            let ordinal = u8::try_from(ordinal).expect("four fits in a byte");
            assert_eq!(corner.ordinal(), ordinal);
            assert_eq!(Corner::from_ordinal(ordinal), Some(corner));
        }
        assert_eq!(Corner::from_ordinal(4), None);
    }

    #[test]
    fn a_record_round_trips_over_the_wire_and_arrives_clean() {
        let mut sent = Preferences::new();
        sent.set_offsets(-3, 400);
        assert!(sent.set_style("raised"));

        let received = Preferences::from_wire(&sent.to_wire()).expect("a legal record");
        assert_eq!(received.corner(), sent.corner());
        assert_eq!(received.horizontal(), -3);
        assert_eq!(received.vertical(), 400);
        assert_eq!(received.style(), "raised");
        assert!(
            !received.is_dirty(),
            "receiving what the other side already knows asks for no send back"
        );
    }

    #[test]
    fn a_complete_style_with_invalid_utf8_is_not_reported_as_truncated() {
        let mut wire = Preferences::new().to_wire();
        *wire.last_mut().expect("the style is nonempty") = 0xFF;
        assert_eq!(
            Preferences::from_wire(&wire),
            Err(PreferencesRefused::NotText)
        );
    }

    #[test]
    fn surplus_bytes_are_not_reported_as_truncation() {
        let mut wire = Preferences::new().to_wire();
        wire.push(0);
        assert_eq!(
            Preferences::from_wire(&wire),
            Err(PreferencesRefused::Trailing)
        );
    }

    #[test]
    fn a_record_off_the_wire_is_refused_every_way_it_can_be_wrong() {
        let wire = Preferences::new().to_wire();
        assert_eq!(
            Preferences::from_wire(&[]),
            Err(PreferencesRefused::Truncated)
        );
        assert_eq!(
            Preferences::from_wire(&wire[..wire.len() - 1]),
            Err(PreferencesRefused::Truncated)
        );

        let mut bad_corner = wire.clone();
        bad_corner[0] = 9;
        assert_eq!(
            Preferences::from_wire(&bad_corner),
            Err(PreferencesRefused::Corner { ordinal: 9 })
        );

        let mut bad_style = vec![0_u8];
        bad_style.extend_from_slice(&0_i32.to_be_bytes());
        bad_style.extend_from_slice(&0_i32.to_be_bytes());
        bad_style.extend_from_slice(&4_i32.to_be_bytes());
        bad_style.extend_from_slice(b"nope");
        assert_eq!(
            Preferences::from_wire(&bad_style),
            Err(PreferencesRefused::Style {
                name: "nope".to_owned()
            })
        );

        let mut lying = wire.clone();
        let header = wire.len() - Preferences::new().style().len() - 4;
        lying[header..header + 4].copy_from_slice(&i32::MAX.to_be_bytes());
        assert_eq!(
            Preferences::from_wire(&lying),
            Err(PreferencesRefused::Truncated)
        );
    }
}
