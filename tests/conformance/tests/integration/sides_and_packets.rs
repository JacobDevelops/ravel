#![forbid(unsafe_code)]

//! Side isolation through declarations and manifests, plus packet routing and framing.

use std::collections::BTreeSet;

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_loader::{
    DispatchRefused, LifecycleEntry, LifecycleMachine, ModSurface, Requirement, admitted,
    deliver_packet,
};
use ravel_package::PackageManifest;
use ravel_resolve::resolve;
use ravel_sdk::network::{
    DeliveryRefused, Direction, PacketDelivery, PacketId, WireError, decode, encode, encode_from,
};
use ravel_sdk::sides::ClientOnly;
use ravel_sdk::{Declared, Mod};
use ravel_test_mod_client_only::{
    BUILT_THE_OVERLAY, ClientOnlyMod, Overlay, REDREW_THE_OVERLAY, WorkLog, overlay_id, work_log,
};
use ravel_test_mod_networking::{NetworkingMod, PING_VERSION, Ping, ping_id};
use ravel_test_mod_noop::NoopMod;
use ravel_testkit::{Host, PipelineRun, TestPackage, run_pipeline};

const EVERY_SIDE: [Side; 3] = Side::ALL;

// Derive server sides so new sides cannot silently escape coverage.
fn server_sides() -> Vec<Side> {
    Side::ALL
        .into_iter()
        .filter(|side| side.is_server())
        .collect()
}

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn environment() -> ravel_core::FailureEnvironment {
    let running = host();
    ravel_core::FailureEnvironment {
        ravel: running.ravel,
        minecraft: running.minecraft,
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    }
}

fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: id(name),
        sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

fn started(side: Side, client_sides: &[Side]) -> (LifecycleMachine, WorkLog) {
    let log = work_log();
    let mut mods: Vec<Box<dyn Mod>> = vec![
        Box::new(ClientOnlyMod::new(side, &log)),
        Box::new(NetworkingMod::new()),
    ];
    let mut machine = LifecycleMachine::new(
        side,
        vec![
            entry("client_only", client_sides),
            entry("networking", &server_sides()),
        ],
    )
    .expect("a legal side-specific lifecycle");
    ravel_loader::start_all(
        &mut ravel_loader::LoadedMods::new(&mut machine, &mut mods)
            .expect("every active member has a mod value"),
        &environment(),
    )
    .expect("a side-mismatched mod is parked, not refused");
    (machine, log)
}

// Borrow mods to inspect their received state after dispatch.
fn running(
    side: Side,
    entries: Vec<LifecycleEntry>,
    mods: &mut [&mut dyn Mod],
) -> LifecycleMachine {
    let mut machine = LifecycleMachine::new(side, entries).expect("a legal lifecycle plan");
    ravel_loader::start_all(
        &mut ravel_loader::LoadedMods::new(&mut machine, mods).expect("all active mods have hosts"),
        &environment(),
    )
    .expect("the production driver starts the set");
    machine
}

#[test]
fn a_client_only_mod_is_parked_on_every_server_side_and_runs_no_code_at_all() {
    for side in server_sides() {
        let (machine, log) = started(side, &ClientOnlyMod::SIDES);

        assert_eq!(
            machine.state(ClientOnlyMod::declared_id()),
            Ok(LifecycleState::Resolved),
            "`{}` did not park the client-only mod",
            side.as_str()
        );
        assert!(
            log.lock().expect("nothing panicked").is_empty(),
            "client-only code ran on `{}`",
            side.as_str()
        );

        assert_eq!(
            machine.state(NetworkingMod::declared_id()),
            Ok(LifecycleState::Running)
        );
    }
}

#[test]
fn the_same_pair_on_the_client_does_run_the_client_only_code() {
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
    for side in server_sides() {
        let (machine, log) = started(side, &EVERY_SIDE);

        assert_eq!(
            machine.state(ClientOnlyMod::declared_id()),
            Ok(LifecycleState::Running),
            "the client-only mod was parked, so this arrangement proved the wrong thing on `{}`",
            side.as_str()
        );
        assert!(
            log.lock().expect("nothing panicked").is_empty(),
            "client-only code ran on `{}` in a mod that loads there",
            side.as_str()
        );
    }
}

