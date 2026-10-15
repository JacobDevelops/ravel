#![forbid(unsafe_code)]

//! A stock provider and shop consumer exercising service ordering.
//!
//! Stock data crosses two migrations with distinct shapes before being read.
//! Failed migrations refuse world opening without partially changing stock.

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

// Read oldest through newest; write the highest version.
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
/// Panics if `prices` is not a legal content path.
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
    /// Panics if [`STOCK_VERSIONS`] is empty or contains zero.
    #[must_use]
    pub fn declaration(&self) -> SavedDataDeclaration {
        SavedDataDeclaration::new(
            Self::declared_id().clone(),
            STOCK_VERSIONS.into_iter().map(version),
        )
        .expect("`STOCK_VERSIONS` is a non-empty list of non-zero versions")
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

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        ConfigurationDeclaration::parse(STOCK_SETTINGS)
            .map(Some)
            .map_err(|error| ModError::Refused {
                mod_id: self.id().clone(),
                state: LifecycleState::Constructed,
                detail: error.to_string(),
            })
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

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        Ok(Some(self.declaration()))
    }

    fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, ModError> {
        Ok(Some(self.chain()))
    }

    /// Migrates the stored payload up to the current version, then reads it.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] for unreadable data, leaving state unchanged.
    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        let Some(envelope) = envelope else {
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

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        ServiceDeclaration::new(Self::declared_id().clone(), [prices()], [])
            .map(Some)
            .map_err(|error| ModError::Refused {
                mod_id: self.id().clone(),
                state: LifecycleState::Constructed,
                detail: error.to_string(),
            })
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
/// let declaration = shop.service_declaration().expect("valid declaration").expect("it requires one service");
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

    /// Returns the provider's answer, or `None` before service wiring.
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

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        ServiceDeclaration::new(Self::declared_id().clone(), [], [prices()])
            .map(Some)
            .map_err(|error| ModError::Refused {
                mod_id: self.id().clone(),
                state: LifecycleState::Constructed,
                detail: error.to_string(),
            })
    }

    /// Calls the one service this mod declared, and refuses without an answer.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the call fails.
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
            .expect("valid configuration declaration")
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
                .expect("valid declaration")
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
        assert!(
            shop.saved_data_declaration()
                .expect("valid declaration")
                .is_none()
        );
        assert!(
            shop.configuration_declaration()
                .expect("valid declaration")
                .is_none()
        );
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
