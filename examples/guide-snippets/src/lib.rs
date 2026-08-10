#![forbid(unsafe_code)]

//! Every Rust block in [`docs/guide/`](../../docs/guide), compiled here.
//!
//! Markdown compiles nowhere, and a guide is prose full of code. So each block in
//! the guide is a copy of a run of lines in this crate, and
//! `tests/conformance/tests/guide_snippets.rs` recomputes that: a block that
//! drifts from the module it was copied from fails the gate, and a block that
//! stops compiling fails it here first.
//!
//! This crate depends on `ravel-sdk` and nothing else, which is the point. A
//! snippet compiled with the workspace in scope proves nothing about whether an
//! author can compile it — the guide exists because that difference was found by
//! hitting it.
//!
//! The tests below drive each snippet rather than only building it. A block that
//! compiles and does something other than what the prose beside it says is the
//! failure a compile check cannot see.
//!
//! Each module names the section it came from. Where the guide shows a fragment
//! rather than an item, the wrapper around it is written outside the copied lines.

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
            Err(ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Constructed,
                detail: "the lantern registry name is already taken".to_owned(),
            })
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
        fn declaration() -> ContentDeclaration {
            let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
            let block = declaration
                .entry(ContentKind::Block, "copper_lantern")
                .expect("legal");
            declaration
                .entry(ContentKind::Item, "copper_lantern")
                .expect("legal");
            declaration
                .tag(&lanterns(ContentKind::Block), &block)
                .expect("blocks are taggable");
            declaration
        }
    }

    #[declare_mod(id = "copper_lanterns")]
    impl Mod for CopperLanterns {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
            claim_all(&Self::declaration(), registrar)
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
    use ravel_sdk::json;
    use ravel_sdk::prelude::*;
    use ravel_sdk::save::{SavedDataEnvelope, SavedDataPlan, SavedPayload};

    /// A mod that owns one list of entries in every world.
    #[derive(Default)]
    pub struct CopperLanterns {
        entries: Vec<String>,
    }

    fn read_entries(current: &SavedPayload) -> Vec<String> {
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
    fn to_two(payload: &SavedPayload) -> Result<SavedPayload, MigrationFailed> {
        let entry = payload
            .get("entry")
            .and_then(SavedPayload::as_str)
            .ok_or_else(|| MigrationFailed::new("version 1 held no `entry` string"))?;
        Ok(json!({ "entries": [entry] }))
    }

    #[declare_mod(id = "copper_lanterns")]
    impl Mod for CopperLanterns {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            let _ = state;
            Ok(())
        }

        fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
            SavedDataDeclaration::new(Self::declared_id().clone(), [v(1), v(2)]).ok()
        }

        fn saved_data_migrations(&self) -> Option<MigrationChain> {
            Some(MigrationChain::new(Self::declared_id().clone()).reaching(v(2), to_two))
        }

        fn open_saved_data(
            &mut self,
            envelope: Option<&SavedDataEnvelope>,
            plan: &SavedDataPlan,
        ) -> Result<(), ModError> {
            let Some(envelope) = envelope else {
                self.entries.clear();
                return Ok(());
            };
            // The payload is at whatever version the world stored, so the chain
            // crosses the steps the plan names before anything is read.
            let current = self
                .saved_data_migrations()
                .expect("this mod declares a chain")
                .apply(plan, envelope.payload())
                .map_err(|failed| ModError::Refused {
                    mod_id: Self::declared_id().clone(),
                    state: LifecycleState::Registered,
                    detail: failed.to_string(),
                })?;
            self.entries = read_entries(&current);
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
    use ravel_sdk::JsonValue;
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
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            // `configuration_declaration` can only answer `Some` or `None`, so a
            // typo in the document becomes `None`, which reads as "declares no
            // settings": `configure` is never called and the mod runs on its own
            // defaults in silence. Refusing here makes that loud.
            ConfigurationDeclaration::parse(SETTINGS).map_err(|error| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state,
                detail: error.to_string(),
            })?;
            Ok(())
        }

        fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
            ConfigurationDeclaration::parse(SETTINGS).ok()
        }

        fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
            let key: SettingKey = CAPACITY.parse().expect("the key this mod declared");
            self.capacity = resolved
                .value(&key)
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| ModError::Refused {
                    mod_id: Self::declared_id().clone(),
                    state: LifecycleState::Constructed,
                    detail: "`lanterns.capacity` resolved to something that is not a count"
                        .to_owned(),
                })?;
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

        fn service_declaration(&self) -> Option<ServiceDeclaration> {
            ServiceDeclaration::with_optional(
                Self::declared_id().clone(),
                [],            // what you answer
                [],            // what you need
                [landmarks()], // what you use if something answers it
            )
            .ok()
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
                Err(failed) => Err(ModError::Refused {
                    mod_id: Self::declared_id().clone(),
                    state: LifecycleState::Constructed,
                    detail: failed.to_string(),
                }),
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
    fn the_refusal_snippet_names_the_mod_and_the_phase() {
        let error = crate::first_mod::CopperLanterns::refuse().expect_err("it refuses");
        assert_eq!(error.state(), LifecycleState::Constructed);
        assert_eq!(
            error.mod_id(),
            crate::first_mod::CopperLanterns::declared_id()
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
            .expect("this mod owns world data");
        state
            .saved_data_migrations()
            .expect("and declares a chain")
            .covers(&declaration)
            .expect("every version above the oldest has a step");

        let one = SavedDataVersion::new(1).expect("1 is not zero");
        let plan =
            SavedDataPlan::open(&declaration, Some(one)).expect("a version-1 world is admissible");
        let stored = SavedDataEnvelope::current(
            crate::state::CopperLanterns::declared_id().clone(),
            one,
            json!({ "entry": "found copper" }),
        );
        state
            .open_saved_data(Some(&stored), &plan)
            .expect("the chain reads a version-1 world");

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
            .expect("the settings document this mod ships parses");
        let declaration = settings
            .configuration_declaration()
            .expect("the document parses");
        let resolved =
            LayeredConfiguration::resolve(declaration, None, None).expect("nothing overrides");
        settings
            .configure(&resolved)
            .expect("the author's default is a count");
        assert_eq!(settings.capacity(), 16);
    }

    /// The guide tells an author the value type is `ravel_sdk::JsonValue` and that
    /// `serde_json` cannot be named from a mod crate. This crate can name only
    /// what a mod can, so writing the type down here is the check.
    #[test]
    fn a_resolved_value_is_a_json_value_a_mod_can_name() {
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
    /// `tests/conformance/tests/optional_services.rs`; this only reaches the three
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
                    ModError::Refused {
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
