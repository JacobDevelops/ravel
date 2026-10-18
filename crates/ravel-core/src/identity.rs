use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

const SEPARATORS: [char; 3] = ['-', '_', '.'];

/// A stable mod identity matching `^[a-z][a-z0-9]*(?:[-_.][a-z0-9]+)*$`.
///
/// # Examples
///
/// ```
/// use ravel_core::{ModId, ModIdError};
///
/// let id: ModId = "example_mod".parse()?;
/// assert_eq!(id.as_str(), "example_mod");
/// assert_eq!(id.to_string(), "example_mod");
///
/// assert!("Example".parse::<ModId>().is_err());
/// assert!("9lives".parse::<ModId>().is_err());
/// assert!("example_".parse::<ModId>().is_err());
/// # Ok::<(), ModIdError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct ModId(String);

impl ModId {
    /// Returns the identity as its canonical string form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Displays a comma-separated list of backtick-quoted identities without allocation.
    ///
    /// An empty list displays as two backticks.
    #[must_use]
    pub fn display_list(ids: &[Self]) -> impl fmt::Display + '_ {
        ModIdList(ids)
    }
}

/// Checks the shared mod-identity and setting-key grammar.
///
/// # Errors
///
/// Returns [`ModIdError`] for an empty candidate, an invalid leading or later character,
/// or a separator without a following letter or digit.
pub fn check_identifier(candidate: &str) -> Result<(), ModIdError> {
    if candidate.is_empty() {
        return Err(ModIdError::Empty);
    }
    let mut pending_separator = None;
    for (index, character) in candidate.chars().enumerate() {
        if index == 0 {
            if !character.is_ascii_lowercase() {
                return Err(ModIdError::Leading { character });
            }
        } else if character.is_ascii_lowercase() || character.is_ascii_digit() {
            pending_separator = None;
        } else if SEPARATORS.contains(&character) {
            if let Some((character, index)) = pending_separator {
                return Err(ModIdError::Separator { character, index });
            }
            pending_separator = Some((character, index));
        } else {
            return Err(ModIdError::Character { character, index });
        }
    }
    if let Some((character, index)) = pending_separator {
        return Err(ModIdError::Separator { character, index });
    }
    Ok(())
}

impl FromStr for ModId {
    type Err = ModIdError;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        check_identifier(candidate)?;
        Ok(Self(candidate.to_owned()))
    }
}

impl TryFrom<String> for ModId {
    type Error = ModIdError;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        check_identifier(&candidate)?;
        Ok(Self(candidate))
    }
}

impl From<ModId> for String {
    fn from(id: ModId) -> Self {
        id.0
    }
}

impl fmt::Display for ModId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

struct ModIdList<'a>(&'a [ModId]);

impl fmt::Display for ModIdList<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("`")?;
        if let Some((first, rest)) = self.0.split_first() {
            write!(formatter, "{first}")?;
            for id in rest {
                write!(formatter, "`, `{id}")?;
            }
        }
        formatter.write_str("`")
    }
}

/// Why a mod identity or setting key fails the shared identifier grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModIdError {
    /// The candidate is empty.
    Empty,
    /// The first character is not an ASCII lowercase letter.
    Leading {
        /// The rejected first character.
        character: char,
    },
    /// A later character is neither `[a-z0-9]` nor a separator.
    Character {
        /// The rejected character.
        character: char,
        /// Zero-based character index of the rejection.
        index: usize,
    },
    /// A separator is not followed by a letter or digit.
    Separator {
        /// The misplaced separator.
        character: char,
        /// Zero-based character index of the rejection.
        index: usize,
    },
}

impl fmt::Display for ModIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("an identifier cannot be empty"),
            Self::Leading { character } => write!(
                formatter,
                "an identifier starts with an ASCII lowercase letter, not `{character}`"
            ),
            Self::Character { character, index } => write!(
                formatter,
                "an identifier holds only `a`-`z`, `0`-`9`, `-`, `_`, and `.`, but character {index} \
                 is `{character}`"
            ),
            Self::Separator { character, index } => write!(
                formatter,
                "an identifier separator is always followed by a letter or a digit, but `{character}` \
                 at character {index} is not"
            ),
        }
    }
}

impl std::error::Error for ModIdError {}

#[cfg(test)]
mod tests {
    use super::{ModId, ModIdError};

    #[test]
    fn identity_lists_quote_names_in_order_including_empty_and_single_lists() {
        let ids: Vec<ModId> = ["first_mod", "second_mod", "third_mod"]
            .into_iter()
            .map(|name| name.parse().expect("valid id"))
            .collect();
        for (count, expected) in [
            (0, "``"),
            (1, "`first_mod`"),
            (2, "`first_mod`, `second_mod`"),
            (3, "`first_mod`, `second_mod`, `third_mod`"),
        ] {
            assert_eq!(ModId::display_list(&ids[..count]).to_string(), expected);
        }
    }

