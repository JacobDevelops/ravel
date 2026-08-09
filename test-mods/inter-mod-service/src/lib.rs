#![forbid(unsafe_code)]

//! Inter-mod service composition test mod.
//!
//! M4-W4's pair, and the whole of what its gate is about. [`StockMod`] answers a
//! service, owns versioned world data and declares settings; [`ShopMod`] requires
//! that service and does nothing until it has an answer. Neither names anything
//! outside `ravel-sdk`, and neither names the other: a consumer reaches a provider
//! through a [`ServiceId`] and a JSON answer, never through the provider's Rust
//! types, which is why `Cargo.toml` beside this file lists one dependency.
//!
//! Both halves live in one crate because they are one fixture — a graph needs two
//! ends — and separating them would make the pair's `Cargo.toml`s the thing under
//! test rather than the graph.
//!
//! # What each property is arranged to falsify
//!
//! **Migration chains.** [`StockMod`] reads three saved-data versions and declares
//! a [`MigrationChain`] with a step into each of the two above the oldest. The
//! shapes differ at every version, so a world at version 1 can only reach the
//! current shape by passing through both steps in order: an implementation that ran
//! one, or ran them out of order, produces a value the current reader refuses.
//!
//! **Service dependencies.** [`ShopMod`] records nothing until its call is
//! answered, so "the provider ran first" is observable rather than asserted:
//! [`ShopMod::prices`] is empty for a shop that was started before its provider.
//!
//! **Failure closure.** Both halves refuse rather than carry on. A migration step
//! that cannot read the shape it was handed returns [`MigrationFailed`], and
//! [`StockMod`] leaves its own state untouched — [`StockMod::stock`] still reads
//! zero, so a refusal cannot be mistaken for a world that opened empty. A shop
//! whose call fails returns [`ModError::Refused`] rather than running with no
//! prices.

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::ContentId;
use ravel_sdk::save::{
    MigrationChain, MigrationFailed, SavedDataDeclaration, SavedDataEnvelope, SavedDataPlan,
    SavedDataVersion, SavedPayload,
};
use ravel_sdk::services::{
    ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest, ServiceResponse,
};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod, json};

/// The identity the provider is packaged under.
pub const STOCK_MOD_ID: &str = "service_stock";

/// The identity the consumer is packaged under.
pub const SHOP_MOD_ID: &str = "service_shop";

/// The saved-data versions [`StockMod`] reads, oldest first.
///
/// The highest is the one it writes. Every version above the oldest needs a
/// migration step, and [`StockMod::chain`] is checked against this list rather
/// than against a second copy of it.
const STOCK_VERSIONS: [u32; 3] = [1, 2, 3];

/// The settings [`StockMod`] declares.
const STOCK_SETTINGS: &str = r#"{
  "schemaVersion": 1,
  "owner": "service_stock",
  "version": 1,
  "settings": [
    {
      "key": "restock.size",
      "title": "Restock size",
      "valueSchema": { "type": "integer", "minimum": 1.0, "maximum": 64.0 },
      "default": 8,
      "side": "common",
      "restartRequired": false
    }
  ]
}"#;

/// The key [`StockMod`] reads its one setting under.
pub const RESTOCK_SIZE: &str = "restock.size";

fn version(raw: u32) -> SavedDataVersion {
    SavedDataVersion::new(raw).expect("every declared version is non-zero")
}

/// The service [`StockMod`] answers and [`ShopMod`] requires.
///
/// # Panics
///
/// Panics if `prices` stops being a legal content path, which would mean this
/// service can no longer be named.
#[must_use]
pub fn prices() -> ServiceId {
    ServiceId::new(
        ContentId::new(StockMod::declared_id().clone(), "prices")
            .expect("`prices` is a legal path"),
    )
}

/// Version 1 held a bare count. Version 2 splits it into a count and a currency.
fn to_two(payload: &SavedPayload) -> Result<SavedPayload, MigrationFailed> {
    let stock = payload
        .get("stock")
        .and_then(SavedPayload::as_u64)
        .ok_or_else(|| MigrationFailed::new("version 1 held no `stock` count"))?;
    Ok(json!({ "stock": stock, "currency": "coin" }))
}

/// Version 3 adds the unit price, which version 2 had no room for.
fn to_three(payload: &SavedPayload) -> Result<SavedPayload, MigrationFailed> {
    let stock = payload
        .get("stock")
        .and_then(SavedPayload::as_u64)
        .ok_or_else(|| MigrationFailed::new("version 2 held no `stock` count"))?;
    let currency = payload
        .get("currency")
        .and_then(SavedPayload::as_str)
        .ok_or_else(|| MigrationFailed::new("version 2 held no `currency`"))?;
    Ok(json!({ "stock": stock, "currency": currency, "unitPrice": 1 }))
}

