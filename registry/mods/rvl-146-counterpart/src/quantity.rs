//! The ingredient with an amount of §7: a matcher and a count, over items or
//! over fluids.
//!
//! It exists because the game's own ingredient carries no quantity. The rule
//! worth keeping is the arithmetic: consuming moves the smaller of what is
//! available and what is outstanding, and nothing at all when the outstanding
//! count is zero or the matcher rejects the candidate.
//!
//! # The changes declared here
//!
//! - **The value is immutable and the results are returned.** The original
//!   reduces the candidate stack in place *and* reduces its own outstanding
//!   count, so it is a mutable accumulator wearing a value's shape: one consume
//!   through a shared instance corrupts it for every other holder, and the copy
//!   operation exists precisely so callers can avoid that. [`Portion::consume`]
//!   takes both by reference and answers with the amount moved and the two new
//!   values, so there is nothing to copy defensively and nothing to share badly.
//! - **The two flavours are a sum type.** The original passes the candidate
//!   untyped, casts it, and offers a runtime predicate so a caller can ask which
//!   flavour it holds. [`Flavour`] removes the cast, the predicate and the
//!   failure mode together: a fluid candidate offered to an item matcher is
//!   simply not a match.
//! - **One bound applies in both directions.** The original's declared form
//!   refuses a count of zero or less while its wire form accepts any thirty-two
//!   bit value, including negatives and values far past any stack size. Here
//!   [`MAX_COUNT`] and the sign are refused at the declaration, at the writer and
//!   at the reader; what the declared form alone still refuses is zero, because a
//!   recipe that asks for none of something is a mistake at the point it is
//!   written.
//!
//! # The two field spellings are not invented here
//!
//! §14 makes the declared form's two field spellings compatibility-required,
//! because a pack author and a dependent write them into recipe files, and
//! withholds them. [`PortionSpellings::declared`] reads them from build-time
//! inputs and [`PortionSpellings::placeholder`] carries this package's own, which
//! agrees with no existing recipe file. [`Portion::read_declared`] reads a
//! document under them: the matcher under one spelling, and an optional count
//! under the other defaulting to one and constrained positive.
//!
//! # Declarable only
//!
//! **What a matcher is.** The game's ingredient is the host's, and Ravel
//! publishes neither it nor its stream form nor its document form. A [`Matcher`]
//! here is a bounded set of identifiers with a flavour, which is enough to state
//! and test every rule above; the declared form carries the matcher's own
//! document opaquely and the wire form carries the host's own bytes opaquely,
//! and what either means is the host's.
//!
//! The wire form is the shape §7 states and nothing more: the matcher's bytes,
//! then the count as four fixed signed bytes. Nothing frames the matcher, so a
//! buffer an original peer wrote parses here, and the price is that the matcher
//! form is whatever precedes the last four bytes rather than a length this
//! package could check on its own.

use std::fmt;

use ravel_sdk::content::ContentId;
use ravel_sdk::{JsonValue, json};

/// The largest count a declaration or a peer may state.
///
/// The wire form is four fixed signed bytes, so this is the largest value that
/// form can carry. The upgrade path is a wider field, which is a wire change.
pub const MAX_COUNT: u32 = i32::MAX.cast_unsigned();

/// The most identifiers one matcher will accept.
pub const MAX_ACCEPTED: usize = 1024;

/// The most bytes a host-supplied matcher form may carry.
pub const MAX_MATCHER_BYTES: usize = 8 * 1024;

/// Which of the two things a matcher is about.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Flavour {
    /// Item stacks.
    Item,
    /// Fluid stacks.
    Fluid,
}

impl Flavour {
    /// Returns the flavour's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Fluid => "fluid",
        }
    }
}

impl fmt::Display for Flavour {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One stack a consume is offered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stack {
    /// Which flavour this is.
    pub flavour: Flavour,
    /// What it is.
    pub id: ContentId,
    /// How much of it there is.
    pub amount: u32,
}

impl Stack {
    /// A stack of `amount` of `id`.
    #[must_use]
    pub const fn new(flavour: Flavour, id: ContentId, amount: u32) -> Self {
        Self {
            flavour,
            id,
            amount,
        }
    }

    /// Returns whether this stack holds nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.amount == 0
    }
}

