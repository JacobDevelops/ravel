//! The attractor: the one item-level data value, the one payload, and the sweep.
//!
//! Section 5.5, 10.1 and 10.2. This is the whole of what section nine says
//! survives a save, an exit and a reload: one boolean per attractor item, given
//! the value `false` on the item itself so a freshly registered one carries it
//! explicitly. Nothing here is world-level; a counterpart that stored anything
//! world-level would have added state the original does not have.
//!
//! The payload is the one place where the specification says a divergence is not
//! optional. The original's handler reads a slot index the sending client names,
//! indexes the open container's slot list with it unchecked, and asks only
//! whether what it found is an attractor. [`Toggle::apply`] bounds the index
//! against the container's own slot count and refuses a slot the sender does not
//! own, before anything is read out of it.

use ravel_sdk::content::ContentIdError;
use ravel_sdk::network::{Direction, PacketContract, PacketReader, PacketWriter, WireError};
use rvl_042_counterpart::arithmetic::Position;

use crate::settings::{contract, to_bytes};

/// The name the item-level boolean is stored and synchronised under.
pub const SWITCHED_ON: &str = "switched_on";

/// The value a freshly registered attractor carries.
pub const SWITCHED_ON_DEFAULT: bool = false;

/// The name the toggle payload travels under.
pub const TOGGLE_PACKET: &str = "toggle_attractor";

/// The radius the sweep reaches by default.
pub const DEFAULT_RADIUS: u32 = 7;

/// The lowest and highest radius the sweep is constrained to.
pub const RADIUS_RANGE: (u32, u32) = (1, 16);

/// The most slots a container this handler will look into may declare.
///
/// A container's slot count comes from the host and is not itself untrusted, but
/// the index that names a slot in it arrives from a client. The ceiling bounds
/// the arithmetic either way and is far above any screen a player opens.
pub const MAXIMUM_SLOTS: u32 = 4096;

/// The sound pitch a toggle plays, chosen from the value being left.
///
/// The original reads the boolean before flipping it, so the pitch a player
/// hears belongs to the state the attractor is leaving rather than the one it is
/// entering. That is reproduced: the specification records it as read behaviour
/// and marks its intent unknown, and the two pitches are equally arbitrary.
#[must_use]
pub const fn pitch(before_the_flip: bool) -> f32 {
    if before_the_flip { 0.5 } else { 1.0 }
}

/// A freshly registered attractor, carrying its declared value explicitly.
///
/// The value is written onto the item rather than left absent, so an attractor
/// that has never been switched reads the same as one switched back off.
#[must_use]
pub fn fresh() -> Slot {
    Slot {
        attractor: true,
        switched_on: SWITCHED_ON_DEFAULT,
        owned_by_sender: true,
    }
}

/// The payload a client sends to ask that an attractor be switched.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Toggle {
    /// The slot the sender says holds the attractor.
    pub slot: u32,
}

/// Why a toggle was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejected {
    /// The index names a slot outside the container's slot list.
    OutOfRange,
    /// The container declares more slots than this handler will look into.
    ContainerTooLarge,
    /// The slot is not one the sender may reach into.
    NotTheSenders,
    /// The slot holds something that is not an attractor.
    NotAnAttractor,
}

/// One slot of the container a toggle names.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Slot {
    /// Whether the slot holds an attractor.
    pub attractor: bool,
    /// Whether that attractor is switched on.
    pub switched_on: bool,
    /// Whether the sending player owns this slot rather than merely seeing it.
    pub owned_by_sender: bool,
}

/// The container the sender has open, as the server sees it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Container {
    /// The slots, in the order the container declares them.
    pub slots: Vec<Slot>,
}

/// What one accepted toggle did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Switched {
    /// The value the attractor held before the flip.
    pub was: bool,
    /// The value it holds after.
    pub now: bool,
}

impl Toggle {
    /// The contract this payload travels under.
    ///
    /// # Errors
    ///
    /// Returns [`ContentIdError`] when the declared name is not a legal path,
    /// which for a constant is a packaging failure.
    pub fn contract() -> Result<PacketContract, ContentIdError> {
        contract(TOGGLE_PACKET, Direction::ToServer)
    }

    /// Writes the body: the one slot index.
    pub fn write(&self, writer: &mut PacketWriter) {
        writer.u32(self.slot);
    }