/// What version 3 holds, which is the only shape this build reads.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stock {
    /// How many are held.
    pub stock: u64,
    /// What they are priced in.
    pub currency: String,
    /// What one costs.
    pub unit_price: u64,
}

/// A mod that answers a service, owns versioned world data, and has settings.
///
/// # Examples
///
/// ```
/// use ravel_sdk::save::SavedDataVersion;
/// use ravel_sdk::Mod;
/// use ravel_test_mod_inter_mod_service::StockMod;
///
/// let stock = StockMod::new();
/// assert_eq!(stock.id().as_str(), "service_stock");
///
/// // Every version above the oldest has a step into it.
/// stock
///     .chain()
///     .covers(&stock.declaration())
///     .expect("the chain covers every declared version");
/// assert_eq!(
///     stock.declaration().current(),
///     SavedDataVersion::new(3).expect("3 is not zero")
/// );
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StockMod {
    stock: Stock,
    restock_size: u64,
    answered: Vec<SavedPayload>,
}

impl StockMod {
    /// Builds the provider.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the versions this build reads.
    ///
    /// # Panics
    ///
    /// Panics if [`STOCK_VERSIONS`] stops being a non-empty list of non-zero
    /// versions, which would mean this mod can no longer own world data.
    #[must_use]
    pub fn declaration(&self) -> SavedDataDeclaration {
        SavedDataDeclaration::new(
            Self::declared_id().clone(),
            STOCK_VERSIONS.into_iter().map(version),
        )
        .expect("this mod declares three versions")
    }

    /// Returns the step into every version above the oldest.
    #[must_use]
    pub fn chain(&self) -> MigrationChain {
        MigrationChain::new(Self::declared_id().clone())
            .reaching(version(2), to_two)
            .reaching(version(3), to_three)
    }

    /// Returns the state this mod holds.
    #[must_use]
    pub const fn stock(&self) -> &Stock {
        &self.stock
    }

    /// Returns the restock size the resolved settings put in force.
    #[must_use]
    pub const fn restock_size(&self) -> u64 {
        self.restock_size
    }

    /// Returns every request this mod answered, in the order it did.
    #[must_use]
    pub fn answered(&self) -> &[SavedPayload] {
        &self.answered
    }

    /// Reads a version-3 payload, refusing anything else.
    fn read(&mut self, payload: &SavedPayload) -> Result<(), ModError> {
        let refuse = |detail: &str| ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Registered,
            detail: detail.to_owned(),
        };
        let stock = payload
            .get("stock")
            .and_then(SavedPayload::as_u64)
            .ok_or_else(|| refuse("the stored stock holds no count"))?;
        let currency = payload
            .get("currency")
            .and_then(SavedPayload::as_str)
            .ok_or_else(|| refuse("the stored stock holds no currency"))?;
        let unit_price = payload
            .get("unitPrice")
            .and_then(SavedPayload::as_u64)
            .ok_or_else(|| refuse("the stored stock holds no unit price"))?;

        self.stock = Stock {
            stock,
            currency: currency.to_owned(),
            unit_price,
        };
        Ok(())
    }
}

#[declare_mod(id = "service_stock")]
impl Mod for StockMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
        ConfigurationDeclaration::parse(STOCK_SETTINGS).ok()
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        let key = RESTOCK_SIZE.parse().map_err(|_| ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Constructed,
            detail: "this mod's own setting key is not a legal key".to_owned(),
        })?;
        self.restock_size = resolved
            .value(&key)
            .and_then(SavedPayload::as_u64)
            .ok_or_else(|| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Constructed,
                detail: format!("`{RESTOCK_SIZE}` resolved to something that is not a count"),
            })?;
        Ok(())
    }

    fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
        Some(self.declaration())
    }

    fn saved_data_migrations(&self) -> Option<MigrationChain> {
        Some(self.chain())
    }

    /// Migrates the stored payload up to the current version, then reads it.
    ///
    /// The chain runs here rather than in the host because the shapes are this
    /// mod's. What the host owns is the decision: the `plan` it was handed already
    /// names the versions to cross, so this method never chooses whether to
    /// migrate — only how.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a step could not read the shape it was
    /// handed, or when the migrated payload is still not one this build reads. The
    /// mod's own state is untouched in both cases, so a refused world leaves this
    /// mod holding nothing rather than holding part of a world.
    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        let Some(envelope) = envelope else {
            // Absence initializes fresh state, which is ADG-7's rule and not this
            // mod's choice.
            return Ok(());
        };
        let current = self
            .chain()
            .apply(plan, envelope.payload())
            .map_err(|refusal| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Registered,
                detail: refusal.to_string(),
            })?;
        self.read(&current)
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        Ok(Some(SavedDataEnvelope::new(
            Self::declared_id().clone(),
            self.declaration().current(),
            Vec::new(),
            None,
            json!({
                "stock": self.stock.stock,
                "currency": self.stock.currency,
                "unitPrice": self.stock.unit_price,
            }),
        )))
    }

    fn service_declaration(&self) -> Option<ServiceDeclaration> {
        ServiceDeclaration::new(Self::declared_id().clone(), [prices()], []).ok()
    }

    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ModError> {
        self.answered.push(request.arguments.clone());
        Ok(ServiceResponse::new(json!({
            "unitPrice": self.stock.unit_price,
            "currency": self.stock.currency,
            "available": self.stock.stock,
        })))
    }
}

