//! Section ten's abstraction and its collections.
//!
//! The wave decision makes this package the owner of the vocabulary a dependent
//! declares against, so the traits below are declared here rather than consumed
//! from anywhere. The specification records that the original's own declarations
//! were absent from the fetched archive and that their signatures are unknown, so
//! nothing here claims to be the original's shape: these are this package's
//! interfaces, written to carry the behaviour section ten does state.
//!
//! What section ten states and this module owes: equality is a property of the
//! query rather than of the stored value, a partitioned collection may only use a
//! partition when the query's conditions are a superset of the partitioning ones,
//! a collapsed collection holds one entry per distinct instance with a summed
//! amount, and a summing view presents an uncollapsed collection as if collapsed
//! without copying it.

use ravel_sdk::JsonValue;

use core::fmt;

/// How much of something is held.
///
/// Unsigned and saturating everywhere: an amount is a count, a wrapped sum is a
/// smaller number than the one already held, and section ten's collections are
/// fed by decoded input.
pub type Amount = u64;

/// The number of condition slots a kind may name.
pub const CONDITION_SLOTS: u32 = 32;

/// A set of the conditions a match is asked to consider.
///
/// A kind names its conditions by slot and a caller asks for a subset of them.
/// The same two instances compare equal under one set and unequal under another,
/// which is the part of section ten that is easy to lose.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Conditions(u32);

impl Conditions {
    /// Match on nothing at all, under which every instance of a kind is alike.
    pub const NONE: Self = Self(0);

    /// Match on every slot a kind could name.
    pub const EVERY: Self = Self(u32::MAX);

    /// Builds a set from its raw bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw bits, which is what a wire form carries.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Adds one slot.
    ///
    /// A slot at or above [`CONDITION_SLOTS`] names no condition and is dropped
    /// rather than shifted into an unrelated one.
    #[must_use]
    pub const fn with(self, slot: u32) -> Self {
        if slot >= CONDITION_SLOTS {
            return self;
        }
        Self(self.0 | (1u32 << slot))
    }

    /// Whether one slot is asked for.
    #[must_use]
    pub const fn holds(self, slot: u32) -> bool {
        if slot >= CONDITION_SLOTS {
            return false;
        }
        self.0 & (1u32 << slot) != 0
    }

    /// Whether this set asks for everything `other` asks for.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// How many slots are asked for.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }
}

/// What a kind refuses when it is handed stored bytes it does not spell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KindError {
    detail: String,
}

impl KindError {
    /// Builds a refusal naming what was wrong with the stored form.
    #[must_use]
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    /// What was wrong.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for KindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for KindError {}

/// A thing that can be counted, matched and stored.
///
/// Items, fluid volumes and energy are the three the original's family uses and
/// none of them is named here: the collections are written against this and not
/// against those three, which is the property section ten states.
pub trait QuantityKind {
    /// One instance of the kind, without its amount.
    type Instance;

    /// Every condition this kind understands.
    fn conditions(&self) -> Conditions;

    /// Whether two instances are alike under the conditions the caller asked for.
    fn matches(&self, left: &Self::Instance, right: &Self::Instance, under: Conditions) -> bool;

    /// A digest consistent with [`QuantityKind::matches`] under the same set.
    ///
    /// Two instances that match under a set must digest alike under it. The
    /// converse is not owed, and every collection here confirms a digest hit
    /// with a match before it believes it.
    fn digest(&self, value: &Self::Instance, under: Conditions) -> u64;

    /// Writes one instance to its stored form.
    fn write(&self, value: &Self::Instance) -> JsonValue;

    /// Reads one instance back.
    ///
    /// # Errors
    ///
    /// Returns [`KindError`] when the stored form does not spell an instance.
    fn read(&self, stored: &JsonValue) -> Result<Self::Instance, KindError>;
}

/// Whether a partitioned lookup may cross partitions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Partitioning {
    /// One category function, and a lookup never leaves its own partition.
    Single,
    /// More than one, and a lookup may intersect the partitions it names.
    Multiple,
}

