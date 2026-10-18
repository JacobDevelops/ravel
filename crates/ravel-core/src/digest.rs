use std::fmt;

use serde::{Deserialize, Serialize};

const DIGEST_TEXT_LENGTH: usize = 64;
const DIGEST_BYTES: usize = 32;

/// A SHA-256 digest with a canonical 64-character lowercase hexadecimal representation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct Sha256Digest([u8; DIGEST_BYTES]);

impl Sha256Digest {
    /// Wraps a computed digest.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
        &self.0
    }

    fn parse(candidate: &str) -> Result<Self, Sha256Error> {
        if candidate.len() != DIGEST_TEXT_LENGTH {
            return Err(Sha256Error::Length {
                length: candidate.chars().count(),
            });
        }

        let mut bytes = [0_u8; DIGEST_BYTES];
        for (index, character) in candidate.chars().enumerate() {
            let Some(value) = hex_value(character) else {
                return Err(Sha256Error::Character { character, index });
            };
            let shift = if index % 2 == 0 { 4 } else { 0 };
            bytes[index.div_euclid(2)] |= value << shift;
        }
        Ok(Self(bytes))
    }
}

fn hex_value(character: char) -> Option<u8> {
    let value = match character {
        '0'..='9' => u32::from(character) - u32::from('0'),
        'a'..='f' => u32::from(character) - u32::from('a') + 10,
        _ => return None,
    };
    u8::try_from(value).ok()
}

impl TryFrom<String> for Sha256Digest {
    type Error = Sha256Error;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::parse(&candidate)
    }
}

impl From<Sha256Digest> for String {
    fn from(digest: Sha256Digest) -> Self {
        digest.to_string()
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Why a candidate string is not a valid [`Sha256Digest`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sha256Error {
    /// The candidate is not 64 characters long.
    Length {
        /// The candidate length in characters.
        length: usize,
    },
    /// A character is not a lowercase hex digit.
    Character {
        /// The rejected character.
        character: char,
        /// Zero-based character index of the rejection.
        index: usize,
    },
}

impl fmt::Display for Sha256Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { length } => write!(
                formatter,
                "a sha256 digest is {DIGEST_TEXT_LENGTH} characters long, not {length}"
            ),
            Self::Character { character, index } => write!(
                formatter,
                "a sha256 digest holds only `0`-`9` and `a`-`f`, but character {index} is \
                 `{character}`"
            ),
        }
    }
}

impl std::error::Error for Sha256Error {}

impl TryFrom<&str> for Sha256Digest {
    type Error = Sha256Error;

    fn try_from(candidate: &str) -> Result<Self, Self::Error> {
        Self::parse(candidate)
    }
}

impl std::str::FromStr for Sha256Digest {
    type Err = Sha256Error;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::parse(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::{Sha256Digest, Sha256Error};
    const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const MIXED_DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    #[test]
    fn a_digest_round_trips_through_its_lowercase_hex_form() {
        for candidate in [ZERO_DIGEST, MIXED_DIGEST, &"f".repeat(64)] {
            let digest = Sha256Digest::try_from(candidate.to_owned()).expect(candidate);
            assert_eq!(digest.to_string(), candidate);
            assert_eq!(String::from(digest), candidate);

            let encoded = serde_json::to_string(&digest).expect("should serialize");
            assert_eq!(encoded, format!("\"{candidate}\""));
            assert_eq!(
                serde_json::from_str::<Sha256Digest>(&encoded).expect("should deserialize"),
                digest
            );
        }
    }

    #[test]
    fn borrowed_and_owned_digest_parsers_enforce_the_same_contract() {
        for candidate in [ZERO_DIGEST, MIXED_DIGEST, "", "ABC", &"A".repeat(64)] {
            let owned = Sha256Digest::try_from(candidate.to_owned());
            assert_eq!(Sha256Digest::try_from(candidate), owned, "{candidate}");
            assert_eq!(candidate.parse::<Sha256Digest>(), owned, "{candidate}");
        }
    }

    #[test]
    fn a_digest_decodes_every_nibble_into_the_byte_it_names() {
        let digest = Sha256Digest::try_from(MIXED_DIGEST.to_owned()).expect("valid digest");
        assert_eq!(digest.as_bytes()[0], 0x01);
        assert_eq!(digest.as_bytes()[7], 0xef);
        assert_eq!(
            digest.as_bytes(),
            &Sha256Digest::from_bytes(*digest.as_bytes()).0
        );
    }

    #[test]
    fn a_digest_refuses_every_shape_the_schema_pattern_refuses() {
        for (candidate, expected) in [
            ("", Sha256Error::Length { length: 0 }),
            (&"a".repeat(63), Sha256Error::Length { length: 63 }),
            (&"a".repeat(65), Sha256Error::Length { length: 65 }),
            (
                &format!("{}A", &MIXED_DIGEST[..63]),
                Sha256Error::Character {
                    character: 'A',
                    index: 63,
                },
            ),
            (
                &format!("g{}", &MIXED_DIGEST[1..]),
                Sha256Error::Character {
                    character: 'g',
                    index: 0,
                },
            ),
            (
                &format!(" {}", &MIXED_DIGEST[1..]),
                Sha256Error::Character {
                    character: ' ',
                    index: 0,
                },
            ),
        ] {
            assert_eq!(
                Sha256Digest::try_from(candidate.to_owned()),
                Err(expected),
                "{candidate}"
            );
        }
    }

    #[test]
    fn a_multibyte_candidate_is_measured_in_characters_rather_than_bytes() {
        let candidate = format!("{}\u{e9}", &MIXED_DIGEST[..62]);
        assert_eq!(candidate.len(), 64);
        assert_eq!(
            Sha256Digest::try_from(candidate),
            Err(Sha256Error::Character {
                character: '\u{e9}',
                index: 62,
            })
        );

        assert_eq!(
            Sha256Digest::try_from("\u{e9}".to_owned()),
            Err(Sha256Error::Length { length: 1 })
        );
    }

    #[test]
    fn every_digest_rejection_renders_the_rule_it_breaks() {
        for (error, expected) in [
            (
                Sha256Error::Length { length: 3 },
                "a sha256 digest is 64 characters long, not 3",
            ),
            (
                Sha256Error::Character {
                    character: 'A',
                    index: 63,
                },
                "a sha256 digest holds only `0`-`9` and `a`-`f`, but character 63 is `A`",
            ),
        ] {
            assert_eq!(error.to_string(), expected);
        }
    }
}
