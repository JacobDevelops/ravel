//! The run-length coder and the smaller primitives of §12.4 and §12.5.
//!
//! Everything here is portable arithmetic and portable framing: a dependent
//! computes a value on one side and reads it on the other, so a rule that is
//! merely equivalent is a rule that disagrees.

use std::collections::BTreeMap;
use std::fmt;

/// The largest run one coded pair can carry.
pub const MAX_RUN: u8 = u8::MAX;

/// The most values a decode will produce.
///
/// A coded buffer arrives from the network, and two bytes there can ask for two
/// hundred and fifty-five. This is the ceiling that keeps a hostile buffer from
/// asking for more memory than a screen's worth of values could ever need; a
/// dependent that legitimately needs more codes more than one block.
pub const MAX_DECODED: usize = 1 << 20;

/// Codes `values` as the flat sequence of count-then-value pairs §12.4 states.
///
/// A run longer than [`MAX_RUN`] continues in a new pair. Values are single
/// bytes, which is the whole domain the coder is defined over.
#[must_use]
pub fn code_runs(values: &[u8]) -> Vec<u8> {
    let mut coded = Vec::new();
    let mut index = 0;
    while index < values.len() {
        let value = values[index];
        let mut run = 0_u8;
        while index < values.len() && values[index] == value && run < MAX_RUN {
            run += 1;
            index += 1;
        }
        coded.push(run);
        coded.push(value);
    }
    coded
}

/// Why a coded buffer was not read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunError {
    /// A coded pair or frame is incomplete, or its frame length is negative.
    Truncated,
    /// The buffer asks for more values than [`MAX_DECODED`] admits.
    TooLong {
        /// How many values the buffer asks for, as far as counting got.
        asked: usize,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter
                .write_str("a coded pair or frame is incomplete or has a negative frame length"),
            Self::TooLong { asked } => write!(
                formatter,
                "a coded buffer asks for {asked} values and {MAX_DECODED} is the most one may ask for"
            ),
        }
    }
}

impl std::error::Error for RunError {}

/// Reads a coded buffer back into the values it carries.
///
/// # Errors
///
/// Returns [`RunError::Truncated`] for an odd-length buffer and
/// [`RunError::TooLong`] for one that asks for more than [`MAX_DECODED`]
/// values. The original signals neither: it reads what is there and then
/// answers zero forever, which is why a caller of it has to already know how
/// many values it expects.
pub fn read_runs(coded: &[u8]) -> Result<Vec<u8>, RunError> {
    if !coded.len().is_multiple_of(2) {
        return Err(RunError::Truncated);
    }
    let pairs = coded.as_chunks::<2>().0;
    let asked: usize = pairs.iter().map(|pair| usize::from(pair[0])).sum();
    if asked > MAX_DECODED {
        return Err(RunError::TooLong { asked });
    }
    let mut values = Vec::with_capacity(asked);
    for pair in pairs {
        values.extend(std::iter::repeat_n(pair[1], usize::from(pair[0])));
    }
    Ok(values)
}

/// A reader over decoded values with an explicit end.
///
/// [`Runs::next_value`] answers `None` past the end, which is the divergence §12.4
/// asks for. [`Runs::next_or_zero`] keeps the original's zero-filling rule for
/// the dependents that rely on it, so both are available and a caller says
/// which it means.
#[derive(Clone, Debug)]
pub struct Runs {
    values: Vec<u8>,
    read: usize,
}

impl Runs {
    /// Builds a reader over already-decoded values.
    #[must_use]
    pub const fn over(values: Vec<u8>) -> Self {
        Self { values, read: 0 }
    }

    /// Reads a coded buffer and builds a reader over it.
    ///
    /// # Errors
    ///
    /// Returns whatever [`read_runs`] refuses.
    pub fn decode(coded: &[u8]) -> Result<Self, RunError> {
        read_runs(coded).map(Self::over)
    }

    /// Returns how many values remain unread.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.values.len() - self.read
    }

    /// Returns the next value, or nothing once the values are exhausted.
    pub fn next_value(&mut self) -> Option<u8> {
        if self.read == self.values.len() {
            return None;
        }
        let value = self.values[self.read];
        self.read += 1;
        Some(value)
    }

    /// Returns the next value, answering zero forever past the end.
    pub fn next_or_zero(&mut self) -> u8 {
        self.next_value().unwrap_or_default()
    }
}

/// Frames a coded buffer as the one optional length-prefixed block §12.4 sends.
#[must_use]
pub fn frame_runs(coded: Option<&[u8]>) -> Vec<u8> {
    let mut framed = Vec::new();
    match coded {
        None => framed.push(0),
        Some(bytes) => {
            framed.push(1);
            framed.extend_from_slice(&crate::arithmetic::signed_len(bytes.len()).to_be_bytes());
            framed.extend_from_slice(bytes);
        }
    }
    framed
}

