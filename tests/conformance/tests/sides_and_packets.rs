#![forbid(unsafe_code)]

//! Whether client-only code ever loads on a server side, and whether a packet
//! contract round-trips.
//!
//! M4-W3's gate names those two properties. Each is checked here against something
//! that can refuse — `ravel_loader::drive`, which is the real host and the same code
//! a running game uses, and `ravel_loader::deliver_packet`, which is the real
//! routing — rather than against a stub that would prove the stub.
//!
//! The mods under test are `test-mods/client-only` and `test-mods/networking`, a pair
//! written against the public facade alone. Everything they do, they do through
//! `ravel-sdk`.
//!
//! # Why the first property needs a log rather than an assertion
//!
//! "Client-only code never loads on a server side" is a claim about code that did
//! *not* run, and a test that only drove the happy path would prove nothing about it:
//! every assertion would pass with the rule broken. So the client-only work appends a
//! line to a log the test holds, and the test requires the log to be **empty** on
//! every server side and non-empty on the client. The negative control is what makes
//! the empty case mean something.
//!
//! The arrangements below are all checked, because they fail independently:
//!
//! 1. a plan that declares the client side, driven on a server — the loader parks the
//!    mod and no code of any kind runs;
//! 2. a plan that declares **every** side, driven on a server — the mod loads, is
//!    entered, and reaches `Running`, and its client-only work still does not run.
//!    This is the arrangement that makes client-only code reachable on a server, and
//!    it is the one a side declaration alone would not catch;
//! 3. a client-bound packet arriving on a server — refused by the routing before the
//!    mod is called, and refused again inside the mod if it were.

use std::collections::BTreeSet;

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_loader::{
    DispatchRefused, LifecycleEntry, LifecycleMachine, Requirement, deliver_packet, drive,
    executes_mod_code,
};
use ravel_sdk::network::{
    DeliveryRefused, Direction, PacketDelivery, PacketId, WireError, decode, encode, encode_from,
};
use ravel_sdk::sides::ClientOnly;
use ravel_sdk::{Declared, Mod};
use ravel_test_mod_client_only::{
    BUILT_THE_OVERLAY, ClientOnlyMod, Overlay, REDREW_THE_OVERLAY, WorkLog, overlay_id, work_log,
};
use ravel_test_mod_networking::{NetworkingMod, PING_VERSION, Ping, ping_id};

/// The sides a mod that runs everywhere declares.
const EVERY_SIDE: [Side; 3] = Side::ALL;

/// The sides the server half declares.
const SERVER_SIDES: [Side; 2] = [Side::IntegratedServer, Side::DedicatedServer];

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: id(name),
        sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

/// Drives the pair to `Running` on `side` and returns the machine and the log.
///
/// `client_sides` is the plan's declaration for the client half, which is what makes
/// arrangements 1 and 2 above two calls to one function rather than two functions.
fn started(side: Side, client_sides: &[Side]) -> (LifecycleMachine, WorkLog) {
    let log = work_log();
    let composed: Vec<(LifecycleEntry, Box<dyn Mod>)> = vec![
        (
            entry("client_only", client_sides),
            Box::new(ClientOnlyMod::new(side, &log)),
        ),
        (
            entry("networking", &SERVER_SIDES),
            Box::new(NetworkingMod::new()),
        ),
    ];

    let (machine, _) = drive(side, composed).expect("a side-mismatched mod is parked, not refused");
    (machine, log)
}

/// Drives a plan to `Running` while the caller keeps the mod values.
///
/// `drive` takes ownership, which is right for a host and wrong for a test that
/// wants to ask a mod what it read. This is the same sequence over borrowed values.
fn running(
    side: Side,
    entries: Vec<LifecycleEntry>,
    mods: &mut [&mut dyn Mod],
) -> LifecycleMachine {
    let mut machine = LifecycleMachine::new(side, entries).expect("a legal plan");
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
        LifecycleState::Registered,
        LifecycleState::Running,
    ] {
        machine.advance_all(state).expect("a legal step");
        if !executes_mod_code(state) {
            continue;
        }
        for host in mods.iter_mut() {
            // A parked mod is at `Resolved` and is not entered, which is the same
            // rule `ravel_loader::drive` applies and the reason this loop asks the
            // machine rather than assuming the whole slice moved.
            if machine.state(host.id()) == Ok(state) {
                host.enter(state).expect("neither mod refuses a step");
            }
        }
    }
    machine
}

// ------------------------------------ client-only code on a server side ---

