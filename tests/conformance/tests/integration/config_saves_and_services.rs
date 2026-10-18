#![forbid(unsafe_code)]

//! Migration chains, service ordering, and fail-closed configuration and saves.

use std::collections::{BTreeMap, BTreeSet};

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_loader::{
    LifecycleEntry, LifecycleMachine, Requirement, SaveRefused, ServiceRefused, open_configuration,
    open_saved_data, open_services,
};
use ravel_save::{
    SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataVersion,
};
use ravel_sdk::JsonValue;
use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
use ravel_sdk::content::ContentId;
use ravel_sdk::services::{ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest};
use ravel_sdk::{Declared, Mod, ModError, declare_mod, json};
use ravel_test_mod_inter_mod_service::{RESTOCK_SIZE, ShopMod, Stock, StockMod, prices};

const SIDE: Side = Side::DedicatedServer;

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn environment() -> ravel_core::FailureEnvironment {
    let running = ravel_bootstrap_runtime::environment();
    ravel_core::FailureEnvironment {
        ravel: running.ravel,
        minecraft: running.minecraft,
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    }
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

fn world(owner: &ModId, at: u32, payload: JsonValue) -> Vec<u8> {
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

#[test]
fn a_world_two_versions_behind_reaches_the_current_shape_through_the_declared_chain() {
    let owner = StockMod::declared_id();
    let bytes = world(owner, 1, json!({ "stock": 12 }));
    let mut stock = StockMod::new();
    let mut machine = machine(vec![entry(owner, &[SIDE])]);

    let aggregate = open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut stock])
            .expect("all active mods have hosts"),
        &environment(),
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect("both declared steps run");

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
        stock.declaration().expect("valid declaration").current(),
        "the aggregate a caller saves from is current too"
    );
}

#[test]
fn the_shipped_chain_covers_every_version_the_shipped_declaration_admits() {
    let stock = StockMod::new();
    stock
        .chain()
        .covers(&stock.declaration().expect("valid declaration"))
        .expect("every declared version has a step into it");
    assert!(
        stock
            .declaration()
            .expect("valid declaration")
            .versions()
            .count()
            > 2,
        "a chain worth testing is longer than one hop"
    );
}

#[test]
fn a_world_already_at_the_current_version_is_read_without_a_step() {
    let owner = StockMod::declared_id();
    let bytes = world(
        owner,
        3,
        json!({ "stock": 4, "currency": "shard", "unitPrice": 7 }),
    );
    let mut stock = StockMod::new();
    let mut machine = machine(vec![entry(owner, &[SIDE])]);

    open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut stock])
            .expect("all active mods have hosts"),
        &environment(),
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
    let mut machine = machine(vec![entry(owner, &[SIDE])]);

    let refusal = open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut stock])
            .expect("all active mods have hosts"),
        &environment(),
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

#[test]
fn a_provider_is_started_before_the_mod_that_requires_it() {
    let mut shop = ShopMod::new();
    let mut stock = StockMod::new();
    let mut machine = machine(vec![
        entry(ShopMod::declared_id(), &[SIDE]),
        entry(StockMod::declared_id(), &[SIDE]),
    ]);

    // Supply the consumer first so slice order cannot satisfy the dependency.
    let opened = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut shop, &mut stock],
        )
        .expect("all active mods have hosts"),
        &environment(),
    )
    .expect("the graph resolves");

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
    let mut shop = ShopMod::new();
    let mut machine = machine(vec![entry(ShopMod::declared_id(), &[SIDE])]);

    let refusal = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut shop])
            .expect("all active mods have hosts"),
        &environment(),
    )
    .expect_err("nothing answers the service");

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
    let mut shop = ShopMod::new();
    let mut stock = StockMod::new();
    let mut machine = machine(vec![
        entry(ShopMod::declared_id(), &[SIDE]),
        entry(StockMod::declared_id(), &[Side::Client]),
    ]);

    assert_eq!(
        open_services(
            &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
                &mut machine,
                &mut [&mut shop, &mut stock],
            )
            .expect("all active mods have hosts"),
            &environment(),
        ),
        Err(ServiceRefused::NoProvider {
            consumer: ShopMod::declared_id().clone(),
            service: prices(),
        })
    );
    assert!(stock.answered().is_empty(), "a parked mod answered a call");
    assert!(shop.prices().is_none());
}

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
    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        Ok(Some(
            ServiceDeclaration::new(
                Self::declared_id().clone(),
                [latch_service()],
                [bolt_service()],
            )
            .expect("the latch service declaration is valid"),
        ))
    }
}