/// Why a matcher or a portion was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuantityRefused {
    /// A matcher accepting nothing, which could only ever reject.
    AcceptsNothing,
    /// A matcher accepting more than [`MAX_ACCEPTED`] identifiers.
    TooManyAccepted(usize),
    /// A declared count of zero.
    ZeroCount,
    /// A count past [`MAX_COUNT`], or a negative one off the wire.
    CountOutOfRange(i64),
    /// A host-supplied matcher form past [`MAX_MATCHER_BYTES`].
    MatcherTooLong(usize),
    /// The buffer ended inside a field.
    Truncated,
    /// A declared document that is not an object.
    NotAnObject,
    /// The matcher field is missing from a declared document.
    MissingMatcher {
        /// The spelling it was looked for under.
        key: String,
    },
    /// The count field is present and is not a whole number.
    CountNotWhole {
        /// The spelling it was read under.
        key: String,
    },
}

impl fmt::Display for QuantityRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AcceptsNothing => formatter.write_str("a matcher accepting nothing"),
            Self::TooManyAccepted(count) => {
                write!(formatter, "{count} accepted names is past {MAX_ACCEPTED}")
            }
            Self::ZeroCount => formatter.write_str("a declared count of none"),
            Self::CountOutOfRange(count) => {
                write!(formatter, "{count} is outside 0 to {MAX_COUNT}")
            }
            Self::MatcherTooLong(length) => {
                write!(formatter, "{length} bytes is past {MAX_MATCHER_BYTES}")
            }
            Self::Truncated => formatter.write_str("the buffer ended inside a field"),
            Self::NotAnObject => formatter.write_str("the declared form is not an object"),
            Self::MissingMatcher { key } => write!(formatter, "`{key}` is missing"),
            Self::CountNotWhole { key } => {
                write!(formatter, "`{key}` is present and is not a whole number")
            }
        }
    }
}

impl std::error::Error for QuantityRefused {}

/// Why the declared form's spellings could not be built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortionSpellingsUnavailable {
    /// The build supplied nothing.
    NotSupplied,
    /// A spelling is empty.
    Empty,
    /// Both spellings are one word, which would make the count and the matcher
    /// one field.
    NotDistinct,
}

impl fmt::Display for PortionSpellingsUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupplied => formatter.write_str("the build supplied no ingredient spellings"),
            Self::Empty => formatter.write_str("an ingredient spelling is empty"),
            Self::NotDistinct => formatter.write_str("both ingredient spellings are one word"),
        }
    }
}

impl std::error::Error for PortionSpellingsUnavailable {}

/// The two field names a recipe file writes a portion under.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortionSpellings {
    matcher: String,
    count: String,
}

impl Default for PortionSpellings {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl PortionSpellings {
    /// The spellings the tests pin the document's **shape** with.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            matcher: "accepts".to_owned(),
            count: "however_many".to_owned(),
        }
    }

    /// Reads the two spellings from build-time inputs.
    ///
    /// # Errors
    ///
    /// Returns [`PortionSpellingsUnavailable::NotSupplied`] when the build supplied
    /// nothing, and what [`PortionSpellings::parse`] returns otherwise. There is
    /// no default: a package that has to agree with a pack author's recipe file
    /// cannot fall back to a placeholder and still claim to agree.
    pub fn declared() -> Result<Self, PortionSpellingsUnavailable> {
        let matcher = option_env!("RVL_146_COUNTERPART_INGREDIENT_MATCHER_KEY")
            .ok_or(PortionSpellingsUnavailable::NotSupplied)?;
        let count = option_env!("RVL_146_COUNTERPART_INGREDIENT_COUNT_KEY")
            .ok_or(PortionSpellingsUnavailable::NotSupplied)?;
        Self::parse(matcher, count)
    }

    /// Reads two spellings.
    ///
    /// # Errors
    ///
    /// Returns [`PortionSpellingsUnavailable::Empty`] for an empty spelling and
    /// [`PortionSpellingsUnavailable::NotDistinct`] when both are one word.
    pub fn parse(matcher: &str, count: &str) -> Result<Self, PortionSpellingsUnavailable> {
        if matcher.is_empty() || count.is_empty() {
            return Err(PortionSpellingsUnavailable::Empty);
        }
        if matcher == count {
            return Err(PortionSpellingsUnavailable::NotDistinct);
        }
        Ok(Self {
            matcher: matcher.to_owned(),
            count: count.to_owned(),
        })
    }

    /// Returns the field the matcher sits under.
    #[must_use]
    pub fn matcher(&self) -> &str {
        &self.matcher
    }

    /// Returns the field the count sits under.
    #[must_use]
    pub fn count(&self) -> &str {
        &self.count
    }
}