#[test]
fn a_client_only_mod_is_parked_on_every_server_side_and_runs_no_code_at_all() {
    for side in SERVER_SIDES {
        let (machine, log) = started(side, &ClientOnlyMod::SIDES);

        assert_eq!(
            machine.state(ClientOnlyMod::declared_id()),
            Ok(LifecycleState::Resolved),
            "`{}` did not park the client-only mod",
            side.name()
        );
        assert!(
            log.lock().expect("nothing panicked").is_empty(),
            "client-only code ran on `{}`",
            side.name()
        );

        // The server half is a mod that does load here, so the plan is not simply
        // one nothing started.
        assert_eq!(
            machine.state(NetworkingMod::declared_id()),
            Ok(LifecycleState::Running)
        );
    }
}

#[test]
fn the_same_pair_on_the_client_does_run_the_client_only_code() {
    // The negative control. Without it, the test above would pass with
    // `ClientOnly::built_on` never calling its closure anywhere, and the property
    // would be "client-only code never runs", which is not the requirement.
    let (machine, log) = started(Side::Client, &ClientOnlyMod::SIDES);

    assert_eq!(
        machine.state(ClientOnlyMod::declared_id()),
        Ok(LifecycleState::Running)
    );
    assert_eq!(
        *log.lock().expect("nothing panicked"),
        [BUILT_THE_OVERLAY],
        "the client did not build the client-only resources"
    );
}

#[test]
fn a_mod_that_loads_on_a_server_side_still_does_not_run_its_client_only_work_there() {
    // The arrangement the gate asks for: the client-only code is reachable, because
    // the plan declares every side and the loader has no reason to park anything.
    // The mod is entered, registers, and reaches `Running` — and the client-only
    // work still does not run, because `ClientOnly` and not the side declaration is
    // what keeps it out.
    for side in SERVER_SIDES {
        let (machine, log) = started(side, &EVERY_SIDE);

        assert_eq!(
            machine.state(ClientOnlyMod::declared_id()),
            Ok(LifecycleState::Running),
            "the client-only mod was parked, so this arrangement proved the wrong thing on `{}`",
            side.name()
        );
        assert!(
            log.lock().expect("nothing panicked").is_empty(),
            "client-only code ran on `{}` in a mod that loads there",
            side.name()
        );
    }
}

#[test]
fn reaching_for_client_only_resources_on_a_server_side_is_refused_and_names_the_side() {
    let log = work_log();
    for side in Side::ALL {
        let host = ClientOnlyMod::new(side, &log);
        if side.is_server() {
            let refusal = host.drawn().expect_err(side.name());
            assert_eq!(refusal.mod_id(), ClientOnlyMod::declared_id());
            assert!(refusal.to_string().contains(side.name()), "{refusal}");
        } else {
            host.drawn().expect("the client holds them");
        }
    }
}

#[test]
fn a_value_built_for_the_client_is_still_refused_to_the_server_half_of_one_process() {
    // The accessor's own half of the rule, and it needs its own case: every test
    // above builds through `ClientOnly::built_on` on a server side, where the value
    // does not exist, so the accessor's side check is masked by the constructor's —
    // a mutation that deleted the check left all of them at exit 0.
    //
    // This is the arrangement where only the accessor can refuse, and it is the real
    // one: single-player runs a client and an integrated server in one process, so a
    // value built for the client half is reachable from code running as the server
    // half. `ClientOnly` is public through `ravel_sdk::sides`, which is how a mod
    // author reaches this too.
    let built_for_the_client = ClientOnly::built_on(Side::Client, || "the overlay");
    assert!(built_for_the_client.is_present(), "the value exists");

    for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
        let refusal = built_for_the_client.get(side).expect_err(side.name());
        assert_eq!(refusal.side(), side);
        assert!(refusal.to_string().contains(side.name()), "{refusal}");
    }
    assert_eq!(built_for_the_client.get(Side::Client), Ok(&"the overlay"));
}

// ------------------------------------------------- packets across the pair ---

#[test]
fn a_client_bound_packet_arriving_on_a_server_side_reaches_no_mod() {
    for side in SERVER_SIDES {
        let log = work_log();
        let mut client = ClientOnlyMod::new(side, &log);
        let mut server = NetworkingMod::new();
        // Every side is declared, so the client-only mod is present and running
        // here: nothing but the direction can refuse this delivery.
        let machine = running(
            side,
            vec![
                entry("client_only", &EVERY_SIDE),
                entry("networking", &SERVER_SIDES),
            ],
            &mut [&mut client, &mut server],
        );

        let arriving = PacketDelivery::new(
            overlay_id(),
            Direction::ToClient,
            side,
            encode(&Overlay::new(vec!["drawn".to_owned()])),
        );
        let refusal = deliver_packet(&machine, &mut [&mut client, &mut server], &arriving)
            .expect_err("a server side is not an end of a client-bound link");

        assert_eq!(
            refusal,
            DispatchRefused::PacketWrongSide {
                id: overlay_id(),
                direction: Direction::ToClient,
                side,
            }
        );
        assert!(refusal.to_string().contains(side.name()), "{refusal}");
        assert!(
            log.lock().expect("nothing panicked").is_empty(),
            "client-only code ran while handling a packet on `{}`",
            side.name()
        );
    }
}