/// Reads the first framed block, leaving any following bytes uninterpreted.
///
/// # Errors
///
/// Returns [`RunError::Truncated`] when the frame ends inside its header, names
/// a negative length, or announces more bytes than the buffer holds.
pub fn read_framed(framed: &[u8]) -> Result<Option<&[u8]>, RunError> {
    let (&present, rest) = framed.split_first().ok_or(RunError::Truncated)?;
    if present == 0 {
        return Ok(None);
    }
    if rest.len() < 4 {
        return Err(RunError::Truncated);
    }
    let (header, body) = rest.split_at(4);
    let announced = i32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let length = usize::try_from(announced).map_err(|_| RunError::Truncated)?;
    if length > body.len() {
        return Err(RunError::Truncated);
    }
    Ok(Some(&body[..length]))
}

/// The process-wide string interner of §12.5.
///
/// Ids run from one upward, are never released and are never persisted. That
/// last part is a rule rather than an observation: an id is stable only within
/// one run, so a saved id names something different next time. The original
/// neither says so nor prevents it; this one says so here and offers no
/// serialisation at all.
#[derive(Clone, Debug, Default)]
pub struct Interner {
    ids: BTreeMap<String, u32>,
    names: Vec<String>,
}

impl Interner {
    /// Builds an interner holding nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ids: BTreeMap::new(),
            names: Vec::new(),
        }
    }

    /// Returns the id `name` holds, issuing the next one if it holds none.
    ///
    /// # Errors
    ///
    /// Returns [`Exhausted`] when every id in the width has been issued. An
    /// interner that never releases can reach its end, and wrapping to one
    /// would hand two names the same id.
    pub fn intern(&mut self, name: &str) -> Result<u32, Exhausted> {
        if let Some(&id) = self.ids.get(name) {
            return Ok(id);
        }
        let id = u32::try_from(self.names.len() + 1).map_err(|_| Exhausted)?;
        self.names.push(name.to_owned());
        self.ids.insert(name.to_owned(), id);
        Ok(id)
    }

    /// Returns the name an id was issued for.
    #[must_use]
    pub fn name(&self, id: u32) -> Option<&str> {
        let index = usize::try_from(id).ok()?.checked_sub(1)?;
        self.names.get(index).map(String::as_str)
    }

    /// Returns how many ids have been issued.
    #[must_use]
    pub const fn issued(&self) -> usize {
        self.names.len()
    }
}

/// Every id in the width has been issued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exhausted;

impl fmt::Display for Exhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("every interned id in the width has been issued")
    }
}

impl std::error::Error for Exhausted {}

/// Rounds toward negative infinity, which is not truncation below zero.
#[must_use]
pub fn floor_of(value: f64) -> i32 {
    saturating(value.floor())
}

/// Rounds a single-precision value toward negative infinity.
#[must_use]
pub fn floor_of_single(value: f32) -> i32 {
    saturating(f64::from(value.floor()))
}

/// Rounds a single-precision value toward positive infinity.
///
/// §12.5 gives the ceiling helper a single-precision form only, and so does
/// this: a double-precision one nothing asked for is a second rule to keep in
/// step with the first.
#[must_use]
pub fn ceiling_of_single(value: f32) -> i32 {
    saturating(f64::from(value.ceil()))
}

fn saturating(value: f64) -> i32 {
    if value.is_nan() {
        return 0;
    }
    if value >= f64::from(i32::MAX) {
        return i32::MAX;
    }
    if value <= f64::from(i32::MIN) {
        return i32::MIN;
    }
    num_traits::ToPrimitive::to_i32(&value.trunc())
        .expect("a finite value between the i32 bounds fits after truncation")
}

/// The six faces a block may be mounted against, in ordinal order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Face {
    /// The face below.
    Down,
    /// The face above.
    Up,
    /// The first face around the vertical axis.
    North,
    /// The face opposite [`Face::North`].
    South,
    /// The face a quarter turn from [`Face::North`].
    West,
    /// The face opposite [`Face::West`].
    East,
}

impl Face {
    /// Every face, in the order this enum declares them.
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    /// Returns the face's canonical lowercase name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::North => "north",
            Self::South => "south",
            Self::West => "west",
            Self::East => "east",
        }
    }
}

/// One of the twenty-four orientations of §12.5.
///
/// A value is a mounting face and a rotation step within it. The ordinal layout
/// is the behaviour and not a presentation choice: the mounting face is the
/// ordinal divided by four and rotating cycles the low two bits inside the group
/// of four, so reordering the values redefines both.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Orientation {
    ordinal: u8,
}