/// What a portion accepts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Matcher {
    flavour: Flavour,
    accepted: Vec<ContentId>,
}

impl Matcher {
    /// A matcher over one flavour, accepting these identifiers.
    ///
    /// # Errors
    ///
    /// Returns [`QuantityRefused::AcceptsNothing`] for an empty set and
    /// [`QuantityRefused::TooManyAccepted`] past [`MAX_ACCEPTED`].
    pub fn accepting(
        flavour: Flavour,
        accepted: impl IntoIterator<Item = ContentId>,
    ) -> Result<Self, QuantityRefused> {
        let accepted: Vec<ContentId> = accepted.into_iter().collect();
        if accepted.is_empty() {
            return Err(QuantityRefused::AcceptsNothing);
        }
        if accepted.len() > MAX_ACCEPTED {
            return Err(QuantityRefused::TooManyAccepted(accepted.len()));
        }
        Ok(Self { flavour, accepted })
    }

    /// Returns which flavour this matcher is about.
    #[must_use]
    pub const fn flavour(&self) -> Flavour {
        self.flavour
    }

    /// Returns the identifiers this matcher accepts.
    #[must_use]
    pub fn accepted(&self) -> &[ContentId] {
        &self.accepted
    }

    /// Returns whether a stack matches.
    ///
    /// A stack of the other flavour does not match, which is where the
    /// original's cast used to be.
    #[must_use]
    pub fn matches(&self, stack: &Stack) -> bool {
        stack.flavour == self.flavour && self.accepted.contains(&stack.id)
    }
}

/// What one consume moved, and the two values it left behind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Consumed {
    /// How much moved.
    pub moved: u32,
    /// The portion with the moved amount taken off its outstanding count.
    pub outstanding: Portion,
    /// The candidate with the moved amount taken off it.
    pub candidate: Stack,
}

/// A matcher and an outstanding count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Portion {
    matcher: Matcher,
    count: u32,
}

impl Portion {
    /// A portion declared in a recipe: a matcher and a count.
    ///
    /// The count defaults to one where a declaration omits it, which is the
    /// caller's `unwrap_or(1)` rather than a second constructor.
    ///
    /// # Errors
    ///
    /// Returns [`QuantityRefused::ZeroCount`] for a declared count of none and
    /// [`QuantityRefused::CountOutOfRange`] past [`MAX_COUNT`].
    pub fn declared(matcher: Matcher, count: u32) -> Result<Self, QuantityRefused> {
        if count == 0 {
            return Err(QuantityRefused::ZeroCount);
        }
        Self::holding(matcher, count)
    }

    /// A portion at any legal outstanding count, including none.
    ///
    /// # Errors
    ///
    /// Returns [`QuantityRefused::CountOutOfRange`] past [`MAX_COUNT`].
    pub fn holding(matcher: Matcher, count: u32) -> Result<Self, QuantityRefused> {
        if count > MAX_COUNT {
            return Err(QuantityRefused::CountOutOfRange(i64::from(count)));
        }
        Ok(Self { matcher, count })
    }

    /// The empty portion for a matcher: nothing outstanding.
    #[must_use]
    pub const fn empty(matcher: Matcher) -> Self {
        Self { matcher, count: 0 }
    }

    /// Returns the matcher.
    #[must_use]
    pub const fn matcher(&self) -> &Matcher {
        &self.matcher
    }

