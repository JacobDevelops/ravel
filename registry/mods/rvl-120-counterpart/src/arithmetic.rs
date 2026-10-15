//! The two change checks and the energy clamp.
//!
//! §12 of the specification is where the specific algorithm *is* the behaviour:
//! one side computes a value, the other compares against it, and a rule that is
//! merely equivalent is a rule that disagrees. The two checks share no seed
//! convention and no bit order, and they are deliberately not unified here.
//!
//! The 32-bit check is stated completely by the specification and is
//! implemented. The 64-bit check is stated except for its polynomial, which is
//! a compatibility-required constant obtainable only from the pinned artifact,
//! so [`Table64`] is derived from a polynomial supplied at build time and
//! [`PLACEHOLDER_POLYNOMIAL`] stands in for the tests. `NOTES.md` says so.

use std::fmt;

/// The reflected polynomial the 32-bit check folds with.
pub const CHECK32_POLYNOMIAL: i32 = 0xEDB8_8320_u32.cast_signed();

/// The build-time name the 64-bit polynomial is read from.
pub const CHECK64_POLYNOMIAL_INPUT: &str = "RVL_120_COUNTERPART_CHECK64_POLYNOMIAL";

/// The polynomial the tests pin the 64-bit format against.
///
/// It is a published, ordinary most-significant-byte-first 64-bit polynomial,
/// chosen because it matches the stated seed and inversion conventions and for
/// no other reason. It is **not** the pinned artifact's polynomial and is not a
/// guess at it: the specification marks that value unobtainable from itself.
pub const PLACEHOLDER_POLYNOMIAL: u64 = 0x42F0_E1EB_A9EA_3693;

/// A 32-bit change check, as §12.1 constructs it.
///
/// A reflected cyclic-redundancy accumulator seeded to all ones and inverted
/// when read, folding a whole 32-bit word at a time and running eight rounds of
/// shift-and-conditional-exclusive-or after each.
///
/// The accumulator is a **signed** 32-bit integer and the shift is arithmetic,
/// so the sign bit propagates where the textbook rule admits a zero. That is the
/// whole difference between this and an ordinary 32-bit check, and it changes
/// every value the check produces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Check32 {
    accumulator: i32,
}

impl Default for Check32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Check32 {
    /// Builds an accumulator seeded to all ones.
    #[must_use]
    pub const fn new() -> Self {
        Self { accumulator: -1 }
    }

    /// Folds one whole 32-bit word in.
    pub const fn fold(&mut self, word: i32) {
        self.accumulator ^= word;
        let mut round = 0_u8;
        while round < 8 {
            let odd = self.accumulator & 1 != 0;
            self.accumulator >>= 1;
            if odd {
                self.accumulator ^= CHECK32_POLYNOMIAL;
            }
            round += 1;
        }
    }

    /// Returns the check, bit-inverted as reading it always is.
    #[must_use]
    pub const fn read(&self) -> i32 {
        !self.accumulator
    }
}

/// Folds a count and then every word, which is the convenience path of §12.1.
///
/// The count goes in first so that two sequences differing only in length
/// cannot collide trivially. A sequence longer than a signed 32-bit count can
/// express folds that maximum rather than wrapping to a shorter one, because a
/// wrapped count is exactly the collision the count is there to prevent.
#[must_use]
pub fn check32_over(words: &[i32]) -> i32 {
    let mut check = Check32::new();
    check.fold(bounded_count(words.len()));
    for &word in words {
        check.fold(word);
    }
    check.read()
}

/// The 256-entry table the 64-bit check consumes a byte through.
#[derive(Clone, Eq, PartialEq)]
pub struct Table64 {
    entries: [u64; 256],
}

