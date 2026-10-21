#![forbid(unsafe_code)]

//! Rejected wire deliveries must not fail their shipped counterpart's lifecycle.

use ravel_core::LifecycleState;
use ravel_loader::{LifecycleMachine, LoadedMods, deliver_packet, start_all};
use ravel_observability::Profiler;
use ravel_sdk::network::{Direction, Packet, PacketDelivery, PacketId};
use ravel_sdk::{Mod, Side};

use super::{lifecycle_entry, manifests};

fn assert_declined_packets_keep_running<M: Mod>(
    mut owner: M,
    side: Side,
    deliveries: impl IntoIterator<Item = PacketDelivery>,
) -> M {
    let owner_id = owner.id().clone();
    let manifest = manifests()
        .into_iter()
        .find(|manifest| manifest.id == owner_id)
        .expect("the owner has a shipped manifest");
    let mut machine =
        LifecycleMachine::new(side, vec![lifecycle_entry(&manifest)]).expect("a legal lifecycle");
    let environment = ravel_minecraft_adapter::environment();
    let mut mods: [&mut dyn Mod; 1] = [&mut owner];
    start_all(
        &mut LoadedMods::new(&mut machine, &mut mods).expect("the owner is hosted"),
        &environment,
    )
    .expect("the owner starts");
    for delivery in deliveries {
        let delivered = deliver_packet(
            &mut LoadedMods::new(&mut machine, &mut mods).expect("the owner is hosted"),
            &delivery,
            &mut Profiler::off(),
            &environment,
        )
        .expect("the loader declines the delivery");
        assert!(delivered.declined().is_some());
        assert!(delivered.failure().is_none());
        assert_eq!(delivered.received(), []);
        assert!(!delivered.is_complete());
        assert_eq!(machine.state(&owner_id), Ok(LifecycleState::Running));
    }
    owner
}

fn undeclared(owner: &dyn Mod, side: Side) -> PacketDelivery {
    PacketDelivery::new(
        PacketId::new(
            format!("{}:undeclared", owner.id())
                .parse()
                .expect("a packet id"),
        ),
        if side == Side::Client {
            Direction::ToClient
        } else {
            Direction::ToServer
        },
        side,
        Vec::new(),
    )
}

#[test]
fn rvl025_malformed_and_undeclared_packets_leave_the_loader_owner_running() {
    let side = Side::DedicatedServer;
    let owner = rvl_025_counterpart::Entry::new(side);
    let malformed = PacketDelivery::new(
        rvl_025_counterpart::network::HeldItem::contract()
            .id()
            .clone(),
        Direction::ToServer,
        side,
        Vec::new(),
    );
    let unknown = undeclared(&owner, side);
    assert_declined_packets_keep_running(owner, side, [malformed, unknown]);
}

#[test]
fn rvl086_malformed_and_undeclared_packets_leave_the_loader_owner_running() {
    let side = Side::Client;
    let owner = rvl_086_counterpart::Entry::new();
    let contract = rvl_086_counterpart::Decisive::contract();
    let malformed = PacketDelivery::new(
        contract.id().clone(),
        contract.direction(),
        side,
        Vec::new(),
    );
    let unknown = undeclared(&owner, side);
    assert_declined_packets_keep_running(owner, side, [malformed, unknown]);
}

#[test]
fn rvl180_a_malformed_readout_leaves_the_loader_owner_running() {
    let side = Side::Client;
    let owner = rvl_180_counterpart::Entry::new(side);
    let contract = rvl_180_counterpart::presentation::Readout::contract();
    let delivery = PacketDelivery::new(
        contract.id().clone(),
        contract.direction(),
        side,
        Vec::new(),
    );
    assert_declined_packets_keep_running(owner, side, [delivery]);
}

#[test]
fn rvl210_an_undeclared_packet_leaves_its_receiving_mod_running() {
    let side = Side::DedicatedServer;
    let expected =
        assert_declined_packets_keep_running(rvl_210_counterpart::Entry::new(), side, []);
    let owner = rvl_210_counterpart::Entry::new();
    let delivery = undeclared(&owner, side);
    let owner = assert_declined_packets_keep_running(owner, side, [delivery]);
    assert_eq!(format!("{owner:?}"), format!("{expected:?}"));
}