    /// Returns the outstanding count.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Returns whether nothing is outstanding.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Consumes from a candidate.
    ///
    /// Nothing moves when nothing is outstanding, and nothing moves when the
    /// matcher rejects the candidate. Otherwise the smaller of the available
    /// amount and the outstanding count moves, and both values come back
    /// reduced by it.
    #[must_use]
    pub fn consume(&self, candidate: &Stack) -> Consumed {
        if self.count == 0 || !self.matcher.matches(candidate) {
            return Consumed {
                moved: 0,
                outstanding: self.clone(),
                candidate: candidate.clone(),
            };
        }
        let moved = self.count.min(candidate.amount);
        Consumed {
            moved,
            outstanding: Self {
                matcher: self.matcher.clone(),
                count: self.count - moved,
            },
            candidate: Stack {
                flavour: candidate.flavour,
                id: candidate.id.clone(),
                amount: candidate.amount - moved,
            },
        }
    }

    /// Writes the declared form: the matcher's own document under one spelling
    /// and the count under the other.
    ///
    /// The count is written even when it is one, because a recipe file this
    /// package wrote is read back by this package and by an original alike, and
    /// the default is a reader's rule rather than a writer's.
    #[must_use]
    pub fn write_declared(
        &self,
        matcher_form: &JsonValue,
        spellings: &PortionSpellings,
    ) -> JsonValue {
        let mut document = json!({});
        document[spellings.matcher()] = matcher_form.clone();
        document[spellings.count()] = json!(self.count);
        document
    }

    /// Reads the declared form of §7, answering the matcher's own document and
    /// the count.
    ///
    /// The count is optional and defaults to one, and it is constrained positive
    /// and bounded by [`MAX_COUNT`] — the same bound the wire form applies, which
    /// is the half the original does not have.
    ///
    /// # Errors
    ///
    /// Returns [`QuantityRefused::NotAnObject`] for a document that is not an object,
    /// [`QuantityRefused::MissingMatcher`] when the matcher field is absent,
    /// [`QuantityRefused::CountNotWhole`] for a count that is present and is not a whole
    /// number, [`QuantityRefused::ZeroCount`] for a declared count of none, and
    /// [`QuantityRefused::CountOutOfRange`] for a negative count or one past
    /// [`MAX_COUNT`].
    pub fn read_declared(
        document: &JsonValue,
        spellings: &PortionSpellings,
    ) -> Result<(JsonValue, u32), QuantityRefused> {
        if !document.is_object() {
            return Err(QuantityRefused::NotAnObject);
        }
        let matcher_form = document
            .get(spellings.matcher())
            .ok_or_else(|| QuantityRefused::MissingMatcher {
                key: spellings.matcher().to_owned(),
            })?
            .clone();
        let count = match document.get(spellings.count()) {
            None => 1,
            Some(value) => {
                let declared = value
                    .as_i64()
                    .ok_or_else(|| QuantityRefused::CountNotWhole {
                        key: spellings.count().to_owned(),
                    })?;
                if declared == 0 {
                    return Err(QuantityRefused::ZeroCount);
                }
                u32::try_from(declared)
                    .ok()
                    .filter(|count| *count <= MAX_COUNT)
                    .ok_or(QuantityRefused::CountOutOfRange(declared))?
            }
        };
        Ok((matcher_form, count))
    }

    /// Writes the wire form: the host's own matcher bytes, then the count as
    /// four fixed signed bytes.
    ///
    /// # Errors
    ///
    /// Returns [`QuantityRefused::MatcherTooLong`] past [`MAX_MATCHER_BYTES`] and
    /// [`QuantityRefused::CountOutOfRange`] past [`MAX_COUNT`]. The count bound is
    /// applied here as well as at the reader, which is the half the original
    /// does not have.
    pub fn write(&self, matcher_form: &[u8]) -> Result<Vec<u8>, QuantityRefused> {
        if matcher_form.len() > MAX_MATCHER_BYTES {
            return Err(QuantityRefused::MatcherTooLong(matcher_form.len()));
        }
        if self.count > MAX_COUNT {
            return Err(QuantityRefused::CountOutOfRange(i64::from(self.count)));
        }
        let mut bytes = Vec::with_capacity(matcher_form.len() + 4);
        bytes.extend_from_slice(matcher_form);
        let count = i32::try_from(self.count)
            .map_err(|_| QuantityRefused::CountOutOfRange(i64::from(self.count)))?;
        bytes.extend_from_slice(&count.to_be_bytes());
        Ok(bytes)
    }

