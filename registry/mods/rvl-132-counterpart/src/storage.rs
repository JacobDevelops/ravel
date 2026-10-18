//! Section eight's store-adapter table.
//!
//! A dependent asks for a store of a given ingredient kind on a given game
//! object and gets one whether or not the object publishes that kind natively.
//! Twelve adapters are installed and three of their behaviours are load-bearing
//! and easy to lose: the combined item store, the all-or-nothing extraction by
//! prototype, and a missing capability answering an empty store rather than an
//! error.
//!
//! The store interfaces themselves are RVL-210's, consumed from there:
//! [`Store`], [`SlottedStore`], [`Attempt`] and [`Slotted`] are its types.

use core::fmt;

use rvl_210_counterpart::quantity::{Amount, Conditions};
use rvl_210_counterpart::storage::{Attempt, Slotted, SlottedStore, Store};

use crate::kinds::{
    Charge, Fluid as KindOfFluid, Ingredient, PORTION, SCALAR, STACKABLE, Scalar, Stackable,
};

/// The most slots a store this package builds may hold.
///
/// A slot count reaches this package from a host object, so it is bounded before
/// anything is allocated against it.
pub const MAX_SLOTS: usize = 1024;

/// The largest charge the host's charge interface carries.
///
/// Defect ten: the original narrows a 64-bit quantity to 32 bits on every insert
/// and extract through a checked cast helper whose behaviour above the range the
/// specification marks unknown. This package saturates here instead, and
/// [`narrowed`] reports whether it did, so a caller is told rather than
/// surprised.
pub const HOST_CHARGE_CEILING: Amount = 0x7fff_ffff;

/// The three object classes an adapter is installed on.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Class {
    /// A block.
    Block,
    /// An item.
    Item,
    /// An entity.
    Entity,
}

impl Class {
    /// All three, in the order section eight names them.
    pub const ALL: [Self; 3] = [Self::Block, Self::Item, Self::Entity];

    /// What the class is called.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Item => "item",
            Self::Entity => "entity",
        }
    }
}

/// The four store shapes an adapter carries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Shape {
    /// The charge kind.
    Charge,
    /// The item kind, slot-addressed.
    SlottedItem,
    /// The item kind, slotless.
    SlotlessItem,
    /// The fluid kind.
    Fluid,
}

impl Shape {
    /// All four, in the order section eight names them.
    pub const ALL: [Self; 4] = [
        Self::Charge,
        Self::SlottedItem,
        Self::SlotlessItem,
        Self::Fluid,
    ];

    /// The ingredient kind this shape carries.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Charge => SCALAR,
            Self::SlottedItem | Self::SlotlessItem => STACKABLE,
            Self::Fluid => PORTION,
        }
    }

    /// Whether the shape is slot-addressed.
    #[must_use]
    pub const fn slotted(self) -> bool {
        matches!(self, Self::SlottedItem)
    }
}

/// One installed adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Adapter {
    /// The store shape it carries.
    pub shape: Shape,
    /// The object class it is installed on.
    pub class: Class,
}

/// The twelve adapters, four shapes on each of three classes.
#[must_use]
pub fn table() -> Vec<Adapter> {
    let mut adapters = Vec::with_capacity(Shape::ALL.len() * Class::ALL.len());
    for shape in Shape::ALL {
        for class in Class::ALL {
            adapters.push(Adapter { shape, class });
        }
    }
    adapters
}

/// Whether an adapter is installed for one shape on one class.
#[must_use]
pub fn installed(shape: Shape, class: Class) -> bool {
    table().contains(&Adapter { shape, class })
}

/// What a game object publishes natively, which is what an adapter adapts.
///
/// A dependent asks for a store of a given kind on a given object and gets one
/// whether or not the object publishes that kind, so this is the input side and
/// [`Adapted`] is the answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Published {
    /// The object publishes nothing of this kind.
    Nothing,
    /// The object publishes an item store in one or both flavours.
    Items {
        /// The slot-addressed half.
        slotted: Option<Shelf<Stackable>>,
        /// The slotless half.
        slotless: Option<Pool<Stackable>>,
    },
    /// The object publishes a fluid store.
    Fluid(Shelf<KindOfFluid>),
    /// The object publishes a charge store.
    Charge(HostCharge),
}

/// The empty store a missing capability answers, of the adapter's own kind.
///
/// Section eight asks for an empty store, not an empty store of some other
/// kind: a dependent reading [`Store::kind`] off what a missing fluid capability
/// yielded has to be told the fluid kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Absent {
    /// The empty item store, which both item shapes answer.
    Items(Empty<Stackable>),
    /// The empty fluid store.
    Fluid(Empty<KindOfFluid>),
    /// The empty charge store.
    Charge(Empty<Scalar>),
}

impl Absent {
    /// The empty store of the kind this shape carries.
    #[must_use]
    pub fn of(shape: Shape) -> Self {
        match shape {
            Shape::Fluid => Self::Fluid(Empty::of(KindOfFluid::default())),
            Shape::Charge => Self::Charge(Empty::of(Scalar)),
            Shape::SlottedItem | Shape::SlotlessItem => {
                Self::Items(Empty::of(Stackable::default()))
            }
        }
    }

    /// The registered name of the kind this empty store carries.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Items(empty) => Store::kind(empty).name(),
            Self::Fluid(empty) => Store::kind(empty).name(),
            Self::Charge(empty) => Store::kind(empty).name(),
        }
    }
}

/// The store an adapter hands back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Adapted {
    /// The empty store, which is what a missing capability answers.
    Absent(Absent),
    /// An item store, combining whichever halves the object published.
    Items(Combined<Stackable>),
    /// A fluid store.
    Fluid(Shelf<KindOfFluid>),
    /// A charge store.
    Charge(HostCharge),
}

impl Adapted {
    /// Whether this is the empty store.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent(_))
    }
}

