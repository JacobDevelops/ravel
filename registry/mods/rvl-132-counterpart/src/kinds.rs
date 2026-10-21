//! Sections four and six: the three ingredient kinds and how they match, hash
//! and order.
//!
//! This is the part dependents depend on most closely. A machine's filter, its
//! search index and its sorted display all run through what is here, so every
//! rule in section six is a compatibility surface rather than an implementation
//! choice.
//!
//! The vocabulary these three are declared into is owned by RVL-210's
//! counterpart and consumed from it: [`QuantityKind`], [`Conditions`],
//! [`Amount`] and [`KindError`] are all its types and none is redeclared here.
//! What this module adds is [`Ingredient`], the extra promises section six asks
//! of a kind that RVL-210's abstraction does not ask of every kind: a quantity
//! that can be read and rebuilt, a total order, and a decomposition into
//! category types.

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

use ravel_sdk::{JsonValue, content::ContentId};
use rvl_210_counterpart::quantity::{Amount, Conditions, KindError, QuantityKind};

use crate::data::{Comparator, Components};
use crate::forms::{Tag, TagError};

/// The name the stackable-item kind is registered under.
///
/// Section four records that the original registers all three in the game's own
/// namespace. A Ravel package declares its own names and never the host's, which
/// is the same rule section 13.2 blocks the original's host-namespace placement
/// under, so these three sit in this package's namespace. Section 17 leaves the
/// original's three spellings unresolved and section four states plainly that a
/// counterpart which need not be save-compatible picks its own.
pub const STACKABLE: &str = "rvl_132_counterpart:stackable";

/// The name the fluid-portion kind is registered under.
pub const PORTION: &str = "rvl_132_counterpart:portion";

/// The name the scalar-charge kind is registered under.
pub const SCALAR: &str = "rvl_132_counterpart:scalar";

/// The three names, in the order they are registered.
pub const REGISTERED: [&str; 3] = [STACKABLE, PORTION, SCALAR];

/// The largest quantity a stackable item or a fluid portion may carry.
///
/// Section 6.2: both maxima are the largest 32-bit signed value.
pub const NARROW_MAXIMUM: Amount = 0x7fff_ffff;

/// The largest quantity a scalar charge may carry.
///
/// Section 6.2: the largest 64-bit signed value. Defects three and four are both
/// about values that live above the low 32 bits of this range, so the range is
/// what makes them reachable.
pub const WIDE_MAXIMUM: Amount = i64::MAX.cast_unsigned();

/// One category type: an axis a kind decomposes into.
///
/// Section four counts seven of these across the three kinds and says dependents
/// index by them, so the count and the roles are behaviour.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Category {
    /// What the axis is called.
    pub name: &'static str,
    /// Whether it is the kind's primary axis.
    pub primary: bool,
    /// The condition slot that selects it.
    pub slot: u32,
    /// Whether it is the quantitative axis.
    pub quantitative: bool,
}

/// What an instance projects onto one of its category types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Projection {
    /// A name, which is what an identity axis projects onto.
    Named(ContentId),
    /// A quantity.
    Counted(Amount),
    /// A data map.
    Detailed(Components),
}

/// A set of match conditions, as section 6.1 describes the arithmetic.
///
/// Two flavours exist and both are reached through this one trait, because
/// section 6.1's whole point is that a dependent writes one filter for all three
/// kinds.
pub trait ConditionSet: Copy + Eq {
    /// The union of two sets.
    #[must_use]
    fn combine(self, other: Self) -> Self;

    /// This set with the other removed.
    #[must_use]
    fn remove(self, other: Self) -> Self;

    /// Whether this set answers to the other.
    #[must_use]
    fn test(self, other: Self) -> bool;
}

/// The flag-set flavour the item and fluid kinds carry.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Flags(Conditions);

impl Flags {
    /// The empty set, under which every instance of a kind is alike.
    pub const NONE: Self = Self(Conditions::NONE);

    /// Builds a set from RVL-210's condition set.
    #[must_use]
    pub const fn of(conditions: Conditions) -> Self {
        Self(conditions)
    }

    /// A set naming one slot.
    #[must_use]
    pub const fn slot(slot: u32) -> Self {
        Self(Conditions::NONE.with(slot))
    }

    /// The conditions this set names, as RVL-210 spells them.
    #[must_use]
    pub const fn conditions(self) -> Conditions {
        self.0
    }

    /// The raw bits, which is what section seven's condition form carries.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0.bits()
    }

    /// Builds a set from the bits section seven's condition form carries.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(Conditions::from_bits(bits))
    }
}

impl ConditionSet for Flags {
    fn combine(self, other: Self) -> Self {
        Self(self.0.union(other.0))
    }

    fn remove(self, other: Self) -> Self {
        Self(Conditions::from_bits(self.0.bits() & !other.0.bits()))
    }

    /// A non-empty bitwise and.
    ///
    /// Section 6.1 states this exactly: the answer is yes when **any** requested
    /// flag is present rather than when all are, which is what makes a partial
    /// filter widen rather than narrow.
    fn test(self, other: Self) -> bool {
        self.0.bits() & other.0.bits() != 0
    }
}

/// The single-truth flavour the charge kind carries.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Truth(bool);

impl Truth {
    /// The set asking for nothing.
    pub const NONE: Self = Self(false);

    /// The set asking for the kind's one condition.
    pub const SET: Self = Self(true);

    /// Whether the one condition is asked for.
    #[must_use]
    pub const fn held(self) -> bool {
        self.0
    }

    /// The byte section seven's condition form carries.
    #[must_use]
    pub const fn byte(self) -> i8 {
        if self.0 { 1 } else { 0 }
    }
}

impl ConditionSet for Truth {
    fn combine(self, other: Self) -> Self {
        Self(self.0 || other.0)
    }

    /// The boolean and-not.
    ///
    /// Defect five: the original returns false when the two agree and the
    /// original otherwise, so removing a condition it does not have leaves the
    /// condition set. Removal is then not the inverse of addition and the
    /// flag-set kinds have no such asymmetry. This is and-not, which is what
    /// [`Flags::remove`] does with bits.
    fn remove(self, other: Self) -> Self {
        Self(self.0 && !other.0)
    }

