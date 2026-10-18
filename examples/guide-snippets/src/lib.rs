#![forbid(unsafe_code)]

//! Compiled author-guide snippets, depending only on `ravel-sdk`.
//!
//! `tests/conformance/tests/integration/guide_snippets.rs` checks source parity with the guide;
//! tests below exercise the snippets. Keep fragment wrappers outside copied lines.

/// `docs/guide/your-first-mod.md` — The mod type, and Refusing.
pub mod first_mod {
    use ravel_sdk::prelude::*;

    #[derive(Default)]
    pub struct CopperLanterns {
        ticks: u64,
    }

    #[declare_mod(id = "copper_lanterns")]
    impl Mod for CopperLanterns {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }
    }

    export_mod!(CopperLanterns::default());

    impl CopperLanterns {
        /// Returns the tick count the guide's struct carries.
        #[must_use]
        pub const fn ticks(&self) -> u64 {
            self.ticks
        }

        /// Raises the refusal the guide's "Refusing" section shows.
        ///
        /// # Errors
        ///
        /// Always. The refusal is what this exists to produce.
        pub fn refuse() -> Result<(), ModError> {
            Err(ModError::refused(
                "the lantern registry name is already taken",
            ))
        }
    }
}

/// `docs/guide/content.md` — Declaring content.
pub mod content {
    use ravel_sdk::prelude::*;
    use ravel_sdk::tags::TagId;

    /// A mod with content and no state.
    #[derive(Default)]
    pub struct CopperLanterns;

    /// The tag of `kind` called `lanterns` in the convention namespace.
    ///
    /// The guide's block calls this; a mod author writes it once beside their
    /// declaration.
    ///
    /// # Panics
    ///
    /// Panics if `c:lanterns` stops being a legal tag of `kind`.
    #[must_use]
    pub fn lanterns(kind: ContentKind) -> TagId {
        let namespace: ModId = "c".parse().expect("`c` is a legal mod id");
        TagId::new(
            kind,
            ContentId::new(namespace, "lanterns").expect("`lanterns` is a legal content path"),
        )
        .expect("a tag groups the registry its kind names")
    }

    impl CopperLanterns {
        fn declaration() -> Result<ContentDeclaration, ModError> {
            let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
            let block = declaration
                .entry(ContentKind::Block, "copper_lantern")
                .map_err(ModError::from_display)?;
            declaration
                .entry(ContentKind::Item, "copper_lantern")
                .map_err(ModError::from_display)?;
            declaration
                .tag(&lanterns(ContentKind::Block), &block)
                .map_err(ModError::from_display)?;
            Ok(declaration)
        }
    }

    #[declare_mod(id = "copper_lanterns")]
    impl Mod for CopperLanterns {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }

        fn content_declaration(
            &self,
        ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
            Self::declaration().map(std::borrow::Cow::Owned).map(Some)
        }
    }
}

/// `docs/guide/running.md` — Sides.
pub mod client_only {
    use ravel_sdk::prelude::*;
    use ravel_sdk::sides::ClientOnly;

    /// What a mod keeps because it is on the client.
    #[derive(Default)]
    pub struct LatestReport {
        /// The lines a client-side overlay would show.
        pub lines: Vec<String>,
    }

    /// A mod that carries its own side, because `ClientOnly` asks for it twice.
    pub struct CopperLanterns {
        side: Side,
        report: ClientOnly<LatestReport>,
    }

    impl CopperLanterns {
        fn new(side: Side) -> Self {
            Self {
                side,
                // On a server side the closure is never called, so the value is not
                // built rather than being built and hidden.
                report: ClientOnly::built_on(side, LatestReport::default),
            }
        }

        fn note(&mut self, line: &str) {
            if let Ok(report) = self.report.get_mut(self.side) {
                report.lines.push(line.to_owned());
            }
        }
    }

    // Outside the copied lines, and below them so the lines above stay a
    // contiguous copy of the guide: the mod half this snippet's struct belongs
    // to, the export `docs/guide/your-first-mod.md` shows for a side-dependent
    // mod, and the two entry points the crate's tests drive the snippet
    // through.
    #[declare_mod(id = "copper_lanterns")]
    impl Mod for CopperLanterns {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }
    }

    export_mod!(|side| CopperLanterns::new(side));

    impl CopperLanterns {
        /// Builds the snippet's mod for `side` and records the one line it records.
        #[must_use]
        pub fn built_and_noted(side: Side) -> Self {
            let mut lanterns = Self::new(side);
            lanterns.note("copper lanterns: 3");
            lanterns
        }

        /// Returns the client-only report the snippet keeps.
        #[must_use]
        pub const fn report(&self) -> &ClientOnly<LatestReport> {
            &self.report
        }
    }
}