#[test]
fn the_same_packet_on_the_client_round_trips_into_the_overlay_the_server_sent() {
    // The negative control for the refusal above, and the client half of the
    // round-trip property: the value the sender built is the value the receiver drew.
    let sent = Overlay::new(vec!["copper lanterns: 3".to_owned(), String::new()]);
    let payload = encode_from(&sent, Side::DedicatedServer).expect("a server sends it");

    let log = work_log();
    let mut client = ClientOnlyMod::new(Side::Client, &log);
    let machine = running(
        Side::Client,
        vec![entry("client_only", &ClientOnlyMod::SIDES)],
        &mut [&mut client],
    );

    let arriving = PacketDelivery::new(overlay_id(), Direction::ToClient, Side::Client, payload);
    let delivered = deliver_packet(&machine, &mut [&mut client], &arriving)
        .expect("the client is the receiving end");

    assert_eq!(delivered.received(), [ClientOnlyMod::declared_id().clone()]);
    assert!(delivered.is_complete());
    assert_eq!(
        client.drawn().expect("the client holds them"),
        sent.lines(),
        "the overlay the client drew is not the overlay the server sent"
    );
    assert_eq!(
        *log.lock().expect("nothing panicked"),
        [BUILT_THE_OVERLAY, REDREW_THE_OVERLAY]
    );
}

#[test]
fn a_server_bound_packet_round_trips_from_the_client_to_the_mod_that_owns_it() {
    let sent = Ping::noted(4_294_967_295, "a note a player typed", true);
    let payload = encode_from(&sent, Side::Client).expect("a client sends it");

    let log = work_log();
    let mut client = ClientOnlyMod::new(Side::DedicatedServer, &log);
    let mut server = NetworkingMod::new();
    let machine = running(
        Side::DedicatedServer,
        vec![
            entry("client_only", &ClientOnlyMod::SIDES),
            entry("networking", &SERVER_SIDES),
        ],
        &mut [&mut client, &mut server],
    );

    let arriving = PacketDelivery::new(
        ping_id(),
        Direction::ToServer,
        Side::DedicatedServer,
        payload,
    );
    let delivered = deliver_packet(&machine, &mut [&mut client, &mut server], &arriving)
        .expect("the server half owns the contract and is running");

    assert_eq!(delivered.received(), [NetworkingMod::declared_id().clone()]);
    assert_eq!(
        server.answered(),
        [sent],
        "the ping the server read is not the ping the client sent"
    );
}

#[test]
fn a_side_that_may_not_send_a_contract_encodes_nothing_for_either_direction() {
    // Both directions of the sending rule, so neither is a rule that happens to
    // hold for the one contract that was tried.
    assert!(matches!(
        encode_from(&Ping::new(1), Side::DedicatedServer),
        Err(DeliveryRefused::WrongSide { .. })
    ));
    assert!(matches!(
        encode_from(&Overlay::new(Vec::new()), Side::Client),
        Err(DeliveryRefused::WrongSide { .. })
    ));
    assert!(encode_from(&Ping::new(1), Side::Client).is_ok());
    assert!(encode_from(&Overlay::new(Vec::new()), Side::IntegratedServer).is_ok());
}

// --------------------------------------------- payloads that are refused ---

/// Routes `payload` to the server half and returns what the mod read, if anything.
///
/// Returning the mod is what makes "refused" mean "read nothing" here rather than
/// "returned an error and did something anyway".
fn ping_arriving(payload: Vec<u8>, id_named: PacketId) -> (Option<String>, NetworkingMod) {
    let mut server = NetworkingMod::new();
    let machine = running(
        Side::DedicatedServer,
        vec![entry("networking", &SERVER_SIDES)],
        &mut [&mut server],
    );

    let arriving = PacketDelivery::new(
        id_named,
        Direction::ToServer,
        Side::DedicatedServer,
        payload,
    );
    let delivered = deliver_packet(&machine, &mut [&mut server], &arriving)
        .expect("the routing reached the owner");
    let failure = delivered.failure().map(ToString::to_string);
    (failure, server)
}