impl Orientation {
    /// How many orientations there are.
    pub const COUNT: u8 = 24;

    /// Builds an orientation from a face and a rotation step.
    ///
    /// A step is taken modulo four rather than refused, because rotation is
    /// defined arithmetically over the low two bits and a fifth step is the
    /// first one.
    #[must_use]
    pub const fn new(face: Face, step: u8) -> Self {
        let face_ordinal = match face {
            Face::Down => 0,
            Face::Up => 1,
            Face::North => 2,
            Face::South => 3,
            Face::West => 4,
            Face::East => 5,
        };
        Self {
            ordinal: face_ordinal * 4 + (step & 3),
        }
    }

    /// Builds an orientation from its ordinal, refusing one outside the layout.
    #[must_use]
    pub const fn from_ordinal(ordinal: u8) -> Option<Self> {
        if ordinal < Self::COUNT {
            Some(Self { ordinal })
        } else {
            None
        }
    }

    /// Returns the ordinal, which is the wire and the arithmetic both.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }

    /// Returns the face this orientation mounts against.
    #[must_use]
    pub const fn face(self) -> Face {
        match self.ordinal.div_euclid(4) {
            0 => Face::Down,
            1 => Face::Up,
            2 => Face::North,
            3 => Face::South,
            4 => Face::West,
            _ => Face::East,
        }
    }

    /// Returns the rotation step within the face.
    #[must_use]
    pub const fn step(self) -> u8 {
        self.ordinal & 3
    }

    /// Returns the next orientation around the mounting face.
    #[must_use]
    pub const fn rotated(self) -> Self {
        Self {
            ordinal: (self.ordinal & !3) | ((self.ordinal + 1) & 3),
        }
    }