/// A kind that also sorts its instances into partitions.
///
/// The partitioning is derived from a subset of the kind's conditions, and that
/// subset is what decides whether a query may use it at all.
pub trait Partitioned: QuantityKind {
    /// The conditions the partitioning is derived from.
    fn axes(&self) -> Conditions;

    /// Whether a lookup may intersect partitions.
    fn partitioning(&self) -> Partitioning;

    /// Which partition an instance falls in.
    fn partition(&self, value: &Self::Instance) -> u64;
}

/// One held instance and how much of it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Held<I> {
    /// The instance.
    pub value: I,
    /// How much of it is held.
    pub amount: Amount,
}

impl<I> Held<I> {
    /// Builds a held entry.
    pub const fn new(value: I, amount: Amount) -> Self {
        Self { value, amount }
    }
}

/// The plain list: one entry per addition, in the order they were added.
#[derive(Clone, Debug)]
pub struct List<K: QuantityKind> {
    kind: K,
    held: Vec<Held<K::Instance>>,
}

impl<K: QuantityKind> List<K> {
    /// An empty list over one kind.
    pub const fn new(kind: K) -> Self {
        Self {
            kind,
            held: Vec::new(),
        }
    }

    /// The kind this list counts.
    pub const fn kind(&self) -> &K {
        &self.kind
    }

    /// How many entries are held, which is additions and not distinct instances.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Every entry in the order it was added.
    pub fn entries(&self) -> &[Held<K::Instance>] {
        &self.held
    }

    /// Adds one entry. A zero amount adds nothing and is not an entry.
    pub fn add(&mut self, value: K::Instance, amount: Amount) {
        if amount == 0 {
            return;
        }
        self.held.push(Held::new(value, amount));
    }

    /// How much matches, under the conditions the caller asked for.
    ///
    /// Saturating, because a list is fed by decoded input and a wrapped total
    /// would answer a smaller number than one entry alone holds.
    pub fn amount_of(&self, value: &K::Instance, under: Conditions) -> Amount {
        self.held
            .iter()
            .filter(|entry| self.kind.matches(&entry.value, value, under))
            .fold(0, |total, entry| total.saturating_add(entry.amount))
    }

    /// Takes up to `amount` of what matches, oldest entry first.
    ///
    /// Returns how much was taken, which is never more than is held.
    pub fn take(&mut self, value: &K::Instance, amount: Amount, under: Conditions) -> Amount {
        let mut left = amount;
        let mut taken: Amount = 0;
        let kind = &self.kind;
        for entry in &mut self.held {
            if left == 0 {
                break;
            }
            if !kind.matches(&entry.value, value, under) {
                continue;
            }
            let step = entry.amount.min(left);
            entry.amount -= step;
            left -= step;
            taken = taken.saturating_add(step);
        }
        self.held.retain(|entry| entry.amount > 0);
        taken
    }

    /// Every entry a predicate admits, without copying the list.
    pub fn filtered<F>(&self, admits: F) -> Filtered<'_, K, F>
    where
        F: Fn(&Held<K::Instance>) -> bool,
    {
        Filtered {
            entries: self.held.iter(),
            admits,
        }
    }

    /// This list presented as if collapsed, without copying it.
    pub const fn summed(&self, under: Conditions) -> Summing<'_, K> {
        Summing { list: self, under }
    }
}

impl<K: QuantityKind> List<K>
where
    K::Instance: Clone,
{
    /// A collapsed copy of this list under one set of conditions.
    pub fn collapsed(&self, under: Conditions) -> Collapsed<K>
    where
        K: Clone,
    {
        let mut collapsed = Collapsed::new(self.kind.clone(), under);
        for entry in &self.held {
            collapsed.add(entry.value.clone(), entry.amount);
        }
        collapsed
    }
}

/// The iterator [`List::filtered`] hands back.
pub struct Filtered<'held, K: QuantityKind, F> {
    entries: core::slice::Iter<'held, Held<K::Instance>>,
    admits: F,
}

