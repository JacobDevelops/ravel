//! Whole-value pattern parsing and budgeted matching, independent of tag validation.

/// Why a pattern was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternError {
    /// The pattern uses a construct outside the supported subset.
    Unsupported(char),
    /// A repetition has no atom before it.
    DanglingRepeat,
    /// A character class does not close.
    UnclosedClass,
    /// An escape has nothing after it.
    DanglingEscape,
    /// The pattern holds more atoms than [`MAX_PATTERN_ATOMS`].
    TooLong(usize),
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(found) => {
                write!(
                    formatter,
                    "`{found}` is outside the supported pattern subset"
                )
            }
            Self::DanglingRepeat => write!(formatter, "a repetition has nothing to repeat"),
            Self::UnclosedClass => write!(formatter, "a character class does not close"),
            Self::DanglingEscape => write!(formatter, "an escape has nothing after it"),
            Self::TooLong(atoms) => write!(formatter, "a pattern holds {atoms} atoms"),
        }
    }
}

impl std::error::Error for PatternError {}

/// The most atoms a pattern may hold.
pub const MAX_PATTERN_ATOMS: usize = 256;

/// The most matching steps one candidate may cost.
///
/// A backtracking matcher is exponential in the worst case and a pattern arrives
/// from a document. The budget makes the cost a refusal rather than a hang; the
/// upgrade path is a matcher that does not backtrack.
pub const MAX_PATTERN_STEPS: u32 = 200_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Repeat {
    Once,
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Atom {
    Any,
    Literal(char),
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

impl Atom {
    fn admits(&self, value: char) -> bool {
        match self {
            Self::Any => true,
            Self::Literal(expected) => *expected == value,
            Self::Class { negated, ranges } => {
                let inside = ranges
                    .iter()
                    .any(|(low, high)| value >= *low && value <= *high);
                inside != *negated
            }
        }
    }
}

/// A pattern that must match a whole value.
///
/// **Declared change.** The original delegates to the platform's own regular
/// expression engine. This package supports a stated subset — literals, `.`, a
/// `[...]` character class with ranges and a leading `^` negation, the
/// repetitions `*`, `+` and `?`, and `\` escaping one of this subset's own
/// metacharacters — and refuses anything else at decode rather than accepting a
/// pattern it would match differently. `\d`, `\w` and `\s` name classes this
/// subset does not have, so each is refused rather than read as its letter. Matching is anchored at both ends either way, which is
/// what the original's whole-value rule means.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pattern {
    source: String,
    pieces: Vec<(Atom, Repeat)>,
}

impl Pattern {
    /// Reads a pattern.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError`] for a construct outside the supported subset, a
    /// repetition with nothing before it, an unclosed class, a dangling escape,
    /// or a pattern past [`MAX_PATTERN_ATOMS`].
    pub fn parse(source: &str) -> Result<Self, PatternError> {
        let characters: Vec<char> = source.chars().collect();
        let mut pieces: Vec<(Atom, Repeat)> = Vec::new();
        let mut index = 0;
        while index < characters.len() {
            let atom = match characters[index] {
                '.' => {
                    index += 1;
                    Atom::Any
                }
                '\\' => {
                    index += 1;
                    let escaped = *characters.get(index).ok_or(PatternError::DanglingEscape)?;
                    index += 1;
                    Atom::Literal(escaped_literal(escaped)?)
                }
                '[' => {
                    index += 1;
                    let (class, next) = parse_class(&characters, index)?;
                    index = next;
                    class
                }
                '*' | '+' | '?' => return Err(PatternError::DanglingRepeat),
                unsupported @ ('(' | ')' | '|' | '{' | '}' | '^' | '$') => {
                    return Err(PatternError::Unsupported(unsupported));
                }
                literal => {
                    index += 1;
                    Atom::Literal(literal)
                }
            };
            let repeat = match characters.get(index) {
                Some('*') => {
                    index += 1;
                    Repeat::ZeroOrMore
                }
                Some('+') => {
                    index += 1;
                    Repeat::OneOrMore
                }
                Some('?') => {
                    index += 1;
                    Repeat::ZeroOrOne
                }
                _ => Repeat::Once,
            };
            if pieces.len() == MAX_PATTERN_ATOMS {
                return Err(PatternError::TooLong(MAX_PATTERN_ATOMS + 1));
            }
            pieces.push((atom, repeat));
        }
        Ok(Self {
            source: source.to_owned(),
            pieces,
        })
    }

    /// Returns the spelling this pattern was read from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns whether the pattern matches the whole of a value.
    ///
    /// A candidate that costs more than [`MAX_PATTERN_STEPS`] is false rather
    /// than a hang.
    #[must_use]
    pub fn matches_whole(&self, candidate: &str) -> bool {
        let characters: Vec<char> = candidate.chars().collect();
        let mut budget = MAX_PATTERN_STEPS;
        walk(&self.pieces, &characters, &mut budget)
    }
}

/// The characters an escape may name, which are this subset's own metacharacters.
///
/// An escape outside this set is refused rather than read as the character
/// itself: a pattern author writing `\d` for a digit means a class this subset
/// does not have, and reading it as the letter `d` would match something else
/// silently.
const ESCAPABLE: [char; 9] = ['.', '[', ']', '*', '+', '?', '\\', '-', '^'];

fn escaped_literal(escaped: char) -> Result<char, PatternError> {
    if ESCAPABLE.contains(&escaped) {
        Ok(escaped)
    } else {
        Err(PatternError::Unsupported(escaped))
    }
}

fn parse_class(characters: &[char], mut index: usize) -> Result<(Atom, usize), PatternError> {
    let negated = characters.get(index) == Some(&'^');
    if negated {
        index += 1;
    }
    let mut ranges = Vec::new();
    loop {
        let current = *characters.get(index).ok_or(PatternError::UnclosedClass)?;
        if current == ']' {
            index += 1;
            break;
        }
        let low = if current == '\\' {
            index += 1;
            escaped_literal(*characters.get(index).ok_or(PatternError::DanglingEscape)?)?
        } else {
            current
        };
        index += 1;
        if characters.get(index) == Some(&'-')
            && let Some(&high) = characters.get(index + 1)
            && high != ']'
        {
            ranges.push((low, high));
            index += 2;
        } else {
            ranges.push((low, low));
        }
        if ranges.len() > MAX_PATTERN_ATOMS {
            return Err(PatternError::TooLong(ranges.len()));
        }
    }
    Ok((Atom::Class { negated, ranges }, index))
}

fn walk(pieces: &[(Atom, Repeat)], candidate: &[char], budget: &mut u32) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let Some(((atom, repeat), rest)) = pieces.split_first() else {
        return candidate.is_empty();
    };
    match repeat {
        Repeat::Once => match candidate.split_first() {
            Some((first, tail)) if atom.admits(*first) => walk(rest, tail, budget),
            _ => false,
        },
        Repeat::ZeroOrOne => {
            if let Some((first, tail)) = candidate.split_first()
                && atom.admits(*first)
                && walk(rest, tail, budget)
            {
                return true;
            }
            walk(rest, candidate, budget)
        }
        Repeat::ZeroOrMore | Repeat::OneOrMore => {
            let mut taken = 0;
            while taken < candidate.len() && atom.admits(candidate[taken]) {
                taken += 1;
            }
            let least = usize::from(*repeat == Repeat::OneOrMore);
            let mut count = taken;
            loop {
                if count < least {
                    return false;
                }
                if walk(rest, &candidate[count..], budget) {
                    return true;
                }
                if count == least {
                    return false;
                }
                count -= 1;
            }
        }
    }
}