impl Adapter {
    /// Adapts what an object publishes into a store of this adapter's shape.
    ///
    /// A shape the object does not publish answers the empty store rather than
    /// an error, which section eight states plainly. The two item shapes differ
    /// in which half they require and agree on what they build: whichever halves
    /// the object published, combined, so an object publishing both gets the
    /// combined store's cheaper answers without asking for them.
    #[must_use]
    pub fn adapt(self, published: Published) -> Adapted {
        match (self.shape, published) {
            (Shape::SlottedItem, Published::Items { slotted, slotless }) if slotted.is_some() => {
                Adapted::Items(Combined::new(Stackable::default(), slotted, slotless))
            }
            (Shape::SlotlessItem, Published::Items { slotted, slotless }) if slotless.is_some() => {
                Adapted::Items(Combined::new(Stackable::default(), slotted, slotless))
            }
            (Shape::Fluid, Published::Fluid(store)) => Adapted::Fluid(store),
            (Shape::Charge, Published::Charge(host)) => Adapted::Charge(host),
            (Shape::SlottedItem | Shape::SlotlessItem | Shape::Fluid | Shape::Charge, _) => {
                Adapted::Absent(Absent::of(self.shape))
            }
        }
    }
}

/// Narrows a quantity to what the host's charge interface carries.
///
/// Answers the narrowed value and whether the ceiling was reached, so a caller
/// can tell a full transfer from a saturated one.
#[must_use]
pub fn narrowed(amount: Amount) -> (Amount, bool) {
    if amount > HOST_CHARGE_CEILING {
        (HOST_CHARGE_CEILING, true)
    } else {
        (amount, false)
    }
}

/// What a store refuses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refused {
    /// More slots than [`MAX_SLOTS`] were asked for.
    TooManySlots {
        /// How many were asked for.
        asked: usize,
        /// The published ceiling.
        limit: usize,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySlots { asked, limit } => {
                write!(formatter, "{asked} slots is past the {limit} admitted")
            }
        }
    }
}

impl std::error::Error for Refused {}

/// A store that holds nothing and accepts nothing.
///
/// Section eight: a missing capability yields an empty store, not an error.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Empty<K> {
    kind: K,
}

impl<K: Ingredient> Empty<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    /// The empty store for one kind.
    pub const fn of(kind: K) -> Self {
        Self { kind }
    }
}

impl<K: Ingredient> Store for Empty<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    type Kind = K;

    fn kind(&self) -> &K {
        &self.kind
    }

    fn insert(&mut self, _value: &K::Instance, _amount: Amount, _attempt: Attempt) -> Amount {
        0
    }

    fn extract(&mut self, _value: &K::Instance, _amount: Amount, _attempt: Attempt) -> Amount {
        0
    }
}

impl<K: Ingredient> SlottedStore for Empty<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    fn slots(&self) -> usize {
        0
    }

    fn insert_slotted(
        &mut self,
        _value: &K::Instance,
        _amount: Amount,
        _attempt: Attempt,
    ) -> Slotted {
        Slotted::NOTHING
    }
}

/// A slot-addressed host store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shelf<K: Ingredient>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    kind: K,
    per_slot: Amount,
    slots: Vec<Option<K::Instance>>,
}

impl<K: Ingredient> Shelf<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    /// Builds a shelf of the given slot count and per-slot capacity.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::TooManySlots`] past [`MAX_SLOTS`].
    pub fn new(kind: K, slots: usize, per_slot: Amount) -> Result<Self, Refused> {
        if slots > MAX_SLOTS {
            return Err(Refused::TooManySlots {
                asked: slots,
                limit: MAX_SLOTS,
            });
        }
        Ok(Self {
            kind,
            per_slot,
            slots: vec![None; slots],
        })
    }

    /// The capacity, which is the sum of the per-slot capacities.
    #[must_use]
    pub fn capacity(&self) -> Amount {
        self.per_slot
            .saturating_mul(Amount::try_from(self.slots.len()).unwrap_or(Amount::MAX))
    }

    /// What one slot holds.
    #[must_use]
    pub fn at(&self, slot: usize) -> Option<&K::Instance> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// Puts an instance in one slot, answering what was there.
    pub fn place(&mut self, slot: usize, value: Option<K::Instance>) -> Option<K::Instance> {
        self.slots
            .get_mut(slot)
            .and_then(|held| core::mem::replace(held, value))
    }

    /// Everything the shelf holds, in slot order.
    pub fn contents(&self) -> impl Iterator<Item = &K::Instance> + '_ {
        self.slots.iter().flatten()
    }

    /// Everything matching a prototype under the given conditions.
    ///
    /// This is the filtered iteration section eight wraps the host's contents
    /// with.
    pub fn filtered<'shelf>(
        &'shelf self,
        prototype: &'shelf K::Instance,
        under: Conditions,
    ) -> impl Iterator<Item = &'shelf K::Instance> + 'shelf {
        self.contents()
            .filter(move |held| self.kind.matches(held, prototype, under))
    }

    /// Extraction by prototype: all of it or none of it.
    ///
    /// Section eight: the walk accumulates across slots until the requested
    /// quantity is met and extracts for real only once the whole quantity is
    /// available, because a machine's ingredient reservation depends on a
    /// partial match not being committed.
    pub fn extract_under(
        &mut self,
        prototype: &K::Instance,
        amount: Amount,
        under: Conditions,
        attempt: Attempt,
    ) -> Amount {
        if amount == 0 {
            return 0;
        }
        let mut available = 0;
        let mut taken_from = Vec::new();
        for (index, held) in self.slots.iter().enumerate() {
            let Some(held) = held else { continue };
            if !self.kind.matches(held, prototype, under) {
                continue;
            }
            let here = self.kind.quantity(held).min(amount - available);
            if here == 0 {
                continue;
            }
            available += here;
            taken_from.push((index, here));
            if available == amount {
                break;
            }
        }
        if available < amount {
            return 0;
        }
        if attempt.changes_anything() {
            // Every remainder is rebuilt before any slot is written, so a kind
            // that refuses one leaves the shelf untouched and the refusal
            // reaches the caller as nothing taken.
            let mut rebuilds = Vec::with_capacity(taken_from.len());
            for (index, here) in taken_from {
                let Some(held) = self.slots.get(index).and_then(Clone::clone) else {
                    continue;
                };
                let left = self.kind.quantity(&held) - here;
                let rebuilt = if left == 0 {
                    None
                } else {
                    match self.kind.with_quantity(&held, left) {
                        Ok(remainder) => Some(remainder),
                        Err(_) => return 0,
                    }
                };
                rebuilds.push((index, rebuilt));
            }
            for (index, rebuilt) in rebuilds {
                self.slots[index] = rebuilt;
            }
        }
        amount
    }

    /// The partial extraction a caller that would rather take what is there
    /// asks for.
    ///
    /// The all-or-nothing rule is what section eight states for extraction by
    /// prototype, so this is a second method rather than a relaxation of the
    /// first.
    pub fn drain_available(
        &mut self,
        prototype: &K::Instance,
        amount: Amount,
        under: Conditions,
        attempt: Attempt,
    ) -> Amount {
        let mut taken = 0;
        for index in 0..self.slots.len() {
            if taken == amount {
                break;
            }
            let Some(held) = self.slots[index].clone() else {
                continue;
            };
            if !self.kind.matches(&held, prototype, under) {
                continue;
            }
            let here = self.kind.quantity(&held).min(amount - taken);
            if here == 0 {
                continue;
            }
            if attempt.changes_anything() {
                let left = self.kind.quantity(&held) - here;
                if left == 0 {
                    self.slots[index] = None;
                } else {
                    // A refused remainder leaves the slot alone and is not
                    // counted, rather than emptying a slot this call cannot
                    // rebuild.
                    let Ok(remainder) = self.kind.with_quantity(&held, left) else {
                        continue;
                    };
                    self.slots[index] = Some(remainder);
                }
            }
            taken += here;
        }
        taken
    }
}

