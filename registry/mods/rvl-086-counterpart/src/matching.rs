//! The one matching language three settings share.
//!
//! Section eight of the specification: a tiny predicate over a single entry and
//! its registry identity. Three settings are lists of these — the tooltip
//! exclusions, the repair blacklist and the ineffective list — and all three ask
//! the same question, so the language is written once and the three share it.
//!
//! A tag is resolved at every call rather than remembered, so a list that names
//! a tag follows a data reload. That is the difference between this and the
//! override table of [`crate::overrides`], which the original resolves once.

use std::fmt;

/// The prefix that introduces a tag selector.
pub const TAG_PREFIX: &str = "tag:";

/// The character that stands for every entry in a namespace.
pub const WILDCARD: &str = "*";

/// One expression, as the language admits it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expression {
    /// Membership of a namespaced tag.
    Tag(String),
    /// Every entry in one namespace.
    Namespace(String),
    /// One entry, compared exactly.
    Entry(String),
    /// Anything the language does not admit, which matches nothing.
    Nothing,
}

impl Expression {
    /// Reads one expression.
    ///
    /// The three admitted shapes are told apart by counting colon-separated
    /// fields, which is what the original does: three fields is a tag and also
    /// has to carry the prefix, two fields is either a namespace wildcard or a
    /// whole identifier, and any other count is not a shape this language has.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Self::Nothing;
        }
        let fields: Vec<&str> = trimmed.split(':').collect();
        match fields.len() {
            3 if trimmed.starts_with(TAG_PREFIX) => {
                let tag = trimmed[TAG_PREFIX.len()..].to_owned();
                if tag.split(':').any(str::is_empty) {
                    Self::Nothing
                } else {
                    Self::Tag(tag)
                }
            }
            2 if fields[0].is_empty() => Self::Nothing,
            2 if fields[1] == WILDCARD => Self::Namespace(fields[0].to_owned()),
            2 if fields[1].is_empty() => Self::Nothing,
            2 => Self::Entry(trimmed.to_owned()),
            _ => Self::Nothing,
        }
    }

    /// Answers whether one entry matches.
    ///
    /// `member_of` answers tag membership for the entry being tested, and is
    /// consulted only by the tag branch, at the moment of the call.
    #[must_use]
    pub fn matches(&self, entry: &str, member_of: &dyn Fn(&str) -> bool) -> bool {
        match self {
            Self::Tag(tag) => member_of(tag),
            Self::Namespace(namespace) => entry
                .split_once(':')
                .is_some_and(|(found, _)| found == namespace),
            Self::Entry(wanted) => wanted == entry,
            Self::Nothing => false,
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag(tag) => write!(formatter, "{TAG_PREFIX}{tag}"),
            Self::Namespace(namespace) => write!(formatter, "{namespace}:{WILDCARD}"),
            Self::Entry(entry) => formatter.write_str(entry),
            Self::Nothing => formatter.write_str(""),
        }
    }
}

/// A whole setting: a list of expressions, matching if any of them does.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Matcher {
    expressions: Vec<Expression>,
}

impl Matcher {
    /// Reads a comma-separated setting into a matcher.
    ///
    /// An entry the language does not admit is kept as [`Expression::Nothing`]
    /// rather than dropped, so the count of what a player wrote and the count of
    /// what the matcher holds agree and a mistyped line can be reported.
    #[must_use]
    pub fn parse(setting: &str) -> Self {
        Self {
            expressions: setting
                .split(',')
                .map(str::trim)
                .filter(|piece| !piece.is_empty())
                .map(Expression::parse)
                .collect(),
        }
    }

    /// Builds a matcher over expressions already read.
    #[must_use]
    pub const fn new(expressions: Vec<Expression>) -> Self {
        Self { expressions }
    }

    /// Returns what this matcher holds, in the order it was written.
    #[must_use]
    pub fn expressions(&self) -> &[Expression] {
        &self.expressions
    }

