//! Packed colours, saturation, interpolation and opacity arithmetic.

use num_traits::ToPrimitive;

/// A colour, as four channels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Colour {
    /// The opacity channel.
    pub opacity: u8,
    /// The red channel.
    pub red: u8,
    /// The green channel.
    pub green: u8,
    /// The blue channel.
    pub blue: u8,
}

impl Colour {
    /// Builds a colour from four channels.
    #[must_use]
    pub const fn new(opacity: u8, red: u8, green: u8, blue: u8) -> Self {
        Self {
            opacity,
            red,
            green,
            blue,
        }
    }

    /// Packs the four channels into one number, opacity highest.
    #[must_use]
    pub const fn pack(self) -> u32 {
        u32::from_be_bytes([self.opacity, self.red, self.green, self.blue])
    }

    /// Reads four channels back out of one number.
    #[must_use]
    pub const fn unpack(packed: u32) -> Self {
        let [opacity, red, green, blue] = packed.to_be_bytes();
        Self {
            opacity,
            red,
            green,
            blue,
        }
    }

    /// Returns this colour with a different opacity.
    #[must_use]
    pub const fn with_opacity(self, opacity: u8) -> Self {
        Self { opacity, ..self }
    }

    /// Scales this colour's saturation, keeping hue, brightness and opacity.
    #[must_use]
    pub fn scale_saturation(self, factor: f64) -> Self {
        let (hue, saturation, brightness) = self.to_hsb();
        Self::from_hsb(hue, (saturation * factor).clamp(0.0, 1.0), brightness)
            .with_opacity(self.opacity)
    }

    /// Interpolates between two colours through hue, saturation and brightness.
    #[must_use]
    pub fn interpolate(self, other: Self, ratio: f64) -> Self {
        let ratio = ratio.clamp(0.0, 1.0);
        let (from_hue, from_saturation, from_brightness) = self.to_hsb();
        let (to_hue, to_saturation, to_brightness) = other.to_hsb();
        // Around the short way, so a hue near one and a hue near zero meet at the
        // wrap rather than sweeping the whole circle between them.
        let mut span = to_hue - from_hue;
        if span > 0.5 {
            span -= 1.0;
        } else if span < -0.5 {
            span += 1.0;
        }
        let hue_step = span * ratio;
        let saturation_step = (to_saturation - from_saturation) * ratio;
        let brightness_step = (to_brightness - from_brightness) * ratio;
        let opacity_step = (f64::from(other.opacity) - f64::from(self.opacity)) * ratio;
        let hue = (from_hue + hue_step).rem_euclid(1.0);
        let saturation = from_saturation + saturation_step;
        let brightness = from_brightness + brightness_step;
        let opacity = f64::from(self.opacity) + opacity_step;
        Self::from_hsb(hue, saturation, brightness).with_opacity(round_channel(opacity))
    }

    fn to_hsb(self) -> (f64, f64, f64) {
        let red = f64::from(self.red) / 255.0;
        let green = f64::from(self.green) / 255.0;
        let blue = f64::from(self.blue) / 255.0;
        let high = red.max(green).max(blue);
        let low = red.min(green).min(blue);
        let span = high - low;
        let saturation = if high == 0.0 { 0.0 } else { span / high };
        let hue = if span == 0.0 {
            0.0
        } else if (high - red).abs() < f64::EPSILON {
            ((green - blue) / span).rem_euclid(6.0)
        } else if (high - green).abs() < f64::EPSILON {
            (blue - red) / span + 2.0
        } else {
            (red - green) / span + 4.0
        };
        (hue / 6.0, saturation, high)
    }

    fn from_hsb(hue: f64, saturation: f64, brightness: f64) -> Self {
        let hue = hue.rem_euclid(1.0) * 6.0;
        let saturation = saturation.clamp(0.0, 1.0);
        let brightness = brightness.clamp(0.0, 1.0);
        let sector = hue.floor();
        let offset = hue - sector;
        let p = brightness * (1.0 - saturation);
        let q_saturation = saturation * offset;
        let t_saturation = saturation * (1.0 - offset);
        let q = brightness * (1.0 - q_saturation);
        let t = brightness * (1.0 - t_saturation);
        // Finite wrapped sectors fit i32; a non-finite hue keeps the original zero sector.
        let (red, green, blue) = match sector.to_i32().unwrap_or(0) % 6 {
            0 => (brightness, t, p),
            1 => (q, brightness, p),
            2 => (p, brightness, t),
            3 => (p, q, brightness),
            4 => (t, p, brightness),
            _ => (brightness, p, q),
        };
        Self::new(
            255,
            round_channel(red * 255.0),
            round_channel(green * 255.0),
            round_channel(blue * 255.0),
        )
    }
}

fn round_channel(value: f64) -> u8 {
    let rounded = value.round();
    if rounded <= 0.0 {
        0
    } else if rounded >= 255.0 {
        255
    } else {
        // The bounds above leave only an in-range channel or NaN, which encodes as zero.
        rounded.to_u8().unwrap_or(0)
    }
}

/// An opacity derived from a ratio.
///
/// The original truncates the ratio to an integer before multiplying, so it can
/// only ever answer zero or the maximum. That is a defect; this computes it in
/// floating point.
#[must_use]
pub fn opacity_from_ratio(ratio: f64) -> u8 {
    round_channel(ratio.clamp(0.0, 1.0) * 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_colour_packs_and_unpacks_through_one_number() {
        let colour = Colour::new(0x80, 0x11, 0x22, 0x33);
        assert_eq!(colour.pack(), 0x8011_2233);
        assert_eq!(Colour::unpack(colour.pack()), colour);
    }

    #[test]
    fn scaling_saturation_to_nothing_leaves_a_grey_of_the_same_brightness() {
        let grey = Colour::new(255, 200, 0, 0).scale_saturation(0.0);
        assert_eq!(grey, Colour::new(255, 200, 200, 200));
    }

    #[test]
    fn interpolating_ends_at_each_end_and_carries_the_opacity_across() {
        let from = Colour::new(0, 255, 0, 0);
        let to = Colour::new(255, 0, 0, 255);
        assert_eq!(from.interpolate(to, 0.0), from);
        assert_eq!(from.interpolate(to, 1.0), to);
        assert_eq!(from.interpolate(to, 0.5).opacity, 128);
    }

    #[test]
    fn an_opacity_from_a_ratio_is_computed_in_floating_point_rather_than_truncated() {
        assert_eq!(opacity_from_ratio(0.0), 0);
        assert_eq!(opacity_from_ratio(1.0), 255);
        assert_eq!(
            opacity_from_ratio(0.5),
            128,
            "truncating the ratio first would answer zero here"
        );
        assert_eq!(opacity_from_ratio(2.0), 255);
    }
}
