#![forbid(unsafe_code)]

//! Migration chains, service dependencies, and whether a failure in either is
//! closed.
//!
//! M4-W4's gate names those three. Each is checked against the code a running host
//! uses — `ravel_loader::open_saved_data` and `ravel_loader::open_services` — over
//! mods written against `ravel-sdk` alone, rather than against a harness that would
//! prove the harness.
//!
//! # Why each property needs more than a passing call
//!
//! **A migration chain** is only interesting when it is longer than one hop and
//! when the shapes differ, because a single-hop chain over one shape is
//! indistinguishable from no migration at all. `ravel-test-mod-inter-mod-service`
//! reads three versions whose shapes differ at every step, so the value below is
//! reachable from version 1 by no shorter route: an implementation that ran one
//! step, or ran them out of order, produces something the current reader refuses.
//!
//! **A service order** is only interesting when the mods are supplied in the wrong
//! order, so every test here hands the consumer to the loader first.
//!
//! **Failure closure** is a claim about what did *not* happen, so each refusal is
//! checked against the state the mod is left holding rather than only against the
//! error: a mod holding nothing after a refused world is what tells a refused world
//! apart from a world that opened empty.

use std::collections::{BTreeMap, BTreeSet};

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_loader::{
    LifecycleEntry, LifecycleMachine, Requirement, SaveRefused, ServiceRefused, open_configuration,
    open_saved_data, open_services,
};
use ravel_save::{
    SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataVersion,
};
use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
use ravel_sdk::content::ContentId;
use ravel_sdk::save::SavedPayload;
use ravel_sdk::services::{ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest};
use ravel_sdk::{Declared, Mod, ModError, declare_mod, json};
use ravel_test_mod_inter_mod_service::{RESTOCK_SIZE, ShopMod, Stock, StockMod, prices};

/// The side every test here runs on.
///
/// A dedicated server, because both halves of the pair run there and nothing in
/// this suite is about a side. The one test that is about a side says so.
const SIDE: Side = Side::DedicatedServer;

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn version(raw: u32) -> SavedDataVersion {
    SavedDataVersion::new(raw).expect("a non-zero version")
}

fn entry(mod_id: &ModId, sides: &[Side]) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: mod_id.clone(),
        sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
    LifecycleMachine::new(SIDE, entries).expect("a legal plan")
}

/// The bytes a world holds for one owner at one version.
///
/// Written as an aggregate rather than handed to the mod directly, because the
/// admission rules the aggregate carries — the size ceiling, the format marker,
/// the required packages — are part of the path a real world takes.
fn world(owner: &ModId, at: u32, payload: SavedPayload) -> Vec<u8> {
    let mut aggregate = SavedDataAggregate::fresh();
    aggregate.replace(SavedDataEnvelope::new(
        owner.clone(),
        version(at),
        Vec::new(),
        None,
        payload,
    ));
    aggregate.to_bytes().expect("a bounded aggregate")
}

// ------------------------------------------------------ migration chains ---