impl<'held, K, F> Iterator for Filtered<'held, K, F>
where
    K: QuantityKind,
    F: Fn(&Held<K::Instance>) -> bool,
{
    type Item = &'held Held<K::Instance>;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.by_ref().find(|entry| (self.admits)(entry))
    }
}

/// A double-ended chain of entries, which is the linked form of the plain list.
///
/// The one behaviour that separates it from [`List`] is that an entry can be
/// added and taken at either end, so a dependent using it as a queue keeps
/// insertion order without paying to shift the whole run.
#[derive(Clone, Debug)]
pub struct Chain<K: QuantityKind> {
    kind: K,
    held: std::collections::VecDeque<Held<K::Instance>>,
}

impl<K: QuantityKind> Chain<K> {
    /// An empty chain over one kind.
    #[must_use]
    pub fn new(kind: K) -> Self {
        Self {
            kind,
            held: std::collections::VecDeque::new(),
        }
    }

    /// The kind this chain counts.
    pub const fn kind(&self) -> &K {
        &self.kind
    }

    /// How many entries are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Adds at the back. A zero amount adds nothing.
    pub fn add_last(&mut self, value: K::Instance, amount: Amount) {
        if amount == 0 {
            return;
        }
        self.held.push_back(Held::new(value, amount));
    }

    /// Adds at the front. A zero amount adds nothing.
    pub fn add_first(&mut self, value: K::Instance, amount: Amount) {
        if amount == 0 {
            return;
        }
        self.held.push_front(Held::new(value, amount));
    }

    /// Takes the entry at the front.
    pub fn take_first(&mut self) -> Option<Held<K::Instance>> {
        self.held.pop_front()
    }

    /// Takes the entry at the back.
    pub fn take_last(&mut self) -> Option<Held<K::Instance>> {
        self.held.pop_back()
    }

    /// Every entry, front to back.
    pub fn entries(&self) -> impl Iterator<Item = &Held<K::Instance>> + '_ {
        self.held.iter()
    }

    /// How much matches, under the conditions the caller asked for.
    pub fn amount_of(&self, value: &K::Instance, under: Conditions) -> Amount {
        self.held
            .iter()
            .filter(|entry| self.kind.matches(&entry.value, value, under))
            .fold(0, |total, entry| total.saturating_add(entry.amount))
    }
}

/// One entry per distinct instance, with the amounts summed.
///
/// Adding the same thing twice yields one entry of doubled amount and iteration
/// yields one element, which is the whole of what separates this from [`List`].
#[derive(Clone, Debug)]
pub struct Collapsed<K: QuantityKind> {
    kind: K,
    under: Conditions,
    held: Vec<(u64, Held<K::Instance>)>,
}

impl<K: QuantityKind> Collapsed<K> {
    /// An empty collapsed collection, distinguishing under one set.
    pub const fn new(kind: K, under: Conditions) -> Self {
        Self {
            kind,
            under,
            held: Vec::new(),
        }
    }

    /// The conditions two entries are held apart by.
    #[must_use]
    pub const fn under(&self) -> Conditions {
        self.under
    }

    /// The kind this collection counts.
    pub const fn kind(&self) -> &K {
        &self.kind
    }

    /// How many distinct instances are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Adds an amount, folding it into a matching entry when there is one.
    pub fn add(&mut self, value: K::Instance, amount: Amount) {
        if amount == 0 {
            return;
        }
        let digest = self.kind.digest(&value, self.under);
        for (held_digest, entry) in &mut self.held {
            if *held_digest == digest && self.kind.matches(&entry.value, &value, self.under) {
                entry.amount = entry.amount.saturating_add(amount);
                return;
            }
        }
        self.held.push((digest, Held::new(value, amount)));
    }

    /// How much of one instance is held.
    pub fn amount_of(&self, value: &K::Instance) -> Amount {
        let digest = self.kind.digest(value, self.under);
        self.held
            .iter()
            .find(|(held_digest, entry)| {
                *held_digest == digest && self.kind.matches(&entry.value, value, self.under)
            })
            .map_or(0, |(_, entry)| entry.amount)
    }