impl fmt::Debug for Table64 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Table64")
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl Table64 {
    /// Derives the table from a most-significant-byte-first polynomial.
    #[must_use]
    pub const fn derive(polynomial: u64) -> Self {
        let mut entries = [0_u64; 256];
        let mut index = 0_usize;
        let mut seed = 0_u64;
        while index < 256 {
            let mut value = seed << 56;
            let mut round = 0_u8;
            while round < 8 {
                let top = value & (1 << 63) != 0;
                value <<= 1;
                if top {
                    value ^= polynomial;
                }
                round += 1;
            }
            entries[index] = value;
            index += 1;
            seed += 1;
        }
        Self { entries }
    }

    /// The table the tests pin the format against.
    #[must_use]
    pub const fn placeholder() -> Self {
        Self::derive(PLACEHOLDER_POLYNOMIAL)
    }

    /// Derives the table from the polynomial supplied at build time.
    ///
    /// # Errors
    ///
    /// Returns [`PolynomialUnavailable`] when the build supplied no polynomial,
    /// or supplied one that is not sixteen hexadecimal digits. A package that
    /// has to agree with the pinned artifact cannot fall back to a placeholder
    /// and pretend to agree, so the absence is an error rather than a default.
    pub fn declared() -> Result<Self, PolynomialUnavailable> {
        Self::supplied(option_env!("RVL_120_COUNTERPART_CHECK64_POLYNOMIAL"))
    }

    /// Derives the table from the polynomial a build supplied.
    ///
    /// Taking the supplied text as an argument is what makes the absence
    /// testable in a build that legitimately supplies one: a test that asserted
    /// the variable is unset would pass only where it happens to be.
    ///
    /// # Errors
    ///
    /// Returns [`PolynomialUnavailable::NotSupplied`] for nothing at all and
    /// [`PolynomialUnavailable::NotHexadecimal`] for text that is not sixteen
    /// hexadecimal digits, optionally prefixed with `0x`.
    pub fn supplied(text: Option<&str>) -> Result<Self, PolynomialUnavailable> {
        let text = text.ok_or(PolynomialUnavailable::NotSupplied)?;
        let digits = text.strip_prefix("0x").unwrap_or(text);
        let polynomial =
            u64::from_str_radix(digits, 16).map_err(|_| PolynomialUnavailable::NotHexadecimal)?;
        Ok(Self::derive(polynomial))
    }
}

/// Why the build-time polynomial did not produce a table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolynomialUnavailable {
    /// The build named no polynomial at all.
    NotSupplied,
    /// The build named something that is not a hexadecimal 64-bit value.
    NotHexadecimal,
}

impl fmt::Display for PolynomialUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupplied => write!(
                formatter,
                "the 64-bit check's polynomial is compatibility-required and \
                 nothing supplied `{CHECK64_POLYNOMIAL_INPUT}` at build time"
            ),
            Self::NotHexadecimal => write!(
                formatter,
                "`{CHECK64_POLYNOMIAL_INPUT}` is not a hexadecimal 64-bit value"
            ),
        }
    }
}

impl std::error::Error for PolynomialUnavailable {}

/// A 64-bit change check, as §12.2 constructs it.
///
/// Table-driven, most-significant-byte-first, seeded to zero and **not**
/// inverted on read. It consumes one byte at a time; the two integer entry
/// points beside that are [`Check64::fold_word`], which feeds a four-byte word
/// least significant byte first, and [`Check64::fold_low_byte`], which keeps
/// only the low byte and discards the other three.
#[derive(Clone, Copy, Debug)]
pub struct Check64<'table> {
    table: &'table Table64,
    accumulator: u64,
}

impl<'table> Check64<'table> {
    /// Builds an accumulator seeded to zero over `table`.
    #[must_use]
    pub const fn new(table: &'table Table64) -> Self {
        Self {
            table,
            accumulator: 0,
        }
    }

    /// Folds one byte in.
    pub const fn fold_byte(&mut self, byte: u8) {
        let byte_index = self.accumulator.to_be_bytes()[0] ^ byte;
        let mut index_bytes = [0; size_of::<usize>()];
        index_bytes[0] = byte_index;
        let index = usize::from_le_bytes(index_bytes);
        self.accumulator = (self.accumulator << 8) ^ self.table.entries[index];
    }

    /// Folds a four-byte word in, least significant byte first.
    pub const fn fold_word(&mut self, word: u32) {
        let bytes = word.to_le_bytes();
        let mut index = 0;
        while index < bytes.len() {
            self.fold_byte(bytes[index]);
            index += 1;
        }
    }

    /// Folds the low byte of `word` in, discarding the other three.
    ///
    /// This is the single-argument entry point of §12.2 and it is deliberately
    /// not the same as [`Check64::fold_word`]. Porting the pair by name folds
    /// different data.
    pub const fn fold_low_byte(&mut self, word: u32) {
        self.fold_byte(word.to_le_bytes()[0]);
    }

    /// Returns the check, uninverted as reading it always is.
    #[must_use]
    pub const fn read(&self) -> u64 {
        self.accumulator
    }
}

/// Folds a count and then every word, which is the convenience path of §12.2.
#[must_use]
pub fn check64_over(table: &Table64, words: &[u32]) -> u64 {
    let mut check = Check64::new(table);
    check.fold_word(bounded_count(words.len()).cast_unsigned());
    for &word in words {
        check.fold_word(word);
    }
    check.read()
}

