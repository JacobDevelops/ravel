//! Wide values through a narrow synchronisation channel, from §6.3.
//!
//! The host's screen-data synchronisation truncates each value to sixteen bits.
//! A dependent that shows a large stored quantity in a screen depends on the
//! full signed thirty-two-bit range surviving that channel, so each logical
//! value is stored as two halves and recomposed on read.
//!
//! The arithmetic is the behaviour rather than an implementation detail and is
//! reproduced exactly, sign handling included. The backing array is twice the
//! declared size and the doubling is hidden from the index arithmetic: a
//! dependent indexes against the size it declared.
//!
//! The narrow pair the host itself calls stays available and is
//! compatibility-only, which is what the original's own source says about it.

/// The most logical values one of these will hold.
///
/// A declared size arrives from a dependent rather than from a document, but the
/// backing array is twice it and an unbounded size is an allocation a caller
/// controls. The ceiling is far above any screen's needs and the upgrade path is
/// a larger constant.
pub const MAX_VALUES: usize = 1 << 16;

/// Why a wide store refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreError {
    /// The declared size is past [`MAX_VALUES`].
    TooLarge {
        /// What was asked for.
        asked: usize,
        /// The ceiling.
        ceiling: usize,
    },
    /// An index is outside the declared size.
    OutOfRange {
        /// The index asked for.
        index: usize,
        /// How many values there are.
        size: usize,
    },
    /// A narrow index is outside the backing array.
    NarrowOutOfRange {
        /// The index asked for.
        index: usize,
        /// How many narrow slots there are.
        slots: usize,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { asked, ceiling } => {
                write!(formatter, "a store of {asked} values is past {ceiling}")
            }
            Self::OutOfRange { index, size } => {
                write!(formatter, "value {index} is outside a store of {size}")
            }
            Self::NarrowOutOfRange { index, slots } => {
                write!(formatter, "narrow slot {index} is outside {slots}")
            }
        }
    }
}

impl std::error::Error for StoreError {}

/// A store of signed thirty-two-bit values behind a sixteen-bit channel.
///
/// The backing array holds two narrow slots per value: the high half first, then
/// the low half. A dependent never sees that.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WideValues {
    halves: Vec<i16>,
}

impl WideValues {
    /// Builds a store of `size` logical values, every one zero.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::TooLarge`] for a size past [`MAX_VALUES`].
    pub fn new(size: usize) -> Result<Self, StoreError> {
        if size > MAX_VALUES {
            return Err(StoreError::TooLarge {
                asked: size,
                ceiling: MAX_VALUES,
            });
        }
        Ok(Self {
            halves: vec![0; size * 2],
        })
    }