    /// Returns whether this matcher holds nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.expressions.is_empty()
    }

    /// Returns how many written entries this matcher could not read.
    #[must_use]
    pub fn unreadable_count(&self) -> usize {
        self.expressions
            .iter()
            .filter(|expression| **expression == Expression::Nothing)
            .count()
    }

    /// Answers whether any expression matches the entry.
    #[must_use]
    pub fn matches(&self, entry: &str, member_of: &dyn Fn(&str) -> bool) -> bool {
        self.expressions
            .iter()
            .any(|expression| expression.matches(entry, member_of))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn nothing_is_tagged(_tag: &str) -> bool {
        false
    }

    #[test]
    fn three_fields_with_the_prefix_is_a_tag_and_three_without_it_is_nothing() {
        assert_eq!(
            Expression::parse("tag:example:tools"),
            Expression::Tag("example:tools".to_owned())
        );
        assert_eq!(Expression::parse("one:two:three"), Expression::Nothing);
    }

    #[test]
    fn two_fields_are_a_namespace_wildcard_or_a_whole_identifier() {
        assert_eq!(
            Expression::parse("example:*"),
            Expression::Namespace("example".to_owned())
        );
        assert_eq!(
            Expression::parse("example:pick"),
            Expression::Entry("example:pick".to_owned())
        );
    }

    #[test]
    fn any_other_field_count_matches_nothing() {
        for written in ["", "pick", "a:b:c:d", ":pick", "example:", "   "] {
            assert_eq!(
                Expression::parse(written),
                Expression::Nothing,
                "{written} is not a shape this language has"
            );
        }
    }

    #[test]
    fn a_tag_expression_asks_the_caller_at_every_call_rather_than_remembering() {
        let expression = Expression::parse("tag:example:tools");
        let asked = Cell::new(0_u32);
        let member = |tag: &str| {
            assert_eq!(tag, "example:tools");
            asked.set(asked.get() + 1);
            asked.get() % 2 == 1
        };
        assert!(expression.matches("example:pick", &member));
        assert!(!expression.matches("example:pick", &member));
        assert!(expression.matches("example:pick", &member));
        assert_eq!(asked.get(), 3);
    }

    #[test]
    fn a_namespace_wildcard_matches_only_that_namespace() {
        let expression = Expression::parse("example:*");
        assert!(expression.matches("example:pick", &nothing_is_tagged));
        assert!(!expression.matches("other:pick", &nothing_is_tagged));
        assert!(!expression.matches("nocolon", &nothing_is_tagged));
    }

    #[test]
    fn an_entry_expression_compares_the_whole_identifier() {
        let expression = Expression::parse("example:pick");
        assert!(expression.matches("example:pick", &nothing_is_tagged));
        assert!(!expression.matches("example:pickaxe", &nothing_is_tagged));
    }

    #[test]
    fn a_list_matches_when_any_entry_matches_and_an_empty_list_matches_nothing() {
        let matcher = Matcher::parse("example:pick, other:*");
        assert!(matcher.matches("example:pick", &nothing_is_tagged));
        assert!(matcher.matches("other:anything", &nothing_is_tagged));
        assert!(!matcher.matches("third:thing", &nothing_is_tagged));
        assert!(!Matcher::parse("").matches("example:pick", &nothing_is_tagged));
        assert!(Matcher::parse("").is_empty());
    }

    #[test]
    fn a_written_entry_the_language_cannot_read_is_kept_and_counted() {
        let matcher = Matcher::parse("example:pick, a:b:c:d, one:two:three");
        assert_eq!(matcher.expressions().len(), 3);
        assert_eq!(matcher.unreadable_count(), 2);
    }

    #[test]
    fn an_expression_reads_back_as_it_was_written() {
        for written in ["tag:example:tools", "example:*", "example:pick"] {
            assert_eq!(Expression::parse(written).to_string(), written);
        }
    }
}
