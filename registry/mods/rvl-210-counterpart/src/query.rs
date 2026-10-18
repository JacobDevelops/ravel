//! Section eleven: the structured-data query language.
//!
//! A query is parsed left to right. At each position the parser tries a fixed,
//! ordered list of twenty-two expression handlers and takes the first that
//! matches a prefix there; a position no handler matches is a parse error.
//! Because the list is ordered and first-match wins, the order is part of the
//! grammar and reordering it is a different language.
//!
//! Evaluating yields a set of matches rather than one value, because the
//! wildcard, slice and union forms are many-valued. Navigation over the parsed
//! form is separately expressible, which is what lets a caller ask whether a
//! path could match without evaluating the query against data.
//!
//! The private selection module owns grammar state and parse-entry validation;
//! this module keeps the public syntax tree, navigation and evaluation.

use ravel_sdk::JsonValue;

use core::fmt;

mod selection;

/// The twenty-two handlers, in the order they are tried.
///
/// The order is grammar. `.length` is read as the length form because the length
/// handler is tried before the named-child one, and `!=` is read as not-equal
/// because that handler is tried before logical not.
pub const HANDLERS: [&str; 22] = [
    "root",
    "length",
    "named child",
    "named child in bracket form",
    "parent",
    "all children",
    "current node",
    "list element by position",
    "list slice",
    "union of expressions",
    "parenthesised grouping",
    "less-than",
    "less-than-or-equal",
    "greater-than",
    "greater-than-or-equal",
    "not-equal",
    "equal",
    "string equality",
    "logical and",
    "logical or",
    "logical not",
    "filter expression",
];

/// The longest query this parser reads.
pub const MAX_LENGTH: usize = 4096;

/// The most steps one query may hold, counting those inside a union or a filter.
pub const MAX_STEPS: usize = 256;

/// How deeply a grouping, union or filter may nest.
pub const MAX_DEPTH: usize = 16;

/// The most matches one evaluation may carry between two steps.
///
/// The wildcard, slice and union forms are many-valued, so a legal query of a
/// few dozen characters can name more matches than a document has room for.
/// The parser's bounds do not bind this, because the match count is a property
/// of the document rather than of the query.
pub const MAX_MATCHES: usize = 4096;

/// One segment of a path into a document.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Segment {
    /// A named child.
    Key(String),
    /// A list position.
    Index(usize),
}

/// One parsed step of a query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
    /// The document root.
    Root,
    /// The number of children of the current node.
    Length,
    /// A named child, written with a dot.
    Child(String),
    /// A named child, written in bracket form.
    BracketChild(String),
    /// The parent of the current node.
    Parent,
    /// Every child of the current node.
    AllChildren,
    /// The current node.
    Current,
    /// One list element, counted from the end when negative.
    Element(i64),
    /// A run of list elements.
    Slice(Option<i64>, Option<i64>),
    /// The union of several expressions.
    Union(Vec<Query>),
    /// A parenthesised grouping.
    Group(Box<Query>),
    /// A filter, which keeps the children its expression admits.
    Filter(Box<Expression>),
}

impl Step {
    /// Whether this step's answer can be worked out from a path alone.
    #[must_use]
    pub const fn navigates_only(&self) -> bool {
        matches!(
            self,
            Self::Root
                | Self::Child(_)
                | Self::BracketChild(_)
                | Self::Parent
                | Self::AllChildren
                | Self::Current
                | Self::Element(_)
        )
    }
}

/// A comparison or a logical combination of them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expression {
    /// The left query's value is below the literal.
    Less(Query, Literal),
    /// Below or equal.
    LessOrEqual(Query, Literal),
    /// Above.
    Greater(Query, Literal),
    /// Above or equal.
    GreaterOrEqual(Query, Literal),
    /// Not equal.
    NotEqual(Query, Literal),
    /// Equal, comparing numbers as numbers.
    Equal(Query, Literal),
    /// Equal, comparing the two as text.
    StringEqual(Query, Literal),
    /// Both.
    And(Box<Self>, Box<Self>),
    /// Either.
    Or(Box<Self>, Box<Self>),
    /// The negation.
    Not(Box<Self>),
}

