//! The readout block of §12: what it looks at, what it asks for, and which side
//! computes the answer.
//!
//! The split is the load-bearing part. The client never computes a readout: it
//! asks, the server answers with a typed map, and the client holds the last
//! answer and the moment it arrived. [`Screen`] is that client-side memory and
//! it has no way to derive a value of its own.
//!
//! Both of §12's defects are fixed rather than reproduced. The lookup behind the
//! block answers with an absence when the block state is not the expected kind,
//! and every caller here tests that absence instead of using it. And the
//! synthesised energy provider is built once and holds what it read, rather than
//! re-querying on every information request.
//!
//! The typed map the answer travels in belongs to the shared library this entry
//! depends on, and §13 says its encoding is that specification's, so the map is
//! the library's own and this entry does not write a second one.
//! [`Readings::to_shared`] is the crossing: what this entry owns is which values
//! a mode collects, what each is called, and the order they are collected in.
//! Ordering matters because the library's wire form is positional, so the order
//! is part of what this entry publishes rather than a presentation detail.
//!
//! The library's map has no 64-bit member and §12 reports energy as long values,
//! so an amount crosses through the library's own energy clamp (RVL-120 §12.3),
//! which is the mechanism that specification publishes for exactly this: a
//! 64-bit amount reaching an interface that takes 32. `NOTES.md` records it.

use std::fmt;

use rvl_120_counterpart::clamp_energy;
use rvl_120_counterpart::values::{Value as SharedValue, ValueMap};

/// How many server ticks pass between two calls to a provider's own tick.
pub const TICK_INTERVAL: u64 = 10;

/// The mode that reports energy as a pair of numbers.
pub const PLAIN_ENERGY: u8 = 0;

/// The mode that reports energy as a drawn bar.
pub const GRAPHICAL_ENERGY: u8 = 1;

/// The two modes every provider supports whatever else it declares.
pub const RESERVED_MODES: [u8; 2] = [PLAIN_ENERGY, GRAPHICAL_ENERGY];

/// The name the synthesised provider reports the stored amount under.
pub const STORED: &str = "stored_energy";

/// The name the synthesised provider reports the capacity under.
pub const CAPACITY: &str = "maximum_energy";

/// One value a readout collects.
///
/// Three shapes, which is what §12's answers need: the two amounts a synthesised
/// energy provider reports, and whatever text or truth value a dependent's own
/// provider adds. The encoding is the shared library's and nothing here writes
/// one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reading {
    /// A whole amount, wide enough for the energy figures of §12.
    Amount(i64),
    /// Text.
    Text(String),
    /// A truth value.
    Flag(bool),
}

impl Reading {
    /// Returns this reading as the shared library's value.
    #[must_use]
    pub fn to_shared(&self) -> SharedValue {
        match self {
            Self::Amount(amount) => SharedValue::Integer(clamp_energy(*amount)),
            Self::Text(text) => SharedValue::Text(text.clone()),
            Self::Flag(flag) => SharedValue::Boolean(*flag),
        }
    }

    /// Reads a reading back out of the shared library's value.
    ///
    /// Nothing for a kind this entry never writes, which is a map claiming a
    /// shape no readout produced.
    #[must_use]
    pub fn from_shared(value: &SharedValue) -> Option<Self> {
        match value {
            SharedValue::Integer(number) => Some(Self::Amount(i64::from(*number))),
            SharedValue::Text(text) => Some(Self::Text(text.clone())),
            SharedValue::Boolean(flag) => Some(Self::Flag(*flag)),
            SharedValue::Short(_) | SharedValue::Float(_) | SharedValue::Enumeration { .. } => None,
        }
    }
}

/// The three kinds of value this entry puts into the shared typed map.
///
/// Built rather than spelled: the tag numbering is the shared library's, so it
/// is read back out of the library's own map instead of restated here, and a
/// library that renumbered would move both halves together.
fn shared_defaults() -> [SharedValue; 3] {
    [
        SharedValue::Integer(0),
        SharedValue::Boolean(false),
        SharedValue::Text(String::new()),
    ]
}

/// Returns the tag the shared library writes for a value's type.
pub(crate) fn shared_tag(value: &SharedValue) -> u8 {
    let mut probe = ValueMap::new();
    probe.declare("probe", value.clone());
    probe.shape().first().map_or(u8::MAX, |(_, tag)| *tag)
}