/// `docs/guide/state.md` — World data is versioned, and you cross the versions.
pub mod state {
    use ravel_sdk::JsonValue;
    use ravel_sdk::json;
    use ravel_sdk::prelude::*;
    use ravel_sdk::save::{SavedDataEnvelope, SavedDataPlan};

    /// A mod that owns one list of entries in every world.
    #[derive(Default)]
    pub struct CopperLanterns {
        entries: Vec<String>,
    }

    fn read_entries(current: &JsonValue) -> Vec<String> {
        current
            .get("entries")
            .and_then(|value| value.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn v(raw: u32) -> SavedDataVersion {
        SavedDataVersion::new(raw).expect("every declared version is non-zero")
    }

    /// Version 1 stored one entry; version 2 stores a list.
    fn to_two(payload: &JsonValue) -> Result<JsonValue, MigrationFailed> {
        let entry = payload
            .get("entry")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| MigrationFailed::new("version 1 held no `entry` string"))?;
        Ok(json!({ "entries": [entry] }))
    }

    #[declare_mod(id = "copper_lanterns")]
    impl Mod for CopperLanterns {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }

        fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
            SavedDataDeclaration::new(Self::declared_id().clone(), [v(1), v(2)])
                .map(Some)
                .map_err(ModError::from_display)
        }

        fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, ModError> {
            Ok(Some(
                MigrationChain::new(Self::declared_id().clone()).reaching(v(2), to_two),
            ))
        }

        fn open_saved_data(
            &mut self,
            envelope: Option<&SavedDataEnvelope>,
            _plan: &SavedDataPlan,
        ) -> Result<(), ModError> {
            let Some(envelope) = envelope else {
                self.entries.clear();
                return Ok(());
            };
            self.entries = read_entries(envelope.payload());
            Ok(())
        }

        fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
            Ok(Some(SavedDataEnvelope::current(
                Self::declared_id().clone(),
                v(2),
                json!({ "entries": self.entries }),
            )))
        }
    }
}

/// `docs/guide/state.md` — Settings are a document, never a screen.
///
/// The document itself is JSON, beside this file; this is the code half.
pub mod settings {
    use ravel_sdk::prelude::*;

    /// A mod whose one setting is how many lanterns a stack holds.
    #[derive(Default)]
    pub struct CopperLanterns {
        capacity: u64,
    }

    impl CopperLanterns {
        /// Returns the capacity the resolved settings gave this mod.
        #[must_use]
        pub const fn capacity(&self) -> u64 {
            self.capacity
        }
    }

    const SETTINGS: &str = include_str!("../configuration.json");
    const CAPACITY: &str = "lanterns.capacity";

    #[declare_mod(id = "copper_lanterns")]
    impl Mod for CopperLanterns {
        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
            ConfigurationDeclaration::parse(SETTINGS)
                .map(Some)
                .map_err(ModError::from_display)
        }

        fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
            let key: SettingKey = CAPACITY.parse().expect("the key this mod declared");
            self.capacity = u64::try_from(resolved.integer(&key).map_err(ModError::from_display)?)
                .map_err(ModError::from_display)?;
            Ok(())
        }
    }
}

/// `docs/guide/package-manifest.md` — Registering content only when the mod it
/// integrates with is running.
pub mod optional_integration {
    use ravel_sdk::prelude::*;

    /// What this mod registers when the lanterns mod is running, and nothing
    /// otherwise.
    ///
    /// # Errors
    ///
    /// Returns a refusal if the integration cannot be declared.
    pub fn integration() -> Result<ContentDeclaration, ModError> {
        let owner = "foundry".parse::<ModId>().map_err(ModError::from_display)?;
        let mut declaration = ContentDeclaration::new(owner);
        declaration
            .entry(ContentKind::Recipe, "lit_copper")
            .map_err(ModError::from_display)?;
        Ok(declaration)
    }

    /// A mod whose recipe exists only when another mod does.
    pub struct Foundry {
        lanterns: ModId,
    }

    impl Default for Foundry {
        fn default() -> Self {
            Self {
                lanterns: "lanterns".parse().expect("`lanterns` is a legal mod id"),
            }
        }
    }

    #[declare_mod(id = "foundry")]
    impl Mod for Foundry {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
            if registrar.loads(&self.lanterns) {
                claim_all(&integration()?, registrar)?;
            }
            Ok(())
        }
    }
}

/// `docs/guide/state.md` — Using a mod only if it is installed.
pub mod optional_service {
    use ravel_sdk::prelude::*;
    use ravel_sdk::{JsonValue, json};