impl<K: Ingredient> Store for Shelf<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    type Kind = K;

    fn kind(&self) -> &K {
        &self.kind
    }

    fn insert(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        self.insert_slotted(value, amount, attempt).amount
    }

    fn extract(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        self.extract_under(
            value,
            amount,
            self.kind.ignoring_quantity_conditions(),
            attempt,
        )
    }
}

impl<K: Ingredient> SlottedStore for Shelf<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    fn slots(&self) -> usize {
        self.slots.len()
    }

    fn insert_slotted(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Slotted {
        if amount == 0 {
            return Slotted::NOTHING;
        }
        let under = self.kind.ignoring_quantity_conditions();
        // A slot already holding the same thing first, an empty slot after.
        for index in 0..self.slots.len() {
            let Some(held) = self.slots[index].clone() else {
                continue;
            };
            if !self.kind.matches(&held, value, under) {
                continue;
            }
            let held_now = self.kind.quantity(&held);
            let room = self.per_slot.saturating_sub(held_now);
            // The rebuild is what commits, so the kind's own ceiling bounds what
            // may be reported as accepted: a slot limit above it would otherwise
            // answer a quantity the rebuild then refuses to store.
            let headroom = self.kind.maximum().saturating_sub(held_now);
            let accepted = room.min(amount).min(headroom);
            if accepted == 0 {
                continue;
            }
            if attempt.changes_anything() {
                let Ok(rebuilt) = self.kind.with_quantity(&held, held_now + accepted) else {
                    return Slotted::NOTHING;
                };
                self.slots[index] = Some(rebuilt);
            }
            return Slotted::at(index, accepted);
        }
        for index in 0..self.slots.len() {
            if self.slots[index].is_some() {
                continue;
            }
            let accepted = self.per_slot.min(amount).min(self.kind.maximum());
            if accepted == 0 {
                continue;
            }
            if attempt.changes_anything() {
                let Ok(placed) = self.kind.with_quantity(value, accepted) else {
                    return Slotted::NOTHING;
                };
                self.slots[index] = Some(placed);
            }
            return Slotted::at(index, accepted);
        }
        Slotted::NOTHING
    }
}

/// A slotless host store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool<K: Ingredient>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    kind: K,
    capacity: Amount,
    contents: Vec<K::Instance>,
}

impl<K: Ingredient> Pool<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    /// An empty pool of the given capacity.
    pub const fn new(kind: K, capacity: Amount) -> Self {
        Self {
            kind,
            capacity,
            contents: Vec::new(),
        }
    }

    /// The capacity, which is the whole store's rather than a slot's.
    #[must_use]
    pub const fn capacity(&self) -> Amount {
        self.capacity
    }

    /// How much is held in total.
    #[must_use]
    pub fn held(&self) -> Amount {
        self.contents
            .iter()
            .map(|held| self.kind.quantity(held))
            .fold(0, Amount::saturating_add)
    }

    /// Everything the pool holds, in the order it was added.
    pub fn contents(&self) -> impl Iterator<Item = &K::Instance> + '_ {
        self.contents.iter()
    }

    /// How many entries the pool holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.contents.len()
    }

    /// Whether the pool holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }
}