    /// Every distinct entry, in the order each was first added.
    pub fn entries(&self) -> impl Iterator<Item = &Held<K::Instance>> + '_ {
        self.held.iter().map(|(_, entry)| entry)
    }
}

/// A view over a [`List`] that reads as if it were collapsed.
///
/// Nothing is copied and the list is not changed: the view walks the entries and
/// folds matching ones as it goes, so a dependent can ask a list a collapsed
/// question without paying for a collapsed collection.
#[derive(Clone, Copy)]
pub struct Summing<'list, K: QuantityKind> {
    list: &'list List<K>,
    under: Conditions,
}

impl<K: QuantityKind> Summing<'_, K> {
    /// How many distinct instances the underlying list holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.folded().len()
    }

    /// Whether the underlying list holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// How much of one instance the underlying list holds in total.
    pub fn amount_of(&self, value: &K::Instance) -> Amount {
        self.list.amount_of(value, self.under)
    }

    /// The summed entries, each pointing at the first entry that produced it.
    #[must_use]
    pub fn entries(&self) -> Vec<(&K::Instance, Amount)> {
        self.folded()
    }

    fn folded(&self) -> Vec<(&K::Instance, Amount)> {
        let kind = self.list.kind();
        let mut folded: Vec<(&K::Instance, Amount)> = Vec::new();
        for entry in self.list.entries() {
            if let Some(slot) = folded
                .iter_mut()
                .find(|(value, _)| kind.matches(value, &entry.value, self.under))
            {
                slot.1 = slot.1.saturating_add(entry.amount);
            } else {
                folded.push((&entry.value, entry.amount));
            }
        }
        folded
    }
}

/// A collection that keeps its contents in partitions.
///
/// For a single-category kind, a lookup restricted to a category scans that
/// category's partition alone. The restriction section ten leaves implicit is
/// enforced here in one place: [`Classified::may_use_partitions`] answers false
/// unless the query's conditions are a superset of the partitioning ones, and
/// every lookup that cannot use one partition falls back to a whole scan.
#[derive(Clone, Debug)]
pub struct Classified<K: Partitioned> {
    kind: K,
    held: Vec<(u64, Held<K::Instance>)>,
}

impl<K: Partitioned> Classified<K> {
    /// An empty classified collection.
    pub const fn new(kind: K) -> Self {
        Self {
            kind,
            held: Vec::new(),
        }
    }

    /// The kind this collection partitions by.
    pub const fn kind(&self) -> &K {
        &self.kind
    }

    /// How many entries are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Adds one entry into its partition. A zero amount adds nothing.
    pub fn add(&mut self, value: K::Instance, amount: Amount) {
        if amount == 0 {
            return;
        }
        let partition = self.kind.partition(&value);
        self.held.push((partition, Held::new(value, amount)));
    }

    /// Whether a query asking for these conditions may use the partitions.
    ///
    /// A query that matches on less than the partitioning was derived from can
    /// call two instances alike that sit in different partitions, so its answer
    /// would be short by whatever the other partitions hold.
    #[must_use]
    pub fn may_use_partitions(&self, query: Conditions) -> bool {
        query.contains(self.kind.axes())
    }

    /// How much matches, and how much of the collection had to be read.
    ///
    /// The second number is the observable half: a single-category lookup that
    /// may use its partition reads that partition alone; every other lookup reads
    /// everything. A dependent that could not tell the two apart would have no
    /// reason to hold a classified collection at all.
    pub fn amount_of(&self, value: &K::Instance, under: Conditions) -> Lookup {
        let mut scanned = 0;
        let mut total: Amount = 0;
        // A multi-category collection may intersect partitions and this trait
        // publishes one category function, so there is no second partition for a
        // lookup to name and reading them all is the only sound answer. Only the
        // single-category form can name the one partition a match could be in.
        // `NOTES.md` carries this as a declared simplification.
        let restrict = (self.may_use_partitions(under)
            && self.kind.partitioning() == Partitioning::Single)
            .then(|| self.kind.partition(value));
        for (partition, entry) in &self.held {
            if let Some(wanted) = restrict
                && *partition != wanted
            {
                continue;
            }
            scanned += 1;
            if self.kind.matches(&entry.value, value, under) {
                total = total.saturating_add(entry.amount);
            }
        }
        Lookup {
            amount: total,
            scanned,
            used_partitions: restrict.is_some(),
        }
    }
}