#[test]
fn a_truncated_payload_is_refused_at_every_length_it_could_be_cut_to() {
    let full = encode(&Ping::noted(7, "note", false));
    for cut in 0..full.len() {
        let (failure, server) = ping_arriving(full[..cut].to_vec(), ping_id());
        assert!(
            failure.is_some(),
            "a payload cut to {cut} bytes was read as a ping"
        );
        assert!(server.answered().is_empty(), "cut at {cut}");
    }

    // And the whole payload is accepted, so the refusals above are about the cut.
    let (failure, server) = ping_arriving(full, ping_id());
    assert_eq!(failure, None);
    assert_eq!(server.answered().len(), 1);
}

#[test]
fn a_length_prefix_past_the_end_is_refused_with_the_length_it_claimed() {
    // A hand-built payload rather than a cut one, and the exact error rather than
    // the presence of one. Both matter: a decoder that clamped an over-long length
    // to what is left still ends in a refusal for these two contracts, because both
    // have a field after the text — so a suite that only asked "was it refused"
    // stayed at exit 0 while a length prefix was being ignored, which a mutation
    // demonstrated. The claimed length is what tells the two apart.
    let mut hostile = Vec::new();
    let name = ping_id().to_string();
    let length = u32::try_from(name.len()).expect("a contract name shorter than 4 GiB");
    hostile.extend_from_slice(&length.to_be_bytes());
    hostile.extend_from_slice(name.as_bytes());
    hostile.extend_from_slice(&PING_VERSION.to_be_bytes());
    hostile.extend_from_slice(&7_u32.to_be_bytes());
    hostile.extend_from_slice(&u32::MAX.to_be_bytes());

    assert_eq!(
        decode::<Ping>(&hostile),
        Err(WireError::Truncated {
            needed: u32::MAX as usize,
            available: 0,
        }),
        "the note's length prefix claims the whole address space and nothing follows it"
    );

    let (failure, server) = ping_arriving(hostile, ping_id());
    let failure = failure.expect("a length prefix past the end");
    assert!(failure.contains("4294967295"), "{failure}");
    assert!(server.answered().is_empty());
}

#[test]
fn a_payload_with_bytes_left_over_is_refused_rather_than_read_as_the_packet_inside_it() {
    let mut trailing = encode(&Ping::new(1));
    trailing.push(0);

    let (failure, server) = ping_arriving(trailing, ping_id());
    let failure = failure.expect("a byte nothing accounts for");
    assert!(failure.contains("accounts for"), "{failure}");
    assert!(server.answered().is_empty());
}

#[test]
fn a_payload_written_under_another_version_is_refused_rather_than_misread() {
    let mut other = encode(&Ping::new(1));
    let offset = 4 + ping_id().to_string().len();
    other[offset..offset + 4].copy_from_slice(&(PING_VERSION + 1).to_be_bytes());

    let (failure, server) = ping_arriving(other.clone(), ping_id());
    let failure = failure.expect("another version");
    assert!(failure.contains("version"), "{failure}");
    assert!(server.answered().is_empty());

    // And the wire error says which version each end speaks, rather than only that
    // something was wrong.
    assert_eq!(
        decode::<Ping>(&other),
        Err(WireError::Version {
            expected: PING_VERSION,
            found: PING_VERSION + 1,
        })
    );
}

#[test]
fn a_payload_written_under_another_contract_is_refused_by_name() {
    // The overlay's bytes, addressed to the ping's contract. Both are real contracts
    // in this pair, so this is the mix-up a running game could actually make.
    let (failure, server) = ping_arriving(encode(&Overlay::new(Vec::new())), ping_id());
    let failure = failure.expect("the payload is an overlay");
    assert!(failure.contains(&overlay_id().to_string()), "{failure}");
    assert!(server.answered().is_empty());
}

#[test]
fn a_delivery_naming_one_contract_and_carrying_another_is_refused() {
    // The delivery's own label is a claim rather than a fact, so a label that
    // disagrees with the framing inside the payload is refused too. The label stays
    // in the server half's namespace, because that is what the routing reads: a
    // label naming another mod would be refused before the payload was looked at,
    // which is a different test.
    let other = PacketId::new("networking:other".parse().expect("a legal content id"));
    let (failure, server) = ping_arriving(encode(&Ping::new(1)), other.clone());

    let failure = failure.expect("the label and the payload disagree");
    assert!(failure.contains(&other.to_string()), "{failure}");
    assert!(server.answered().is_empty());
}