/// Returns the shared value one of this entry's tags names, or nothing.
///
/// Nothing for a tag naming a kind this entry never puts into the map, which is
/// a readout answer claiming a shape this entry does not produce.
pub(crate) fn shared_from_tag(tag: u8) -> Option<SharedValue> {
    shared_defaults()
        .into_iter()
        .find(|value| shared_tag(value) == tag)
}

/// The named values one mode collects, in the order they are collected.
///
/// Insertion order is the order, and a replacement keeps the position its name
/// already held rather than moving it, because moving a name would renumber
/// every value after it for a reader that walks positionally.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Readings {
    entries: Vec<(String, Reading)>,
}

impl Readings {
    /// Builds an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Declares a named value, or replaces the one already under that name.
    pub fn declare(&mut self, name: &str, reading: Reading) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|(existing, _)| existing == name)
        {
            existing.1 = reading;
            return;
        }
        self.entries.push((name.to_owned(), reading));
    }

    /// Returns the value under `name`.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&Reading> {
        self.entries
            .iter()
            .find(|(existing, _)| existing == name)
            .map(|(_, reading)| reading)
    }

    /// Returns every name, in order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// Writes these readings into the shared library's typed map.
    ///
    /// An amount crosses through the library's own energy clamp, because the map
    /// has no 64-bit member and RVL-120 §12.3 is the mechanism that
    /// specification publishes for a 64-bit amount reaching a 32-bit interface.
    #[must_use]
    pub fn to_shared(&self) -> ValueMap {
        let mut map = ValueMap::new();
        for (name, reading) in &self.entries {
            map.declare(name, reading.to_shared());
        }
        map
    }

    /// Reads readings back out of the shared library's typed map.
    ///
    /// A value of a kind this entry never writes is dropped rather than guessed
    /// at, which is a map claiming a shape no readout of this entry produced.
    #[must_use]
    pub fn from_shared(map: &ValueMap) -> Self {
        let mut readings = Self::new();
        for name in map.names() {
            if let Some(reading) = map.value(name).and_then(Reading::from_shared) {
                readings.declare(name, reading);
            }
        }
        readings
    }

    /// Returns how many values there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether nothing was collected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for Reading {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Amount(amount) => write!(formatter, "{amount}"),
            Self::Text(text) => formatter.write_str(text),
            Self::Flag(flag) => write!(formatter, "{flag}"),
        }
    }
}

/// Which way the readout block faces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Facing {
    /// Towards lower height.
    Down,
    /// Towards greater height.
    Up,
    /// Towards lower depth.
    North,
    /// Towards greater depth.
    South,
    /// Towards lower width.
    West,
    /// Towards greater width.
    East,
}

impl Facing {
    /// Every facing, in the order this enum declares them.
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    /// Returns the step from a position to the one this facing points at.
    #[must_use]
    pub const fn step(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
        }
    }
}

/// Returns the position opposite the readout's own facing.
///
/// The facing is an [`Option`] because §12's first defect is a lookup that
/// answers with nothing when the block state is not the expected kind. Answering
/// with an absence and requiring every caller to test it is what turns that
/// defect into a branch. A step that would leave the addressable world answers
/// with an absence too, rather than wrapping into a position somewhere else.
#[must_use]
pub fn behind(at: (i32, i32, i32), facing: Option<Facing>) -> Option<(i32, i32, i32)> {
    let facing = facing?;
    let (x, y, z) = facing.step();
    Some((
        at.0.checked_sub(x)?,
        at.1.checked_sub(y)?,
        at.2.checked_sub(z)?,
    ))
}

/// What a block behind the readout offers it.
pub trait Provider {
    /// Returns the modes this provider supports, most usual first.
    ///
    /// The two reserved modes are supported by every provider; anything after
    /// them is the provider's own.
    fn supported_modes(&self) -> Vec<u8>;

    /// Lets the provider advance whatever it derives from a rate.
    ///
    /// Called once every [`TICK_INTERVAL`] server ticks and never on a client.
    fn advance(&mut self);

    /// Returns the values one mode needs, in the order the answer carries them.
    fn collect(&self, mode: u8) -> Readings;
}

/// The provider the readout synthesises when the block behind publishes none.
///
/// §12 says the original re-queries the energy capability on every information
/// request. This reads it once, at construction, and reports what it read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredEnergy {
    stored: i64,
    capacity: i64,
}

impl StoredEnergy {
    /// Builds the synthesised provider from what the block behind holds.
    #[must_use]
    pub const fn read(stored: i64, capacity: i64) -> Self {
        Self { stored, capacity }
    }
}