#[declare_mod(id = "cycle_bolt")]
impl Mod for Bolt {
    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        Ok(Some(
            ServiceDeclaration::new(
                Self::declared_id().clone(),
                [bolt_service()],
                [latch_service()],
            )
            .expect("the bolt service declaration is valid"),
        ))
    }
}

#[test]
fn two_mods_that_require_each_other_are_refused_and_both_are_named() {
    let mut latch = Latch;
    let mut bolt = Bolt;
    let mut machine = machine(vec![
        entry(Latch::declared_id(), &[SIDE]),
        entry(Bolt::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut latch, &mut bolt],
        )
        .expect("all active mods have hosts"),
        &environment(),
    )
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
    assert!(
        ServiceDeclaration::new(
            Latch::declared_id().clone(),
            [latch_service()],
            [latch_service()],
        )
        .is_err()
    );
}

#[test]
fn a_stored_shape_the_chain_cannot_read_withholds_the_world_and_leaves_the_mod_empty() {
    let owner = StockMod::declared_id();
    let bytes = world(owner, 1, json!({ "count": 12 }));
    let mut stock = StockMod::new();
    let mut machine = machine(vec![entry(owner, &[SIDE])]);

    let refusal = open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut stock])
            .expect("all active mods have hosts"),
        &environment(),
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect_err("the first step cannot read this shape");

    assert_eq!(refusal.mod_id(), Some(owner));
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
    let owner = StockMod::declared_id();
    let mut machine = machine(vec![entry(owner, &[SIDE])]);
    let mut stock = StockMod::new();

    open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut stock])
            .expect("all active mods have hosts"),
        &environment(),
        Some(&world(owner, 1, json!({ "count": 12 }))),
        &BTreeMap::new(),
    )
    .expect_err("the first step cannot read this shape");
    assert_eq!(machine.state(owner), Ok(LifecycleState::Failed));
    assert_eq!(stock.stock(), &Stock::default());

    // A different world has its own admission session; the failed session is not replayed.
    let mut next_machine = self::machine(vec![entry(owner, &[SIDE])]);
    let mut next_stock = StockMod::new();

    open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut next_machine,
            &mut [&mut next_stock],
        )
        .expect("all active mods have hosts"),
        &environment(),
        Some(&world(owner, 1, json!({ "stock": 5 }))),
        &BTreeMap::new(),
    )
    .expect("a legal version-1 world");
    assert_eq!(next_stock.stock().stock, 5);
    assert_eq!(stock.stock(), &Stock::default());
    assert_eq!(machine.state(owner), Ok(LifecycleState::Failed));
}

#[derive(Default)]
struct Deaf;

#[declare_mod(id = "deaf")]
impl Mod for Deaf {
    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        Ok(Some(
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
            .expect("the deaf configuration declaration is valid"),
        ))
    }
}

#[test]
fn a_mod_that_declares_settings_and_never_reads_them_is_refused_rather_than_ignored() {
    let mut deaf = Deaf;
    let mut shop = ShopMod::new();
    let mut pair = machine(vec![
        entry(Deaf::declared_id(), &[SIDE]),
        entry(ShopMod::declared_id(), &[SIDE]),
    ]);

    let refused = open_configuration(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut pair, &mut [&mut deaf, &mut shop])
            .expect("all active mods have hosts"),
        &BTreeMap::new(),
        |_| Ok(None),
        &ravel_bootstrap_runtime::environment(),
    )
    .expect_err("settings nothing reads are settings a player changes with no effect");

    assert_eq!(refused.mod_id(), Some(Deaf::declared_id()));
    assert!(
        refused.to_string().contains("deaf"),
        "the refusal names the mod: {refused}"
    );

    let mut shop = ShopMod::new();
    let mut alone = machine(vec![entry(ShopMod::declared_id(), &[SIDE])]);
    let configured = open_configuration(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut alone, &mut [&mut shop])
            .expect("all active mods have hosts"),
        &BTreeMap::new(),
        |_| Ok(None),
        &ravel_bootstrap_runtime::environment(),
    )
    .expect("a mod with no settings has nothing to read");
    assert_eq!(configured.order(), []);
}