    fn test(self, other: Self) -> bool {
        self.0 == other.0
    }
}

/// A stackable item: an identity, a count and its data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stack {
    /// What item it is; always a valid namespaced identity.
    pub identity: ContentId,
    /// How many. Zero alone denotes absence.
    pub count: Amount,
    /// The typed data it carries.
    pub data: Components,
}

impl Default for Stack {
    fn default() -> Self {
        Self::of(
            "minecraft:air"
                .parse()
                .expect("a valid static item identity"),
            0,
        )
    }
}

impl Stack {
    /// The identity axis's condition slot.
    pub const IDENTITY: u32 = 0;
    /// The count axis's condition slot.
    pub const COUNT: u32 = 1;
    /// The data axis's condition slot.
    pub const DATA: u32 = 2;

    /// The kind's three category types.
    pub const CATEGORIES: [Category; 3] = [
        Category {
            name: "identity",
            primary: true,
            slot: Self::IDENTITY,
            quantitative: false,
        },
        Category {
            name: "count",
            primary: false,
            slot: Self::COUNT,
            quantitative: true,
        },
        Category {
            name: "data",
            primary: false,
            slot: Self::DATA,
            quantitative: false,
        },
    ];

    /// A stack of one named item.
    #[must_use]
    pub fn of(identity: ContentId, count: Amount) -> Self {
        Self {
            identity,
            count,
            data: Components::new(),
        }
    }

    /// The empty stack.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether this is the empty stack.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The same stack carrying the given data.
    #[must_use]
    pub fn carrying(mut self, data: Components) -> Self {
        self.data = data;
        self
    }
}

/// A fluid portion: a fluid, a volume and its data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Portion {
    /// What fluid it is; always a valid namespaced identity.
    pub fluid: ContentId,
    /// How much. Zero alone denotes absence.
    pub volume: Amount,
    /// The typed data it carries.
    pub data: Components,
}

impl Default for Portion {
    fn default() -> Self {
        Self::of(
            "minecraft:empty"
                .parse()
                .expect("a valid static fluid identity"),
            0,
        )
    }
}

impl Portion {
    /// The fluid axis's condition slot.
    pub const FLUID: u32 = 0;
    /// The volume axis's condition slot.
    pub const VOLUME: u32 = 1;
    /// The data axis's condition slot.
    pub const DATA: u32 = 2;

    /// The kind's three category types.
    pub const CATEGORIES: [Category; 3] = [
        Category {
            name: "fluid",
            primary: true,
            slot: Self::FLUID,
            quantitative: false,
        },
        Category {
            name: "volume",
            primary: false,
            slot: Self::VOLUME,
            quantitative: true,
        },
        Category {
            name: "data",
            primary: false,
            slot: Self::DATA,
            quantitative: false,
        },
    ];

    /// A portion of one named fluid.
    #[must_use]
    pub fn of(fluid: ContentId, volume: Amount) -> Self {
        Self {
            fluid,
            volume,
            data: Components::new(),
        }
    }

    /// The empty portion.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether this is the empty portion.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.volume == 0
    }

    /// The same portion carrying the given data.
    #[must_use]
    pub fn carrying(mut self, data: Components) -> Self {
        self.data = data;
        self
    }
}

/// A scalar charge: nothing but its quantity.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Charge {
    /// How much charge.
    pub amount: Amount,
}

impl Charge {
    /// The charge axis's condition slot.
    pub const CHARGE: u32 = 0;

    /// The kind's one category type.
    pub const CATEGORIES: [Category; 1] = [Category {
        name: "charge",
        primary: true,
        slot: Self::CHARGE,
        quantitative: true,
    }];

    /// A charge of the given size.
    #[must_use]
    pub const fn of(amount: Amount) -> Self {
        Self { amount }
    }
}

/// What section six asks of an ingredient kind beyond RVL-210's abstraction.
///
/// A kind reports a quantity, can be rebuilt with a different one, orders its
/// instances totally, and decomposes into category types. Every one of those is
/// something a dependent reads and would break without.
pub trait Ingredient: QuantityKind + Clone + fmt::Debug + Eq
where
    Self::Instance: Clone + fmt::Debug + Eq,
{
    /// The condition flavour this kind carries.
    type Conditions: ConditionSet;

    /// The name this kind is registered under.
    fn name(&self) -> &'static str;

    /// The kind's category types.
    fn categories(&self) -> &'static [Category];

    /// The largest quantity an instance may carry.
    fn maximum(&self) -> Amount;

    /// The quantity an instance carries.
    fn quantity(&self, value: &Self::Instance) -> Amount;

    /// The same instance with a different quantity.
    ///
    /// # Errors
    ///
    /// Returns [`KindError`] past [`Ingredient::maximum`], and for the one case
    /// section 6.2 records as defect six.
    fn with_quantity(
        &self,
        value: &Self::Instance,
        quantity: Amount,
    ) -> Result<Self::Instance, KindError>;

    /// The total order of section 6.3.
    fn compare(&self, left: &Self::Instance, right: &Self::Instance) -> Ordering;

    /// Match anything: every instance is alike.
    fn anything(&self) -> Self::Conditions;

    /// Match exactly: every axis is compared.
    fn exactly(&self) -> Self::Conditions;

    /// Match exactly but ignore the quantitative axis.
    fn ignoring_quantity(&self) -> Self::Conditions;

    /// The same set as [`Ingredient::ignoring_quantity`], as the vocabulary
    /// spells a condition set.
    ///
    /// The two flavours of section 6.1 are different types, and the collections
    /// and stores RVL-210 publishes take its own [`Conditions`]. This is the one
    /// place the two meet, so a caller writing one filter for all three kinds
    /// has one method to call.
    fn ignoring_quantity_conditions(&self) -> Conditions;

    /// The set [`Ingredient::exactly`] names, as the vocabulary spells one.
    fn exact_conditions(&self) -> Conditions {
        self.conditions()
    }

    /// One instance projected onto one of its category types.
    fn project(&self, value: &Self::Instance, category: &Category) -> Option<Projection>;
}