    #[test]
    fn accepted_ids_cover_every_position_of_the_grammar() {
        for candidate in [
            "a",
            "ab",
            "mod9",
            "example_mod",
            "example-mod",
            "example.mod",
            "a-b_c.d9",
            "z9.z9",
            &"a".repeat(200),
        ] {
            let id: ModId = candidate.parse().expect("candidate should be a valid id");
            assert_eq!(id.as_str(), candidate);
            assert_eq!(id.to_string(), candidate, "display must round-trip parse");
        }
    }

    #[test]
    fn rejected_ids_report_the_rule_they_break() {
        for (candidate, expected) in [
            ("", ModIdError::Empty),
            ("Example", ModIdError::Leading { character: 'E' }),
            ("9lives", ModIdError::Leading { character: '9' }),
            ("_leading", ModIdError::Leading { character: '_' }),
            (".leading", ModIdError::Leading { character: '.' }),
            ("-leading", ModIdError::Leading { character: '-' }),
            (
                "example mod",
                ModIdError::Character {
                    character: ' ',
                    index: 7,
                },
            ),
            (
                "exampleMod",
                ModIdError::Character {
                    character: 'M',
                    index: 7,
                },
            ),
            (
                "example/mod",
                ModIdError::Character {
                    character: '/',
                    index: 7,
                },
            ),
            (
                "example:mod",
                ModIdError::Character {
                    character: ':',
                    index: 7,
                },
            ),
            (
                "example_",
                ModIdError::Separator {
                    character: '_',
                    index: 7,
                },
            ),
            (
                "example__mod",
                ModIdError::Separator {
                    character: '_',
                    index: 7,
                },
            ),
            (
                "example-.mod",
                ModIdError::Separator {
                    character: '-',
                    index: 7,
                },
            ),
            (
                "a-",
                ModIdError::Separator {
                    character: '-',
                    index: 1,
                },
            ),
        ] {
            assert_eq!(candidate.parse::<ModId>(), Err(expected), "{candidate}");
        }
    }

    #[test]
    fn doubled_separator_diagnostic_names_the_separator_without_a_letter_after_it() {
        let refusal = "example-.mod"
            .parse::<ModId>()
            .expect_err("adjacent separators");
        assert!(
            refusal.to_string().contains("`-` at character 7"),
            "{refusal}"
        );
        assert!(
            !refusal.to_string().contains("`.` at character 8"),
            "{refusal}"
        );
    }

    #[test]
    fn a_multibyte_candidate_is_rejected_at_the_character_that_breaks_the_rule() {
        assert_eq!(
            "\u{e9}".parse::<ModId>(),
            Err(ModIdError::Leading {
                character: '\u{e9}'
            })
        );
        assert_eq!(
            "a\u{e9}".parse::<ModId>(),
            Err(ModIdError::Character {
                character: '\u{e9}',
                index: 1,
            })
        );
    }

    #[test]
    fn every_rejection_renders_the_broken_rule() {
        for (error, expected) in [
            (ModIdError::Empty, "an identifier cannot be empty"),
            (
                ModIdError::Leading { character: 'E' },
                "an identifier starts with an ASCII lowercase letter, not `E`",
            ),
            (
                ModIdError::Character {
                    character: '/',
                    index: 7,
                },
                "an identifier holds only `a`-`z`, `0`-`9`, `-`, `_`, and `.`, but character 7 is `/`",
            ),
            (
                ModIdError::Separator {
                    character: '_',
                    index: 7,
                },
                "an identifier separator is always followed by a letter or a digit, but `_` at \
                 character 7 is not",
            ),
        ] {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn an_id_round_trips_through_its_serialized_string() {
        let id: ModId = "example_mod".parse().expect("valid id");
        let encoded = serde_json::to_string(&id).expect("id should serialize");
        assert_eq!(encoded, "\"example_mod\"");
        assert_eq!(
            serde_json::from_str::<ModId>(&encoded).expect("id should deserialize"),
            id
        );
        assert_eq!(String::from(id), "example_mod");
    }

    #[test]
    fn deserialization_rejects_an_invalid_id_rather_than_carrying_it() {
        let error = serde_json::from_str::<ModId>("\"Example\"").expect_err("invalid id");
        assert!(
            error
                .to_string()
                .contains("an identifier starts with an ASCII lowercase letter"),
            "{error}"
        );
    }
}