impl<K: Ingredient> Store for Pool<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    type Kind = K;

    fn kind(&self) -> &K {
        &self.kind
    }

    fn insert(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        let under = self.kind.ignoring_quantity_conditions();
        let found = self
            .contents
            .iter()
            .position(|held| self.kind.matches(held, value, under));
        // The entry ceiling is a property of the pool, not of the attempt, so a
        // simulation that answered a quantity the commit then refuses would
        // break a caller that reserves by simulating.
        if found.is_none() && self.contents.len() >= MAX_SLOTS {
            return 0;
        }
        let held_now = found.map_or(0, |index| self.kind.quantity(&self.contents[index]));
        let room = self.capacity.saturating_sub(self.held());
        // The kind's own ceiling bounds what may be reported: a capacity above
        // it would otherwise answer a quantity the rebuild refuses to store.
        let headroom = self.kind.maximum().saturating_sub(held_now);
        let accepted = room.min(amount).min(headroom);
        if accepted == 0 {
            return 0;
        }
        if attempt.changes_anything() {
            if let Some(index) = found {
                let held = self.contents[index].clone();
                let Ok(rebuilt) = self.kind.with_quantity(&held, held_now + accepted) else {
                    return 0;
                };
                self.contents[index] = rebuilt;
            } else {
                let Ok(placed) = self.kind.with_quantity(value, accepted) else {
                    return 0;
                };
                self.contents.push(placed);
            }
        }
        accepted
    }

    /// Slotless extraction, which is all-or-nothing for the same reason the
    /// slot-addressed one is.
    fn extract(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        if amount == 0 {
            return 0;
        }
        let under = self.kind.ignoring_quantity_conditions();
        let available: Amount = self
            .contents
            .iter()
            .filter(|held| self.kind.matches(held, value, under))
            .map(|held| self.kind.quantity(held))
            .fold(0, Amount::saturating_add);
        if available < amount {
            return 0;
        }
        if attempt.changes_anything() {
            let mut left = amount;
            let mut rebuilt = Vec::with_capacity(self.contents.len());
            for held in &self.contents {
                if left == 0 || !self.kind.matches(held, value, under) {
                    rebuilt.push(held.clone());
                    continue;
                }
                let here = self.kind.quantity(held).min(left);
                left -= here;
                let remaining = self.kind.quantity(held) - here;
                if remaining > 0 {
                    // The rebuilt list is local until it is complete, so a
                    // refused remainder answers nothing taken rather than
                    // dropping the entry it could not rebuild.
                    let Ok(entry) = self.kind.with_quantity(held, remaining) else {
                        return 0;
                    };
                    rebuilt.push(entry);
                }
            }
            self.contents = rebuilt;
        }
        amount
    }
}

/// The combined item store of section eight.
///
/// When an object publishes both a slot-addressed item store and a slotless one,
/// the slotless one answers iteration, filtering, capacity and unslotted access
/// and the slot-addressed one answers everything indexed, so a dependent gets
/// the cheaper answer without asking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Combined<K: Ingredient>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    kind: K,
    slotted: Option<Shelf<K>>,
    slotless: Option<Pool<K>>,
}

impl<K: Ingredient> Combined<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    /// Builds a combined store over whichever halves the object publishes.
    pub const fn new(kind: K, slotted: Option<Shelf<K>>, slotless: Option<Pool<K>>) -> Self {
        Self {
            kind,
            slotted,
            slotless,
        }
    }

    /// Whether the object publishes neither half, in which case every answer is
    /// the empty one.
    #[must_use]
    pub const fn absent(&self) -> bool {
        self.slotted.is_none() && self.slotless.is_none()
    }

    /// Whether iteration reads the slotless half.
    #[must_use]
    pub const fn iterates_slotless(&self) -> bool {
        self.slotless.is_some()
    }

    /// The capacity, from the slotless half when there is one.
    #[must_use]
    pub fn capacity(&self) -> Amount {
        self.slotless.as_ref().map_or_else(
            || self.slotted.as_ref().map_or(0, Shelf::capacity),
            Pool::capacity,
        )
    }

    /// Everything held, from the slotless half when there is one.
    #[must_use]
    pub fn contents(&self) -> Vec<K::Instance> {
        if let Some(slotless) = &self.slotless {
            return slotless.contents().cloned().collect();
        }
        self.slotted
            .as_ref()
            .map_or_else(Vec::new, |slotted| slotted.contents().cloned().collect())
    }

    /// Everything held matching a prototype, from the slotless half when there
    /// is one.
    #[must_use]
    pub fn filtered(&self, prototype: &K::Instance, under: Conditions) -> Vec<K::Instance> {
        self.contents()
            .into_iter()
            .filter(|held| self.kind.matches(held, prototype, under))
            .collect()
    }

    /// How many slots there are, which only the slot-addressed half can answer.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.slotted.as_ref().map_or(0, SlottedStore::slots)
    }

    /// What one slot holds, which only the slot-addressed half can answer.
    #[must_use]
    pub fn at(&self, slot: usize) -> Option<&K::Instance> {
        self.slotted.as_ref().and_then(|shelf| shelf.at(slot))
    }
}

impl<K: Ingredient> Store for Combined<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    type Kind = K;

    fn kind(&self) -> &K {
        &self.kind
    }

    fn insert(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        if let Some(slotless) = &mut self.slotless {
            return slotless.insert(value, amount, attempt);
        }
        if let Some(slotted) = &mut self.slotted {
            return slotted.insert(value, amount, attempt);
        }
        0
    }

    fn extract(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        if let Some(slotless) = &mut self.slotless {
            return slotless.extract(value, amount, attempt);
        }
        if let Some(slotted) = &mut self.slotted {
            return slotted.extract(value, amount, attempt);
        }
        0
    }
}

/// A kind's slotless store presented as a host store.
///
/// Section eight records that the original reports **one more slot than it
/// holds**, so that the last index always reads empty and can be inserted into:
/// a host store interface has no insert-anywhere operation, and the phantom slot
/// is the only way to expose one. RVL-210's [`Store`] has a slotless insert, so
/// there is nothing here for a phantom slot to work around and this reports the
/// entries it holds and no more. Section eight says in as many words that a
/// counterpart with a slotless insert should not have one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsHost<K: Ingredient>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    source: Pool<K>,
}