/// A literal on the right of a comparison.
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
    /// A truth value.
    Truth(bool),
}

impl Eq for Literal {}

/// A parsed query.
///
/// This is the type a dependent catches [`ParseError`] around, and the type
/// navigation is expressed over.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Query {
    steps: Vec<Step>,
}

impl Query {
    /// Parses one query.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when a position matches no handler, when the query
    /// is longer than [`MAX_LENGTH`], when it holds more than [`MAX_STEPS`]
    /// steps, or when it nests deeper than [`MAX_DEPTH`].
    pub fn parse(written: &str) -> Result<Self, ParseError> {
        selection::parse(written)
    }

    /// The steps, in the order they were parsed.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Whether every step's answer follows from a path alone.
    #[must_use]
    pub fn navigates_only(&self) -> bool {
        self.steps.iter().all(Step::navigates_only)
    }

    /// Whether this query could match a path, without reading any document.
    ///
    /// Answers nothing when a step's answer depends on data, which is the honest
    /// answer for a length, a slice, a union or a filter: those cannot be
    /// decided from a path.
    ///
    /// A wildcard stands for one segment of any name, so the walk continues past
    /// it and the steps after it are answered about too.
    #[must_use]
    pub fn could_match(&self, path: &[Segment]) -> Option<bool> {
        if !self.navigates_only() {
            return None;
        }
        let mut walked: Vec<Walked> = Vec::new();
        for step in &self.steps {
            match step {
                Step::Root => walked.clear(),
                Step::Current => {}
                Step::Parent => {
                    walked.pop();
                }
                Step::Child(name) | Step::BracketChild(name) => {
                    walked.push(Walked::Exact(Segment::Key(name.clone())));
                }
                Step::Element(index) => {
                    let Ok(index) = usize::try_from(*index) else {
                        return Some(false);
                    };
                    walked.push(Walked::Exact(Segment::Index(index)));
                }
                Step::AllChildren => walked.push(Walked::Any),
                Step::Length
                | Step::Slice(_, _)
                | Step::Union(_)
                | Step::Group(_)
                | Step::Filter(_) => {
                    return None;
                }
            }
        }
        Some(
            walked.len() == path.len()
                && walked
                    .iter()
                    .zip(path)
                    .all(|(walked, segment)| walked.covers(segment)),
        )
    }

    /// Evaluates this query against a document.
    ///
    /// The result is a set of matches, each carrying the path it was found at.
    /// A value a step produced rather than found keeps the path of the node it
    /// was produced from.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluateError::TooManyMatches`] when a step names more matches
    /// than [`MAX_MATCHES`].
    pub fn evaluate(&self, document: &JsonValue) -> Result<Vec<Match>, EvaluateError> {
        let mut matches = vec![Match {
            path: Vec::new(),
            value: document.clone(),
        }];
        for step in &self.steps {
            matches = apply(step, document, &matches)?;
        }
        Ok(matches)
    }
}

/// One segment of a walked path: either a named one or a wildcard's placeholder.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Walked {
    Exact(Segment),
    Any,
}

impl Walked {
    fn covers(&self, segment: &Segment) -> bool {
        match self {
            Self::Exact(held) => held == segment,
            Self::Any => true,
        }
    }
}

/// One result of an evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Match {
    /// Where in the document it was found.
    pub path: Vec<Segment>,
    /// What is there.
    pub value: JsonValue,
}