/// What a classified lookup answered and what it cost.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Lookup {
    /// How much matched.
    pub amount: Amount,
    /// How many entries were read to answer.
    pub scanned: usize,
    /// Whether the lookup read one partition rather than the whole collection.
    pub used_partitions: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::json;

    /// Two conditions: slot zero is the name and slot one is the mark. A mark of
    /// zero is not special; matching on the mark is what the caller asks for.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Token {
        name: u32,
        mark: u32,
    }

    #[derive(Clone, Copy, Debug)]
    struct Tokens;

    const NAME: u32 = 0;
    const MARK: u32 = 1;

    impl QuantityKind for Tokens {
        type Instance = Token;

        fn conditions(&self) -> Conditions {
            Conditions::NONE.with(NAME).with(MARK)
        }

        fn matches(&self, left: &Token, right: &Token, under: Conditions) -> bool {
            (!under.holds(NAME) || left.name == right.name)
                && (!under.holds(MARK) || left.mark == right.mark)
        }

        fn digest(&self, value: &Token, under: Conditions) -> u64 {
            let name = if under.holds(NAME) {
                u64::from(value.name)
            } else {
                0
            };
            let mark = if under.holds(MARK) {
                u64::from(value.mark)
            } else {
                0
            };
            name.wrapping_mul(31).wrapping_add(mark)
        }

        fn write(&self, value: &Token) -> JsonValue {
            json!({ "name": value.name, "mark": value.mark })
        }

        fn read(&self, stored: &JsonValue) -> Result<Token, KindError> {
            let field = |key: &str| {
                stored
                    .get(key)
                    .and_then(JsonValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| KindError::new(format!("no {key} in the stored form")))
            };
            Ok(Token {
                name: field("name")?,
                mark: field("mark")?,
            })
        }
    }

    impl Partitioned for Tokens {
        fn axes(&self) -> Conditions {
            Conditions::NONE.with(NAME)
        }

        fn partitioning(&self) -> Partitioning {
            Partitioning::Single
        }

        fn partition(&self, value: &Token) -> u64 {
            u64::from(value.name)
        }
    }

    const fn token(name: u32, mark: u32) -> Token {
        Token { name, mark }
    }

    fn both() -> Conditions {
        Conditions::NONE.with(NAME).with(MARK)
    }

    fn name_only() -> Conditions {
        Conditions::NONE.with(NAME)
    }

    #[test]
    fn a_condition_slot_beyond_the_published_count_names_nothing() {
        let asked = Conditions::NONE
            .with(CONDITION_SLOTS)
            .with(CONDITION_SLOTS + 9);
        assert_eq!(asked, Conditions::NONE);
        assert!(!asked.holds(CONDITION_SLOTS));
        assert_eq!(Conditions::NONE.with(31).count(), 1);
    }

    #[test]
    fn a_superset_contains_the_set_it_extends_and_the_reverse_is_not_true() {
        assert!(both().contains(name_only()));
        assert!(!name_only().contains(both()));
        assert!(Conditions::EVERY.contains(both()));
        assert!(both().contains(Conditions::NONE));
    }

    #[test]
    fn the_same_collection_answers_differently_under_different_conditions() {
        let mut list = List::new(Tokens);
        list.add(token(1, 0), 4);
        list.add(token(1, 7), 6);
        assert_eq!(list.amount_of(&token(1, 0), both()), 4);
        assert_eq!(list.amount_of(&token(1, 0), name_only()), 10);
        assert_eq!(list.amount_of(&token(2, 0), name_only()), 0);
    }

    #[test]
    fn a_list_holds_one_entry_per_addition_and_a_collapsed_collection_one_per_instance() {
        let mut list = List::new(Tokens);
        list.add(token(1, 0), 4);
        list.add(token(1, 0), 4);
        assert_eq!(list.len(), 2);
        let collapsed = list.collapsed(both());
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed.amount_of(&token(1, 0)), 8);
    }

    #[test]
    fn a_zero_amount_is_not_an_entry_in_any_collection() {
        let mut list = List::new(Tokens);
        list.add(token(1, 0), 0);
        let mut collapsed = Collapsed::new(Tokens, both());
        collapsed.add(token(1, 0), 0);
        let mut classified = Classified::new(Tokens);
        classified.add(token(1, 0), 0);
        let mut chain = Chain::new(Tokens);
        chain.add_last(token(1, 0), 0);
        assert!(list.is_empty() && collapsed.is_empty() && classified.is_empty());
        assert!(chain.is_empty());
    }

    #[test]
    fn a_summing_view_reads_as_collapsed_without_changing_the_list() {
        let mut list = List::new(Tokens);
        list.add(token(1, 0), 4);
        list.add(token(1, 0), 5);
        list.add(token(2, 0), 1);
        let view = list.summed(both());
        assert_eq!(view.len(), 2);
        assert_eq!(view.amount_of(&token(1, 0)), 9);
        assert_eq!(view.entries().len(), 2);
        assert_eq!(list.len(), 3, "the view copied nothing");
    }

    #[test]
    fn a_summed_total_saturates_rather_than_wrapping() {
        let mut list = List::new(Tokens);
        list.add(token(1, 0), Amount::MAX);
        list.add(token(1, 0), 9);
        assert_eq!(list.amount_of(&token(1, 0), both()), Amount::MAX);
        let mut collapsed = Collapsed::new(Tokens, both());
        collapsed.add(token(1, 0), Amount::MAX);
        collapsed.add(token(1, 0), 9);
        assert_eq!(collapsed.amount_of(&token(1, 0)), Amount::MAX);
    }

    #[test]
    fn taking_never_yields_more_than_is_held_and_empties_the_entries_it_drains() {
        let mut list = List::new(Tokens);
        list.add(token(1, 0), 4);
        list.add(token(1, 0), 4);
        assert_eq!(list.take(&token(1, 0), 6, both()), 6);
        assert_eq!(list.len(), 1);
        assert_eq!(list.amount_of(&token(1, 0), both()), 2);
        assert_eq!(list.take(&token(1, 0), 99, both()), 2);
        assert!(list.is_empty());
    }

    #[test]
    fn a_partitioned_lookup_reads_one_partition_only_when_the_query_permits_it() {
        let mut classified = Classified::new(Tokens);
        for name in 0..4u32 {
            classified.add(token(name, 0), 2);
            classified.add(token(name, 1), 3);
        }
        let narrow = classified.amount_of(&token(2, 0), both());
        assert!(narrow.used_partitions);
        assert_eq!(narrow.scanned, 2, "one partition, not all eight entries");
        assert_eq!(narrow.amount, 2);

        let wide = classified.amount_of(&token(2, 0), Conditions::NONE.with(MARK));
        assert!(
            !wide.used_partitions,
            "the query matches on less than the partitioning was derived from"
        );
        assert_eq!(wide.scanned, 8);
        assert_eq!(wide.amount, 8, "every mark of zero, across every partition");
    }

    #[derive(Clone, Copy, Debug)]
    struct ManyTokens;

    impl QuantityKind for ManyTokens {
        type Instance = Token;

        fn conditions(&self) -> Conditions {
            Tokens.conditions()
        }

        fn matches(&self, left: &Token, right: &Token, under: Conditions) -> bool {
            Tokens.matches(left, right, under)
        }

        fn digest(&self, value: &Token, under: Conditions) -> u64 {
            Tokens.digest(value, under)
        }

        fn write(&self, value: &Token) -> JsonValue {
            Tokens.write(value)
        }

        fn read(&self, stored: &JsonValue) -> Result<Token, KindError> {
            Tokens.read(stored)
        }
    }

    impl Partitioned for ManyTokens {
        fn axes(&self) -> Conditions {
            Conditions::NONE.with(NAME)
        }

        fn partitioning(&self) -> Partitioning {
            Partitioning::Multiple
        }

        fn partition(&self, value: &Token) -> u64 {
            u64::from(value.name)
        }
    }

    #[test]
    fn a_multi_category_collection_reads_everything_even_where_the_query_permits_a_partition() {
        let mut classified = Classified::new(ManyTokens);
        for name in 0..4u32 {
            classified.add(token(name, 0), 2);
            classified.add(token(name, 1), 3);
        }
        assert!(
            classified.may_use_partitions(both()),
            "the query names the axis the partitioning was derived from"
        );
        let found = classified.amount_of(&token(2, 0), both());
        assert_eq!(found.amount, 2, "the answer is right either way");
        assert_eq!(
            found.scanned, 8,
            "one category function names one partition, so a lookup that may \
             intersect several can name none of them and reads them all"
        );
        assert!(!found.used_partitions);
    }

    #[test]
    fn the_partition_restriction_is_the_superset_rule_and_nothing_else() {
        let classified = Classified::new(Tokens);
        assert!(classified.may_use_partitions(both()));
        assert!(classified.may_use_partitions(name_only()));
        assert!(!classified.may_use_partitions(Conditions::NONE.with(MARK)));
        assert!(!classified.may_use_partitions(Conditions::NONE));
    }

    #[test]
    fn a_chain_adds_and_takes_at_both_ends_and_keeps_the_order_between() {
        let mut chain = Chain::new(Tokens);
        chain.add_last(token(2, 0), 2);
        chain.add_first(token(1, 0), 1);
        chain.add_last(token(3, 0), 3);
        let order: Vec<u32> = chain.entries().map(|entry| entry.value.name).collect();
        assert_eq!(order, vec![1, 2, 3]);
        assert_eq!(chain.take_first().map(|entry| entry.value.name), Some(1));
        assert_eq!(chain.take_last().map(|entry| entry.value.name), Some(3));
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.amount_of(&token(2, 0), both()), 2);
    }

    #[test]
    fn a_filtered_iterator_reads_the_list_it_filters_rather_than_copying_it() {
        let mut list = List::new(Tokens);
        list.add(token(1, 0), 4);
        list.add(token(2, 0), 5);
        list.add(token(3, 0), 6);
        let admitted: Vec<Amount> = list
            .filtered(|entry| entry.value.name % 2 == 1)
            .map(|entry| entry.amount)
            .collect();
        assert_eq!(admitted, vec![4, 6]);
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn a_kind_round_trips_an_instance_and_refuses_a_stored_form_it_does_not_spell() {
        let stored = Tokens.write(&token(5, 6));
        assert_eq!(Tokens.read(&stored), Ok(token(5, 6)));
        let refused = Tokens.read(&json!({ "name": 5 }));
        assert!(refused.is_err());
        let refused = Tokens.read(&json!({ "name": -1, "mark": 0 }));
        assert!(refused.is_err());
        let refused = Tokens.read(&json!({ "name": 5, "mark": u64::from(u32::MAX) + 1 }));
        assert!(refused.is_err(), "a value past the instance's own width");
    }

    #[test]
    fn the_digest_agrees_with_the_matcher_under_every_set_it_is_asked_about() {
        let sets = [
            Conditions::NONE,
            name_only(),
            Conditions::NONE.with(MARK),
            both(),
        ];
        let values = [token(1, 0), token(1, 1), token(2, 0), token(2, 1)];
        for under in sets {
            for left in values {
                for right in values {
                    if Tokens.matches(&left, &right, under) {
                        assert_eq!(
                            Tokens.digest(&left, under),
                            Tokens.digest(&right, under),
                            "a matcher and its hasher disagree under {under:?}"
                        );
                    }
                }
            }
        }
    }
}