impl<K: Ingredient> AsHost<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    /// Wraps a slotless source.
    pub const fn wrapping(source: Pool<K>) -> Self {
        Self { source }
    }

    /// The slot count the host sees.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.source.len()
    }

    /// What the host reads at one index.
    #[must_use]
    pub fn at(&self, slot: usize) -> Option<&K::Instance> {
        self.source.contents().nth(slot)
    }

    /// The source, so the wrapping is reversible.
    #[must_use]
    pub const fn source(&self) -> &Pool<K> {
        &self.source
    }

    /// The source, for an insertion that reaches the slotless operation.
    pub const fn source_mut(&mut self) -> &mut Pool<K> {
        &mut self.source
    }
}

/// The charge store the host publishes, which carries 32-bit quantities.
///
/// Every insertion and extraction crosses this boundary, and this is where
/// defect ten's narrowing is made explicit: the amount saturates at
/// [`HOST_CHARGE_CEILING`] and [`HostCharge::saturated`] answers whether it did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostCharge {
    held: Amount,
    capacity: Amount,
    saturated: bool,
}

impl HostCharge {
    /// An empty charge store of the given capacity, itself narrowed.
    #[must_use]
    pub fn new(capacity: Amount) -> Self {
        let (capacity, saturated) = narrowed(capacity);
        Self {
            held: 0,
            capacity,
            saturated,
        }
    }

    /// How much is held.
    #[must_use]
    pub const fn held(&self) -> Amount {
        self.held
    }

    /// The capacity the host carries.
    #[must_use]
    pub const fn capacity(&self) -> Amount {
        self.capacity
    }

    /// Whether any quantity was saturated at the host boundary.
    #[must_use]
    pub const fn saturated(&self) -> bool {
        self.saturated
    }
}

impl Store for HostCharge {
    type Kind = Scalar;

    fn kind(&self) -> &Scalar {
        &Scalar
    }

    fn insert(&mut self, _value: &Charge, amount: Amount, attempt: Attempt) -> Amount {
        let (amount, saturated) = narrowed(amount);
        let accepted = self.capacity.saturating_sub(self.held).min(amount);
        if attempt.changes_anything() {
            self.held += accepted;
            self.saturated |= saturated;
        }
        accepted
    }