/// Saturates a 64-bit energy amount into the 32-bit width §12.3 describes.
///
/// Only the upper bound is guarded: a value above the signed 32-bit maximum
/// comes back as that maximum, and anything else is truncated to its low 32
/// bits, so a negative amount passes through and one below the signed 32-bit
/// minimum wraps. That is the original's rule and it is kept, because a
/// dependent comparing against its own clamp would disagree with a corrected
/// one on exactly the values a correction would change.
#[must_use]
pub const fn clamp_energy(amount: i64) -> i32 {
    if amount > 0x7fff_ffff_i64 {
        i32::MAX
    } else {
        let [a, b, c, d, ..] = amount.to_le_bytes();
        i32::from_le_bytes([a, b, c, d])
    }
}

/// Reports a stored amount against a capacity, ignoring the capacity.
///
/// The capacity argument is part of the shape §12.3 states and is read by
/// nothing. The fuller scheme that would use it is disabled in the original and
/// its own comment calls it too risky, so it is not implemented here either.
#[must_use]
pub const fn reported_energy(amount: i64, capacity: i64) -> i32 {
    let _ = capacity;
    clamp_energy(amount)
}

/// Narrows a length to a signed count, saturating rather than wrapping.
pub(crate) fn bounded_count(length: usize) -> i32 {
    i32::try_from(length).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_thirty_two_bit_check_reads_zero_because_the_seed_is_inverted() {
        assert_eq!(Check32::new().read(), 0);
    }

    #[test]
    fn the_thirty_two_bit_check_pins_the_vectors_the_stated_arithmetic_produces() {
        let mut zero = Check32::new();
        zero.fold(0);
        assert_eq!(zero.read(), 117_632_909);

        let mut one = Check32::new();
        one.fold(1);
        assert_eq!(one.read(), 235_265_819);

        let mut three = Check32::new();
        for word in [1, 2, 3] {
            three.fold(word);
        }
        assert_eq!(three.read(), 503_786_269);
    }

    #[test]
    fn folding_all_ones_into_the_seed_leaves_the_check_at_all_ones() {
        let mut check = Check32::new();
        check.fold(-1);
        assert_eq!(
            check.read(),
            -1,
            "the accumulator returns to zero and inverts"
        );
    }

    #[test]
    fn the_thirty_two_bit_shift_is_arithmetic_and_not_logical() {
        // A logical shift would admit a zero at the top on the very first round
        // of the very first fold, because the seed is negative. These two
        // vectors are what separates the two rules.
        let mut logical = 0xFFFF_FFFF_u32;
        logical ^= 1;
        for _ in 0..8 {
            let odd = logical & 1 != 0;
            logical >>= 1;
            if odd {
                logical ^= 0xEDB8_8320;
            }
        }
        let mut signed = Check32::new();
        signed.fold(1);
        assert_ne!(
            signed.read(),
            (!logical).cast_signed(),
            "the signed accumulator must not agree with the textbook rule"
        );
    }

    #[test]
    fn the_thirty_two_bit_convenience_path_folds_the_count_before_the_elements() {
        assert_eq!(check32_over(&[1, 2, 3]), 169_269_418);
        assert_eq!(check32_over(&[1, 2]), 281_375_761);
        assert_eq!(check32_over(&[]), 117_632_909);
    }

    #[test]
    fn two_sequences_differing_only_in_length_do_not_collide_under_the_count() {
        assert_ne!(check32_over(&[0]), check32_over(&[0, 0]));
        assert_ne!(check32_over(&[0, 0]), check32_over(&[0, 0, 0]));
    }

    #[test]
    fn the_sixty_four_bit_table_is_derived_rather_than_transcribed() {
        let table = Table64::placeholder();
        assert_eq!(table.entries[0], 0);
        assert_eq!(table.entries[1], PLACEHOLDER_POLYNOMIAL);
        assert_eq!(table.entries[255], 0x9AFC_E626_CE85_B507);
        assert_eq!(table.entries.len(), 256);
    }

    #[test]
    fn a_fresh_sixty_four_bit_check_reads_zero_and_is_never_inverted() {
        let table = Table64::placeholder();
        let check = Check64::new(&table);
        assert_eq!(check.read(), 0);

        let mut zero_byte = Check64::new(&table);
        zero_byte.fold_byte(0);
        assert_eq!(
            zero_byte.read(),
            0,
            "a zero byte into a zero seed stays zero"
        );
    }

    #[test]
    fn the_sixty_four_bit_check_pins_the_placeholder_format_by_bytes() {
        let table = Table64::placeholder();
        let mut check = Check64::new(&table);
        for byte in [1, 2, 3] {
            check.fold_byte(byte);
        }
        assert_eq!(check.read(), 0xF805_609E_CE1E_CBF3);
    }

    #[test]
    fn the_word_entry_point_feeds_least_significant_byte_first() {
        let table = Table64::placeholder();
        let mut word = Check64::new(&table);
        word.fold_word(1);

        let mut bytes = Check64::new(&table);
        for byte in [1, 0, 0, 0] {
            bytes.fold_byte(byte);
        }
        assert_eq!(word.read(), bytes.read());
        assert_eq!(word.read(), 0xE59C_4CF9_0CE5_976B);
    }

    #[test]
    fn the_single_argument_entry_point_keeps_only_the_low_byte() {
        let table = Table64::placeholder();
        let mut low = Check64::new(&table);
        low.fold_low_byte(0x1234);

        let mut byte = Check64::new(&table);
        byte.fold_byte(0x34);
        assert_eq!(low.read(), byte.read());

        let mut word = Check64::new(&table);
        word.fold_word(0x1234);
        assert_ne!(
            low.read(),
            word.read(),
            "the two integer entry points fold different data"
        );
    }

    #[test]
    fn the_sixty_four_bit_convenience_path_folds_the_count_before_the_elements() {
        let table = Table64::placeholder();
        assert_eq!(check64_over(&table, &[1, 2, 3]), 0xA044_210D_A3F7_4689);
        assert_ne!(
            check64_over(&table, &[0]),
            check64_over(&table, &[0, 0]),
            "the count separates two sequences of the same content"
        );
    }

    #[test]
    fn the_two_checks_share_no_seed_and_no_bit_order() {
        let table = Table64::placeholder();
        assert_eq!(Check32::new().read(), 0);
        assert_eq!(Check64::new(&table).read(), 0);

        let mut narrow = Check32::new();
        narrow.fold(1);
        let mut wide = Check64::new(&table);
        wide.fold_word(1);
        assert_ne!(u64::from(narrow.read().cast_unsigned()), wide.read());
    }

    #[test]
    fn the_declared_polynomial_is_absent_rather_than_defaulted_when_the_build_supplies_none() {
        assert_eq!(
            Table64::supplied(None),
            Err(PolynomialUnavailable::NotSupplied),
            "a polynomial nobody supplied must not resolve to a placeholder"
        );
        assert_eq!(
            Table64::supplied(Some("not hexadecimal")),
            Err(PolynomialUnavailable::NotHexadecimal)
        );
        assert_eq!(
            Table64::supplied(Some("0x42F0E1EBA9EA3693")),
            Ok(Table64::derive(0x42F0_E1EB_A9EA_3693))
        );
        assert_eq!(
            Table64::declared(),
            Table64::supplied(option_env!("RVL_120_COUNTERPART_CHECK64_POLYNOMIAL")),
            "what this build supplies is what the package reads, whatever that is"
        );
    }

    #[test]
    fn the_energy_clamp_saturates_above_and_truncates_everywhere_else() {
        assert_eq!(clamp_energy(0), 0);
        assert_eq!(clamp_energy(i64::from(i32::MAX)), i32::MAX);
        assert_eq!(clamp_energy(i64::from(i32::MAX) + 1), i32::MAX);
        assert_eq!(clamp_energy(i64::MAX), i32::MAX);
    }

    #[test]
    fn only_the_upper_bound_is_guarded_so_a_negative_amount_passes_through() {
        assert_eq!(clamp_energy(-1), -1);
        assert_eq!(clamp_energy(i64::from(i32::MIN)), i32::MIN);
        assert_eq!(
            clamp_energy(i64::from(i32::MIN) - 1),
            i32::MAX,
            "a value below the signed minimum wraps rather than clamping to zero"
        );
    }

    #[test]
    fn the_reporting_helper_ignores_the_capacity_it_is_handed() {
        for capacity in [0, 1, i64::MAX, -1] {
            assert_eq!(reported_energy(1_000, capacity), 1_000);
        }
        assert_eq!(reported_energy(i64::MAX, 1), i32::MAX);
    }

    #[test]
    fn a_count_beyond_the_signed_width_saturates_rather_than_wrapping() {
        assert_eq!(bounded_count(0), 0);
        assert_eq!(bounded_count(7), 7);
        assert_eq!(bounded_count(usize::MAX), i32::MAX);
    }
}