/// What the parser refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseError {
    /// No handler matched a prefix at this position.
    NoHandler {
        /// Where in the written query.
        at: usize,
        /// The first characters there.
        rest: String,
    },
    /// A form was opened and not closed.
    Unclosed {
        /// What was opened.
        opened: &'static str,
        /// Where.
        at: usize,
    },
    /// The written query is longer than this parser reads.
    TooLong {
        /// The published limit.
        limit: usize,
        /// How long it was.
        length: usize,
    },
    /// The query holds more steps than one may.
    TooManySteps {
        /// The published limit.
        limit: usize,
    },
    /// The query nests deeper than one may.
    TooDeep {
        /// The published limit.
        limit: usize,
        /// Where the nesting passed it, which is where the parser stopped
        /// descending rather than where the deepest form ends.
        at: usize,
    },
}

/// What an evaluation refuses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluateError {
    /// A step named more matches than one evaluation may carry.
    TooManyMatches {
        /// The published limit.
        limit: usize,
    },
}

impl fmt::Display for EvaluateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyMatches { limit } => {
                write!(formatter, "an evaluation carries at most {limit} matches")
            }
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHandler { at, rest } => {
                write!(formatter, "no handler matches at {at}: {rest}")
            }
            Self::Unclosed { opened, at } => {
                write!(formatter, "a {opened} opened at {at} was never closed")
            }
            Self::TooLong { limit, length } => {
                write!(
                    formatter,
                    "a query of {length} is past the limit of {limit}"
                )
            }
            Self::TooManySteps { limit } => {
                write!(formatter, "a query holds at most {limit} steps")
            }
            Self::TooDeep { limit, at } => {
                write!(
                    formatter,
                    "a query nests at most {limit} deep, and does at {at}"
                )
            }
        }
    }
}

fn at_path<'document>(
    document: &'document JsonValue,
    path: &[Segment],
) -> Option<&'document JsonValue> {
    let mut node = document;
    for segment in path {
        node = match segment {
            Segment::Key(key) => node.get(key)?,
            Segment::Index(index) => node.get(*index)?,
        };
    }
    Some(node)
}

fn resolve(index: i64, length: usize) -> Option<usize> {
    let length = i64::try_from(length).ok()?;
    let resolved = if index < 0 { length + index } else { index };
    if resolved < 0 || resolved >= length {
        return None;
    }
    usize::try_from(resolved).ok()
}

/// Applies one step to the carried matches.
///
/// A re-rooting step is the whole document again and answers one match for the
/// step, not one per match carried into it: re-rooting is absolute, so it does
/// not depend on what the steps before it found. Handling it per carried match
/// multiplies the set instead, and a query alternating re-roots with wildcards
/// then grows as a power of the document's width.
fn apply(
    step: &Step,
    document: &JsonValue,
    matches: &[Match],
) -> Result<Vec<Match>, EvaluateError> {
    if matches!(step, Step::Root) {
        return Ok(vec![Match {
            path: Vec::new(),
            value: document.clone(),
        }]);
    }
    let mut next = Vec::new();
    for found in matches {
        match step {
            // Answered for the whole step above, so this arm never runs; a
            // panic here would be a worse answer than doing nothing.
            Step::Root => {}
            Step::Current => push(&mut next, found.clone())?,
            Step::Parent => apply_parent(found, document, &mut next)?,
            Step::Length => apply_length(found, &mut next)?,
            Step::Child(name) | Step::BracketChild(name) => {
                if let Some(value) = found.value.get(name) {
                    let mut path = found.path.clone();
                    path.push(Segment::Key(name.clone()));
                    push(
                        &mut next,
                        Match {
                            path,
                            value: value.clone(),
                        },
                    )?;
                }
            }
            Step::AllChildren => append_children(found, &mut next)?,
            Step::Element(index) => apply_element(found, *index, &mut next)?,
            Step::Slice(from, to) => append_slice(found, *from, *to, &mut next)?,
            Step::Union(branches) => {
                for branch in branches {
                    for found in apply_all(branch, document, std::slice::from_ref(found))? {
                        if !next.contains(&found) {
                            push(&mut next, found)?;
                        }
                    }
                }
            }
            Step::Group(inner) => {
                for found in apply_all(inner, document, std::slice::from_ref(found))? {
                    push(&mut next, found)?;
                }
            }
            Step::Filter(expression) => {
                for child in apply(&Step::AllChildren, document, std::slice::from_ref(found))? {
                    if admits(expression, document, &child)? {
                        push(&mut next, child)?;
                    }
                }
            }
        }
    }
    Ok(next)
}