    fn extract(&mut self, _value: &Charge, amount: Amount, attempt: Attempt) -> Amount {
        let (amount, saturated) = narrowed(amount);
        let taken = self.held.min(amount);
        if attempt.changes_anything() {
            self.held -= taken;
            self.saturated |= saturated;
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{Fluid, Portion, Stack};
    use rvl_210_counterpart::quantity::QuantityKind;

    fn shelf(slots: usize, per_slot: Amount) -> Shelf<Stackable> {
        Shelf::new(Stackable::default(), slots, per_slot).expect("inside the ceiling")
    }

    fn pool(capacity: Amount) -> Pool<Stackable> {
        Pool::new(Stackable::default(), capacity)
    }

    fn under() -> Conditions {
        Stackable::default().ignoring_quantity_conditions()
    }

    #[test]
    fn a_pool_never_reports_more_than_the_kind_can_hold() {
        // The rebuild is what commits, and the stackable kind refuses a count
        // past its maximum, so a capacity above that range must not answer a
        // quantity the pool then does not hold.
        let mut pool = pool(Amount::MAX);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        let accepted = pool.insert(&stone, 3_000_000_000, Attempt::Real);
        assert_eq!(accepted, crate::kinds::NARROW_MAXIMUM);
        assert_eq!(pool.held(), accepted);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn a_shelf_never_reports_more_than_the_kind_can_hold() {
        let mut shelf = shelf(1, Amount::MAX);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        let placed = shelf.insert_slotted(&stone, 3_000_000_000, Attempt::Real);
        assert_eq!(placed, Slotted::at(0, crate::kinds::NARROW_MAXIMUM));
        assert_eq!(
            shelf.at(0).map(|held| held.count),
            Some(crate::kinds::NARROW_MAXIMUM)
        );
    }

    #[test]
    fn a_shelf_merging_into_a_full_slot_reports_nothing_rather_than_the_room() {
        let mut shelf = shelf(1, Amount::MAX);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(
            shelf.insert(&stone, crate::kinds::NARROW_MAXIMUM, Attempt::Real),
            crate::kinds::NARROW_MAXIMUM
        );
        assert_eq!(shelf.insert(&stone, 10, Attempt::Real), 0);
        assert_eq!(
            shelf.at(0).map(|held| held.count),
            Some(crate::kinds::NARROW_MAXIMUM)
        );
    }

    #[test]
    fn a_full_pool_refuses_a_new_entry_the_same_way_simulated_and_real() {
        let mut pool = pool(Amount::MAX);
        for index in 0..MAX_SLOTS {
            let entry = Stack::of(
                format!("minecraft:k{index}")
                    .parse()
                    .expect("a valid content identity"),
                1,
            );
            assert_eq!(pool.insert(&entry, 1, Attempt::Real), 1);
        }
        let extra = Stack::of(
            "minecraft:one_too_many"
                .parse()
                .expect("a valid content identity"),
            1,
        );
        assert_eq!(pool.insert(&extra, 5, Attempt::Simulated), 0);
        assert_eq!(pool.insert(&extra, 5, Attempt::Real), 0);
    }

    #[test]
    fn a_missing_capability_answers_an_empty_store_of_the_adapters_own_kind() {
        let published = Published::Items {
            slotted: None,
            slotless: None,
        };
        for (shape, kind) in [
            (Shape::Fluid, PORTION),
            (Shape::Charge, SCALAR),
            (Shape::SlottedItem, STACKABLE),
            (Shape::SlotlessItem, STACKABLE),
        ] {
            let adapted = Adapter {
                shape,
                class: Class::Block,
            }
            .adapt(published.clone());
            let Adapted::Absent(absent) = adapted else {
                panic!("a shape the object does not publish answers the empty store");
            };
            assert_eq!(absent.kind(), kind);
        }
    }

    #[test]
    fn twelve_adapters_are_installed_four_shapes_on_each_of_three_classes() {
        let table = table();
        assert_eq!(table.len(), 12);
        for shape in Shape::ALL {
            for class in Class::ALL {
                assert!(installed(shape, class), "{shape:?} on {class:?}");
            }
        }
        let mut sorted = table;
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 12);
    }

    #[test]
    fn the_two_item_shapes_name_one_kind_and_the_other_two_name_theirs() {
        assert_eq!(Shape::SlottedItem.kind(), STACKABLE);
        assert_eq!(Shape::SlotlessItem.kind(), STACKABLE);
        assert_eq!(Shape::Fluid.kind(), PORTION);
        assert_eq!(Shape::Charge.kind(), SCALAR);
        assert!(Shape::SlottedItem.slotted());
        assert!(!Shape::SlotlessItem.slotted());
        assert_eq!(Class::Block.as_str(), "block");
    }

    #[test]
    fn an_adapter_over_the_shape_its_object_publishes_hands_back_that_store() {
        let mut shelf = shelf(2, 64);
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                3,
            )),
        );
        let adapted = Adapter {
            shape: Shape::SlottedItem,
            class: Class::Block,
        }
        .adapt(Published::Items {
            slotted: Some(shelf),
            slotless: None,
        });
        let Adapted::Items(combined) = adapted else {
            panic!("a slot-addressed item store was published");
        };
        assert_eq!(combined.slots(), 2);
        assert_eq!(combined.at(0).map(|held| held.count), Some(3));
    }

    #[test]
    fn an_item_adapter_over_both_halves_builds_the_combined_store() {
        let mut pool = pool(1_000);
        pool.insert(
            &Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                1,
            ),
            20,
            Attempt::Real,
        );
        let published = Published::Items {
            slotted: Some(shelf(2, 64)),
            slotless: Some(pool),
        };
        for shape in [Shape::SlottedItem, Shape::SlotlessItem] {
            let adapted = Adapter {
                shape,
                class: Class::Entity,
            }
            .adapt(published.clone());
            let Adapted::Items(combined) = adapted else {
                panic!("both halves were published");
            };
            assert!(combined.iterates_slotless());
            assert_eq!(combined.capacity(), 1_000);
            assert_eq!(combined.slots(), 2);
        }
    }

    #[test]
    fn an_adapter_whose_half_the_object_does_not_publish_answers_the_empty_store() {
        let only_slotless = Published::Items {
            slotted: None,
            slotless: Some(pool(1_000)),
        };
        assert!(
            Adapter {
                shape: Shape::SlottedItem,
                class: Class::Item
            }
            .adapt(only_slotless.clone())
            .is_absent()
        );
        assert!(
            !Adapter {
                shape: Shape::SlotlessItem,
                class: Class::Item
            }
            .adapt(only_slotless)
            .is_absent()
        );
    }

    #[test]
    fn every_adapter_over_an_object_publishing_nothing_answers_the_empty_store() {
        for adapter in table() {
            assert!(adapter.adapt(Published::Nothing).is_absent(), "{adapter:?}");
        }
    }

    #[test]
    fn an_adapter_of_one_shape_does_not_take_another_shapes_store() {
        assert!(
            Adapter {
                shape: Shape::Fluid,
                class: Class::Block
            }
            .adapt(Published::Charge(HostCharge::new(100)))
            .is_absent()
        );
        assert!(
            !Adapter {
                shape: Shape::Charge,
                class: Class::Block
            }
            .adapt(Published::Charge(HostCharge::new(100)))
            .is_absent()
        );
        let fluid = Shelf::new(KindOfFluid::default(), 1, 1_000).expect("one tank");
        assert!(
            !Adapter {
                shape: Shape::Fluid,
                class: Class::Block
            }
            .adapt(Published::Fluid(fluid))
            .is_absent()
        );
    }

    #[test]
    fn a_missing_capability_yields_an_empty_store_rather_than_an_error() {
        let mut empty = Empty::of(Stackable::default());
        let stack = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            4,
        );
        assert_eq!(empty.insert(&stack, 4, Attempt::Real), 0);
        assert_eq!(empty.extract(&stack, 4, Attempt::Real), 0);
        assert_eq!(empty.slots(), 0);
        assert_eq!(
            empty.insert_slotted(&stack, 4, Attempt::Real),
            Slotted::NOTHING
        );
    }

    #[test]
    fn a_shelf_capacity_is_the_sum_of_its_per_slot_capacities() {
        assert_eq!(shelf(4, 64).capacity(), 256);
        assert_eq!(shelf(0, 64).capacity(), 0);
    }

    #[test]
    fn a_shelf_past_the_slot_ceiling_is_refused_before_it_allocates() {
        assert_eq!(
            Shelf::new(Stackable::default(), MAX_SLOTS + 1, 64),
            Err(Refused::TooManySlots {
                asked: MAX_SLOTS + 1,
                limit: MAX_SLOTS
            })
        );
        assert!(Shelf::new(Stackable::default(), MAX_SLOTS, 64).is_ok());
    }

    #[test]
    fn an_insertion_fills_a_matching_slot_before_an_empty_one() {
        let mut shelf = shelf(3, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            10,
        );
        assert_eq!(
            shelf.insert_slotted(&stone, 10, Attempt::Real),
            Slotted::at(0, 10)
        );
        assert_eq!(
            shelf.insert_slotted(&stone, 20, Attempt::Real),
            Slotted::at(0, 20)
        );
        assert_eq!(shelf.at(0).map(|held| held.count), Some(30));
        assert_eq!(shelf.at(1), None);
    }

    #[test]
    fn an_insertion_is_capped_by_the_per_slot_capacity() {
        let mut shelf = shelf(2, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(shelf.insert(&stone, 100, Attempt::Real), 64);
        assert_eq!(shelf.at(0).map(|held| held.count), Some(64));
        assert_eq!(shelf.insert(&stone, 100, Attempt::Real), 64);
        assert_eq!(shelf.insert(&stone, 100, Attempt::Real), 0);
    }

    #[test]
    fn a_simulated_insertion_answers_the_same_number_and_changes_nothing() {
        let mut shelf = shelf(2, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(shelf.insert(&stone, 30, Attempt::Simulated), 30);
        assert_eq!(shelf.at(0), None);
        assert_eq!(shelf.insert(&stone, 30, Attempt::Real), 30);
    }

    #[test]
    fn extraction_by_prototype_accumulates_across_slots() {
        let mut shelf = shelf(3, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                5,
            )),
        );
        shelf.place(
            1,
            Some(Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                50,
            )),
        );
        shelf.place(
            2,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                7,
            )),
        );
        assert_eq!(shelf.extract_under(&stone, 12, under(), Attempt::Real), 12);
        assert_eq!(shelf.at(0), None);
        assert_eq!(shelf.at(2), None);
        assert_eq!(shelf.at(1).map(|held| held.count), Some(50));
    }

    #[test]
    fn a_partial_match_is_not_committed() {
        let mut shelf = shelf(3, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                5,
            )),
        );
        shelf.place(
            2,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                4,
            )),
        );
        assert_eq!(shelf.extract_under(&stone, 12, under(), Attempt::Real), 0);
        assert_eq!(shelf.at(0).map(|held| held.count), Some(5));
        assert_eq!(shelf.at(2).map(|held| held.count), Some(4));
    }

    #[test]
    fn a_partial_extraction_is_available_for_a_caller_that_wants_one() {
        let mut shelf = shelf(3, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                5,
            )),
        );
        assert_eq!(
            shelf.drain_available(&stone, 12, under(), Attempt::Simulated),
            5
        );
        assert_eq!(shelf.at(0).map(|held| held.count), Some(5));
        assert_eq!(shelf.drain_available(&stone, 12, under(), Attempt::Real), 5);
        assert_eq!(shelf.at(0), None);
    }

    #[test]
    fn extraction_leaves_a_remainder_where_one_slot_holds_more_than_asked() {
        let mut shelf = shelf(2, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                20,
            )),
        );
        assert_eq!(shelf.extract_under(&stone, 8, under(), Attempt::Real), 8);
        assert_eq!(shelf.at(0).map(|held| held.count), Some(12));
    }

    #[test]
    fn a_simulated_extraction_commits_nothing() {
        let mut shelf = shelf(2, 64);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                20,
            )),
        );
        assert_eq!(
            shelf.extract_under(&stone, 8, under(), Attempt::Simulated),
            8
        );
        assert_eq!(shelf.at(0).map(|held| held.count), Some(20));
    }

    #[test]
    fn extraction_of_nothing_takes_nothing() {
        let mut shelf = shelf(2, 64);
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                20,
            )),
        );
        assert_eq!(
            shelf.extract_under(
                &Stack::of(
                    "minecraft:stone".parse().expect("a valid content identity"),
                    1
                ),
                0,
                under(),
                Attempt::Real
            ),
            0
        );
        assert_eq!(shelf.at(0).map(|held| held.count), Some(20));
    }

    #[test]
    fn the_filtered_walk_yields_only_what_matches_the_prototype() {
        let mut shelf = shelf(3, 64);
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                5,
            )),
        );
        shelf.place(
            1,
            Some(Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                5,
            )),
        );
        shelf.place(
            2,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                9,
            )),
        );
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        let kind = Stackable::default();
        let matched: Vec<Amount> = shelf
            .contents()
            .filter(|held| kind.matches(held, &stone, under()))
            .map(|held| held.count)
            .collect();
        assert_eq!(matched, vec![5, 9]);
    }

    #[test]
    fn the_shelfs_filtered_walk_wraps_its_contents_with_the_matcher() {
        let mut shelf = shelf(3, 64);
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                5,
            )),
        );
        shelf.place(
            1,
            Some(Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                5,
            )),
        );
        shelf.place(
            2,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                9,
            )),
        );
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        let matched: Vec<Amount> = shelf
            .filtered(&stone, under())
            .map(|held| held.count)
            .collect();
        assert_eq!(matched, vec![5, 9]);
        let dirt = Stack::of(
            "minecraft:gold_ingot"
                .parse()
                .expect("a valid content identity"),
            1,
        );
        assert_eq!(shelf.filtered(&dirt, under()).count(), 0);
    }

    #[test]
    fn a_pool_holds_one_entry_per_distinct_instance_and_reports_its_total() {
        let mut pool = pool(1_000);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(pool.insert(&stone, 40, Attempt::Real), 40);
        assert_eq!(pool.insert(&stone, 40, Attempt::Real), 40);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.held(), 80);
        assert_eq!(
            pool.insert(
                &Stack::of(
                    "minecraft:dirt".parse().expect("a valid content identity"),
                    1
                ),
                5,
                Attempt::Real
            ),
            5
        );
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn a_pool_insertion_is_capped_by_the_whole_stores_capacity() {
        let mut pool = pool(50);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(pool.insert(&stone, 80, Attempt::Real), 50);
        assert_eq!(pool.insert(&stone, 1, Attempt::Real), 0);
        assert_eq!(pool.capacity(), 50);
    }

    #[test]
    fn a_pool_extraction_is_all_or_nothing_too() {
        let mut pool = pool(1_000);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        pool.insert(&stone, 10, Attempt::Real);
        assert_eq!(pool.extract(&stone, 40, Attempt::Real), 0);
        assert_eq!(pool.held(), 10);
        assert_eq!(pool.extract(&stone, 10, Attempt::Real), 10);
        assert!(pool.is_empty());
    }

    #[test]
    fn the_combined_store_reads_the_slotless_half_and_indexes_the_slotted_one() {
        let mut shelf = shelf(2, 64);
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                3,
            )),
        );
        let mut pool = pool(1_000);
        pool.insert(
            &Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                1,
            ),
            20,
            Attempt::Real,
        );
        let combined = Combined::new(Stackable::default(), Some(shelf), Some(pool));
        assert!(combined.iterates_slotless());
        assert_eq!(combined.capacity(), 1_000);
        assert_eq!(combined.contents().len(), 1);
        assert_eq!(
            combined.contents()[0].identity.to_string(),
            "minecraft:dirt"
        );
        assert_eq!(combined.slots(), 2);
        assert_eq!(combined.at(0).map(|held| held.count), Some(3));
    }

    #[test]
    fn the_combined_store_falls_back_to_the_slotted_half_when_there_is_no_other() {
        let mut shelf = shelf(2, 64);
        shelf.place(
            0,
            Some(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                3,
            )),
        );
        let combined = Combined::new(Stackable::default(), Some(shelf), None);
        assert!(!combined.iterates_slotless());
        assert_eq!(combined.capacity(), 128);
        assert_eq!(combined.contents().len(), 1);
        assert_eq!(
            combined
                .filtered(
                    &Stack::of(
                        "minecraft:stone".parse().expect("a valid content identity"),
                        1
                    ),
                    under()
                )
                .len(),
            1
        );
        assert_eq!(
            combined
                .filtered(
                    &Stack::of(
                        "minecraft:dirt".parse().expect("a valid content identity"),
                        1
                    ),
                    under()
                )
                .len(),
            0
        );
    }

    #[test]
    fn a_combined_store_over_neither_half_answers_the_empty_answer_everywhere() {
        let mut combined: Combined<Stackable> = Combined::new(Stackable::default(), None, None);
        assert!(combined.absent());
        assert_eq!(combined.capacity(), 0);
        assert_eq!(combined.slots(), 0);
        assert_eq!(combined.contents().len(), 0);
        assert_eq!(combined.at(0), None);
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(combined.insert(&stone, 5, Attempt::Real), 0);
        assert_eq!(combined.extract(&stone, 5, Attempt::Real), 0);
    }

    #[test]
    fn a_slotless_source_wrapped_as_a_host_store_reports_no_phantom_slot() {
        let mut pool = pool(1_000);
        pool.insert(
            &Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                1,
            ),
            5,
            Attempt::Real,
        );
        pool.insert(
            &Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                1,
            ),
            5,
            Attempt::Real,
        );
        let wrapped = AsHost::wrapping(pool);
        assert_eq!(wrapped.slots(), 2);
        assert!(wrapped.at(2).is_none());
        assert_eq!(wrapped.source().len(), 2);
    }

    #[test]
    fn an_empty_slotless_source_wrapped_reports_no_slots_at_all() {
        let wrapped = AsHost::wrapping(pool(1_000));
        assert_eq!(wrapped.slots(), 0);
        assert!(wrapped.at(0).is_none());
    }

    #[test]
    fn a_wrapped_slotless_source_still_takes_an_insertion() {
        let mut wrapped = AsHost::wrapping(pool(1_000));
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(wrapped.source_mut().insert(&stone, 7, Attempt::Real), 7);
        assert_eq!(wrapped.slots(), 1);
    }

    #[test]
    fn the_charge_boundary_saturates_and_says_that_it_did() {
        assert_eq!(narrowed(5), (5, false));
        assert_eq!(narrowed(HOST_CHARGE_CEILING), (HOST_CHARGE_CEILING, false));
        assert_eq!(
            narrowed(HOST_CHARGE_CEILING + 1),
            (HOST_CHARGE_CEILING, true)
        );
        assert_eq!(
            narrowed(crate::kinds::WIDE_MAXIMUM),
            (HOST_CHARGE_CEILING, true)
        );
    }

    #[test]
    fn a_charge_insertion_above_the_host_ceiling_saturates_rather_than_wrapping() {
        let mut host = HostCharge::new(crate::kinds::WIDE_MAXIMUM);
        assert!(host.saturated());
        assert_eq!(host.capacity(), HOST_CHARGE_CEILING);
        let accepted = host.insert(&Charge::of(1), crate::kinds::WIDE_MAXIMUM, Attempt::Real);
        assert_eq!(accepted, HOST_CHARGE_CEILING);
        assert_eq!(host.held(), HOST_CHARGE_CEILING);
    }

    #[test]
    fn a_charge_transfer_inside_the_host_range_is_not_marked_saturated() {
        let mut host = HostCharge::new(1_000);
        assert!(!host.saturated());
        assert_eq!(host.insert(&Charge::of(1), 400, Attempt::Real), 400);
        assert_eq!(host.extract(&Charge::of(1), 150, Attempt::Real), 150);
        assert_eq!(host.held(), 250);
        assert!(!host.saturated());
    }

    #[test]
    fn a_simulated_charge_transfer_changes_nothing() {
        let mut host = HostCharge::new(1_000);
        assert_eq!(host.insert(&Charge::of(1), 400, Attempt::Simulated), 400);
        assert_eq!(host.held(), 0);
        assert_eq!(host.extract(&Charge::of(1), 400, Attempt::Simulated), 0);
    }

    #[test]
    fn a_fluid_shelf_carries_the_fluid_kind_through_the_same_adapters() {
        let mut shelf = Shelf::new(Fluid::default(), 2, 1_000).expect("inside the ceiling");
        let water = Portion::of(
            "minecraft:water".parse().expect("a valid fluid identity"),
            1,
        );
        assert_eq!(shelf.insert(&water, 600, Attempt::Real), 600);
        assert_eq!(shelf.insert(&water, 600, Attempt::Real), 400);
        assert_eq!(shelf.capacity(), 2_000);
        assert_eq!(
            shelf.extract_under(
                &water,
                1_000,
                Fluid::default().ignoring_quantity_conditions(),
                Attempt::Real
            ),
            1_000
        );
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            Refused::TooManySlots { asked: 9, limit: 8 }
                .to_string()
                .contains("past the 8")
        );
    }
}