/// Refuses a quantity above the maximum this ingredient kind carries.
fn within_maximum(quantity: Amount, maximum: Amount, what: &str) -> Result<(), KindError> {
    if quantity > maximum {
        return Err(KindError::new(format!(
            "a {what} of {quantity} is past the {maximum} this kind carries"
        )));
    }
    Ok(())
}

/// The stackable-item kind.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stackable {
    comparator: Comparator,
}

impl Stackable {
    /// Builds the kind against the data comparator in force.
    ///
    /// Section three rebuilds the comparator on every settings load and reload,
    /// so a kind is rebuilt with it rather than reaching for a mutable one.
    #[must_use]
    pub const fn with(comparator: Comparator) -> Self {
        Self { comparator }
    }

    /// The data comparator in force.
    #[must_use]
    pub const fn comparator(&self) -> &Comparator {
        &self.comparator
    }
}

impl QuantityKind for Stackable {
    type Instance = Stack;

    fn conditions(&self) -> Conditions {
        Conditions::NONE
            .with(Stack::IDENTITY)
            .with(Stack::COUNT)
            .with(Stack::DATA)
    }

    fn matches(&self, left: &Stack, right: &Stack, under: Conditions) -> bool {
        if under.holds(Stack::IDENTITY) && left.identity != right.identity {
            return false;
        }
        if under.holds(Stack::COUNT) && left.count != right.count {
            return false;
        }
        if under.holds(Stack::DATA) && !self.comparator.equal(&left.data, &right.data) {
            return false;
        }
        true
    }

    fn digest(&self, value: &Stack, under: Conditions) -> u64 {
        let mut hasher = DefaultHasher::new();
        if under.holds(Stack::IDENTITY) {
            value.identity.hash(&mut hasher);
        }
        if under.holds(Stack::COUNT) {
            value.count.hash(&mut hasher);
        }
        if under.holds(Stack::DATA) {
            self.comparator.digest(&value.data).hash(&mut hasher);
        }
        hasher.finish()
    }

    fn write(&self, value: &Stack) -> JsonValue {
        // The vocabulary's writer cannot refuse, so an instance carrying more
        // than this kind's maximum is written as nothing readable rather than as
        // a different number: `read` then fails loudly where a clamp would have
        // returned a stack quietly unequal to the one written.
        crate::forms::stack_tag(value).map_or(JsonValue::Null, |tag| tag.to_json())
    }

    fn read(&self, stored: &JsonValue) -> Result<Stack, KindError> {
        let tag = Tag::from_json(stored).map_err(TagError::into_kind_error)?;
        crate::forms::stack_from(&tag).map_err(TagError::into_kind_error)
    }
}

/// Compares the serialized namespace:path spelling without allocating it.
fn compare_identity(left: &ContentId, right: &ContentId) -> Ordering {
    left.namespace()
        .as_str()
        .bytes()
        .chain(core::iter::once(b':'))
        .chain(left.path().bytes())
        .cmp(
            right
                .namespace()
                .as_str()
                .bytes()
                .chain(core::iter::once(b':'))
                .chain(right.path().bytes()),
        )
}

impl Ingredient for Stackable {
    type Conditions = Flags;

    fn name(&self) -> &'static str {
        STACKABLE
    }

    fn categories(&self) -> &'static [Category] {
        &Stack::CATEGORIES
    }

    fn maximum(&self) -> Amount {
        NARROW_MAXIMUM
    }

    fn quantity(&self, value: &Stack) -> Amount {
        value.count
    }

    fn with_quantity(&self, value: &Stack, quantity: Amount) -> Result<Stack, KindError> {
        within_maximum(quantity, NARROW_MAXIMUM, "count")?;
        if value.is_empty() && quantity != 0 {
            return Err(KindError::new(
                "a zero-count stack cannot be rebuilt at a non-zero count",
            ));
        }
        let mut rebuilt = value.clone();
        rebuilt.count = quantity;
        Ok(rebuilt)
    }

    /// Empty before non-empty, then identity **by name**, then count, then data.
    ///
    /// Defect nine: the original falls back to the game's numeric item index
    /// here while the fluid kind falls back to a registered name. A numeric
    /// index is not stable across differing mod sets, so an order built on it is
    /// stable within one session and not between two, and a dependent that
    /// persists a sorted order sees it change under an unrelated mod being
    /// added. Both kinds order by name here.
    fn compare(&self, left: &Stack, right: &Stack) -> Ordering {
        match (left.is_empty(), right.is_empty()) {
            // Two empty stacks still fall through to identity: `is_empty` holds
            // for a zero count with an identity set, and calling those equal
            // would collapse two stacks `matches` calls different into one key
            // of a map built on this order.
            (true, true) => return compare_identity(&left.identity, &right.identity),
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {}
        }
        compare_identity(&left.identity, &right.identity)
            .then_with(|| left.count.cmp(&right.count))
            .then_with(|| self.comparator.compare(&left.data, &right.data))
    }

    fn anything(&self) -> Flags {
        Flags::NONE
    }

    fn exactly(&self) -> Flags {
        Flags::of(self.conditions())
    }

    fn ignoring_quantity(&self) -> Flags {
        Flags::of(self.ignoring_quantity_conditions())
    }

    fn ignoring_quantity_conditions(&self) -> Conditions {
        Conditions::NONE.with(Stack::IDENTITY).with(Stack::DATA)
    }

    fn project(&self, value: &Stack, category: &Category) -> Option<Projection> {
        if !self.categories().contains(category) {
            return None;
        }
        match category.slot {
            Stack::IDENTITY => Some(Projection::Named(value.identity.clone())),
            Stack::COUNT => Some(Projection::Counted(value.count)),
            Stack::DATA => Some(Projection::Detailed(value.data.clone())),
            _ => None,
        }
    }
}

/// The fluid-portion kind.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Fluid {
    comparator: Comparator,
}

impl Fluid {
    /// Builds the kind against the data comparator in force.
    #[must_use]
    pub const fn with(comparator: Comparator) -> Self {
        Self { comparator }
    }

