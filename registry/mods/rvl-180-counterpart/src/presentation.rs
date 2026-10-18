//! The eighteen translated lines, and the two read-only displays behind them.
//!
//! §5 of the specification recovers the whole of what a player ever sees of this
//! entry from one language document of eighteen keys: nine belonging to three
//! commands and nine to two displays. The language-parity test is against those
//! eighteen, so they are a fixed table here rather than something assembled at
//! run time.
//!
//! Both displays are read-only presentations of a quantity a dependent supplies.
//! §8's units are player-visible and are kept: energy is counted in RF and fluid
//! in mB.
//!
//! Nothing here draws. ADG-6 puts every Ravel-owned screen in data drawn by
//! Java, and Ravel publishes no drawing path for a mod at all, so what this
//! module owns is the numbers, the labels and the fill a drawing would use.

use std::fmt;

/// The one locale this package supplies text for.
pub const LOCALE: &str = "en_us";

/// The unit an energy display counts in.
pub const ENERGY_UNIT: &str = "RF";

/// The unit a fluid display counts in.
pub const FLUID_UNIT: &str = "mB";

/// Every translated line, in the order the specification counts them.
///
/// Nine command lines and nine display lines. §5 counts eighteen keys, and this
/// table is what the language-parity test is held to.
pub const LINES: [(&str, &str); 18] = [
    ("command.tally.total", "%s entities are loaded."),
    ("command.tally.none", "No entity is loaded."),
    ("command.reclaim.before", "Held before: %s."),
    ("command.reclaim.after", "Held after: %s."),
    ("command.reclaim.done", "Reclaimed %s held entries."),
    ("command.sweep.total", "Removed %s entities."),
    ("command.sweep.kind", "%s: %s."),
    (
        "command.sweep.none",
        "Nothing matched, so nothing was removed.",
    ),
    (
        "command.sweep.refused",
        "That selection holds players, so nothing was removed.",
    ),
    ("display.energy.title", "Energy storage"),
    ("display.energy.capacity", "Capacity"),
    ("display.energy.stored", "Stored"),
    ("display.energy.unit", ENERGY_UNIT),
    ("display.fluid.title", "Fluid storage"),
    ("display.fluid.capacity", "Capacity"),
    ("display.fluid.contained", "Contained"),
    ("display.fluid.stored", "Stored"),
    ("display.fluid.unit", FLUID_UNIT),
];

/// How many translated lines this package supplies.
pub const LINE_COUNT: usize = LINES.len();

/// Returns the English text of `key`, if this package supplies it.
#[must_use]
pub fn text(key: &str) -> Option<&'static str> {
    LINES
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, text)| *text)
}

/// Which of the two displays a set of labels belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Display {
    /// The energy display, counted in RF.
    Energy,
    /// The fluid display, counted in mB.
    Fluid,
}

impl Display {
    /// Both displays.
    pub const ALL: [Self; 2] = [Self::Energy, Self::Fluid];

    /// Returns the key of the display's title.
    #[must_use]
    pub const fn title_key(self) -> &'static str {
        match self {
            Self::Energy => "display.energy.title",
            Self::Fluid => "display.fluid.title",
        }
    }

    /// Returns the key of the display's unit.
    #[must_use]
    pub const fn unit_key(self) -> &'static str {
        match self {
            Self::Energy => "display.energy.unit",
            Self::Fluid => "display.fluid.unit",
        }
    }

    /// Returns the unit the display counts in.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Energy => ENERGY_UNIT,
            Self::Fluid => FLUID_UNIT,
        }
    }

    /// Returns every key this display labels itself with.
    #[must_use]
    pub const fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Energy => &[
                "display.energy.title",
                "display.energy.capacity",
                "display.energy.stored",
                "display.energy.unit",
            ],
            Self::Fluid => &[
                "display.fluid.title",
                "display.fluid.capacity",
                "display.fluid.contained",
                "display.fluid.stored",
                "display.fluid.unit",
            ],
        }
    }
}

impl fmt::Display for Display {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Energy => "energy",
            Self::Fluid => "fluid",
        })
    }
}

/// How long the name of a contained fluid may be, in bytes.
///
/// The name arrives from a dependent or off the wire, so it is bounded where it
/// is read rather than trusted for the length of a display. The bound is over
/// bytes rather than characters because bytes are what a readout costs on the
/// wire: sixty-four characters of a script outside Latin is up to four times
/// that in UTF-8.
pub const MAX_FLUID_NAME: usize = 64;

/// A read-only view of a quantity held against a capacity.
///
/// The stored amount is clamped to the capacity at construction, so a display
/// can never show more held than can be held and [`Readout::filled`] can never
/// leave zero to one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Readout {
    display: Display,
    capacity: u32,
    stored: u32,
    contained: Option<String>,
}

impl Readout {
    /// Builds an energy display over a capacity and what is in it.
    #[must_use]
    pub fn energy(capacity: u32, stored: u32) -> Self {
        Self {
            display: Display::Energy,
            capacity,
            stored: stored.min(capacity),
            contained: None,
        }
    }

    /// Builds a fluid display over a capacity, what is in it, and what that is.
    ///
    /// The name is truncated to [`MAX_FLUID_NAME`] bytes rather than refused,
    /// because a display with a long name is still a display and refusing one
    /// would leave a dependent with nothing to show.
    #[must_use]
    pub fn fluid(capacity: u32, stored: u32, contained: &str) -> Self {
        Self {
            display: Display::Fluid,
            capacity,
            stored: stored.min(capacity),
            contained: Some(bounded(contained)),
        }
    }