    /// Returns the serialised form, which is the value's own name.
    #[must_use]
    pub fn name(self) -> String {
        format!("{}_{}", self.face().as_str(), self.step())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_is_coded_as_a_count_and_then_a_value() {
        assert_eq!(code_runs(&[7, 7, 7]), vec![3, 7]);
        assert_eq!(code_runs(&[1, 1, 2]), vec![2, 1, 1, 2]);
        assert_eq!(code_runs(&[]), [0_u8; 0]);
    }

    #[test]
    fn a_run_longer_than_the_cap_continues_in_a_new_pair() {
        let coded = code_runs(&[9_u8; 300]);
        assert_eq!(coded, vec![255, 9, 45, 9]);
        assert_eq!(
            read_runs(&coded).expect("a well-formed buffer"),
            vec![9_u8; 300]
        );
    }

    #[test]
    fn every_coded_buffer_reads_back_to_what_was_coded() {
        for values in [vec![], vec![0], vec![1, 1, 2, 3, 3, 3], vec![255_u8; 511]] {
            let coded = code_runs(&values);
            assert_eq!(read_runs(&coded).expect("a well-formed buffer"), values);
        }
    }

    #[test]
    fn an_odd_length_buffer_is_refused_rather_than_read_short() {
        assert_eq!(read_runs(&[3]), Err(RunError::Truncated));
        assert_eq!(read_runs(&[3, 7, 2]), Err(RunError::Truncated));
    }

    #[test]
    fn a_buffer_asking_for_more_than_the_ceiling_is_refused_before_it_allocates() {
        let hostile: Vec<u8> = (0..8_192).flat_map(|_| [255_u8, 0_u8]).collect();
        match read_runs(&hostile) {
            Err(RunError::TooLong { asked }) => assert!(asked > MAX_DECODED),
            other => panic!("a hostile buffer must be refused: {other:?}"),
        }
    }

    #[test]
    fn the_reader_has_an_explicit_end_and_a_zero_filling_form_beside_it() {
        let mut runs = Runs::decode(&code_runs(&[4, 5])).expect("a well-formed buffer");
        assert_eq!(runs.remaining(), 2);
        assert_eq!(runs.next_value(), Some(4));
        assert_eq!(runs.next_value(), Some(5));
        assert_eq!(
            runs.next_value(),
            None,
            "the end is reported rather than filled"
        );

        let mut filling = Runs::over(vec![4]);
        assert_eq!(filling.next_or_zero(), 4);
        assert_eq!(filling.next_or_zero(), 0);
        assert_eq!(
            filling.next_or_zero(),
            0,
            "past the end it answers zero forever"
        );
    }

    #[test]
    fn the_frame_carries_an_absent_block_and_a_present_one_differently() {
        assert_eq!(frame_runs(None), vec![0]);
        assert_eq!(read_framed(&frame_runs(None)).expect("a legal frame"), None);

        let coded = code_runs(&[1, 1, 2]);
        let framed = frame_runs(Some(&coded));
        assert_eq!(framed.len(), 1 + 4 + coded.len());
        assert_eq!(
            read_framed(&framed).expect("a legal frame"),
            Some(coded.as_slice())
        );
    }

    #[test]
    fn a_frame_that_announces_more_than_it_carries_is_refused() {
        let mut lying = frame_runs(Some(&[1, 2]));
        lying[1..5].copy_from_slice(&64_i32.to_be_bytes());
        assert_eq!(read_framed(&lying), Err(RunError::Truncated));

        let mut negative = frame_runs(Some(&[1, 2]));
        negative[1..5].copy_from_slice(&(-1_i32).to_be_bytes());
        assert_eq!(read_framed(&negative), Err(RunError::Truncated));

        assert_eq!(read_framed(&[]), Err(RunError::Truncated));
        assert_eq!(read_framed(&[1, 0, 0]), Err(RunError::Truncated));
    }

    #[test]
    fn reading_one_frame_does_not_interpret_following_bytes() {
        let mut present = frame_runs(Some(&[1, 2]));
        present.push(0xFF);
        assert_eq!(read_framed(&present), Ok(Some(&[1, 2][..])));
        assert_eq!(read_framed(&[0, 0xFF]), Ok(None));
    }

    #[test]
    fn interned_ids_start_at_one_and_are_never_reissued() {
        let mut interner = Interner::new();
        assert_eq!(interner.intern("first"), Ok(1));
        assert_eq!(interner.intern("second"), Ok(2));
        assert_eq!(
            interner.intern("first"),
            Ok(1),
            "a name keeps the id it was given"
        );
        assert_eq!(interner.issued(), 2);
        assert_eq!(interner.name(1), Some("first"));
        assert_eq!(interner.name(0), None, "zero is never issued");
        assert_eq!(interner.name(3), None);
    }

    #[test]
    fn the_floor_helpers_round_toward_negative_infinity_and_not_toward_zero() {
        assert_eq!(floor_of(-1.5), -2);
        assert_eq!(floor_of(1.5), 1);
        assert_eq!(floor_of(-0.0), 0);
        assert_eq!(floor_of_single(-1.5), -2);
        assert_eq!(floor_of_single(2.99), 2);
        assert_ne!(floor_of(-1.5), -1, "truncation would answer minus one");
    }

    #[test]
    fn the_ceiling_helper_rounds_toward_positive_infinity() {
        assert_eq!(ceiling_of_single(1.01), 2);
        assert_eq!(ceiling_of_single(-1.5), -1);
        assert_eq!(ceiling_of_single(3.0), 3);
    }

    #[test]
    fn a_value_outside_the_signed_width_saturates_rather_than_wrapping() {
        assert_eq!(floor_of(f64::INFINITY), i32::MAX);
        assert_eq!(floor_of(f64::NEG_INFINITY), i32::MIN);
        assert_eq!(floor_of(f64::NAN), 0);
        assert_eq!(floor_of(1e30), i32::MAX);
    }

    #[test]
    fn there_are_twenty_four_orientations_and_the_ordinal_decides_the_face() {
        assert_eq!(Orientation::COUNT, 24);
        for ordinal in 0..Orientation::COUNT {
            let orientation = Orientation::from_ordinal(ordinal).expect("inside the layout");
            assert_eq!(
                orientation.face(),
                Face::ALL[usize::from(ordinal.div_euclid(4))]
            );
            assert_eq!(orientation.step(), ordinal % 4);
        }
        assert_eq!(Orientation::from_ordinal(24), None);
    }

    #[test]
    fn rotating_cycles_the_low_two_bits_and_never_leaves_the_face() {
        for face in Face::ALL {
            let start = Orientation::new(face, 0);
            let mut here = start;
            for step in 1..4 {
                here = here.rotated();
                assert_eq!(here.face(), face);
                assert_eq!(here.step(), step);
            }
            assert_eq!(here.rotated(), start, "four rotations return to the start");
        }
    }

    #[test]
    fn a_step_beyond_the_group_of_four_is_the_first_one_again() {
        assert_eq!(
            Orientation::new(Face::West, 4),
            Orientation::new(Face::West, 0)
        );
        assert_eq!(Orientation::new(Face::West, 5).step(), 1);
    }

    #[test]
    fn the_serialised_form_is_the_value_own_name_and_every_one_is_distinct() {
        let mut names: Vec<String> = (0..Orientation::COUNT)
            .filter_map(Orientation::from_ordinal)
            .map(Orientation::name)
            .collect();
        assert_eq!(names[0], "down_0");
        assert_eq!(names[23], "east_3");
        names.sort();
        names.dedup();
        assert_eq!(names.len(), usize::from(Orientation::COUNT));
    }
}