#[test]
fn a_world_two_versions_behind_reaches_the_current_shape_through_the_declared_chain() {
    let owner = StockMod::declared_id();
    let bytes = world(owner, 1, json!({ "stock": 12 }));
    let mut stock = StockMod::new();
    let machine = machine(vec![entry(owner, &[SIDE])]);

    let aggregate = open_saved_data(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect("both declared steps run");

    // Version 1 held a count alone. `currency` is what the first step adds and
    // `unitPrice` is what the second adds, so this value exists only if both ran,
    // in that order.
    assert_eq!(
        stock.stock(),
        &Stock {
            stock: 12,
            currency: "coin".to_owned(),
            unit_price: 1,
        }
    );
    assert_eq!(
        aggregate
            .envelope(owner)
            .expect("the owner is still in the aggregate")
            .data_version(),
        stock.declaration().current(),
        "the aggregate a caller saves from is current too"
    );
}

#[test]
fn the_shipped_chain_covers_every_version_the_shipped_declaration_admits() {
    // The check a host makes before it reads a payload, made here against the
    // mod as it ships. A version added without its step fails this without any
    // world being involved.
    let stock = StockMod::new();
    stock
        .chain()
        .covers(&stock.declaration())
        .expect("every declared version has a step into it");
    assert!(
        stock.declaration().versions().count() > 2,
        "a chain worth testing is longer than one hop"
    );
}

#[test]
fn a_world_already_at_the_current_version_is_read_without_a_step() {
    // The negative control for the chain: without it, "reaches the current shape"
    // would hold for an implementation that ignored the stored version and rebuilt
    // the current shape from scratch.
    let owner = StockMod::declared_id();
    let bytes = world(
        owner,
        3,
        json!({ "stock": 4, "currency": "shard", "unitPrice": 7 }),
    );
    let mut stock = StockMod::new();
    let machine = machine(vec![entry(owner, &[SIDE])]);

    open_saved_data(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect("nothing to migrate");

    assert_eq!(stock.stock().currency, "shard", "the stored value was lost");
    assert_eq!(stock.stock().unit_price, 7);
}

#[test]
fn a_world_written_by_a_newer_build_is_refused_rather_than_read_backwards() {
    let owner = StockMod::declared_id();
    let bytes = world(owner, 4, json!({ "anything": true }));
    let mut stock = StockMod::new();
    let machine = machine(vec![entry(owner, &[SIDE])]);

    let refusal = open_saved_data(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect_err("version 4 is above what this build writes");

    assert!(matches!(
        refusal,
        SaveRefused::Data(SavedDataError::FutureVersion { .. })
    ));
    assert_eq!(stock.stock(), &Stock::default());
}

// ---------------------------------------------------- service dependencies ---

#[test]
fn a_provider_is_started_before_the_mod_that_requires_it() {
    let mut shop = ShopMod::new();
    let mut stock = StockMod::new();
    let machine = machine(vec![
        entry(ShopMod::declared_id(), &[SIDE]),
        entry(StockMod::declared_id(), &[SIDE]),
    ]);

    // The consumer is supplied first on purpose: an order taken from the slice
    // would call it before anything could answer.
    let opened = open_services(&machine, &mut [&mut shop, &mut stock]).expect("the graph resolves");

    assert_eq!(
        opened.order(),
        [
            StockMod::declared_id().clone(),
            ShopMod::declared_id().clone()
        ]
    );
    assert_eq!(
        shop.prices(),
        Some(&json!({ "unitPrice": 0, "currency": "", "available": 0 })),
        "the shop was started without an answer"
    );
    assert_eq!(stock.answered(), [json!({ "item": "lantern" })]);
}

#[test]
fn a_service_nothing_answers_names_the_mod_that_wanted_it_and_starts_nothing() {
    // The provider is simply not installed, which is the ordinary form of this:
    // a player installed one half of a pair.
    let mut shop = ShopMod::new();
    let machine = machine(vec![entry(ShopMod::declared_id(), &[SIDE])]);

    let refusal =
        open_services(&machine, &mut [&mut shop]).expect_err("nothing answers the service");

    assert_eq!(
        refusal,
        ServiceRefused::NoProvider {
            consumer: ShopMod::declared_id().clone(),
            service: prices(),
        }
    );
    let rendered = refusal.to_string();
    assert!(
        rendered.contains(ShopMod::declared_id().as_str()),
        "{rendered}"
    );
    assert!(rendered.contains(&prices().to_string()), "{rendered}");
    assert!(
        shop.prices().is_none(),
        "the shop holds an answer nothing gave it"
    );
}

#[test]
fn a_provider_this_side_parks_is_a_service_nothing_answers() {
    // The provider is installed and declares the client alone. Waiting for a mod
    // that will not run this session is the deadlock this refusal avoids, and it is
    // the one place in this suite where a side matters.
    let mut shop = ShopMod::new();
    let mut stock = StockMod::new();
    let machine = machine(vec![
        entry(ShopMod::declared_id(), &[SIDE]),
        entry(StockMod::declared_id(), &[Side::Client]),
    ]);

    assert_eq!(
        open_services(&machine, &mut [&mut shop, &mut stock]),
        Err(ServiceRefused::NoProvider {
            consumer: ShopMod::declared_id().clone(),
            service: prices(),
        })
    );
    assert!(stock.answered().is_empty(), "a parked mod answered a call");
    assert!(shop.prices().is_none());
}

/// Half of a requirement cycle, written here because a well-formed fixture pair
/// cannot be one.
///
/// Two mods that each require the other's service is a mistake two authors make
/// between them, so it cannot be a property of one shipped test mod. Both halves
/// are ordinary mods written against `ravel-sdk`, which is what makes the refusal
/// below a refusal of real declarations.
#[derive(Default)]
struct Latch;

#[derive(Default)]
struct Bolt;

fn latch_service() -> ServiceId {
    ServiceId::new(ContentId::new(Latch::declared_id().clone(), "open").expect("a legal path"))
}

fn bolt_service() -> ServiceId {
    ServiceId::new(ContentId::new(Bolt::declared_id().clone(), "shut").expect("a legal path"))
}

#[declare_mod(id = "cycle_latch")]
impl Mod for Latch {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn service_declaration(&self) -> Option<ServiceDeclaration> {
        ServiceDeclaration::new(
            Self::declared_id().clone(),
            [latch_service()],
            [bolt_service()],
        )
        .ok()
    }
}

#[declare_mod(id = "cycle_bolt")]
impl Mod for Bolt {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn service_declaration(&self) -> Option<ServiceDeclaration> {
        ServiceDeclaration::new(
            Self::declared_id().clone(),
            [bolt_service()],
            [latch_service()],
        )
        .ok()
    }
}

#[test]
fn two_mods_that_require_each_other_are_refused_and_both_are_named() {
    let mut latch = Latch;
    let mut bolt = Bolt;
    let machine = machine(vec![
        entry(Latch::declared_id(), &[SIDE]),
        entry(Bolt::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(&machine, &mut [&mut latch, &mut bolt])
        .expect_err("no order starts either of them");

    assert_eq!(
        refusal,
        ServiceRefused::Cycle {
            participants: vec![Bolt::declared_id().clone(), Latch::declared_id().clone()],
        }
    );
    let rendered = refusal.to_string();
    for named in [Latch::declared_id(), Bolt::declared_id()] {
        assert!(rendered.contains(named.as_str()), "{rendered}");
    }
}

#[test]
fn a_mod_that_requires_the_service_it_answers_is_refused_when_it_declares_it() {
    // The one-mod form of a cycle, refused where the declaration is built rather
    // than left for the loader to find as a cycle of one.
    assert!(
        ServiceDeclaration::new(
            Latch::declared_id().clone(),
            [latch_service()],
            [latch_service()],
        )
        .is_err()
    );
}

// -------------------------------------------------------- failure closure ---

#[test]
fn a_stored_shape_the_chain_cannot_read_withholds_the_world_and_leaves_the_mod_empty() {
    // A version-1 world whose payload is not the version-1 shape, which is what a
    // torn or hand-edited save looks like. The refusal has to be a refusal: no
    // aggregate comes back, so there is nothing to save from, and the mod holds
    // nothing, so a refused world cannot be mistaken for an empty one.
    let owner = StockMod::declared_id();
    let bytes = world(owner, 1, json!({ "count": 12 }));
    let mut stock = StockMod::new();
    let machine = machine(vec![entry(owner, &[SIDE])]);

    let refusal = open_saved_data(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect_err("the first step cannot read this shape");

    assert_eq!(refusal.owner(), Some(owner));
    assert!(refusal.to_string().contains("no `stock`"), "{refusal}");
    assert_eq!(
        stock.stock(),
        &Stock::default(),
        "the mod kept part of a world that was refused"
    );
    assert!(
        stock.save_saved_data().is_ok(),
        "the mod can still be asked, and answers with what it holds rather than with a half-world"
    );
}

#[test]
fn a_mod_whose_migration_refuses_does_not_stop_another_worlds_owner_from_being_admitted() {
    // The other half of closure: a refusal is about this world, not about the
    // build. The same mod opens a legal world afterwards.
    let owner = StockMod::declared_id();
    let machine = machine(vec![entry(owner, &[SIDE])]);
    let mut stock = StockMod::new();

    open_saved_data(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        Some(&world(owner, 1, json!({ "count": 12 }))),
        &BTreeMap::new(),
    )
    .expect_err("the first step cannot read this shape");

    open_saved_data(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        Some(&world(owner, 1, json!({ "stock": 5 }))),
        &BTreeMap::new(),
    )
    .expect("a legal version-1 world");
    assert_eq!(stock.stock().stock, 5);
}

/// A mod that declares settings and never reads them.
///
/// The defect this is about is quiet: a player changes a value, nothing happens,
/// and no gate notices. So the default [`Mod::configure`] refuses rather than
/// accepting, and this is what holds that to something a test can see.
#[derive(Default)]
struct Deaf;

#[declare_mod(id = "deaf")]
impl Mod for Deaf {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
        ConfigurationDeclaration::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "deaf",
              "version": 1,
              "settings": [{
                "key": "ignored",
                "valueSchema": { "type": "boolean" },
                "default": true,
                "side": "common",
                "restartRequired": false
              }]
            }"#,
        )
        .ok()
    }
}

#[test]
fn a_mod_that_declares_settings_and_never_reads_them_is_refused_rather_than_ignored() {
    // Driven over a set through `open_configuration`, not called directly. A
    // default nothing walks is a refusal that exists in the type and in no
    // composition, which is what W4's review found here.
    let mut deaf = Deaf;
    let mut shop = ShopMod::new();
    let pair = machine(vec![
        entry(Deaf::declared_id(), &[SIDE]),
        entry(ShopMod::declared_id(), &[SIDE]),
    ]);

    let refused = open_configuration(
        &pair,
        &mut [&mut deaf as &mut dyn Mod, &mut shop],
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .expect_err("settings nothing reads are settings a player changes with no effect");

    assert_eq!(refused.owner(), Deaf::declared_id());
    assert!(
        refused.to_string().contains("deaf"),
        "the refusal names the mod: {refused}"
    );

    // And a mod that declares nothing is not refused, so the refusal above is
    // about the declaration and not about every mod that never implemented
    // `configure`. `ShopMod` declares no settings and the same walk accepts it.
    let mut shop = ShopMod::new();
    let alone = machine(vec![entry(ShopMod::declared_id(), &[SIDE])]);
    let configured = open_configuration(
        &alone,
        &mut [&mut shop as &mut dyn Mod],
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .expect("a mod with no settings has nothing to read");
    assert_eq!(configured.order(), []);
}

#[test]
fn the_walk_hands_each_mod_the_layer_in_force_and_reports_what_the_player_lost() {
    // The positive half of the same walk, over a mod that does read its settings.
    // `restock.size` has an author default of 8; the pack lowers it and the
    // player's illegal value is dropped rather than becoming the value in force.
    let owner = StockMod::declared_id();
    let mut stock = StockMod::new();
    let machine = machine(vec![entry(owner, &[SIDE])]);
    let pack = ValueDocument::parse(
        r#"{
          "schemaVersion": 1,
          "owner": "service_stock",
          "values": { "restock.size": 3 }
        }"#,
    )
    .expect("a legal pack document");
    let player = ValueDocument::parse(
        r#"{
          "schemaVersion": 1,
          "owner": "service_stock",
          "values": { "restock.colour": "green" }
        }"#,
    )
    .expect("a legal player document");

    let configured = open_configuration(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        &BTreeMap::from([(owner.clone(), pack)]),
        &BTreeMap::from([(owner.clone(), player)]),
    )
    .expect("a dropped value is not a refusal");

    assert_eq!(configured.order(), std::slice::from_ref(owner));
    assert_eq!(
        stock.restock_size(),
        3,
        "the mod holds the pack's value rather than its own default"
    );
    let dropped = configured.dropped();
    assert_eq!(dropped.len(), 1, "one key the declaration does not declare");
    assert_eq!(&dropped[0].0, owner);
    assert_eq!(dropped[0].1.key.as_str(), "restock.colour");
}

#[test]
fn a_player_value_the_declaration_refuses_is_dropped_and_the_default_stays_in_force() {
    // Closure on the configuration side: an illegal value does not reach the mod
    // and does not silently become the value in force. It is dropped, named, and
    // the layer below it applies.
    let mut stock = StockMod::new();
    let declaration = stock
        .configuration_declaration()
        .expect("it declares one setting");
    let player = ValueDocument::parse(
        r#"{
          "schemaVersion": 1,
          "owner": "service_stock",
          "values": { "restock.size": 0, "restock.colour": "green" }
        }"#,
    )
    .expect("a legal override document");

    let resolved = LayeredConfiguration::resolve(declaration, None, Some(&player))
        .expect("a player document is layered rather than refused");
    stock.configure(&resolved).expect("the default is legal");

    assert_eq!(
        stock.restock_size(),
        8,
        "an out-of-range player value became the value in force"
    );
    let dropped: Vec<String> = resolved
        .rejected()
        .iter()
        .map(|rejection| rejection.key.as_str().to_owned())
        .collect();
    // Key order, which is the order a `LayeredConfiguration` reports rejections in.
    assert_eq!(dropped, ["restock.colour", "restock.size"]);
}

#[test]
fn a_pack_may_state_a_legal_value_and_the_mod_reads_that_one() {
    // The negative control for the test above: the layers do reach the mod when
    // the value is one the declaration admits.
    let mut stock = StockMod::new();
    let declaration = stock
        .configuration_declaration()
        .expect("it declares one setting");
    let pack = ValueDocument::parse(
        r#"{
          "schemaVersion": 1,
          "owner": "service_stock",
          "values": { "restock.size": 32 }
        }"#,
    )
    .expect("a legal override document");

    let resolved =
        LayeredConfiguration::resolve(declaration, Some(&pack), None).expect("a declared key");
    stock.configure(&resolved).expect("a legal value");
    assert_eq!(stock.restock_size(), 32);

    let key = RESTOCK_SIZE.parse().expect("a legal setting key");
    assert_eq!(
        resolved.source(&key),
        Some(ravel_sdk::config::ValueSource::Pack)
    );
}

/// A consumer that refuses whatever it is answered.
///
/// It requires the shipped provider's service, so the graph resolves and the
/// provider does run; what is under test is what happens after the answer.
#[derive(Default)]
struct Fussy {
    called: bool,
}

#[declare_mod(id = "fussy")]
impl Mod for Fussy {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn service_declaration(&self) -> Option<ServiceDeclaration> {
        ServiceDeclaration::new(Self::declared_id().clone(), [], [prices()]).ok()
    }

    fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
        self.called = true;
        let _ = directory.call(&ServiceRequest::new(prices(), json!(null)));
        Err(ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Constructed,
            detail: "the answer is not one this mod accepts".to_owned(),
        })
    }
}