    /// Returns how many logical values this holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.halves.len() / 2
    }

    /// Returns whether this holds no value at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.halves.is_empty()
    }

    /// Returns how many narrow slots back the store.
    ///
    /// This is what the host sees, and it is twice [`WideValues::len`].
    #[must_use]
    pub const fn narrow_len(&self) -> usize {
        self.halves.len()
    }

    /// Writes a value, splitting it into its two halves.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::OutOfRange`] for an index outside the declared
    /// size.
    pub fn set(&mut self, index: usize, value: i32) -> Result<(), StoreError> {
        let (high, low) = self.slots(index)?;
        self.halves[high] = (value >> 16) as i16;
        self.halves[low] = (value & 0xFFFF) as i16;
        Ok(())
    }

    /// Reads a value, recomposing it from its two halves.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::OutOfRange`] for an index outside the declared
    /// size.
    pub fn get(&self, index: usize) -> Result<i32, StoreError> {
        let (high, low) = self.slots(index)?;
        Ok((i32::from(self.halves[high]) << 16) | (i32::from(self.halves[low]) & 0xFFFF))
    }

    /// Writes one narrow slot, as the host does.
    ///
    /// Compatibility only. The host still calls the narrow pair, so both stay.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NarrowOutOfRange`] for a slot outside the backing
    /// array.
    pub fn set_narrow(&mut self, index: usize, half: i16) -> Result<(), StoreError> {
        if index >= self.halves.len() {
            return Err(StoreError::NarrowOutOfRange {
                index,
                slots: self.halves.len(),
            });
        }
        self.halves[index] = half;
        Ok(())
    }

    /// Reads one narrow slot, as the host does.
    ///
    /// Compatibility only, for the same reason [`WideValues::set_narrow`] is.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NarrowOutOfRange`] for a slot outside the backing
    /// array.
    pub fn get_narrow(&self, index: usize) -> Result<i16, StoreError> {
        self.halves.get(index).copied().ok_or({
            StoreError::NarrowOutOfRange {
                index,
                slots: self.halves.len(),
            }
        })
    }

    fn slots(&self, index: usize) -> Result<(usize, usize), StoreError> {
        let size = self.len();
        if index >= size {
            return Err(StoreError::OutOfRange { index, size });
        }
        Ok((index * 2, index * 2 + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_VALUES, StoreError, WideValues};

    fn store(size: usize) -> WideValues {
        WideValues::new(size).expect("a small store is inside the ceiling")
    }

    #[test]
    fn the_backing_array_is_twice_the_declared_size_and_the_index_hides_it() {
        let values = store(4);
        assert_eq!(values.len(), 4);
        assert_eq!(values.narrow_len(), 8);
        assert!(values.get(3).is_ok());
        assert!(values.get(4).is_err());
    }

    #[test]
    fn the_whole_signed_range_survives_the_narrow_channel() {
        let mut values = store(1);
        for value in [
            0,
            1,
            -1,
            i32::MAX,
            i32::MIN,
            0x0001_0000,
            -0x0001_0000,
            0x7FFF_FFFE,
            -123_456_789,
        ] {
            values.set(0, value).expect("index zero exists");
            assert_eq!(values.get(0).expect("index zero exists"), value);
        }
    }

    #[test]
    fn a_value_larger_than_sixteen_bits_is_not_the_low_half_alone() {
        let mut values = store(1);
        values.set(0, 0x0002_0003).expect("index zero exists");
        assert_eq!(values.get_narrow(0).expect("the high slot"), 2);
        assert_eq!(values.get_narrow(1).expect("the low slot"), 3);
        assert_ne!(values.get(0).expect("index zero exists"), 3);
    }

    #[test]
    fn the_low_half_is_masked_rather_than_sign_extended() {
        let mut values = store(1);
        values.set(0, 0x0001_FFFF).expect("index zero exists");
        assert_eq!(
            values.get_narrow(1).expect("the low slot"),
            -1,
            "the low half is a signed sixteen-bit value on the wire"
        );
        assert_eq!(
            values.get(0).expect("index zero exists"),
            0x0001_FFFF,
            "and the read masks it rather than letting the sign bleed up"
        );
    }

    #[test]
    fn the_high_half_carries_the_sign_and_an_arithmetic_shift_produces_it() {
        let mut values = store(1);
        values.set(0, -1).expect("index zero exists");
        assert_eq!(values.get_narrow(0).expect("the high slot"), -1);
        assert_eq!(values.get_narrow(1).expect("the low slot"), -1);
        assert_eq!(values.get(0).expect("index zero exists"), -1);
    }

    #[test]
    fn each_value_keeps_its_own_pair_of_slots() {
        let mut values = store(3);
        values.set(0, 111_111).expect("index zero exists");
        values.set(1, -222_222).expect("index one exists");
        values.set(2, 333_333).expect("index two exists");
        assert_eq!(values.get(0).expect("index zero exists"), 111_111);
        assert_eq!(values.get(1).expect("index one exists"), -222_222);
        assert_eq!(values.get(2).expect("index two exists"), 333_333);
    }

    #[test]
    fn the_narrow_pair_the_host_calls_remains_available() {
        let mut values = store(2);
        values.set_narrow(2, 5).expect("slot two exists");
        values.set_narrow(3, 7).expect("slot three exists");
        assert_eq!(values.get(1).expect("index one exists"), 0x0005_0007);
        assert_eq!(values.get_narrow(2).expect("slot two exists"), 5);
    }

    #[test]
    fn an_index_outside_the_declared_size_is_refused_rather_than_reaching_the_second_half() {
        let mut values = store(2);
        assert_eq!(
            values.get(2),
            Err(StoreError::OutOfRange { index: 2, size: 2 })
        );
        assert_eq!(
            values.set(9, 1),
            Err(StoreError::OutOfRange { index: 9, size: 2 })
        );
    }

    #[test]
    fn a_narrow_slot_outside_the_backing_array_is_refused() {
        let mut values = store(2);
        assert_eq!(
            values.get_narrow(4),
            Err(StoreError::NarrowOutOfRange { index: 4, slots: 4 })
        );
        assert!(values.set_narrow(4, 1).is_err());
    }

    #[test]
    fn a_size_past_the_ceiling_is_refused_before_anything_is_allocated() {
        assert_eq!(
            WideValues::new(MAX_VALUES + 1),
            Err(StoreError::TooLarge {
                asked: MAX_VALUES + 1,
                ceiling: MAX_VALUES,
            })
        );
        assert!(WideValues::new(MAX_VALUES).is_ok());
    }

    #[test]
    fn an_empty_store_holds_nothing_and_says_so() {
        let values = store(0);
        assert_eq!(values.len(), 0);
        assert_eq!(values.narrow_len(), 0);
        assert!(values.get(0).is_err());
    }
}