#[test]
fn the_walk_hands_each_mod_the_layer_in_force_and_reports_what_the_player_lost() {
    let owner = StockMod::declared_id();
    let mut stock = StockMod::new();
    let mut machine = machine(vec![entry(owner, &[SIDE])]);
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
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut stock])
            .expect("all active mods have hosts"),
        &BTreeMap::from([(owner.clone(), pack)]),
        |_| Ok(Some(player.clone())),
        &ravel_bootstrap_runtime::environment(),
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
    let mut stock = StockMod::new();
    let declaration = stock
        .configuration_declaration()
        .expect("the configuration declaration succeeds")
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
    assert_eq!(dropped, ["restock.colour", "restock.size"]);
}

#[test]
fn a_pack_may_state_a_legal_value_and_the_mod_reads_that_one() {
    let mut stock = StockMod::new();
    let declaration = stock
        .configuration_declaration()
        .expect("the configuration declaration succeeds")
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

#[derive(Default)]
struct Fussy {
    called: bool,
}

#[declare_mod(id = "fussy")]
impl Mod for Fussy {
    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        Ok(Some(
            ServiceDeclaration::new(Self::declared_id().clone(), [], [prices()])
                .expect("the fussy service declaration is valid"),
        ))
    }

    fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
        self.called = true;
        directory
            .call(&ServiceRequest::new(prices(), json!(null)))
            .expect("the provider answers before the consumer refuses");
        Err(ModError::refused("the answer is not one this mod accepts"))
    }
}

#[test]
fn a_consumer_that_refuses_its_answer_refuses_the_whole_set_rather_than_starting() {
    // `fussy` precedes `shop`; refusal must leave the latter unwired.
    let mut fussy = Fussy::default();
    let mut shop = ShopMod::new();
    let mut stock = StockMod::new();
    let mut machine = machine(vec![
        entry(Fussy::declared_id(), &[SIDE]),
        entry(ShopMod::declared_id(), &[SIDE]),
        entry(StockMod::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut shop, &mut fussy, &mut stock],
        )
        .expect("all active mods have hosts"),
        &environment(),
    )
    .expect_err("`fussy` refuses");

    assert!(matches!(
        refusal,
        ServiceRefused::Mod { ref mod_id, .. } if mod_id == Fussy::declared_id()
    ));
    assert!(fussy.called, "the refusing mod was never asked");
    assert!(
        shop.prices().is_none(),
        "a mod after the refusal was wired anyway"
    );
}

#[test]
fn what_the_pair_writes_is_what_the_pair_reads_back() {
    let owner = StockMod::declared_id();
    let mut machine = machine(vec![entry(owner, &[SIDE])]);
    let mut stock = StockMod::new();

    open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut stock])
            .expect("all active mods have hosts"),
        &environment(),
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
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut reopened])
            .expect("all active mods have hosts"),
        &environment(),
        Some(&bytes),
        &BTreeMap::new(),
    )
    .expect("its own shape at its own version");
    assert_eq!(reopened.stock(), stock.stock());
}

#[test]
fn the_declarations_this_suite_is_written_against_are_the_ones_the_mods_ship() {
    let stock = StockMod::new();
    let shop = ShopMod::new();

    assert_eq!(
        stock
            .service_declaration()
            .expect("the service declaration succeeds")
            .expect("the provider declares one")
            .provides()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["service_stock:prices"]
    );
    assert_eq!(
        shop.service_declaration()
            .expect("the service declaration succeeds")
            .expect("the consumer declares one")
            .requires()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["service_stock:prices"]
    );
    assert_eq!(
        stock
            .configuration_declaration()
            .expect("the configuration declaration succeeds")
            .expect("the provider declares settings")
            .settings()
            .iter()
            .map(|setting| setting.key.as_str().to_owned())
            .collect::<Vec<_>>(),
        [RESTOCK_SIZE]
    );
    assert_eq!(
        stock
            .saved_data_declaration()
            .expect("the saved-data declaration succeeds")
            .expect("the provider owns world data")
            .versions()
            .map(SavedDataVersion::get)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        stock
            .saved_data_migrations()
            .expect("the saved-data migrations succeed")
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
        .expect("the saved-data declaration succeeds")
        .expect("the provider owns world data");
    assert_eq!(declaration.owner(), StockMod::declared_id());
    assert_eq!(
        stock
            .saved_data_migrations()
            .expect("the saved-data migrations succeed")
            .expect("a chain")
            .owner(),
        StockMod::declared_id()
    );
    assert_eq!(prices().owner(), StockMod::declared_id());
}