    /// Reads the wire form, answering the host's matcher bytes and the count.
    ///
    /// The matcher's form is everything before the last four bytes, because §7's
    /// wire shape is the matcher followed by the count and nothing frames the
    /// matcher. Only the host can say how many of those bytes its own reader
    /// would have consumed, so what is answered here is all of them.
    ///
    /// # Errors
    ///
    /// Returns [`QuantityRefused::Truncated`] for a buffer with no room for the count,
    /// [`QuantityRefused::MatcherTooLong`] for a matcher form past
    /// [`MAX_MATCHER_BYTES`], and [`QuantityRefused::CountOutOfRange`] for a negative
    /// count.
    pub fn read(bytes: &[u8]) -> Result<(Vec<u8>, u32), QuantityRefused> {
        let end = bytes
            .len()
            .checked_sub(4)
            .ok_or(QuantityRefused::Truncated)?;
        if end > MAX_MATCHER_BYTES {
            return Err(QuantityRefused::MatcherTooLong(end));
        }
        let count_bytes: [u8; 4] = bytes
            .get(end..)
            .ok_or(QuantityRefused::Truncated)?
            .try_into()
            .map_err(|_| QuantityRefused::Truncated)?;
        let count = i32::from_be_bytes(count_bytes);
        if count < 0 {
            return Err(QuantityRefused::CountOutOfRange(i64::from(count)));
        }
        Ok((bytes[..end].to_vec(), count.unsigned_abs()))
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::ContentId;

    use ravel_sdk::json;

    use super::{
        Consumed, Flavour, MAX_ACCEPTED, MAX_COUNT, MAX_MATCHER_BYTES, Matcher, Portion,
        PortionSpellings, PortionSpellingsUnavailable, QuantityRefused, Stack,
    };

    fn id(path: &str) -> ContentId {
        ContentId::new(
            "rvl_146_counterpart"
                .parse()
                .expect("the declared identity is a legal mod id"),
            path,
        )
        .expect("a legal content path")
    }

    fn matcher() -> Matcher {
        Matcher::accepting(Flavour::Item, [id("ingot"), id("nugget")]).expect("two names")
    }

    fn portion(count: u32) -> Portion {
        Portion::declared(matcher(), count).expect("a positive count inside the bound")
    }

    #[test]
    fn consuming_moves_the_smaller_of_what_is_available_and_what_is_outstanding() {
        let stack = Stack::new(Flavour::Item, id("ingot"), 3);
        let consumed = portion(8).consume(&stack);
        assert_eq!(consumed.moved, 3);
        assert_eq!(consumed.outstanding.count(), 5);
        assert!(consumed.candidate.is_empty());

        let consumed = portion(2).consume(&stack);
        assert_eq!(consumed.moved, 2);
        assert!(consumed.outstanding.is_empty());
        assert_eq!(consumed.candidate.amount, 1);
    }

    #[test]
    fn neither_the_portion_nor_the_candidate_is_changed_by_consuming_from_it() {
        let stack = Stack::new(Flavour::Item, id("ingot"), 3);
        let portion = portion(8);
        let first = portion.consume(&stack);
        let second = portion.consume(&stack);
        assert_eq!(first, second, "a shared portion is not an accumulator");
        assert_eq!(portion.count(), 8);
        assert_eq!(stack.amount, 3);
    }

    #[test]
    fn nothing_moves_when_nothing_is_outstanding() {
        let stack = Stack::new(Flavour::Item, id("ingot"), 3);
        let empty = Portion::empty(matcher());
        assert!(empty.is_empty());
        assert_eq!(
            empty.consume(&stack),
            Consumed {
                moved: 0,
                outstanding: empty,
                candidate: stack,
            }
        );
    }

    #[test]
    fn nothing_moves_when_the_matcher_rejects_the_candidate() {
        let portion = portion(4);
        for stack in [
            Stack::new(Flavour::Item, id("plate"), 9),
            Stack::new(Flavour::Fluid, id("ingot"), 9),
        ] {
            let consumed = portion.consume(&stack);
            assert_eq!(consumed.moved, 0);
            assert_eq!(consumed.outstanding.count(), 4);
            assert_eq!(consumed.candidate, stack);
        }
    }

    #[test]
    fn a_fluid_portion_and_an_item_portion_are_the_same_type_and_never_confused() {
        let fluid = Portion::declared(
            Matcher::accepting(Flavour::Fluid, [id("molten")]).expect("one name"),
            1000,
        )
        .expect("a positive count");
        assert_eq!(fluid.matcher().flavour(), Flavour::Fluid);
        assert_eq!(
            fluid
                .consume(&Stack::new(Flavour::Fluid, id("molten"), 250))
                .moved,
            250
        );
        assert_eq!(
            fluid
                .consume(&Stack::new(Flavour::Item, id("molten"), 250))
                .moved,
            0
        );
    }

    #[test]
    fn a_declared_count_of_none_is_refused_and_an_empty_portion_is_not_declared() {
        assert_eq!(
            Portion::declared(matcher(), 0),
            Err(QuantityRefused::ZeroCount),
            "a recipe asking for none of something is a mistake where it is written"
        );
        assert!(Portion::empty(matcher()).is_empty());
    }

    #[test]
    fn a_count_past_the_bound_is_refused_at_the_declaration_and_at_the_writer() {
        assert_eq!(
            Portion::holding(matcher(), MAX_COUNT + 1),
            Err(QuantityRefused::CountOutOfRange(i64::from(MAX_COUNT) + 1))
        );
        assert!(Portion::holding(matcher(), MAX_COUNT).is_ok());
    }

    #[test]
    fn a_matcher_accepting_nothing_or_too_much_is_refused() {
        assert_eq!(
            Matcher::accepting(Flavour::Item, []),
            Err(QuantityRefused::AcceptsNothing)
        );
        let many: Vec<ContentId> = (0..=MAX_ACCEPTED).map(|n| id(&format!("one{n}"))).collect();
        assert_eq!(
            Matcher::accepting(Flavour::Item, many),
            Err(QuantityRefused::TooManyAccepted(MAX_ACCEPTED + 1))
        );
    }

    #[test]
    fn the_wire_form_round_trips_the_hosts_matcher_bytes_and_the_count() {
        let bytes = portion(7).write(&[1, 2, 3]).expect("a small matcher form");
        assert_eq!(Portion::read(&bytes), Ok((vec![1, 2, 3], 7)));
    }

    #[test]
    fn a_negative_count_off_the_wire_is_refused_rather_than_accepted() {
        let mut bytes = portion(7).write(&[]).expect("an empty matcher form");
        let count = bytes.len() - 4;
        bytes[count..].copy_from_slice(&(-5i32).to_be_bytes());
        assert_eq!(
            Portion::read(&bytes),
            Err(QuantityRefused::CountOutOfRange(-5))
        );
    }

    #[test]
    fn a_matcher_form_past_the_bound_is_refused_before_anything_is_taken() {
        let long = vec![0u8; MAX_MATCHER_BYTES + 5];
        assert_eq!(
            Portion::read(&long),
            Err(QuantityRefused::MatcherTooLong(MAX_MATCHER_BYTES + 1))
        );
    }

    #[test]
    fn a_wire_form_with_no_room_for_the_count_is_refused() {
        assert_eq!(Portion::read(&[]), Err(QuantityRefused::Truncated));
        assert_eq!(Portion::read(&[0, 0, 0]), Err(QuantityRefused::Truncated));
    }

    #[test]
    fn nothing_frames_the_matcher_so_its_form_is_whatever_precedes_the_count() {
        let bytes = portion(7).write(&[1, 2, 3]).expect("a small matcher form");
        assert_eq!(bytes.len(), 7, "three matcher bytes and four count bytes");
        assert_eq!(Portion::read(&bytes), Ok((vec![1, 2, 3], 7)));
        // §7's wire shape is the matcher's own form followed by the count, with
        // no length in front of it, so a buffer an original peer wrote parses
        // here. The price is that a byte appended to it is a matcher byte: only
        // the host's own reader knows where its form ends.
        let mut appended = bytes;
        appended.push(0);
        assert_eq!(Portion::read(&appended), Ok((vec![1, 2, 3, 0], 1792)));
    }

    #[test]
    fn a_declared_portion_is_written_and_read_under_the_supplied_spellings() {
        let spellings = PortionSpellings::placeholder();
        let matcher_form = json!({"tag": "forge:ingots/iron"});
        let document = portion(4).write_declared(&matcher_form, &spellings);
        assert_eq!(document[spellings.matcher()], matcher_form);
        assert_eq!(
            Portion::read_declared(&document, &spellings),
            Ok((matcher_form, 4))
        );
    }

    #[test]
    fn supplied_spellings_change_the_document_a_recipe_file_is_read_under() {
        let supplied = PortionSpellings::parse("ingredient", "amount").expect("two words");
        let matcher_form = json!("minecraft:iron_ingot");
        let document = portion(2).write_declared(&matcher_form, &supplied);
        assert_eq!(document["ingredient"], matcher_form);
        assert_eq!(document["amount"], json!(2));
        assert_eq!(
            Portion::read_declared(&document, &PortionSpellings::placeholder()),
            Err(QuantityRefused::MissingMatcher {
                key: "accepts".to_owned()
            }),
            "a document under one spelling does not read under another"
        );
    }

    #[test]
    fn three_ways_the_declared_spellings_are_refused_and_the_declared_pair_is_absent() {
        assert_eq!(
            PortionSpellings::parse("", "count"),
            Err(PortionSpellingsUnavailable::Empty)
        );
        assert_eq!(
            PortionSpellings::parse("matcher", ""),
            Err(PortionSpellingsUnavailable::Empty)
        );
        assert_eq!(
            PortionSpellings::parse("both", "both"),
            Err(PortionSpellingsUnavailable::NotDistinct)
        );
        assert_eq!(
            PortionSpellings::declared(),
            Err(PortionSpellingsUnavailable::NotSupplied),
            "no build input supplies them here, and nothing defaults"
        );
    }

    #[test]
    fn a_declared_count_defaults_to_one_and_is_bounded_the_way_the_wire_form_is() {
        let spellings = PortionSpellings::placeholder();
        let matcher_form = json!({"item": "minecraft:iron_ingot"});
        let mut bare = json!({});
        bare[spellings.matcher()] = matcher_form.clone();
        assert_eq!(
            Portion::read_declared(&bare, &spellings),
            Ok((matcher_form.clone(), 1)),
            "an omitted count is one"
        );

        let with = |count: i64| {
            let mut document = json!({});
            document[spellings.matcher()] = matcher_form.clone();
            document[spellings.count()] = json!(count);
            document
        };
        assert_eq!(
            Portion::read_declared(&with(0), &spellings),
            Err(QuantityRefused::ZeroCount)
        );
        assert_eq!(
            Portion::read_declared(&with(-3), &spellings),
            Err(QuantityRefused::CountOutOfRange(-3)),
            "the wire form's bound applies to the declared form too"
        );
        assert_eq!(
            Portion::read_declared(&with(i64::from(MAX_COUNT) + 1), &spellings),
            Err(QuantityRefused::CountOutOfRange(i64::from(MAX_COUNT) + 1))
        );
    }

    #[test]
    fn a_declared_document_of_the_wrong_shape_is_refused() {
        let spellings = PortionSpellings::placeholder();
        assert_eq!(
            Portion::read_declared(&json!([1, 2]), &spellings),
            Err(QuantityRefused::NotAnObject)
        );
        assert_eq!(
            Portion::read_declared(&json!({}), &spellings),
            Err(QuantityRefused::MissingMatcher {
                key: spellings.matcher().to_owned()
            })
        );
        let mut wrong = json!({});
        wrong[spellings.matcher()] = json!("minecraft:iron_ingot");
        wrong[spellings.count()] = json!("two");
        assert_eq!(
            Portion::read_declared(&wrong, &spellings),
            Err(QuantityRefused::CountNotWhole {
                key: spellings.count().to_owned()
            })
        );
    }

    #[test]
    fn a_matcher_form_past_the_bound_is_refused_at_the_writer() {
        let long = vec![0u8; MAX_MATCHER_BYTES + 1];
        assert_eq!(
            portion(1).write(&long),
            Err(QuantityRefused::MatcherTooLong(MAX_MATCHER_BYTES + 1))
        );
    }

    #[test]
    fn a_matcher_reports_what_it_accepts_and_a_flavour_names_itself() {
        assert_eq!(matcher().accepted(), [id("ingot"), id("nugget")]);
        assert_eq!(Flavour::Item.as_str(), "item");
        assert_eq!(Flavour::Fluid.to_string(), "fluid");
    }
}