#[test]
fn a_consumer_that_refuses_its_answer_refuses_the_whole_set_rather_than_starting() {
    // `fussy` sorts before `shop`, so the shop is after it in the order and is
    // never offered its services at all: a refusal leaves nothing half-wired.
    let mut fussy = Fussy::default();
    let mut shop = ShopMod::new();
    let mut stock = StockMod::new();
    let machine = machine(vec![
        entry(Fussy::declared_id(), &[SIDE]),
        entry(ShopMod::declared_id(), &[SIDE]),
        entry(StockMod::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(&machine, &mut [&mut shop, &mut fussy, &mut stock])
        .expect_err("`fussy` refuses");

    assert!(matches!(
        refusal,
        ServiceRefused::Consumer { ref mod_id, .. } if mod_id == Fussy::declared_id()
    ));
    assert!(fussy.called, "the refusing mod was never asked");
    assert!(
        shop.prices().is_none(),
        "a mod after the refusal was wired anyway"
    );
}

#[test]
fn what_the_pair_writes_is_what_the_pair_reads_back() {
    // The round trip, so the migration tests are about migrating rather than about
    // a reader that happens to accept anything.
    let owner = StockMod::declared_id();
    let machine = machine(vec![entry(owner, &[SIDE])]);
    let mut stock = StockMod::new();

    open_saved_data(
        &machine,
        &mut [&mut stock as &mut dyn Mod],
        Some(&world(owner, 1, json!({ "stock": 9 }))),
        &BTreeMap::new(),
    )
    .expect("a legal version-1 world");

    let envelope = stock
        .save_saved_data()
        .expect("this mod owns world data")
        .expect("and produces an envelope");
    let mut written = SavedDataAggregate::fresh();
    written.replace(envelope);
    let bytes = written.to_bytes().expect("a bounded aggregate");

    let mut reopened = StockMod::new();
    open_saved_data(
        &machine,
        &mut [&mut reopened as &mut dyn Mod],
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect("its own shape at its own version");
    assert_eq!(reopened.stock(), stock.stock());
}

#[test]
fn the_declarations_this_suite_is_written_against_are_the_ones_the_mods_ship() {
    // A floor, for the reason M4-W2's content suite has one: every assertion above
    // would still pass if a declaration quietly shrank to nothing.
    let stock = StockMod::new();
    let shop = ShopMod::new();

    assert_eq!(
        stock
            .service_declaration()
            .expect("the provider declares one")
            .provides()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["service_stock:prices"]
    );
    assert_eq!(
        shop.service_declaration()
            .expect("the consumer declares one")
            .requires()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["service_stock:prices"]
    );
    assert_eq!(
        stock
            .configuration_declaration()
            .expect("the provider declares settings")
            .settings
            .iter()
            .map(|setting| setting.key.as_str().to_owned())
            .collect::<Vec<_>>(),
        [RESTOCK_SIZE]
    );
    assert_eq!(
        stock
            .saved_data_declaration()
            .expect("the provider owns world data")
            .versions()
            .map(SavedDataVersion::get)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        stock
            .saved_data_migrations()
            .expect("the provider declares a chain")
            .reaches()
            .map(SavedDataVersion::get)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(id("service_stock"), *StockMod::declared_id());
}

#[test]
fn the_owner_of_a_declaration_and_the_owner_of_its_chain_are_the_same_mod() {
    let stock = StockMod::new();
    let declaration: SavedDataDeclaration = stock
        .saved_data_declaration()
        .expect("the provider owns world data");
    assert_eq!(declaration.owner(), StockMod::declared_id());
    assert_eq!(
        stock.saved_data_migrations().expect("a chain").owner(),
        StockMod::declared_id()
    );
    assert_eq!(prices().owner(), StockMod::declared_id());
}