/// A mod that cannot start until another mod answers it.
///
/// # Examples
///
/// ```
/// use ravel_sdk::Mod;
/// use ravel_test_mod_inter_mod_service::{ShopMod, prices};
///
/// let shop = ShopMod::new();
/// assert_eq!(shop.id().as_str(), "service_shop");
/// // Nothing has answered yet, and that is observable rather than asserted.
/// assert!(shop.prices().is_none());
///
/// let declaration = shop.service_declaration().expect("it requires one service");
/// assert_eq!(
///     declaration.requires().collect::<Vec<_>>(),
///     [&prices()]
/// );
/// assert_eq!(declaration.provides().count(), 0);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShopMod {
    prices: Option<SavedPayload>,
}

impl ShopMod {
    /// Builds the consumer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns what the provider answered, when it has.
    ///
    /// `None` is the whole point: a shop that was offered its services before the
    /// provider ran has nothing here, so the order is checkable from outside.
    #[must_use]
    pub const fn prices(&self) -> Option<&SavedPayload> {
        self.prices.as_ref()
    }
}

#[declare_mod(id = "service_shop")]
impl Mod for ShopMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn service_declaration(&self) -> Option<ServiceDeclaration> {
        ServiceDeclaration::new(Self::declared_id().clone(), [], [prices()]).ok()
    }

    /// Calls the one service this mod declared, and refuses without an answer.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the call did not produce an answer. A
    /// shop with no prices is not a shop running in a degraded mode; it is a mod
    /// that has not started, and saying so is what keeps a half-wired set out of a
    /// world.
    fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
        let answer = directory
            .call(&ServiceRequest::new(prices(), json!({ "item": "lantern" })))
            .map_err(|failed| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Constructed,
                detail: failed.to_string(),
            })?;
        self.prices = Some(answer.value().clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::save::{SavedDataEnvelope, SavedDataPlan, SavedDataVersion};
    use ravel_sdk::{Declared, LifecycleState, Mod, ModError, json};

    use ravel_sdk::save::SavedPayload;

    use super::{
        SHOP_MOD_ID, STOCK_MOD_ID, STOCK_VERSIONS, ShopMod, Stock, StockMod, prices, version,
    };

    fn envelope(at: u32, payload: SavedPayload) -> SavedDataEnvelope {
        SavedDataEnvelope::new(
            StockMod::declared_id().clone(),
            version(at),
            Vec::new(),
            None,
            payload,
        )
    }

    fn opened(at: u32, payload: SavedPayload) -> Result<StockMod, ModError> {
        let mut stock = StockMod::new();
        let plan = SavedDataPlan::open(&stock.declaration(), Some(version(at)))
            .expect("a declared version");
        stock.open_saved_data(Some(&envelope(at, payload)), &plan)?;
        Ok(stock)
    }

    #[test]
    fn both_halves_are_packaged_under_the_identities_they_report() {
        assert_eq!(StockMod::declared_id().as_str(), STOCK_MOD_ID);
        assert_eq!(ShopMod::declared_id().as_str(), SHOP_MOD_ID);
        assert_eq!(prices().owner(), StockMod::declared_id());
    }

    #[test]
    fn the_chain_covers_every_version_the_declaration_admits() {
        let stock = StockMod::new();
        stock
            .chain()
            .covers(&stock.declaration())
            .expect("every declared version has a step into it");
        assert_eq!(
            stock.declaration().current(),
            version(*STOCK_VERSIONS.last().expect("three versions"))
        );
    }

    #[test]
    fn a_world_at_the_oldest_version_arrives_through_both_steps() {
        // The version-3 shape holds a unit price that version 1 does not, and a
        // currency that only the first step supplies, so this value is reachable
        // from version 1 by no shorter route.
        let stock = opened(1, json!({ "stock": 12 })).expect("both steps run");
        assert_eq!(
            stock.stock(),
            &Stock {
                stock: 12,
                currency: "coin".to_owned(),
                unit_price: 1,
            }
        );
    }

    #[test]
    fn a_world_at_the_current_version_is_read_without_a_step() {
        let stock = opened(
            3,
            json!({ "stock": 4, "currency": "shard", "unitPrice": 7 }),
        )
        .expect("nothing to migrate");
        assert_eq!(stock.stock().currency, "shard");
        assert_eq!(stock.stock().unit_price, 7);
    }

    #[test]
    fn a_stored_shape_a_step_cannot_read_refuses_and_leaves_the_mod_empty() {
        let refusal = opened(1, json!({ "count": 12 })).expect_err("version 1 held no `stock`");
        assert_eq!(refusal.mod_id(), StockMod::declared_id());
        assert!(refusal.to_string().contains("no `stock`"), "{refusal}");

        // And nothing was read: a refused world leaves this mod holding nothing,
        // which is what stops a refusal reading as a world that opened empty.
        let mut stock = StockMod::new();
        let plan = SavedDataPlan::open(&stock.declaration(), Some(version(1)))
            .expect("a declared version");
        assert!(
            stock
                .open_saved_data(Some(&envelope(1, json!({ "count": 12 }))), &plan)
                .is_err()
        );
        assert_eq!(stock.stock(), &Stock::default());
    }

    #[test]
    fn absence_leaves_fresh_state_rather_than_refusing() {
        let mut stock = StockMod::new();
        let plan = SavedDataPlan::open(&stock.declaration(), None).expect("absence is fresh");
        stock
            .open_saved_data(None, &plan)
            .expect("absence initializes");
        assert_eq!(stock.stock(), &Stock::default());
    }

    #[test]
    fn what_this_mod_writes_is_what_it_reads_back() {
        let opened = opened(1, json!({ "stock": 12 })).expect("both steps run");
        let envelope = opened
            .save_saved_data()
            .expect("this mod owns world data")
            .expect("and produces an envelope");
        assert_eq!(envelope.data_version(), opened.declaration().current());

        let mut reopened = StockMod::new();
        let plan = SavedDataPlan::open(&reopened.declaration(), Some(envelope.data_version()))
            .expect("the current version");
        reopened
            .open_saved_data(Some(&envelope), &plan)
            .expect("its own shape");
        assert_eq!(reopened.stock(), opened.stock());
    }

    #[test]
    fn the_declared_settings_resolve_to_the_authors_default_with_no_pack_or_player() {
        use ravel_sdk::config::LayeredConfiguration;

        let mut stock = StockMod::new();
        let declaration = stock
            .configuration_declaration()
            .expect("this mod declares settings");
        let resolved =
            LayeredConfiguration::resolve(declaration, None, None).expect("nothing to layer");
        stock.configure(&resolved).expect("its own default");
        assert_eq!(stock.restock_size(), 8);
    }

    #[test]
    fn a_shop_answers_nothing_until_a_provider_has() {
        let shop = ShopMod::new();
        assert!(shop.prices().is_none());
        assert_eq!(
            shop.service_declaration()
                .expect("it requires one service")
                .requires()
                .collect::<Vec<_>>(),
            [&prices()]
        );
    }

    #[test]
    fn a_provider_answers_with_what_it_currently_holds() {
        let mut stock = opened(
            3,
            json!({ "stock": 4, "currency": "shard", "unitPrice": 7 }),
        )
        .expect("its own shape");
        let request =
            ravel_sdk::services::ServiceRequest::new(prices(), json!({ "item": "lantern" }));
        let answer = stock
            .answer_service(&request)
            .expect("the provider answers what it declared");

        assert_eq!(
            answer.value(),
            &json!({ "unitPrice": 7, "currency": "shard", "available": 4 })
        );
        assert_eq!(stock.answered(), [json!({ "item": "lantern" })]);
    }

    #[test]
    fn a_mod_that_declares_nothing_extra_still_answers_the_defaults() {
        // `enter` is the whole of what either half had to write for the lifecycle,
        // so the defaulted methods still answer for both.
        let shop = ShopMod::new();
        assert!(shop.saved_data_declaration().is_none());
        assert!(shop.configuration_declaration().is_none());
        assert!(
            shop.save_saved_data()
                .expect("a mod that owns no world data produces no envelope")
                .is_none()
        );
        let mut shop = shop;
        shop.enter(LifecycleState::Running).expect("accepted");
    }

    #[test]
    fn a_version_no_declared_step_reaches_is_refused_before_a_payload_is_read() {
        // A chain missing its middle link, which is the shape a build ships with
        // when a version is added and its step is not.
        let stock = StockMod::new();
        let broken = ravel_sdk::save::MigrationChain::new(StockMod::declared_id().clone())
            .reaching(
                SavedDataVersion::new(3).expect("3 is not zero"),
                |payload| Ok(payload.clone()),
            );
        let refusal = broken
            .covers(&stock.declaration())
            .expect_err("version 2 has no step");
        assert!(refusal.to_string().contains('2'), "{refusal}");
    }
}