    /// Returns which display this is.
    #[must_use]
    pub const fn display(&self) -> Display {
        self.display
    }

    /// Returns how much can be held.
    #[must_use]
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Returns how much is held.
    #[must_use]
    pub const fn stored(&self) -> u32 {
        self.stored
    }

    /// Returns what is held, for a display that names it.
    #[must_use]
    pub fn contained(&self) -> Option<&str> {
        self.contained.as_deref()
    }

    /// Returns the fraction of the capacity that is held, from zero to one.
    ///
    /// A capacity of zero answers zero rather than dividing by it, so the value
    /// is always finite. A drawing that scaled a bar by a value that was not a
    /// number would leave the bar in whatever state it was last drawn in, which
    /// is exactly the stuck display this guard exists to prevent.
    #[must_use]
    pub fn filled(&self) -> f64 {
        if self.capacity == 0 {
            return 0.0;
        }
        (f64::from(self.stored) / f64::from(self.capacity)).clamp(0.0, 1.0)
    }
}

/// Truncates `name` to [`MAX_FLUID_NAME`] bytes, on a character boundary.
///
/// Cutting on the bound alone would split a multi-byte character and leave
/// bytes that are not text, so the cut walks back to the boundary below it. A
/// name whose first character is longer than the bound truncates to nothing,
/// which is a display naming nothing rather than a display naming rubbish.
fn bounded(name: &str) -> String {
    if name.len() <= MAX_FLUID_NAME {
        return name.to_owned();
    }
    let mut end = MAX_FLUID_NAME;
    while end > 0 && !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::{Display, ENERGY_UNIT, FLUID_UNIT, LINES, MAX_FLUID_NAME, Readout, text};
    use std::collections::BTreeSet;

    #[test]
    fn eighteen_lines_are_supplied_and_each_key_is_spelled_once() {
        let keys: BTreeSet<&str> = LINES.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys.len(), 18);
        assert!(LINES.iter().all(|(_, line)| !line.is_empty()));
    }

    #[test]
    fn nine_lines_belong_to_the_commands_and_nine_to_the_displays() {
        let commands = LINES
            .iter()
            .filter(|(key, _)| key.starts_with("command."))
            .count();
        let displays = LINES
            .iter()
            .filter(|(key, _)| key.starts_with("display."))
            .count();
        assert_eq!(commands, 9);
        assert_eq!(displays, 9);
    }

    #[test]
    fn the_removal_command_keeps_a_line_of_its_own_for_the_player_refusal() {
        let refusal = text("command.sweep.refused").expect("the refusal is supplied");
        assert!(refusal.contains("nothing was removed"));
        assert!(text("command.sweep.none").is_some());
        assert!(text("nothing.supplies.this").is_none());
    }

    #[test]
    fn every_display_key_is_supplied_and_the_units_are_the_two_the_player_reads() {
        for display in Display::ALL {
            for key in display.keys() {
                assert!(text(key).is_some(), "{key} is labelled and not supplied");
            }
            assert_eq!(
                text(display.unit_key()),
                Some(display.unit()),
                "the unit line has to be the unit"
            );
            assert!(text(display.title_key()).is_some());
        }
        assert_eq!(Display::Energy.unit(), ENERGY_UNIT);
        assert_eq!(Display::Fluid.unit(), FLUID_UNIT);
        assert_eq!(Display::Energy.keys().len(), 4);
        assert_eq!(Display::Fluid.keys().len(), 5);
    }

    #[test]
    fn a_readout_clamps_what_is_stored_to_what_can_be_held() {
        let readout = Readout::energy(1_000, 4_000);
        assert_eq!(readout.stored(), 1_000);
        assert!((readout.filled() - 1.0).abs() < f64::EPSILON);
        assert_eq!(readout.contained(), None);
    }

    #[test]
    fn a_readout_over_no_capacity_answers_zero_rather_than_a_value_that_is_not_a_number() {
        let readout = Readout::energy(0, 0);
        let filled = readout.filled();
        assert!(filled.is_finite(), "a stuck display is what this prevents");
        assert!(!filled.is_nan());
        assert!((filled - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_fluid_readout_names_what_it_holds_and_bounds_the_name() {
        let readout = Readout::fluid(500, 250, "water");
        assert_eq!(readout.display(), Display::Fluid);
        assert_eq!(readout.capacity(), 500);
        assert_eq!(readout.contained(), Some("water"));
        assert!((readout.filled() - 0.5).abs() < f64::EPSILON);

        let long = Readout::fluid(1, 1, &"n".repeat(MAX_FLUID_NAME * 4));
        assert_eq!(
            long.contained().expect("a fluid names what it holds").len(),
            MAX_FLUID_NAME
        );
    }

    #[test]
    fn a_multibyte_name_is_bounded_in_bytes_and_cut_on_a_character() {
        for name in ["\u{6c34}", "\u{1f9ca}", "e\u{301}"] {
            let long = Readout::fluid(1, 1, &name.repeat(MAX_FLUID_NAME * 4));
            let held = long.contained().expect("a fluid names what it holds");
            assert!(
                held.len() <= MAX_FLUID_NAME,
                "`{name}` repeated held {} bytes, and the wire pays in bytes",
                held.len()
            );
            assert!(
                held.chars().count() > 0,
                "a name cut to nothing would be a display naming nothing"
            );
            assert!(name.starts_with(held.chars().next().expect("a first character")));
        }
    }
}