    /// Reads the body.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the bytes do not spell this payload.
    pub fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            slot: reader.u32()?,
        })
    }

    /// Writes this payload to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        to_bytes(&|writer| self.write(writer))
    }

    /// Applies an accepted toggle to the sender's own container.
    ///
    /// The three checks happen in this order and all three happen before the
    /// slot is read: the container is a size this handler will look into, the
    /// index names a slot that exists, and the slot is one the sender owns. Only
    /// then is the item asked whether it is an attractor.
    ///
    /// # Errors
    ///
    /// Returns the first [`Rejected`] the request earns. Nothing is changed on
    /// any refusal.
    pub fn apply(self, container: &mut Container) -> Result<Switched, Rejected> {
        let count = u32::try_from(container.slots.len()).unwrap_or(u32::MAX);
        if count > MAXIMUM_SLOTS {
            return Err(Rejected::ContainerTooLarge);
        }
        if self.slot >= count {
            return Err(Rejected::OutOfRange);
        }
        let index = usize::try_from(self.slot).map_err(|_| Rejected::OutOfRange)?;
        let slot = container.slots.get_mut(index).ok_or(Rejected::OutOfRange)?;
        if !slot.owned_by_sender {
            return Err(Rejected::NotTheSenders);
        }
        if !slot.attractor {
            return Err(Rejected::NotAnAttractor);
        }
        let was = slot.switched_on;
        slot.switched_on = !was;
        Ok(Switched { was, now: !was })
    }
}

/// What using an attractor in the hand did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Used {
    /// The values either side of the flip.
    pub switched: Switched,
    /// The pitch the sound plays at, read from the value being left.
    pub pitch: f32,
    /// Whether the use reports success, which it does whenever it flipped.
    pub success: bool,
}

/// Flips an attractor held in the hand.
///
/// The direct path of section 10.2, which reaches the same boolean as the
/// payload without a container or a slot index in the way: it flips the value,
/// plays a sound at [`pitch`] read from the value being left, and reports
/// success. Anything that is not an attractor is left alone and reports nothing,
/// so the use passes to whatever would otherwise handle it.
pub fn use_in_hand(held: &mut Slot) -> Option<Used> {
    if !held.attractor {
        return None;
    }
    let was = held.switched_on;
    held.switched_on = !was;
    Some(Used {
        switched: Switched { was, now: !was },
        pitch: pitch(was),
        success: true,
    })
}

/// Where a client may press to send a toggle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    /// The player's own inventory.
    OwnInventory,
    /// The optional accessory screen, recognised by asking whether it has that
    /// capability rather than by comparing its class name as a string.
    Accessory,
    /// Any other screen, where the press is left alone.
    Other,
}

/// What the client half saw when a player pressed the right mouse button.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pressed {
    /// The screen the press happened on.
    pub screen: Screen,
    /// The slot under the cursor, or nothing when the cursor is over no slot.
    pub slot: Option<u32>,
    /// Whether something is being carried on the cursor.
    pub carrying: bool,
    /// Whether the slot under the cursor holds an attractor.
    pub attractor: bool,
}

/// Whether a press sends a toggle, and which slot it names.
///
/// A press is only ever sent from the player's own inventory or from the
/// accessory screen, only over a slot holding an attractor, and only with an
/// empty cursor. A press that sends is also the press that is consumed.
#[must_use]
pub fn press(pressed: Pressed) -> Option<Toggle> {
    if pressed.carrying || !pressed.attractor {
        return None;
    }
    match pressed.screen {
        Screen::OwnInventory | Screen::Accessory => pressed.slot.map(|slot| Toggle { slot }),
        Screen::Other => None,
    }
}

/// The ownership and timing that can withhold a freshly dropped item.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Pickup {
    /// Whether the holder is the one who threw it.
    pub thrown_by_the_holder: bool,
    /// Whether it is still inside its pickup delay.
    pub delayed: bool,
}

/// One loose thing the sweep may collect.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Loose {
    /// Where it is.
    pub at: Position,
    /// Whether it is an experience orb rather than a dropped item.
    pub orb: bool,
    /// Whether it has already been removed.
    pub dead: bool,
    /// Whether it carries the host's marker forbidding remote movement.
    pub immovable: bool,
    /// The ownership and timing of pickup.
    pub pickup: Pickup,
}