    /// The service the atlas answers, which this mod uses when one is installed.
    ///
    /// # Panics
    ///
    /// Panics if `atlas:landmarks` stops being a legal service name, which would
    /// mean the mod this one may use can no longer be named.
    #[must_use]
    pub fn landmarks() -> ServiceId {
        let atlas = "atlas".parse().expect("a legal mod id");
        ServiceId::new(ContentId::new(atlas, "landmarks").expect("a legal path"))
    }

    /// A mod that is better with the atlas installed and works without it.
    #[derive(Default)]
    pub struct Compass {
        landmarks: Option<JsonValue>,
        alone: bool,
    }

    impl Compass {
        /// Returns what an atlas named, when one did.
        #[must_use]
        pub const fn landmarks(&self) -> Option<&JsonValue> {
            self.landmarks.as_ref()
        }

        /// Returns whether this mod asked and found no atlas.
        #[must_use]
        pub const fn alone(&self) -> bool {
            self.alone
        }
    }

    #[declare_mod(id = "compass")]
    impl Mod for Compass {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            ServiceDeclaration::with_optional(
                Self::declared_id().clone(),
                [],            // what you answer
                [],            // what you need
                [landmarks()], // what you use if something answers it
            )
            .map(Some)
            .map_err(ModError::from_display)
        }

        fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
            match directory.call(&ServiceRequest::new(landmarks(), json!({ "within": 512 }))) {
                Ok(answer) => {
                    self.landmarks = answer.value().get("landmarks").cloned();
                    Ok(())
                }
                // The one outcome you carry on past: nothing here answers it.
                Err(failed) if failed.is_absent() => {
                    self.alone = true;
                    Ok(())
                }
                // The atlas is installed and said no, which is a different thing.
                Err(failed) => Err(ModError::from_display(failed)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::config::LayeredConfiguration;
    use ravel_sdk::prelude::*;
    use ravel_sdk::save::{SavedDataEnvelope, SavedDataPlan};
    use ravel_sdk::services::{ServiceCallFailed, ServiceUnavailable};
    use ravel_sdk::{JsonValue, json};

    /// A registrar that accepts everything, as the guide tells an author to write.
    #[derive(Default)]
    struct Accepting {
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Accepting {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    #[test]
    fn the_first_mod_snippet_reports_its_declared_identity() {
        let mut first = crate::first_mod::CopperLanterns::default();
        assert_eq!(first.id().as_str(), "copper_lanterns");
        assert_eq!(first.ticks(), 0);
        first
            .enter(LifecycleState::Loaded)
            .expect("a complete mod accepts every step");
    }

    #[test]
    fn the_refusal_snippet_reports_the_authors_detail() {
        let error = crate::first_mod::CopperLanterns::refuse().expect_err("it refuses");
        assert_eq!(
            error,
            ModError::refused("the lantern registry name is already taken")
        );
    }

    #[test]
    fn the_content_snippet_claims_the_entries_it_declared() {
        let mut content = crate::content::CopperLanterns;
        let mut registrar = Accepting::default();
        content
            .register(&mut registrar)
            .expect("an accepting registrar refuses nothing");
        assert_eq!(
            registrar
                .claimed()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [
                "block:copper_lanterns:copper_lantern",
                "item:copper_lanterns:copper_lantern"
            ]
        );
    }

    #[test]
    fn the_client_only_snippet_builds_nothing_on_a_server_side() {
        let server = crate::client_only::CopperLanterns::built_and_noted(Side::DedicatedServer);
        assert!(!server.report().is_present());
        assert!(server.report().get(Side::DedicatedServer).is_err());

        let client = crate::client_only::CopperLanterns::built_and_noted(Side::Client);
        assert_eq!(
            client
                .report()
                .get(Side::Client)
                .expect("present on the client")
                .lines
                .len(),
            1
        );
    }

    #[test]
    fn the_state_snippet_crosses_the_declared_step() {
        let mut state = crate::state::CopperLanterns::default();
        let declaration = state
            .saved_data_declaration()
            .expect("valid saved-data declaration")
            .expect("this mod owns world data");
        let chain = state
            .saved_data_migrations()
            .expect("valid saved-data migrations")
            .expect("and declares a chain");
        chain
            .covers(&declaration)
            .expect("every version above the oldest has a step");

        let one = SavedDataVersion::new(1).expect("1 is not zero");
        let plan =
            SavedDataPlan::open(&declaration, Some(one)).expect("a version-1 world is admissible");
        let payload = chain
            .apply(&plan, &json!({ "entry": "found copper" }))
            .expect("the loader migrates a version-1 world");
        let current = declaration.current();
        let stored = SavedDataEnvelope::current(
            crate::state::CopperLanterns::declared_id().clone(),
            current,
            payload,
        );
        let plan = SavedDataPlan::open(&declaration, Some(current))
            .expect("the current version is admissible");
        state
            .open_saved_data(Some(&stored), &plan)
            .expect("the callback reads the loader's current payload");

        let saved = state
            .save_saved_data()
            .expect("saving succeeds")
            .expect("this mod owns data");
        assert_eq!(saved.payload(), &json!({ "entries": ["found copper"] }));
    }

    #[test]
    fn a_world_holding_nothing_this_mod_owns_initializes_fresh() {
        let mut state = crate::state::CopperLanterns::default();
        let declaration = state
            .saved_data_declaration()
            .expect("valid saved-data declaration")
            .expect("this mod owns world data");
        let plan = SavedDataPlan::open(&declaration, None).expect("absence initializes");
        state
            .open_saved_data(None, &plan)
            .expect("absence is not a refusal");
        let saved = state
            .save_saved_data()
            .expect("saving succeeds")
            .expect("this mod owns data");
        assert_eq!(saved.payload(), &json!({ "entries": [] }));
    }

    #[test]
    fn the_settings_snippet_reads_the_authors_default() {
        let mut settings = crate::settings::CopperLanterns::default();
        settings
            .enter(LifecycleState::Loaded)
            .expect("the settings mod accepts the loaded state");
        let declaration = settings
            .configuration_declaration()
            .expect("valid configuration declaration")
            .expect("the document parses");
        let resolved =
            LayeredConfiguration::resolve(declaration, None, None).expect("nothing overrides");
        settings
            .configure(&resolved)
            .expect("the author's default is a count");
        assert_eq!(settings.capacity(), 16);
    }

    /// JSON payload types remain available through the SDK even when a mod does
    /// not need a direct `serde_json` dependency.
    #[test]
    fn a_json_payload_is_a_value_a_mod_can_name() {
        fn capacity(value: &JsonValue) -> Option<u64> {
            value.as_u64()
        }

        assert_eq!(capacity(&json!(16)), Some(16));
        assert_eq!(capacity(&json!("sixteen")), None);
    }

    /// A directory that answers one fixed way, so each branch can be driven.
    ///
    /// What a real host does with these declarations is checked over
    /// `ravel_loader::open_services` in
    /// `tests/conformance/tests/integration/optional_services.rs`; this only reaches the three
    /// branches the guide tells an author to write.
    struct Fixed(Result<ServiceResponse, ServiceCallFailed>);

    impl ServiceDirectory for Fixed {
        fn call(
            &mut self,
            _request: &ServiceRequest,
        ) -> Result<ServiceResponse, ServiceCallFailed> {
            self.0.clone()
        }
    }

    #[test]
    fn the_optional_service_snippet_declares_nothing_required() {
        let compass = crate::optional_service::Compass::default();
        let declaration = compass
            .service_declaration()
            .expect("valid service declaration")
            .expect("the snippet declares one service");
        // The guide's claim: this mod never stops a set from loading.
        assert_eq!(declaration.requires().count(), 0);
        assert!(declaration.may_do_without(&crate::optional_service::landmarks()));
        assert!(declaration.calls(&crate::optional_service::landmarks()));
    }

    #[test]
    fn the_optional_service_snippet_carries_on_past_absence_and_nothing_else() {
        let mut answered = crate::optional_service::Compass::default();
        answered
            .use_services(&mut Fixed(Ok(ServiceResponse::new(
                json!({ "landmarks": ["spire"] }),
            ))))
            .expect("an answered call");
        assert_eq!(answered.landmarks(), Some(&json!(["spire"])));
        assert!(!answered.alone());

        let mut alone = crate::optional_service::Compass::default();
        alone
            .use_services(&mut Fixed(Err(ServiceCallFailed::Unavailable(
                ServiceUnavailable::Absent {
                    service: crate::optional_service::landmarks(),
                },
            ))))
            .expect("absence is not failure");
        assert!(alone.alone() && alone.landmarks().is_none());

        // An atlas that was reached and refused is a different event, and the
        // guide tells an author not to swallow it.
        let mut refused = crate::optional_service::Compass::default();
        assert!(
            refused
                .use_services(&mut Fixed(Err(ServiceCallFailed::Refused(
                    ravel_sdk::HostError::Refused {
                        mod_id: crate::optional_service::Compass::declared_id().clone(),
                        state: LifecycleState::Running,
                        detail: "no survey loaded".to_owned(),
                    }
                ))))
                .is_err()
        );
        assert!(!refused.alone());
    }
}