    /// The data comparator in force.
    #[must_use]
    pub const fn comparator(&self) -> &Comparator {
        &self.comparator
    }
}

impl QuantityKind for Fluid {
    type Instance = Portion;

    fn conditions(&self) -> Conditions {
        Conditions::NONE
            .with(Portion::FLUID)
            .with(Portion::VOLUME)
            .with(Portion::DATA)
    }

    fn matches(&self, left: &Portion, right: &Portion, under: Conditions) -> bool {
        if under.holds(Portion::FLUID) && left.fluid != right.fluid {
            return false;
        }
        if under.holds(Portion::VOLUME) && left.volume != right.volume {
            return false;
        }
        if under.holds(Portion::DATA) && !self.comparator.equal(&left.data, &right.data) {
            return false;
        }
        true
    }

    fn digest(&self, value: &Portion, under: Conditions) -> u64 {
        let mut hasher = DefaultHasher::new();
        if under.holds(Portion::FLUID) {
            value.fluid.hash(&mut hasher);
        }
        if under.holds(Portion::VOLUME) {
            value.volume.hash(&mut hasher);
        }
        // Section 6.3: the fluid hash folds fluid, volume and data, skipping
        // data when it is empty.
        if under.holds(Portion::DATA) && !value.data.is_empty() {
            self.comparator.digest(&value.data).hash(&mut hasher);
        }
        hasher.finish()
    }

    fn write(&self, value: &Portion) -> JsonValue {
        crate::forms::portion_tag(value).map_or(JsonValue::Null, |tag| tag.to_json())
    }

    fn read(&self, stored: &JsonValue) -> Result<Portion, KindError> {
        let tag = Tag::from_json(stored).map_err(TagError::into_kind_error)?;
        crate::forms::portion_from(&tag).map_err(TagError::into_kind_error)
    }
}

impl Ingredient for Fluid {
    type Conditions = Flags;

    fn name(&self) -> &'static str {
        PORTION
    }

    fn categories(&self) -> &'static [Category] {
        &Portion::CATEGORIES
    }

    fn maximum(&self) -> Amount {
        NARROW_MAXIMUM
    }

    fn quantity(&self, value: &Portion) -> Amount {
        value.volume
    }

    /// Rebuilding at zero yields the empty portion; rebuilding an empty portion
    /// at a non-zero quantity is refused.
    ///
    /// Defect six: the original answers a portion of the game's default fluid
    /// there, so a caller that meant "this much of nothing" silently receives
    /// water.
    fn with_quantity(&self, value: &Portion, quantity: Amount) -> Result<Portion, KindError> {
        within_maximum(quantity, NARROW_MAXIMUM, "volume")?;
        if quantity == 0 {
            return Ok(Portion::empty());
        }
        if value.is_empty() {
            return Err(KindError::new(
                "a zero-volume portion cannot be rebuilt at a non-zero volume",
            ));
        }
        let mut rebuilt = value.clone();
        rebuilt.volume = quantity;
        Ok(rebuilt)
    }

    fn compare(&self, left: &Portion, right: &Portion) -> Ordering {
        match (left.is_empty(), right.is_empty()) {
            // The same fall-through the stackable order needs: a zero volume
            // with a fluid named is empty, and two of those are not one key.
            (true, true) => return compare_identity(&left.fluid, &right.fluid),
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {}
        }
        compare_identity(&left.fluid, &right.fluid)
            .then_with(|| left.volume.cmp(&right.volume))
            .then_with(|| self.comparator.compare(&left.data, &right.data))
    }

    fn anything(&self) -> Flags {
        Flags::NONE
    }

    fn exactly(&self) -> Flags {
        Flags::of(self.conditions())
    }

    fn ignoring_quantity(&self) -> Flags {
        Flags::of(self.ignoring_quantity_conditions())
    }

    fn ignoring_quantity_conditions(&self) -> Conditions {
        Conditions::NONE.with(Portion::FLUID).with(Portion::DATA)
    }

    fn project(&self, value: &Portion, category: &Category) -> Option<Projection> {
        if !self.categories().contains(category) {
            return None;
        }
        match category.slot {
            Portion::FLUID => Some(Projection::Named(value.fluid.clone())),
            Portion::VOLUME => Some(Projection::Counted(value.volume)),
            Portion::DATA => Some(Projection::Detailed(value.data.clone())),
            _ => None,
        }
    }
}

/// The scalar-charge kind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Scalar;

impl QuantityKind for Scalar {
    type Instance = Charge;

    fn conditions(&self) -> Conditions {
        Conditions::NONE.with(Charge::CHARGE)
    }

    /// Defect three: the original narrows both 64-bit values to 32 bits before
    /// comparing, so two charges differing only above the low 32 bits compare
    /// equal while the kind's own maximum makes those values reachable. This
    /// compares at full width.
    fn matches(&self, left: &Charge, right: &Charge, under: Conditions) -> bool {
        !under.holds(Charge::CHARGE) || left.amount == right.amount
    }

    fn digest(&self, value: &Charge, under: Conditions) -> u64 {
        let mut hasher = DefaultHasher::new();
        if under.holds(Charge::CHARGE) {
            value.amount.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn write(&self, value: &Charge) -> JsonValue {
        crate::forms::charge_tag(*value).map_or(JsonValue::Null, |tag| tag.to_json())
    }

    fn read(&self, stored: &JsonValue) -> Result<Charge, KindError> {
        let tag = Tag::from_json(stored).map_err(TagError::into_kind_error)?;
        crate::forms::charge_from(&tag).map_err(TagError::into_kind_error)
    }
}

impl Ingredient for Scalar {
    type Conditions = Truth;

    fn name(&self) -> &'static str {
        SCALAR
    }

    fn categories(&self) -> &'static [Category] {
        &Charge::CATEGORIES
    }

    fn maximum(&self) -> Amount {
        WIDE_MAXIMUM
    }

    fn quantity(&self, value: &Charge) -> Amount {
        value.amount
    }

    fn with_quantity(&self, _value: &Charge, quantity: Amount) -> Result<Charge, KindError> {
        within_maximum(quantity, WIDE_MAXIMUM, "charge")?;
        Ok(Charge::of(quantity))
    }