impl Loose {
    /// Builds a dropped item at a position, with none of the exclusions set.
    #[must_use]
    pub const fn item(at: Position) -> Self {
        Self {
            at,
            orb: false,
            dead: false,
            immovable: false,
            pickup: Pickup {
                thrown_by_the_holder: false,
                delayed: false,
            },
        }
    }

    /// Builds an experience orb at a position.
    #[must_use]
    pub const fn orb(at: Position) -> Self {
        Self {
            orb: true,
            ..Self::item(at)
        }
    }
}

// Keep host-facing diagnostics flat even though pickup policy is grouped.
impl std::fmt::Debug for Loose {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Loose")
            .field("at", &self.at)
            .field("orb", &self.orb)
            .field("dead", &self.dead)
            .field("immovable", &self.immovable)
            .field("thrown_by_the_holder", &self.pickup.thrown_by_the_holder)
            .field("pickup_delay", &self.pickup.delayed)
            .finish()
    }
}

/// Reads the radius in force, held to the range the setting declares.
#[must_use]
pub fn radius(written: u32) -> u32 {
    written.clamp(RADIUS_RANGE.0, RADIUS_RANGE.1)
}

/// Whether one loose thing is inside the box the holder's bounding box inflates
/// to.
///
/// The region is a box rather than a sphere, because the radius is applied by
/// inflating a bounding box. A distance test would collect a visibly different
/// set, so the box is what is written here.
#[must_use]
pub fn inside(holder: Position, radius: u32, at: Position) -> bool {
    let reach = i64::from(radius);
    [
        (i64::from(at.0), i64::from(holder.0)),
        (i64::from(at.1), i64::from(holder.1)),
        (i64::from(at.2), i64::from(holder.2)),
    ]
    .iter()
    .all(|(there, here)| (there - here).abs() <= reach)
}

/// Whether the sweep collects one loose thing.
///
/// A dead thing is skipped; a thing the host has marked as not to be moved
/// remotely is skipped; and a thing the holder threw that is still inside its
/// pickup delay is skipped, so a player's own dropped items are not snatched
/// back. An experience orb gets none of those three tests.
#[must_use]
pub const fn collects(loose: &Loose) -> bool {
    if loose.orb {
        return true;
    }
    !(loose.dead || loose.immovable || (loose.pickup.thrown_by_the_holder && loose.pickup.delayed))
}

/// What one tick of a switched-on attractor moved.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Collected {
    /// The indices of the loose things moved to the holder, in the order given.
    pub moved: Vec<usize>,
}

/// Runs one inventory tick of a switched-on attractor.
///
/// The scan runs on the server alone. The original runs it on both sides and
/// performs the move on the server only, so the client scan produces nothing;
/// running it once is the declared change and is what makes this a scan a client
/// does not pay for.
#[must_use]
pub fn tick(
    switched_on: bool,
    holder: Position,
    written_radius: u32,
    loose: &[Loose],
    server_side: bool,
) -> Collected {
    if !switched_on || !server_side {
        return Collected::default();
    }
    let reach = radius(written_radius);
    Collected {
        moved: loose
            .iter()
            .enumerate()
            .filter(|(_, thing)| inside(holder, reach, thing.at) && collects(thing))
            .map(|(index, _)| index)
            .collect(),
    }
}

