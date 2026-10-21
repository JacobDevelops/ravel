//! The colour of §8: four channels held as floating-point values, and the three
//! packings they produce.
//!
//! Nothing clamps, so a channel outside the unit interval is representable, and
//! every packing multiplies by two hundred and fifty-five and truncates toward
//! zero rather than rounding. A colour built from bytes therefore round-trips
//! exactly and one built from floats loses up to one part in two hundred and
//! fifty-five.
//!
//! Equality and the hash are both defined on the alpha-red-green-blue packing, so
//! two colours differing by less than one part in two hundred and fifty-five
//! compare equal and the hash is the packed integer itself. That is
//! compatibility-required in effect rather than on the wire: a dependent that
//! stores colours in a hash structure gets different lookups without it.
//!
//! # The change declared here
//!
//! **The two byte-and-float constructors have distinct names.** In the original
//! they differ only in argument type, so a bare literal triple selects the
//! integer overload and a written triple means one two-hundred-and-fifty-fifth of
//! full intensity rather than full intensity. [`Colour::from_floats`] and
//! [`Colour::from_bytes`] cannot be confused, and Rust would not have offered the
//! overload anyway.
//!
//! # Two shapes worth stating
//!
//! **A channel outside the unit interval packs at the edge.** Nothing clamps the
//! stored channel — a colour built from two is a colour holding two, which is
//! what §8 says is representable — but a packing has one byte per channel and
//! two hundred and fifty-five times two does not fit in one. This package packs
//! it as full intensity rather than letting the overflow run into the byte
//! beside it, so a channel can never change another channel's value. §8 does not
//! say what the original does with the overflow and §14 does not record it, so
//! this edge is a choice rather than something obtained: a colour outside the
//! interval packs differently there, and a colour inside it — every colour a
//! dependent writes deliberately — packs identically.
//!
//! **A channel that is not a number packs as zero** rather than as an arbitrary
//! byte, and two such colours compare equal. Nothing here lets a `NaN` make a
//! value that is unequal to itself, because these are compared and hashed by
//! their packing and never by their channels.

use std::hash::{Hash, Hasher};

/// A colour, held as four channels nominally in the unit interval.
#[derive(Clone, Copy, Debug)]
pub struct Colour {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

/// Truncates a channel toward zero into a byte, as every packing does.
///
/// A channel outside the unit interval packs at the edge, which is this
/// package's own answer and is declared in the module documentation above.
fn channel_byte(value: f32) -> u8 {
    num_traits::AsPrimitive::<u8>::as_(value * 255.0)
}

impl Colour {
    /// Opaque white, the colour nothing was said about.
    pub const WHITE: Self = Self {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };

    /// Four channels, as fractions.
    #[must_use]
    pub const fn from_floats(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Three channels as fractions, fully opaque.
    #[must_use]
    pub const fn opaque_floats(red: f32, green: f32, blue: f32) -> Self {
        Self::from_floats(red, green, blue, 1.0)
    }

    /// Four channels, as byte intensities.
    #[must_use]
    pub fn from_bytes(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red: f32::from(red) / 255.0,
            green: f32::from(green) / 255.0,
            blue: f32::from(blue) / 255.0,
            alpha: f32::from(alpha) / 255.0,
        }
    }

    /// Three channels as byte intensities, fully opaque.
    #[must_use]
    pub fn opaque_bytes(red: u8, green: u8, blue: u8) -> Self {
        Self::from_bytes(red, green, blue, 255)
    }

    /// One packed value, read alpha-red-green-blue from the high byte down.
    ///
    /// This is the declared form: a colour written into a document is this one
    /// integer.
    #[must_use]
    pub fn from_packed(packed: u32) -> Self {
        let [alpha, red, green, blue] = packed.to_be_bytes();
        Self::from_bytes(red, green, blue, alpha)
    }

    /// Returns the red channel as a fraction.
    #[must_use]
    pub const fn red(self) -> f32 {
        self.red
    }

    /// Returns the green channel as a fraction.
    #[must_use]
    pub const fn green(self) -> f32 {
        self.green
    }

    /// Returns the blue channel as a fraction.
    #[must_use]
    pub const fn blue(self) -> f32 {
        self.blue
    }

    /// Returns the alpha channel as a fraction.
    #[must_use]
    pub const fn alpha(self) -> f32 {
        self.alpha
    }

    /// Returns the alpha-red-green-blue packing, which is also the declared form
    /// and the value equality and the hash are defined on.
    #[must_use]
    pub fn packed(self) -> u32 {
        (u32::from(channel_byte(self.alpha)) << 24)
            | (u32::from(channel_byte(self.red)) << 16)
            | (u32::from(channel_byte(self.green)) << 8)
            | u32::from(channel_byte(self.blue))
    }

    /// Returns the red-green-blue-alpha packing.
    #[must_use]
    pub fn packed_trailing_alpha(self) -> u32 {
        (u32::from(channel_byte(self.red)) << 24)
            | (u32::from(channel_byte(self.green)) << 16)
            | (u32::from(channel_byte(self.blue)) << 8)
            | u32::from(channel_byte(self.alpha))
    }