fn apply_parent(
    found: &Match,
    document: &JsonValue,
    next: &mut Vec<Match>,
) -> Result<(), EvaluateError> {
    let mut path = found.path.clone();
    path.pop();
    if let Some(value) = at_path(document, &path) {
        push(
            next,
            Match {
                path,
                value: value.clone(),
            },
        )?;
    }
    Ok(())
}

fn apply_length(found: &Match, next: &mut Vec<Match>) -> Result<(), EvaluateError> {
    let length = match &found.value {
        JsonValue::Array(entries) => entries.len(),
        JsonValue::Object(entries) => entries.len(),
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => 0,
    };
    push(
        next,
        Match {
            path: found.path.clone(),
            value: JsonValue::from(length),
        },
    )
}

fn apply_element(found: &Match, index: i64, next: &mut Vec<Match>) -> Result<(), EvaluateError> {
    if let JsonValue::Array(entries) = &found.value
        && let Some(resolved) = resolve(index, entries.len())
        && let Some(value) = entries.get(resolved)
    {
        let mut path = found.path.clone();
        path.push(Segment::Index(resolved));
        push(
            next,
            Match {
                path,
                value: value.clone(),
            },
        )?;
    }
    Ok(())
}

fn append_children(found: &Match, next: &mut Vec<Match>) -> Result<(), EvaluateError> {
    match &found.value {
        JsonValue::Array(entries) => {
            for (index, value) in entries.iter().enumerate() {
                let mut path = found.path.clone();
                path.push(Segment::Index(index));
                push(
                    next,
                    Match {
                        path,
                        value: value.clone(),
                    },
                )?;
            }
        }
        JsonValue::Object(entries) => {
            for (key, value) in entries {
                let mut path = found.path.clone();
                path.push(Segment::Key(key.clone()));
                push(
                    next,
                    Match {
                        path,
                        value: value.clone(),
                    },
                )?;
            }
        }
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => {}
    }
    Ok(())
}