#[test]
fn reaching_for_client_only_resources_on_a_server_side_is_refused_and_names_the_side() {
    let log = work_log();
    for side in Side::ALL {
        let host = ClientOnlyMod::new(side, &log);
        if side.is_server() {
            let refusal = host.drawn().expect_err(side.as_str());
            assert!(refusal.to_string().contains(side.as_str()), "{refusal}");
        } else {
            host.drawn().expect("the client holds them");
        }
    }
}

#[test]
fn a_value_built_for_the_client_is_still_refused_to_the_server_half_of_one_process() {
    // Integrated servers can reach values constructed by the client in the same process.
    let built_for_the_client = ClientOnly::built_on(Side::Client, || "the overlay");
    assert!(built_for_the_client.is_present(), "the value exists");

    for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
        let refusal = built_for_the_client.get(side).expect_err(side.as_str());
        assert_eq!(refusal.side(), Some(side));
        assert!(refusal.to_string().contains(side.as_str()), "{refusal}");
    }
    assert_eq!(built_for_the_client.get(Side::Client), Ok(&"the overlay"));
}

const CLIENT_ONLY_PACKAGE: &str = "test-mods/client-only";
const NOOP_PACKAGE: &str = "tests/fixtures/public/packages/noop";

use super::repository_root;

fn discovered(directory: &str) -> TestPackage {
    TestPackage::read(repository_root().join(directory)).expect("the fixture package is readable")
}

fn host() -> Host {
    let manifest = PackageManifest::parse(&discovered(CLIENT_ONLY_PACKAGE).document)
        .expect("the fixture manifest parses");
    let noop = PackageManifest::parse(&discovered(NOOP_PACKAGE).document)
        .expect("the companion fixture manifest parses");
    let minecraft = noop
        .compatibility
        .minecraft
        .iter()
        .map(ToString::to_string)
        .find(|version| manifest.compatibility.check_minecraft(version).is_ok())
        .expect("the real package admits the companion fixture's Minecraft version");
    assert!(
        repository_root()
            .join("minecraft/versions")
            .join(&minecraft)
            .is_dir(),
        "the client-only package declares a Minecraft version `minecraft/versions/` does not pin"
    );
    Host {
        ravel: env!("CARGO_PKG_VERSION")
            .parse()
            .expect("the crate version is semver"),
        minecraft,
    }
}

fn pipeline_on(side: Side) -> (PipelineRun, WorkLog) {
    let log = work_log();
    let mut client = ClientOnlyMod::new(side, &log);
    let mut noop = NoopMod::new();
    let completed = run_pipeline(
        &[discovered(CLIENT_ONLY_PACKAGE), discovered(NOOP_PACKAGE)],
        &host(),
        side,
        &mut [&mut client, &mut noop],
    )
    .expect("a side-mismatched package is parked, not refused");
    (completed, log)
}

#[test]
fn a_manifest_declaring_the_client_side_alone_is_parked_by_the_pipeline_on_every_server_side() {
    for side in server_sides() {
        let (completed, log) = pipeline_on(side);
        let client_only = ClientOnlyMod::declared_id();

        assert_eq!(
            completed.states_of(client_only),
            [LifecycleState::Validated, LifecycleState::Resolved],
            "`{}` did not park the mod its manifest excludes",
            side.as_str()
        );
        assert!(
            !completed.plan().order().contains(client_only),
            "the resolver ordered a package that does not load on `{}`",
            side.as_str()
        );
        assert!(
            log.lock().expect("nothing panicked").is_empty(),
            "client-only code ran on `{}`",
            side.as_str()
        );

        assert_eq!(
            *completed
                .states_of(&"noop".parse::<ModId>().expect("a legal mod id"))
                .last()
                .expect("the no-op package loads on every side"),
            LifecycleState::Stopped
        );
    }
}

#[test]
fn the_same_manifest_on_the_client_loads_and_runs_the_client_only_code() {
    let (completed, log) = pipeline_on(Side::Client);
    let client_only = ClientOnlyMod::declared_id();

    assert!(completed.plan().order().contains(client_only));
    assert_eq!(
        *completed
            .states_of(client_only)
            .last()
            .expect("the package loads on the client"),
        LifecycleState::Stopped
    );
    assert_eq!(
        *log.lock().expect("nothing panicked"),
        [BUILT_THE_OVERLAY],
        "the client did not run the client-only work"
    );
}