    /// Defect four: the original returns the 32-bit difference of two 64-bit
    /// values, which overflows and can report a larger value as smaller, so it
    /// is not a total order over the kind's own range. This compares.
    fn compare(&self, left: &Charge, right: &Charge) -> Ordering {
        left.amount.cmp(&right.amount)
    }

    fn anything(&self) -> Truth {
        Truth::NONE
    }

    fn exactly(&self) -> Truth {
        Truth::SET
    }

    /// The same as match-anything, because a charge has nothing but its
    /// quantity. Section 6.1 states this.
    fn ignoring_quantity(&self) -> Truth {
        Truth::NONE
    }

    fn ignoring_quantity_conditions(&self) -> Conditions {
        Conditions::NONE
    }

    fn project(&self, value: &Charge, category: &Category) -> Option<Projection> {
        if !self.categories().contains(category) {
            return None;
        }
        match category.slot {
            Charge::CHARGE => Some(Projection::Counted(value.amount)),
            _ => None,
        }
    }
}

/// What a registration into the shared kind registry refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotRegistered {
    /// Two registrations name one kind.
    Duplicate {
        /// The name registered twice.
        name: String,
    },
    /// The registry holds as many kinds as it publishes.
    Full {
        /// The published limit.
        limit: usize,
    },
    /// A kind this package registered did not resolve.
    Unresolved {
        /// The name that did not resolve.
        name: String,
    },
}

impl fmt::Display for NotRegistered {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { name } => write!(formatter, "{name} is registered twice"),
            Self::Full { limit } => {
                write!(formatter, "the registry holds at most {limit} kinds")
            }
            Self::Unresolved { name } => {
                write!(formatter, "{name} did not resolve at common setup")
            }
        }
    }
}

impl std::error::Error for NotRegistered {}

/// The most kinds the shared registry holds.
pub const MAX_KINDS: usize = 512;

/// One kind's registration, as the shared registry holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registered {
    /// The name it is registered under.
    pub name: String,
    /// Its category types.
    pub categories: Vec<Category>,
    /// Whether this package registered it, as opposed to a dependent.
    pub own: bool,
    /// Whether it is subscribed to the capability-gathering event.
    pub subscribed: bool,
}

/// The shared ingredient-kind registry of section two.
///
/// Two of its rules are contracts rather than implementation details. Kinds are
/// registered at a deliberately late priority, so a dependent registering a kind
/// of its own is free to run earlier; and at common setup **every** registered
/// kind is subscribed to the capability-gathering event, this package's own
/// three and any a dependent added. Section two says in as many words that a
/// counterpart wiring only its own three would silently break a dependent's
/// kind.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Kinds {
    registered: Vec<Registered>,
}