fn append_slice(
    found: &Match,
    from: Option<i64>,
    to: Option<i64>,
    next: &mut Vec<Match>,
) -> Result<(), EvaluateError> {
    if let JsonValue::Array(entries) = &found.value {
        let length = entries.len();
        let start = from.map_or(0, |bound| clamp(bound, length));
        let end = to.map_or(length, |bound| clamp(bound, length));
        for index in start..end.min(length) {
            if let Some(value) = entries.get(index) {
                let mut path = found.path.clone();
                path.push(Segment::Index(index));
                push(
                    next,
                    Match {
                        path,
                        value: value.clone(),
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn push(next: &mut Vec<Match>, found: Match) -> Result<(), EvaluateError> {
    if next.len() >= MAX_MATCHES {
        return Err(EvaluateError::TooManyMatches { limit: MAX_MATCHES });
    }
    next.push(found);
    Ok(())
}

fn clamp(bound: i64, length: usize) -> usize {
    let length = i64::try_from(length).unwrap_or(i64::MAX);
    let resolved = if bound < 0 { length + bound } else { bound };
    usize::try_from(resolved.clamp(0, length)).unwrap_or(0)
}

fn apply_all(
    query: &Query,
    document: &JsonValue,
    matches: &[Match],
) -> Result<Vec<Match>, EvaluateError> {
    let mut carried = matches.to_vec();
    for step in query.steps() {
        carried = apply(step, document, &carried)?;
    }
    Ok(carried)
}

fn admits(
    expression: &Expression,
    document: &JsonValue,
    against: &Match,
) -> Result<bool, EvaluateError> {
    let compare = |query: &Query,
                   literal: &Literal,
                   decide: &dyn Fn(&JsonValue, &Literal) -> bool|
     -> Result<bool, EvaluateError> {
        Ok(apply_all(query, document, std::slice::from_ref(against))?
            .iter()
            .any(|found| decide(&found.value, literal)))
    };
    match expression {
        Expression::Less(query, literal) => compare(query, literal, &|value, literal| {
            order(value, literal).is_some_and(|order| order == core::cmp::Ordering::Less)
        }),
        Expression::LessOrEqual(query, literal) => compare(query, literal, &|value, literal| {
            order(value, literal).is_some_and(|order| order != core::cmp::Ordering::Greater)
        }),
        Expression::Greater(query, literal) => compare(query, literal, &|value, literal| {
            order(value, literal).is_some_and(|order| order == core::cmp::Ordering::Greater)
        }),
        Expression::GreaterOrEqual(query, literal) => compare(query, literal, &|value, literal| {
            order(value, literal).is_some_and(|order| order != core::cmp::Ordering::Less)
        }),
        Expression::Equal(query, literal) => compare(query, literal, &alike),
        Expression::NotEqual(query, literal) => Ok(!compare(query, literal, &alike)?),
        Expression::StringEqual(query, literal) => compare(query, literal, &|value, literal| {
            let written = match value {
                JsonValue::String(text) => text.clone(),
                JsonValue::Null
                | JsonValue::Bool(_)
                | JsonValue::Number(_)
                | JsonValue::Array(_)
                | JsonValue::Object(_) => value.to_string(),
            };
            match literal {
                Literal::Text(text) => &written == text,
                Literal::Number(number) => written == number.to_string(),
                Literal::Truth(truth) => written == truth.to_string(),
            }
        }),
        Expression::And(left, right) => {
            Ok(admits(left, document, against)? && admits(right, document, against)?)
        }
        Expression::Or(left, right) => {
            Ok(admits(left, document, against)? || admits(right, document, against)?)
        }
        Expression::Not(inner) => Ok(!admits(inner, document, against)?),
    }
}

fn alike(value: &JsonValue, literal: &Literal) -> bool {
    match literal {
        Literal::Number(number) => value
            .as_f64()
            .is_some_and(|held| (held - number).abs() < f64::EPSILON),
        Literal::Text(text) => value.as_str() == Some(text.as_str()),
        Literal::Truth(truth) => value.as_bool() == Some(*truth),
    }
}

fn order(value: &JsonValue, literal: &Literal) -> Option<core::cmp::Ordering> {
    match literal {
        Literal::Number(number) => value.as_f64()?.partial_cmp(number),
        Literal::Text(text) => value.as_str()?.partial_cmp(text.as_str()),
        Literal::Truth(truth) => value.as_bool()?.partial_cmp(truth),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::json;

    fn document() -> JsonValue {
        json!({
            "length": "a child that is really called this",
            "entries": [1, 2, 3, 4],
            "held": { "count": 7, "name": "one" },
            "rows": [{ "count": 1 }, { "count": 5 }, { "count": 9 }]
        })
    }

    #[test]
    fn the_published_handler_list_is_twenty_two_long_and_in_the_stated_order() {
        assert_eq!(HANDLERS.len(), 22);
        assert_eq!(HANDLERS[0], "root");
        assert_eq!(HANDLERS[1], "length");
        assert_eq!(HANDLERS[2], "named child");
        assert_eq!(HANDLERS[15], "not-equal");
        assert_eq!(HANDLERS[20], "logical not");
        assert_eq!(HANDLERS[21], "filter expression");
    }

    #[test]
    fn the_length_handler_wins_over_the_named_child_because_it_is_tried_first() {
        let query = Query::parse("$.length").expect("a legal query");
        assert_eq!(query.steps(), &[Step::Root, Step::Length]);
        let found = query.evaluate(&document()).expect("within the match limit");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].value, json!(4), "the root has four children");
    }

    #[test]
    fn a_child_that_is_really_called_length_is_only_reachable_in_bracket_form() {
        let query = Query::parse("$['length']").expect("a legal query");
        assert_eq!(
            query.steps(),
            &[Step::Root, Step::BracketChild("length".to_owned())]
        );
        let found = query.evaluate(&document()).expect("within the match limit");
        assert_eq!(found[0].value, document()["length"]);
    }

    #[test]
    fn the_not_equal_handler_wins_over_logical_not_because_it_is_tried_first() {
        let query = Query::parse("$.rows?(@.count != 5)").expect("a legal query");
        let found = query.evaluate(&document()).expect("within the match limit");
        let counts: Vec<JsonValue> = found
            .iter()
            .map(|found| found.value["count"].clone())
            .collect();
        assert_eq!(counts, vec![json!(1), json!(9)]);
    }

    #[test]
    fn logical_not_is_still_reachable_on_its_own() {
        let query = Query::parse("$.rows?(!@.count > 4)").expect("a legal query");
        let found = query.evaluate(&document()).expect("within the match limit");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].value["count"], json!(1));
    }

    #[test]
    fn every_navigation_form_finds_what_it_names() {
        let document = document();
        let cases: [(&str, usize); 7] = [
            ("$", 1),
            ("$.entries", 1),
            ("$.entries.*", 4),
            ("$.entries[2]", 1),
            ("$.entries[1:3]", 2),
            ("$.held.count^", 1),
            ("$.held@", 1),
        ];
        for (written, expected) in cases {
            let query = Query::parse(written).expect("a legal query");
            assert_eq!(
                query
                    .evaluate(&document)
                    .expect("within the match limit")
                    .len(),
                expected,
                "{written} matched the wrong number of nodes"
            );
        }
    }

    #[test]
    fn a_negative_element_counts_from_the_end_and_one_past_either_end_matches_nothing() {
        let document = document();
        assert_eq!(
            Query::parse("$.entries[-1]")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")[0]
                .value,
            json!(4)
        );
        assert_eq!(
            Query::parse("$.entries[4]")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")
                .len(),
            0
        );
        assert_eq!(
            Query::parse("$.entries[-9]")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")
                .len(),
            0
        );
    }

    #[test]
    fn a_slice_past_the_end_is_bounded_rather_than_refused() {
        let document = document();
        let found = Query::parse("$.entries[2:99]")
            .expect("legal")
            .evaluate(&document)
            .expect("within the match limit");
        assert_eq!(found.len(), 2);
        let found = Query::parse("$.entries[-99:2]")
            .expect("legal")
            .evaluate(&document)
            .expect("within the match limit");
        assert_eq!(found.len(), 2);
        let found = Query::parse("$.entries[3:1]")
            .expect("legal")
            .evaluate(&document)
            .expect("within the match limit");
        assert_eq!(found.len(), 0, "a backwards slice is empty, not a panic");
    }

    #[test]
    fn a_union_is_many_valued_and_reports_each_node_once() {
        let query = Query::parse("$[.entries|.entries]").expect("a legal query");
        let found = query.evaluate(&document()).expect("within the match limit");
        assert_eq!(found.len(), 1, "the same node twice is one match");
        let query = Query::parse("$[.entries|.held]").expect("a legal query");
        assert_eq!(
            query
                .evaluate(&document())
                .expect("within the match limit")
                .len(),
            2
        );
    }

    #[test]
    fn a_grouping_evaluates_as_the_steps_it_holds() {
        let plain = Query::parse("$.held.count")
            .expect("legal")
            .evaluate(&document())
            .expect("within the match limit");
        let grouped = Query::parse("$(.held.count)")
            .expect("legal")
            .evaluate(&document())
            .expect("within the match limit");
        assert_eq!(plain, grouped);
    }

    #[test]
    fn every_comparison_form_parses_and_decides() {
        let document = document();
        let cases: [(&str, usize); 7] = [
            ("$.rows?(@.count < 5)", 1),
            ("$.rows?(@.count <= 5)", 2),
            ("$.rows?(@.count > 5)", 1),
            ("$.rows?(@.count >= 5)", 2),
            ("$.rows?(@.count != 5)", 2),
            ("$.rows?(@.count == 5)", 1),
            ("$.rows?(@.count eq '5')", 1),
        ];
        for (written, expected) in cases {
            let query = Query::parse(written).expect("a legal query");
            assert_eq!(
                query
                    .evaluate(&document)
                    .expect("within the match limit")
                    .len(),
                expected,
                "{written} admitted the wrong number of rows"
            );
        }
    }

    #[test]
    fn the_three_logical_forms_are_new_and_all_three_work() {
        let document = document();
        assert_eq!(
            Query::parse("$.rows?(@.count > 1 && @.count < 9)")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")
                .len(),
            1
        );
        assert_eq!(
            Query::parse("$.rows?(@.count == 1 || @.count == 9)")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")
                .len(),
            2
        );
        assert_eq!(
            Query::parse("$.rows?(!(@.count == 5))")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")
                .len(),
            2
        );
    }

    #[test]
    fn a_position_no_handler_matches_is_a_parse_error_naming_where() {
        assert!(matches!(
            Query::parse("$%bad"),
            Err(ParseError::NoHandler { at: 1, .. })
        ));
        assert!(matches!(
            Query::parse("$.entries[1"),
            Err(ParseError::Unclosed { .. })
        ));
        assert!(matches!(
            Query::parse("$(.held"),
            Err(ParseError::Unclosed {
                opened: "grouping",
                ..
            })
        ));
        assert!(matches!(
            Query::parse("$.rows?(@.count"),
            Err(ParseError::NoHandler { .. })
        ));
    }

    #[test]
    fn a_query_past_any_published_bound_is_refused_rather_than_read() {
        let long = "$".to_owned() + &".a".repeat(MAX_LENGTH);
        assert!(matches!(
            Query::parse(&long),
            Err(ParseError::TooLong { .. })
        ));
        let many = "$".to_owned() + &".a".repeat(MAX_STEPS + 1);
        assert!(matches!(
            Query::parse(&many),
            Err(ParseError::TooManySteps { limit: MAX_STEPS })
        ));
        let deep = "$".to_owned() + &"(".repeat(MAX_DEPTH + 2) + &")".repeat(MAX_DEPTH + 2);
        assert!(matches!(
            Query::parse(&deep),
            Err(ParseError::TooDeep { .. })
        ));
    }

    #[test]
    fn navigation_over_the_parsed_form_answers_without_a_document() {
        let query = Query::parse("$.held.count").expect("legal");
        assert!(query.navigates_only());
        assert_eq!(
            query.could_match(&[
                Segment::Key("held".to_owned()),
                Segment::Key("count".to_owned())
            ]),
            Some(true)
        );
        assert_eq!(
            query.could_match(&[Segment::Key("held".to_owned())]),
            Some(false)
        );
    }

    #[test]
    fn a_query_whose_answer_depends_on_data_says_so_rather_than_guessing() {
        for written in [
            "$.length",
            "$.entries[0:2]",
            "$[.a|.b]",
            "$.rows?(@.count > 1)",
        ] {
            let query = Query::parse(written).expect("a legal query");
            assert!(!query.navigates_only(), "{written}");
            assert_eq!(query.could_match(&[]), None, "{written}");
        }
    }

    #[test]
    fn a_parent_step_off_the_root_stays_at_the_root() {
        let found = Query::parse("$^")
            .expect("legal")
            .evaluate(&document())
            .expect("within the match limit");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, Vec::new());
    }

    #[test]
    fn a_navigation_into_a_node_that_is_not_there_matches_nothing() {
        let document = document();
        assert_eq!(
            Query::parse("$.absent.count")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")
                .len(),
            0
        );
        assert_eq!(
            Query::parse("$.held[0]")
                .expect("legal")
                .evaluate(&document)
                .expect("within the match limit")
                .len(),
            0,
            "an object is not indexable by position"
        );
    }

    fn eight_keys() -> JsonValue {
        let mut document = json!({});
        if let Some(object) = document.as_object_mut() {
            for index in 0..8_u64 {
                object.insert(format!("k{index}"), JsonValue::from(index));
            }
        }
        document
    }

    #[test]
    fn a_nesting_of_negations_stops_descending_at_the_published_depth() {
        // The position is the proof rather than the refusal: a negation nesting
        // is refused by `query` at the bottom either way, so what says the depth
        // bound holds is that the parser stopped at the seventeenth negation
        // rather than after putting all two hundred on the stack.
        let written = format!("$.rows?({}@.count == 1)", "!".repeat(200));
        assert!(
            written.len() < MAX_LENGTH,
            "the length bound is not what refuses this"
        );
        assert_eq!(
            Query::parse(&written),
            Err(ParseError::TooDeep {
                limit: MAX_DEPTH,
                at: 24,
            }),
            "eight characters of filter and sixteen negations, not two hundred"
        );
    }

    #[test]
    fn a_nesting_of_conjunctions_stops_descending_at_the_published_depth_too() {
        let written = format!("$.rows?({}@.count == 1)", "@.count == 1 && ".repeat(200));
        assert!(
            written.len() < MAX_LENGTH,
            "the length bound is not what refuses this"
        );
        assert_eq!(
            Query::parse(&written),
            Err(ParseError::TooDeep {
                limit: MAX_DEPTH,
                at: 248,
            }),
            "fifteen conjuncts in, not two hundred"
        );
    }

    #[test]
    fn a_re_rooting_step_answers_one_match_for_the_whole_step_rather_than_one_per_match() {
        let document = eight_keys();
        let count = |written: &str| {
            Query::parse(written)
                .expect("a legal query")
                .evaluate(&document)
                .expect("within the match limit")
                .len()
        };
        assert_eq!(count("$.*"), 8);
        assert_eq!(count("$.*$"), 1, "re-rooting collapses the carried set");
        assert_eq!(count("$.*$.*"), 8);
        assert_eq!(count("$.*$.*$.*$.*"), 8, "a re-root multiplies nothing");
    }

    #[test]
    fn an_evaluation_naming_more_matches_than_the_published_limit_is_refused() {
        let mut document = json!({});
        if let Some(object) = document.as_object_mut() {
            for index in 0..100_u64 {
                let row: Vec<JsonValue> = (0..100_u64).map(JsonValue::from).collect();
                object.insert(format!("k{index}"), JsonValue::from(row));
            }
        }
        let query = Query::parse("$.*.*").expect("a legal query");
        assert_eq!(
            query.evaluate(&document),
            Err(EvaluateError::TooManyMatches { limit: MAX_MATCHES }),
            "ten thousand matches are past the published bound"
        );
        assert!(
            Query::parse("$.*")
                .expect("a legal query")
                .evaluate(&document)
                .is_ok_and(|found| found.len() == 100)
        );
    }

    #[test]
    fn a_wildcard_is_walked_through_so_the_steps_after_it_are_answered_about_too() {
        let query = Query::parse("$.*.a").expect("a legal query");
        assert_eq!(
            query.could_match(&[Segment::Key("b".to_owned()), Segment::Key("a".to_owned()),]),
            Some(true),
            "any first segment, then a"
        );
        assert_eq!(
            query.could_match(&[Segment::Key("b".to_owned())]),
            Some(false),
            "the query names two segments, not one"
        );
        assert_eq!(
            query.could_match(&[Segment::Key("b".to_owned()), Segment::Key("c".to_owned()),]),
            Some(false)
        );
        assert_eq!(
            Query::parse("$.*")
                .expect("a legal query")
                .could_match(&[Segment::Key("held".to_owned())]),
            Some(true),
            "a trailing wildcard still matches one segment of any name"
        );
    }
}