impl Provider for StoredEnergy {
    fn supported_modes(&self) -> Vec<u8> {
        RESERVED_MODES.to_vec()
    }

    fn advance(&mut self) {}

    fn collect(&self, mode: u8) -> Readings {
        let mut readings = Readings::new();
        if !RESERVED_MODES.contains(&mode) {
            return readings;
        }
        readings.declare(STORED, Reading::Amount(self.stored));
        readings.declare(CAPACITY, Reading::Amount(self.capacity));
        readings
    }
}

/// The readout block's own state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Readout {
    mode: u8,
    ticks: u64,
}

impl Readout {
    /// Builds a readout in the first reserved mode.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: PLAIN_ENERGY,
            ticks: 0,
        }
    }

    /// Returns the mode in force, which is the one byte this block saves.
    #[must_use]
    pub const fn mode(&self) -> u8 {
        self.mode
    }

    /// Advances to the next mode the provider supports, wrapping.
    ///
    /// A mode that is not in the provider's list leaves the mode unchanged,
    /// which is what makes a provider that shrank its list safe: the readout
    /// stays where it was rather than jumping to an index the provider no longer
    /// has. The answer says whether the mode moved.
    pub fn toggle(&mut self, supported: &[u8]) -> bool {
        let Some(position) = supported.iter().position(|mode| *mode == self.mode) else {
            return false;
        };
        let next = supported[(position + 1) % supported.len()];
        let moved = next != self.mode;
        self.mode = next;
        moved
    }

    /// Runs one server tick against the block behind.
    ///
    /// The provider is advanced on every tenth tick and only when one is there,
    /// so an absent provider is a branch rather than a use of nothing.
    pub fn server_tick(&mut self, provider: Option<&mut dyn Provider>) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        if !self.ticks.is_multiple_of(TICK_INTERVAL) {
            return false;
        }
        match provider {
            Some(provider) => {
                provider.advance();
                true
            }
            None => false,
        }
    }

    /// Collects the values the current mode needs, on the server.
    ///
    /// This is the whole of what travels to the client, so a client that asked
    /// and got nothing shows nothing rather than deriving a figure of its own.
    #[must_use]
    pub fn collect(&self, provider: Option<&dyn Provider>) -> Readings {
        match provider {
            Some(provider) => provider.collect(self.mode),
            None => Readings::new(),
        }
    }
}

/// What a client remembers of the last answer it was given.
///
/// A client holds an answer and the moment it arrived, and nothing else. There
/// is deliberately no way to put a value in here that the server did not send.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Screen {
    answer: Option<(Readings, u64)>,
}