#[test]
fn the_mod_list_marks_a_parked_package_as_installed_and_not_loading_here() {
    let side = Side::DedicatedServer;
    let log = work_log();
    let manifests: Vec<PackageManifest> = [CLIENT_ONLY_PACKAGE, NOOP_PACKAGE]
        .into_iter()
        .map(|directory| {
            PackageManifest::parse(&discovered(directory).document).expect("a legal manifest")
        })
        .collect();
    let plan = resolve(&manifests, side).expect("a legal package set");
    let running = host();
    let entries = admitted(&plan, &manifests, &running.ravel, &running.minecraft)
        .expect("the fixture packages declare the version this repository pins");
    let mut mods: Vec<Box<dyn Mod>> = entries
        .iter()
        .map(|entry| -> Box<dyn Mod> {
            if entry.mod_id == *ClientOnlyMod::declared_id() {
                Box::new(ClientOnlyMod::new(side, &log))
            } else {
                Box::new(NoopMod::new())
            }
        })
        .collect();
    let mut machine = LifecycleMachine::new(side, entries).expect("a legal admitted lifecycle");
    ravel_loader::start_all(
        &mut ravel_loader::LoadedMods::new(&mut machine, &mut mods)
            .expect("every active member has a mod value"),
        &environment(),
    )
    .expect("a parked package does not fail the set");

    let surface = ModSurface::of(&machine);
    let listed = surface
        .entries()
        .iter()
        .find(|entry| entry.mod_id == *ClientOnlyMod::declared_id())
        .expect("a parked package is still listed");
    assert!(
        !listed.loads_here,
        "the mod list says a client-only package loads on a dedicated server"
    );
    assert_eq!(listed.state, LifecycleState::Resolved);
    assert!(!surface.has_failure(), "parking is not a failure");
}

#[test]
fn a_client_bound_packet_arriving_on_a_server_side_reaches_no_mod() {
    for side in server_sides() {
        let log = work_log();
        let mut client = ClientOnlyMod::new(side, &log);
        let mut server = NetworkingMod::new();
        // Keep the recipient running so only packet direction can refuse delivery.
        let mut machine = running(
            side,
            vec![
                entry("client_only", &EVERY_SIDE),
                entry("networking", &server_sides()),
            ],
            &mut [&mut client, &mut server],
        );

        let arriving = PacketDelivery::new(
            overlay_id(),
            Direction::ToClient,
            side,
            encode(&Overlay::new(vec!["drawn".to_owned()])),
        );
        let refusal = deliver_packet(
            &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
                &mut machine,
                &mut [&mut client, &mut server],
            )
            .expect("all active mods have hosts"),
            &arriving,
            &mut ravel_observability::Profiler::off(),
            &ravel_minecraft_adapter::environment(),
        )
        .expect_err("a server side is not an end of a client-bound link");

        assert_eq!(
            refusal,
            DispatchRefused::PacketWrongSide {
                id: overlay_id(),
                direction: Direction::ToClient,
                side,
            }
        );
        assert!(refusal.to_string().contains(side.as_str()), "{refusal}");
        assert!(
            log.lock().expect("nothing panicked").is_empty(),
            "client-only code ran while handling a packet on `{}`",
            side.as_str()
        );
    }
}

#[test]
fn the_same_packet_on_the_client_round_trips_into_the_overlay_the_server_sent() {
    let sent = Overlay::new(vec!["copper lanterns: 3".to_owned(), String::new()]);
    let payload = encode_from(&sent, Side::DedicatedServer).expect("a server sends it");

    let log = work_log();
    let mut client = ClientOnlyMod::new(Side::Client, &log);
    let mut machine = running(
        Side::Client,
        vec![entry("client_only", &ClientOnlyMod::SIDES)],
        &mut [&mut client],
    );

    let arriving = PacketDelivery::new(overlay_id(), Direction::ToClient, Side::Client, payload);
    let delivered = deliver_packet(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut client])
            .expect("all active mods have hosts"),
        &arriving,
        &mut ravel_observability::Profiler::off(),
        &ravel_minecraft_adapter::environment(),
    )
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
    let mut machine = running(
        Side::DedicatedServer,
        vec![
            entry("client_only", &ClientOnlyMod::SIDES),
            entry("networking", &server_sides()),
        ],
        &mut [&mut client, &mut server],
    );

    let arriving = PacketDelivery::new(
        ping_id(),
        Direction::ToServer,
        Side::DedicatedServer,
        payload,
    );
    let delivered = deliver_packet(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut client, &mut server],
        )
        .expect("all active mods have hosts"),
        &arriving,
        &mut ravel_observability::Profiler::off(),
        &ravel_minecraft_adapter::environment(),
    )
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