impl Kinds {
    /// An empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registered: Vec::new(),
        }
    }

    /// Registers one kind.
    ///
    /// # Errors
    ///
    /// Returns [`NotRegistered::Duplicate`] for a name already held and
    /// [`NotRegistered::Full`] past [`MAX_KINDS`].
    pub fn register(
        &mut self,
        name: &str,
        categories: &[Category],
        own: bool,
    ) -> Result<(), NotRegistered> {
        if self.registered.iter().any(|held| held.name == name) {
            return Err(NotRegistered::Duplicate {
                name: name.to_owned(),
            });
        }
        if self.registered.len() >= MAX_KINDS {
            return Err(NotRegistered::Full { limit: MAX_KINDS });
        }
        self.registered.push(Registered {
            name: name.to_owned(),
            categories: categories.to_vec(),
            own,
            subscribed: false,
        });
        Ok(())
    }

    /// Registers this package's own three, in the order section four names them.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Kinds::register`] refuses.
    pub fn register_own(&mut self) -> Result<(), NotRegistered> {
        self.register(STACKABLE, &Stack::CATEGORIES, true)?;
        self.register(PORTION, &Portion::CATEGORIES, true)?;
        self.register(SCALAR, &Charge::CATEGORIES, true)?;
        Ok(())
    }

    /// How many kinds are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.registered.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registered.is_empty()
    }

    /// Every registration, in registration order.
    pub fn entries(&self) -> impl Iterator<Item = &Registered> + '_ {
        self.registered.iter()
    }

    /// One registration by name.
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<&Registered> {
        self.registered.iter().find(|held| held.name == name)
    }

    /// Common setup: assert all three of this package's kinds resolved, then
    /// subscribe every registered kind to capability gathering.
    ///
    /// # Errors
    ///
    /// Returns [`NotRegistered::Unresolved`] naming the first of the three that
    /// is not there. Section two says this fails loudly rather than carrying on.
    pub fn subscribe_all(&mut self) -> Result<usize, NotRegistered> {
        for name in REGISTERED {
            if self.entry(name).is_none() {
                return Err(NotRegistered::Unresolved {
                    name: name.to_owned(),
                });
            }
        }
        let mut wired = 0;
        for held in &mut self.registered {
            if !held.subscribed {
                held.subscribed = true;
                wired += 1;
            }
        }
        Ok(wired)
    }

    /// How many registered kinds are subscribed.
    #[must_use]
    pub fn subscribed_count(&self) -> usize {
        self.registered
            .iter()
            .filter(|held| held.subscribed)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Value;

    fn stackable() -> Stackable {
        Stackable::default()
    }

    fn fluid() -> Fluid {
        Fluid::default()
    }

    #[test]
    fn the_three_kinds_decompose_into_seven_category_types() {
        assert_eq!(
            Stack::CATEGORIES.iter().filter(|c| c.primary).count()
                + Portion::CATEGORIES.iter().filter(|c| c.primary).count()
                + Charge::CATEGORIES.iter().filter(|c| c.primary).count(),
            3
        );
        assert_eq!(
            Stack::CATEGORIES.iter().filter(|c| c.quantitative).count()
                + Portion::CATEGORIES
                    .iter()
                    .filter(|c| c.quantitative)
                    .count()
                + Charge::CATEGORIES.iter().filter(|c| c.quantitative).count(),
            3
        );
    }

    #[test]
    fn every_category_names_a_slot_its_kind_understands() {
        let kind = stackable();
        for category in kind.categories() {
            assert!(kind.conditions().holds(category.slot), "{}", category.name);
        }
        let kind = fluid();
        for category in kind.categories() {
            assert!(kind.conditions().holds(category.slot), "{}", category.name);
        }
        for category in Scalar.categories() {
            assert!(Scalar.conditions().holds(category.slot));
        }
    }

    #[test]
    fn empty_stacks_cannot_be_rebuilt_into_nonempty_defaults() {
        assert!(stackable().with_quantity(&Stack::empty(), 30).is_err());
        assert_eq!(
            stackable().with_quantity(&Stack::empty(), 0),
            Ok(Stack::empty())
        );
    }

    #[test]
    fn identity_order_keeps_the_serialized_namespace_separator() {
        let left = "a-b:x".parse::<ContentId>().expect("a valid identity");
        let right = "a:x".parse::<ContentId>().expect("a valid identity");
        for quantity in [0, 1] {
            assert_eq!(
                stackable().compare(
                    &Stack::of(left.clone(), quantity),
                    &Stack::of(right.clone(), quantity)
                ),
                Ordering::Less,
            );
            assert_eq!(
                fluid().compare(
                    &Portion::of(left.clone(), quantity),
                    &Portion::of(right.clone(), quantity)
                ),
                Ordering::Less,
            );
        }
    }

    #[test]
    fn quantity_alone_controls_absence_while_identities_remain_valid() {
        let item = "minecraft:air"
            .parse::<ContentId>()
            .expect("a valid item identity");
        assert!(Stack::of(item.clone(), 0).is_empty());
        assert!(!Stack::of(item, 1).is_empty());
        let fluid = "minecraft:empty"
            .parse::<ContentId>()
            .expect("a valid fluid identity");
        assert!(Portion::of(fluid.clone(), 0).is_empty());
        assert!(!Portion::of(fluid, 1).is_empty());
        assert_eq!(Stack::empty().count, 0);
        assert_eq!(Portion::empty().volume, 0);
    }

    #[test]
    fn an_instance_projects_onto_each_of_its_categories_and_onto_nothing_else() {
        let kind = stackable();
        let stack = crate::stack("minecraft:stone", 4);
        assert_eq!(
            kind.project(&stack, &Stack::CATEGORIES[0]),
            Some(Projection::Named(stack.identity.clone()))
        );
        assert_eq!(
            kind.project(&stack, &Stack::CATEGORIES[1]),
            Some(Projection::Counted(4))
        );
        assert_eq!(
            kind.project(&stack, &Stack::CATEGORIES[2]),
            Some(Projection::Detailed(Components::new()))
        );
        assert_eq!(kind.project(&stack, &Charge::CATEGORIES[0]), None);
        let renamed = Category {
            name: "unrecognised",
            ..Stack::CATEGORIES[0]
        };
        assert_eq!(kind.project(&stack, &renamed), None);
        assert_eq!(kind.project(&stack, &Portion::CATEGORIES[0]), None);
        assert_eq!(
            fluid().project(
                &Portion::of(
                    "minecraft:water".parse().expect("a valid content identity"),
                    9
                ),
                &Portion::CATEGORIES[1]
            ),
            Some(Projection::Counted(9))
        );
        assert_eq!(
            Scalar.project(&Charge::of(7), &Charge::CATEGORIES[0]),
            Some(Projection::Counted(7))
        );
        assert_eq!(Scalar.project(&Charge::of(7), &Stack::CATEGORIES[0]), None);
    }

    #[test]
    fn testing_a_flag_set_answers_yes_for_any_flag_rather_than_all() {
        let identity = Flags::slot(Stack::IDENTITY);
        let both = identity.combine(Flags::slot(Stack::DATA));
        assert!(both.test(identity));
        assert!(identity.test(both));
        assert!(!identity.test(Flags::slot(Stack::DATA)));
        assert!(!Flags::NONE.test(both));
    }

    #[test]
    fn removing_a_flag_is_the_inverse_of_adding_it() {
        let identity = Flags::slot(Stack::IDENTITY);
        let data = Flags::slot(Stack::DATA);
        assert_eq!(identity.combine(data).remove(data), identity);
        assert_eq!(identity.remove(data), identity);
    }

    #[test]
    fn the_truth_flavour_removes_by_and_not() {
        // Defect five: the original clears on removing a condition from itself
        // and leaves it set on removing one it does not have.
        assert_eq!(Truth::SET.remove(Truth::SET), Truth::NONE);
        assert_eq!(Truth::SET.remove(Truth::NONE), Truth::SET);
        assert_eq!(Truth::NONE.remove(Truth::SET), Truth::NONE);
        assert_eq!(Truth::NONE.remove(Truth::NONE), Truth::NONE);
    }

    #[test]
    fn the_truth_flavour_combines_by_or_and_tests_by_equality() {
        assert_eq!(Truth::NONE.combine(Truth::SET), Truth::SET);
        assert_eq!(Truth::NONE.combine(Truth::NONE), Truth::NONE);
        assert!(Truth::SET.test(Truth::SET));
        assert!(Truth::NONE.test(Truth::NONE));
        assert!(!Truth::SET.test(Truth::NONE));
    }

    #[test]
    fn the_charge_kinds_ignoring_quantity_is_its_match_anything() {
        assert_eq!(Scalar.ignoring_quantity(), Scalar.anything());
        assert_ne!(Scalar.exactly(), Scalar.anything());
    }

    #[test]
    fn two_zero_count_stacks_of_different_items_are_not_one_key() {
        // Section 6.3's order is what a dependent builds a sorted map on, and
        // `is_empty` holds for a zero count with an identity set, so calling
        // every empty stack equal would collapse two the matcher calls
        // different into one key.
        let kind = Stackable::default();
        let left = crate::stack("minecraft:stone", 0);
        let right = crate::stack("minecraft:dirt", 0);
        assert_ne!(kind.compare(&left, &right), Ordering::Equal);
        assert!(!kind.matches(&left, &right, kind.exact_conditions()));
        assert_eq!(
            kind.compare(&Stack::empty(), &Stack::empty()),
            Ordering::Equal
        );
    }

    #[test]
    fn two_zero_volume_portions_of_different_fluids_are_not_one_key() {
        let kind = Fluid::default();
        let left = crate::portion("minecraft:water", 0);
        let right = crate::portion("minecraft:lava", 0);
        assert_ne!(kind.compare(&left, &right), Ordering::Equal);
        assert!(!kind.matches(&left, &right, kind.exact_conditions()));
    }

    #[test]
    fn ignoring_quantity_drops_only_the_quantitative_axis() {
        let kind = stackable();
        let dropped = kind.exactly().remove(kind.ignoring_quantity());
        assert_eq!(dropped, Flags::slot(Stack::COUNT));
        let kind = fluid();
        let dropped = kind.exactly().remove(kind.ignoring_quantity());
        assert_eq!(dropped, Flags::slot(Portion::VOLUME));
    }

    #[test]
    fn two_stacks_alike_but_for_count_match_when_quantity_is_ignored() {
        let kind = stackable();
        let one = crate::stack("minecraft:stone", 1);
        let many = crate::stack("minecraft:stone", 40);
        assert!(kind.matches(&one, &many, kind.ignoring_quantity().conditions()));
        assert!(!kind.matches(&one, &many, kind.exactly().conditions()));
        assert!(kind.matches(&one, &many, kind.anything().conditions()));
    }

    #[test]
    fn matching_instances_digest_alike_under_the_same_set() {
        let kind = stackable();
        let data = Components::from([("wear".to_owned(), Value::Whole(3))]);
        let one = crate::stack("minecraft:axe", 1).carrying(data.clone());
        let other = crate::stack("minecraft:axe", 1).carrying(data);
        let under = kind.exactly().conditions();
        assert!(kind.matches(&one, &other, under));
        assert_eq!(kind.digest(&one, under), kind.digest(&other, under));
    }

    #[test]
    fn an_ignored_component_kind_makes_two_stacks_match_and_digest_alike() {
        let comparator = Comparator::new(["wear".to_owned()]).expect("a small list");
        let kind = Stackable::with(comparator);
        let sharp = crate::stack("minecraft:axe", 1)
            .carrying(Components::from([("wear".to_owned(), Value::Whole(0))]));
        let blunt = crate::stack("minecraft:axe", 1)
            .carrying(Components::from([("wear".to_owned(), Value::Whole(90))]));
        let under = kind.exactly().conditions();
        assert!(kind.matches(&sharp, &blunt, under));
        assert_eq!(kind.digest(&sharp, under), kind.digest(&blunt, under));
    }

    #[test]
    fn a_stack_orders_empty_first_then_by_name_then_count_then_data() {
        let kind = stackable();
        assert_eq!(
            kind.compare(
                &Stack::empty(),
                &Stack::of(
                    "minecraft:stone".parse().expect("a valid content identity"),
                    1
                )
            ),
            Ordering::Less
        );
        assert_eq!(
            kind.compare(&Stack::empty(), &Stack::empty()),
            Ordering::Equal
        );
        assert_eq!(
            kind.compare(
                &Stack::of(
                    "minecraft:apple".parse().expect("a valid content identity"),
                    9
                ),
                &Stack::of(
                    "minecraft:stone".parse().expect("a valid content identity"),
                    1
                )
            ),
            Ordering::Less
        );
        assert_eq!(
            kind.compare(
                &Stack::of(
                    "minecraft:stone".parse().expect("a valid content identity"),
                    1
                ),
                &Stack::of(
                    "minecraft:stone".parse().expect("a valid content identity"),
                    2
                )
            ),
            Ordering::Less
        );
        let plain = crate::stack("minecraft:stone", 1);
        let detailed = crate::stack("minecraft:stone", 1)
            .carrying(Components::from([("a".to_owned(), Value::Whole(1))]));
        assert_eq!(kind.compare(&plain, &detailed), Ordering::Less);
    }

    #[test]
    fn both_kinds_order_by_name_rather_than_one_by_an_index() {
        // Defect nine: the two orders are built differently upstream and only
        // one of them is stable across sessions. Both are names here.
        let items = stackable();
        let fluids = fluid();
        assert_eq!(
            items.compare(
                &Stack::of("minecraft:b".parse().expect("a valid content identity"), 1),
                &Stack::of("minecraft:a".parse().expect("a valid content identity"), 99)
            ),
            Ordering::Greater
        );
        assert_eq!(
            fluids.compare(
                &Portion::of("minecraft:b".parse().expect("a valid content identity"), 1),
                &Portion::of("minecraft:a".parse().expect("a valid content identity"), 99)
            ),
            Ordering::Greater
        );
    }

    #[test]
    fn a_portion_orders_empty_first_then_by_fluid_then_volume() {
        let kind = fluid();
        assert_eq!(
            kind.compare(
                &Portion::empty(),
                &Portion::of(
                    "minecraft:water".parse().expect("a valid content identity"),
                    1
                )
            ),
            Ordering::Less
        );
        assert_eq!(
            kind.compare(
                &Portion::of(
                    "minecraft:water".parse().expect("a valid content identity"),
                    5
                ),
                &Portion::of(
                    "minecraft:water".parse().expect("a valid content identity"),
                    6
                )
            ),
            Ordering::Less
        );
    }

    #[test]
    fn a_charge_orders_at_full_width_where_a_narrowing_difference_would_not() {
        // Defect four: the 32-bit difference of these two overflows to zero.
        let low = Charge::of(1);
        let high = Charge::of(1 + (1 << 32));
        assert_eq!(Scalar.compare(&low, &high), Ordering::Less);
        assert_eq!(Scalar.compare(&high, &low), Ordering::Greater);
    }

    #[test]
    fn two_charges_differing_above_the_low_thirty_two_bits_do_not_match() {
        // Defect three: narrowed to 32 bits these two are the same value.
        let low = Charge::of(5);
        let high = Charge::of(5 + (1 << 32));
        let under = Scalar.exactly();
        assert!(!Scalar.matches(&low, &high, Conditions::NONE.with(Charge::CHARGE)));
        assert!(under.held());
        assert!(Scalar.matches(&low, &high, Conditions::NONE));
    }

    #[test]
    fn the_two_narrow_kinds_and_the_wide_one_publish_the_stated_maxima() {
        assert_eq!(stackable().maximum(), 2_147_483_647);
        assert_eq!(fluid().maximum(), 2_147_483_647);
        assert_eq!(Scalar.maximum(), 9_223_372_036_854_775_807);
    }

    #[test]
    fn rebuilding_past_the_maximum_is_refused_for_every_kind() {
        assert!(
            stackable()
                .with_quantity(
                    &Stack::of("minecraft:a".parse().expect("a valid content identity"), 1),
                    NARROW_MAXIMUM + 1
                )
                .is_err()
        );
        assert!(
            fluid()
                .with_quantity(
                    &Portion::of("minecraft:a".parse().expect("a valid content identity"), 1),
                    NARROW_MAXIMUM + 1
                )
                .is_err()
        );
        assert!(
            Scalar
                .with_quantity(&Charge::of(1), WIDE_MAXIMUM + 1)
                .is_err()
        );
        assert!(
            stackable()
                .with_quantity(
                    &Stack::of("minecraft:a".parse().expect("a valid content identity"), 1),
                    NARROW_MAXIMUM
                )
                .is_ok()
        );
        assert!(Scalar.with_quantity(&Charge::of(1), WIDE_MAXIMUM).is_ok());
    }

    #[test]
    fn rebuilding_an_unchanged_quantity_answers_an_equal_value() {
        let kind = stackable();
        let stack = crate::stack("minecraft:stone", 3);
        assert_eq!(kind.with_quantity(&stack, 3), Ok(stack.clone()));
        assert_eq!(kind.with_quantity(&stack, 4).map(|s| s.count), Ok(4));
        assert_eq!(stack.count, 3);
    }

    #[test]
    fn rebuilding_a_portion_at_zero_yields_the_empty_portion() {
        let kind = fluid();
        assert_eq!(
            kind.with_quantity(
                &Portion::of(
                    "minecraft:water".parse().expect("a valid content identity"),
                    50
                ),
                0
            ),
            Ok(Portion::empty())
        );
    }

    #[test]
    fn rebuilding_an_empty_portion_at_a_non_zero_quantity_is_refused() {
        // Defect six: the original answers a portion of the game's default
        // fluid, so a caller that meant nothing silently receives water.
        let kind = fluid();
        let refusal = kind
            .with_quantity(&Portion::empty(), 100)
            .expect_err("an empty portion has zero volume");
        assert_eq!(
            refusal.detail(),
            "a zero-volume portion cannot be rebuilt at a non-zero volume"
        );
    }

    #[test]
    fn the_registry_registers_this_packages_three_in_order() {
        let mut kinds = Kinds::new();
        kinds.register_own().expect("a fresh registry");
        assert_eq!(kinds.len(), 3);
        let names: Vec<&str> = kinds.entries().map(|held| held.name.as_str()).collect();
        assert_eq!(names, REGISTERED);
        assert!(kinds.entries().all(|held| held.own));
    }

    #[test]
    fn registering_one_kind_twice_is_refused() {
        let mut kinds = Kinds::new();
        kinds.register_own().expect("a fresh registry");
        assert_eq!(
            kinds.register(STACKABLE, &Stack::CATEGORIES, false),
            Err(NotRegistered::Duplicate {
                name: STACKABLE.to_owned()
            })
        );
    }

    #[test]
    fn common_setup_subscribes_a_dependents_kind_as_well_as_this_packages_three() {
        let mut kinds = Kinds::new();
        kinds.register_own().expect("a fresh registry");
        kinds
            .register("other:gas", &Portion::CATEGORIES, false)
            .expect("a dependent's kind");
        assert_eq!(kinds.subscribed_count(), 0);
        assert_eq!(kinds.subscribe_all(), Ok(4));
        assert_eq!(kinds.subscribed_count(), 4);
        assert!(kinds.entry("other:gas").expect("registered").subscribed);
    }

    #[test]
    fn subscribing_twice_subscribes_nothing_further() {
        let mut kinds = Kinds::new();
        kinds.register_own().expect("a fresh registry");
        assert_eq!(kinds.subscribe_all(), Ok(3));
        assert_eq!(kinds.subscribe_all(), Ok(0));
        assert_eq!(kinds.subscribed_count(), 3);
    }

    #[test]
    fn common_setup_fails_loudly_when_one_of_the_three_did_not_resolve() {
        let mut kinds = Kinds::new();
        kinds
            .register(STACKABLE, &Stack::CATEGORIES, true)
            .expect("a fresh registry");
        assert_eq!(
            kinds.subscribe_all(),
            Err(NotRegistered::Unresolved {
                name: PORTION.to_owned()
            })
        );
        assert_eq!(kinds.subscribed_count(), 0);
    }

    #[test]
    fn the_registry_has_an_enforced_ceiling() {
        let mut kinds = Kinds::new();
        for index in 0..MAX_KINDS {
            kinds
                .register(&format!("other:k{index}"), &Charge::CATEGORIES, false)
                .expect("inside the ceiling");
        }
        assert_eq!(
            kinds.register("other:one_too_many", &Charge::CATEGORIES, false),
            Err(NotRegistered::Full { limit: MAX_KINDS })
        );
        assert_eq!(kinds.len(), MAX_KINDS);
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            NotRegistered::Duplicate {
                name: "a".to_owned()
            }
            .to_string()
            .contains("twice")
        );
        assert!(
            NotRegistered::Full { limit: 2 }
                .to_string()
                .contains("at most")
        );
        assert!(
            NotRegistered::Unresolved {
                name: "a".to_owned()
            }
            .to_string()
            .contains("did not resolve")
        );
    }

    #[test]
    fn an_empty_registry_reports_itself_empty() {
        let kinds = Kinds::new();
        assert!(kinds.is_empty());
        assert_eq!(kinds.len(), 0);
        assert_eq!(kinds.entry(STACKABLE), None);
    }
}