    /// Returns the twenty-four-bit red-green-blue packing.
    #[must_use]
    pub fn packed_opaque(self) -> u32 {
        self.packed() & 0x00ff_ffff
    }

    /// Writes the wire form: the declared packing as four fixed bytes.
    #[must_use]
    pub fn write(self) -> [u8; 4] {
        self.packed().to_be_bytes()
    }

    /// Reads the wire form.
    #[must_use]
    pub fn read(bytes: [u8; 4]) -> Self {
        Self::from_packed(u32::from_be_bytes(bytes))
    }
}

impl Default for Colour {
    fn default() -> Self {
        Self::WHITE
    }
}

impl PartialEq for Colour {
    fn eq(&self, other: &Self) -> bool {
        self.packed() == other.packed()
    }
}

impl Eq for Colour {}

impl Hash for Colour {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.packed());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::Colour;

    fn hashed(colour: Colour) -> u64 {
        let mut hasher = DefaultHasher::new();
        colour.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn a_colour_built_from_bytes_round_trips_exactly() {
        for packed in [0x0000_0000, 0xffff_ffff, 0x8040_2010, 0x00ff_00ff] {
            assert_eq!(Colour::from_packed(packed).packed(), packed);
        }
        let colour = Colour::from_bytes(1, 2, 3, 4);
        assert_eq!(Colour::read(colour.write()), colour);
    }

    #[test]
    fn every_packing_truncates_toward_zero_rather_than_rounding() {
        // A channel one part short of full intensity truncates down a step.
        let colour = Colour::from_floats(0.999, 0.5, 0.0, 1.0);
        assert_eq!(colour.packed(), 0xfffe_7f00);
        assert_eq!(colour.packed_trailing_alpha(), 0xfe7f_00ff);
        assert_eq!(colour.packed_opaque(), 0x00fe_7f00);
    }

    #[test]
    fn a_channel_outside_the_unit_interval_is_representable_and_packs_at_the_edge() {
        let over = Colour::from_floats(2.0, -1.0, 0.0, 1.0);
        assert!(over.red() > 1.0, "nothing clamps the stored channel");
        assert!(over.green() < 0.0);
        // The edge is this package's answer to an overflow §8 does not describe,
        // declared in this module's documentation. What it buys is that a
        // channel never changes another channel's byte.
        assert_eq!(over.packed(), 0xffff_0000);
    }

    #[test]
    fn two_colours_closer_than_one_part_in_two_hundred_and_fifty_five_are_one_colour() {
        let first = Colour::from_floats(0.5, 0.5, 0.5, 1.0);
        let second = Colour::from_floats(0.501, 0.5, 0.5, 1.0);
        assert_eq!(first, second);
        assert_eq!(hashed(first), hashed(second));
        assert_eq!(hashed(first), hashed(Colour::from_packed(first.packed())));
    }

    #[test]
    fn a_colour_is_usable_as_a_key_and_finds_itself_by_its_packing() {
        let mut table = HashMap::new();
        table.insert(Colour::from_floats(0.5, 0.25, 0.125, 1.0), "one");
        assert_eq!(
            table.get(&Colour::from_packed(
                Colour::from_floats(0.5, 0.25, 0.125, 1.0).packed()
            )),
            Some(&"one")
        );
    }

    #[test]
    fn the_two_three_channel_constructors_cannot_be_confused() {
        assert_eq!(Colour::opaque_floats(1.0, 1.0, 1.0), Colour::WHITE);
        assert_eq!(Colour::opaque_bytes(255, 255, 255), Colour::WHITE);
        assert_ne!(
            Colour::opaque_bytes(1, 1, 1),
            Colour::opaque_floats(1.0, 1.0, 1.0),
            "a byte triple of ones is not full intensity, and it cannot be written as one by accident"
        );
        assert_eq!(Colour::default(), Colour::WHITE);
    }

    #[test]
    fn a_channel_that_is_not_a_number_packs_as_none_of_it_and_equals_itself() {
        let broken = Colour::from_floats(f32::NAN, 0.0, 0.0, 1.0);
        assert_eq!(broken.packed(), 0xff00_0000);
        assert_eq!(broken, broken, "a colour is compared by its packing");
        assert_eq!(
            hashed(broken),
            hashed(Colour::from_floats(0.0, 0.0, 0.0, 1.0))
        );
    }

    #[test]
    fn infinite_channels_saturate_and_half_bytes_truncate() {
        assert_eq!(super::channel_byte(f32::INFINITY), 255);
        assert_eq!(super::channel_byte(f32::NEG_INFINITY), 0);
        assert_eq!(super::channel_byte(0.5), 127);
    }

    #[test]
    fn the_alpha_channel_sits_where_each_packing_says_it_does() {
        let half = Colour::from_bytes(0x11, 0x22, 0x33, 0x44);
        assert_eq!(half.packed(), 0x4411_2233);
        assert_eq!(half.packed_trailing_alpha(), 0x1122_3344);
        assert_eq!(half.packed_opaque(), 0x0011_2233);
    }
}