fn ping_arriving(payload: Vec<u8>, id_named: PacketId) -> (Option<String>, NetworkingMod) {
    let mut server = NetworkingMod::new();
    let mut machine = running(
        Side::DedicatedServer,
        vec![entry("networking", &server_sides())],
        &mut [&mut server],
    );

    let arriving = PacketDelivery::new(
        id_named,
        Direction::ToServer,
        Side::DedicatedServer,
        payload,
    );
    let delivered = deliver_packet(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut server])
            .expect("all active mods have hosts"),
        &arriving,
        &mut ravel_observability::Profiler::off(),
        &ravel_minecraft_adapter::environment(),
    )
    .expect("the routing reached the owner");
    assert!(
        delivered.failure().is_none(),
        "packet declines do not fail the owner"
    );
    assert!(
        delivered.failed_by().is_none(),
        "packet declines are not quarantines"
    );
    let decline = delivered.declined().map(ToString::to_string);
    assert_eq!(delivered.is_complete(), decline.is_none());
    (decline, server)
}

#[test]
fn a_truncated_payload_is_refused_at_every_length_it_could_be_cut_to() {
    let full = encode(&Ping::noted(7, "note", false));
    for cut in 0..full.len() {
        let (decline, server) = ping_arriving(full[..cut].to_vec(), ping_id());
        assert!(
            decline.is_some(),
            "a payload cut to {cut} bytes was read as a ping"
        );
        assert!(server.answered().is_empty(), "cut at {cut}");
    }

    let (decline, server) = ping_arriving(full, ping_id());
    assert_eq!(decline, None);
    assert_eq!(server.answered().len(), 1);
}

#[test]
fn a_length_prefix_past_the_end_is_refused_with_the_length_it_claimed() {
    // A clamped prefix still fails on later fields; assert the exact claimed length.
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
            needed: usize::try_from(u32::MAX).expect("the packet length fits usize"),
            available: 0,
        }),
        "the note's length prefix claims the whole address space and nothing follows it"
    );

    let (decline, server) = ping_arriving(hostile, ping_id());
    let decline = decline.expect("a length prefix past the end");
    assert!(decline.contains("4294967295"), "{decline}");
    assert_eq!(server.answered(), []);
}

#[test]
fn a_payload_with_bytes_left_over_is_refused_rather_than_read_as_the_packet_inside_it() {
    let mut trailing = encode(&Ping::new(1));
    trailing.push(0);

    let (decline, server) = ping_arriving(trailing, ping_id());
    let decline = decline.expect("a byte nothing accounts for");
    assert!(decline.contains("accounts for"), "{decline}");
    assert_eq!(server.answered(), []);
}

#[test]
fn a_payload_written_under_another_version_is_refused_rather_than_misread() {
    let mut other = encode(&Ping::new(1));
    let offset = 4 + ping_id().to_string().len();
    other[offset..offset + 4].copy_from_slice(&(PING_VERSION + 1).to_be_bytes());

    let (decline, server) = ping_arriving(other.clone(), ping_id());
    let decline = decline.expect("another version");
    assert!(decline.contains("version"), "{decline}");
    assert_eq!(server.answered(), []);

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
    let (decline, server) = ping_arriving(encode(&Overlay::new(Vec::new())), ping_id());
    let decline = decline.expect("the payload is an overlay");
    assert!(decline.contains(&overlay_id().to_string()), "{decline}");
    assert_eq!(server.answered(), []);
}

#[test]
fn a_delivery_naming_one_contract_and_carrying_another_is_refused() {
    // Keep the label in the owner's namespace so routing reaches payload validation.
    let other = PacketId::new("networking:other".parse().expect("a legal content id"));
    let (decline, server) = ping_arriving(encode(&Ping::new(1)), other.clone());

    let decline = decline.expect("the label and the payload disagree");
    assert!(decline.contains(&other.to_string()), "{decline}");
    assert_eq!(server.answered(), []);
}