/// Whether the attractor draws with the enchanted-item highlight.
#[must_use]
pub const fn highlighted(switched_on: bool) -> bool {
    switched_on
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container(slots: Vec<Slot>) -> Container {
        Container { slots }
    }

    fn mine(attractor: bool, switched_on: bool) -> Slot {
        Slot {
            attractor,
            switched_on,
            owned_by_sender: true,
        }
    }

    #[test]
    fn the_payload_carries_one_slot_index_and_round_trips() {
        let sent = Toggle { slot: 37 };
        let bytes = sent.to_bytes();
        let mut reader = PacketReader::new(&bytes);
        let read = Toggle::read(&mut reader).expect("the payload round-trips");
        reader.finish().expect("the payload is read whole");
        assert_eq!(read, sent);
    }

    #[test]
    fn a_buffer_that_ends_inside_the_index_is_refused() {
        let bytes = Toggle { slot: 1 }.to_bytes();
        let mut short = PacketReader::new(&bytes[..bytes.len() - 1]);
        assert!(Toggle::read(&mut short).is_err());
    }

    #[test]
    fn the_payload_travels_from_a_client_to_a_server() {
        let contract = Toggle::contract().expect("the declared name is legal");
        assert_eq!(contract.direction(), Direction::ToServer);
        assert_eq!(contract.id().id().path(), TOGGLE_PACKET);
    }

    #[test]
    fn an_index_outside_the_container_is_refused_before_any_slot_is_read() {
        let mut open = container(vec![mine(true, false)]);
        for named in [1, 2, u32::MAX] {
            assert_eq!(
                Toggle { slot: named }.apply(&mut open),
                Err(Rejected::OutOfRange),
                "slot {named} is outside a container of one"
            );
        }
        assert!(!open.slots[0].switched_on, "nothing was changed");
    }

    #[test]
    fn an_empty_container_refuses_every_index_including_zero() {
        let mut open = container(Vec::new());
        assert_eq!(
            Toggle { slot: 0 }.apply(&mut open),
            Err(Rejected::OutOfRange)
        );
    }

    #[test]
    fn a_container_larger_than_the_ceiling_is_refused_rather_than_scanned() {
        let mut open = container(vec![
            mine(true, false);
            usize::try_from(MAXIMUM_SLOTS).unwrap_or(usize::MAX) + 1
        ]);
        assert_eq!(
            Toggle { slot: 0 }.apply(&mut open),
            Err(Rejected::ContainerTooLarge)
        );
    }

    #[test]
    fn a_slot_the_sender_does_not_own_is_refused_even_when_it_holds_an_attractor() {
        let mut open = container(vec![Slot {
            attractor: true,
            switched_on: false,
            owned_by_sender: false,
        }]);
        assert_eq!(
            Toggle { slot: 0 }.apply(&mut open),
            Err(Rejected::NotTheSenders)
        );
        assert!(!open.slots[0].switched_on);
    }

    #[test]
    fn a_slot_holding_something_else_is_refused() {
        let mut open = container(vec![mine(false, false)]);
        assert_eq!(
            Toggle { slot: 0 }.apply(&mut open),
            Err(Rejected::NotAnAttractor)
        );
    }

    #[test]
    fn an_accepted_toggle_flips_the_one_boolean_and_reports_both_values() {
        let mut open = container(vec![mine(true, false)]);
        assert_eq!(
            Toggle { slot: 0 }.apply(&mut open),
            Ok(Switched {
                was: false,
                now: true
            })
        );
        assert!(open.slots[0].switched_on);
        assert_eq!(
            Toggle { slot: 0 }.apply(&mut open),
            Ok(Switched {
                was: true,
                now: false
            })
        );
        assert!(!open.slots[0].switched_on);
    }

    #[test]
    fn a_fresh_attractor_carries_the_value_off_rather_than_carrying_nothing() {
        let made = fresh();
        assert!(made.attractor);
        assert!(!made.switched_on, "a fresh attractor is switched off");
        assert_eq!(SWITCHED_ON, "switched_on");
        assert!(!highlighted(made.switched_on));
    }

    #[test]
    fn the_pitch_is_chosen_from_the_value_being_left() {
        assert!((pitch(false) - 1.0).abs() < f32::EPSILON);
        assert!((pitch(true) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn using_an_attractor_in_hand_flips_it_and_reports_success() {
        let mut held = fresh();
        let first = use_in_hand(&mut held).expect("an attractor in hand flips");
        assert_eq!(
            first.switched,
            Switched {
                was: false,
                now: true
            }
        );
        assert!(first.success);
        assert!(held.switched_on);

        let second = use_in_hand(&mut held).expect("it flips back");
        assert_eq!(
            second.switched,
            Switched {
                was: true,
                now: false
            }
        );
        assert!(!held.switched_on);
    }

    #[test]
    fn using_something_that_is_not_an_attractor_reports_nothing_and_changes_nothing() {
        let mut held = mine(false, true);
        assert_eq!(use_in_hand(&mut held), None);
        assert!(held.switched_on, "nothing was flipped");
    }

    #[test]
    fn the_hand_and_the_payload_play_the_same_pre_flip_pitch() {
        for switched_on in [false, true] {
            let mut held = mine(true, switched_on);
            let used = use_in_hand(&mut held).expect("an attractor flips");

            let mut open = container(vec![mine(true, switched_on)]);
            let sent = Toggle { slot: 0 }
                .apply(&mut open)
                .expect("the payload flips the same value");

            assert_eq!(used.switched, sent);
            assert!((used.pitch - pitch(sent.was)).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn a_press_sends_only_from_the_two_screens_over_an_attractor_with_an_empty_cursor() {
        let base = Pressed {
            screen: Screen::OwnInventory,
            slot: Some(4),
            carrying: false,
            attractor: true,
        };
        assert_eq!(press(base), Some(Toggle { slot: 4 }));
        assert_eq!(
            press(Pressed {
                screen: Screen::Accessory,
                ..base
            }),
            Some(Toggle { slot: 4 })
        );
        assert_eq!(
            press(Pressed {
                screen: Screen::Other,
                ..base
            }),
            None
        );
        assert_eq!(
            press(Pressed {
                carrying: true,
                ..base
            }),
            None
        );
        assert_eq!(
            press(Pressed {
                attractor: false,
                ..base
            }),
            None
        );
        assert_eq!(press(Pressed { slot: None, ..base }), None);
    }

    #[test]
    fn the_radius_is_held_to_the_range_the_setting_declares() {
        assert_eq!(radius(0), RADIUS_RANGE.0);
        assert_eq!(radius(DEFAULT_RADIUS), DEFAULT_RADIUS);
        assert_eq!(radius(u32::MAX), RADIUS_RANGE.1);
    }

    #[test]
    fn the_region_is_a_box_rather_than_a_sphere() {
        let holder = (0, 0, 0);
        // A corner of the box is further away than the radius, and is inside.
        assert!(inside(holder, 7, (7, 7, 7)));
        assert!(inside(holder, 7, (-7, 0, 7)));
        assert!(!inside(holder, 7, (8, 0, 0)));
    }

    #[test]
    fn the_box_test_does_not_overflow_on_a_position_at_the_edge_of_its_range() {
        assert!(!inside((i32::MIN, 0, 0), 16, (i32::MAX, 0, 0)));
        assert!(inside((i32::MAX, 0, 0), 1, (i32::MAX, 0, 0)));
    }

    #[test]
    fn a_dead_or_immovable_item_is_skipped_and_an_orb_is_never_tested() {
        let mut dead = Loose::item((0, 0, 0));
        dead.dead = true;
        assert!(!collects(&dead));

        let mut immovable = Loose::item((0, 0, 0));
        immovable.immovable = true;
        assert!(!collects(&immovable));

        let mut orb = Loose::orb((0, 0, 0));
        orb.dead = true;
        orb.immovable = true;
        orb.pickup.thrown_by_the_holder = true;
        orb.pickup.delayed = true;
        assert!(collects(&orb), "an orb gets none of the three tests");
    }

    #[test]
    fn an_item_the_holder_just_dropped_is_left_until_its_pickup_delay_runs_out() {
        let mut just_dropped = Loose::item((0, 0, 0));
        just_dropped.pickup.thrown_by_the_holder = true;
        just_dropped.pickup.delayed = true;
        assert!(!collects(&just_dropped));

        just_dropped.pickup.delayed = false;
        assert!(collects(&just_dropped), "the delay is what withholds it");

        let mut someone_elses = Loose::item((0, 0, 0));
        someone_elses.pickup.delayed = true;
        assert!(collects(&someone_elses), "another thrower is not withheld");
    }

    #[test]
    fn a_switched_off_attractor_and_a_client_side_scan_both_move_nothing() {
        let loose = [Loose::item((0, 0, 0))];
        assert_eq!(tick(false, (0, 0, 0), 7, &loose, true).moved.len(), 0);
        assert_eq!(tick(true, (0, 0, 0), 7, &loose, false).moved.len(), 0);
    }

    #[test]
    fn a_tick_moves_what_is_inside_the_box_and_passes_the_exclusions() {
        let mut skipped = Loose::item((1, 0, 0));
        skipped.dead = true;
        let loose = [
            Loose::item((1, 0, 0)),
            skipped,
            Loose::item((100, 0, 0)),
            Loose::orb((2, 0, 0)),
        ];
        assert_eq!(tick(true, (0, 0, 0), 7, &loose, true).moved, vec![0, 3]);
    }

    #[test]
    fn the_highlight_is_drawn_exactly_when_the_attractor_is_switched_on() {
        assert!(highlighted(true));
        assert!(!highlighted(false));
    }
}