impl Screen {
    /// Builds a screen that has been told nothing yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { answer: None }
    }

    /// Records an answer and the tick it arrived on.
    pub fn accept(&mut self, values: Readings, at_tick: u64) {
        self.answer = Some((values, at_tick));
    }

    /// Returns the last answer, if there has been one.
    #[must_use]
    pub fn answer(&self) -> Option<&Readings> {
        self.answer.as_ref().map(|(values, _)| values)
    }

    /// Returns the tick the last answer arrived on.
    #[must_use]
    pub fn stamped(&self) -> Option<u64> {
        self.answer.as_ref().map(|(_, at)| *at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct Counting {
        advanced: usize,
        modes: Vec<u8>,
    }

    impl Provider for Counting {
        fn supported_modes(&self) -> Vec<u8> {
            self.modes.clone()
        }

        fn advance(&mut self) {
            self.advanced += 1;
        }

        fn collect(&self, mode: u8) -> Readings {
            let mut readings = Readings::new();
            readings.declare("mode", Reading::Amount(i64::from(mode)));
            readings
        }
    }

    #[test]
    fn the_readout_looks_at_the_position_opposite_its_own_facing() {
        assert_eq!(behind((0, 0, 0), Some(Facing::North)), Some((0, 0, 1)));
        assert_eq!(behind((0, 0, 0), Some(Facing::Up)), Some((0, -1, 0)));
        assert_eq!(behind((4, 5, 6), Some(Facing::East)), Some((3, 5, 6)));
    }

    #[test]
    fn a_block_state_that_is_not_the_expected_kind_answers_with_an_absence() {
        assert_eq!(
            behind((0, 0, 0), None),
            None,
            "the lookup answers with nothing rather than a position nobody checked"
        );
    }

    #[test]
    fn a_step_that_would_leave_the_addressable_world_answers_with_an_absence() {
        assert_eq!(behind((0, i32::MAX, 0), Some(Facing::Down)), None);
        assert_eq!(behind((i32::MIN, 0, 0), Some(Facing::East)), None);
    }

    #[test]
    fn the_provider_is_advanced_on_every_tenth_tick_and_never_without_one() {
        let mut readout = Readout::new();
        let mut provider = Counting::default();

        for _ in 1..TICK_INTERVAL {
            assert!(!readout.server_tick(Some(&mut provider)));
        }
        assert!(readout.server_tick(Some(&mut provider)));
        assert_eq!(provider.advanced, 1);

        for _ in 0..TICK_INTERVAL {
            readout.server_tick(None);
        }
        assert_eq!(
            provider.advanced, 1,
            "an absent provider is a branch, not a use of nothing"
        );
    }

    #[test]
    fn toggling_walks_the_provider_list_and_wraps_at_the_end() {
        let mut readout = Readout::new();
        let supported = [PLAIN_ENERGY, GRAPHICAL_ENERGY, 7];

        assert!(readout.toggle(&supported));
        assert_eq!(readout.mode(), GRAPHICAL_ENERGY);
        assert!(readout.toggle(&supported));
        assert_eq!(readout.mode(), 7);
        assert!(readout.toggle(&supported));
        assert_eq!(readout.mode(), PLAIN_ENERGY);
    }

    #[test]
    fn a_mode_the_provider_does_not_support_leaves_the_mode_unchanged() {
        let mut readout = Readout::new();
        readout.toggle(&[PLAIN_ENERGY, 7]);
        assert_eq!(readout.mode(), 7);

        assert!(!readout.toggle(&[PLAIN_ENERGY, GRAPHICAL_ENERGY]));
        assert_eq!(readout.mode(), 7);
        assert!(!readout.toggle(&[]));
        assert_eq!(readout.mode(), 7);
    }

    #[test]
    fn a_single_supported_mode_toggles_to_itself_and_reports_no_movement() {
        let mut readout = Readout::new();
        assert!(!readout.toggle(&[PLAIN_ENERGY]));
        assert_eq!(readout.mode(), PLAIN_ENERGY);
    }

    #[test]
    fn the_synthesised_provider_reports_what_it_read_at_construction() {
        let provider = StoredEnergy::read(1_200, 60_000);
        let values = provider.collect(PLAIN_ENERGY);

        assert_eq!(values.names(), [STORED, CAPACITY]);
        assert_eq!(values.value(STORED), Some(&Reading::Amount(1_200)));
        assert_eq!(values.value(CAPACITY), Some(&Reading::Amount(60_000)));
        assert_eq!(values.len(), 2);
        assert_eq!(provider.supported_modes(), RESERVED_MODES.to_vec());
    }

    #[test]
    fn the_two_amounts_are_reported_at_full_width_rather_than_narrowed() {
        let provider = StoredEnergy::read(i64::MAX, i64::MIN);
        let values = provider.collect(GRAPHICAL_ENERGY);

        assert_eq!(values.value(STORED), Some(&Reading::Amount(i64::MAX)));
        assert_eq!(values.value(CAPACITY), Some(&Reading::Amount(i64::MIN)));
    }

    #[test]
    fn a_replaced_value_keeps_the_position_its_name_already_held() {
        let mut readings = Readings::new();
        readings.declare("first", Reading::Amount(1));
        readings.declare("second", Reading::Flag(true));
        readings.declare("first", Reading::Text("again".to_owned()));

        assert_eq!(readings.names(), ["first", "second"]);
        assert_eq!(
            readings.value("first"),
            Some(&Reading::Text("again".to_owned()))
        );
    }

    #[test]
    fn a_mode_the_synthesised_provider_does_not_serve_collects_nothing() {
        let provider = StoredEnergy::read(1, 2);
        assert_eq!(provider.collect(9).len(), 0);
    }

    #[test]
    fn the_server_collects_and_a_client_only_remembers_what_it_was_told() {
        let readout = Readout::new();
        let provider = StoredEnergy::read(10, 20);
        let collected = readout.collect(Some(&provider));

        let mut screen = Screen::new();
        assert!(screen.answer().is_none());
        assert!(screen.stamped().is_none());

        screen.accept(collected.clone(), 42);
        assert_eq!(screen.answer(), Some(&collected));
        assert_eq!(screen.stamped(), Some(42));
    }

    #[test]
    fn collecting_with_no_provider_answers_with_an_empty_map() {
        let readout = Readout::new();
        assert_eq!(readout.collect(None).len(), 0);
    }
}
