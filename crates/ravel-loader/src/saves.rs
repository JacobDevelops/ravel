use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{
    DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, LifecycleState, ModId,
    StructuredFailure, Version,
};
use ravel_host_api::{HostError, HostMod};
use ravel_save::{
    MigrationChain, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope, SavedDataError,
    SavedDataPlan,
};

use crate::lifecycle::{LifecycleMachine, Requirement};

/// Validates and migrates all saved data before handing entries to owners in load order.
///
/// Parked mods are skipped. Failed owners' prior envelopes remain opaque without
/// consulting metadata, invoking their code, or migrating their payloads. Active owners
/// receive current payloads and a current plan. Optional callback failures preserve prior data;
/// required callback failures and invalid data prevent world opening.
/// Callback failures are attributed using the supplied session `environment`, including
/// its Minecraft and Ravel versions; persistence failures have no failure surface.
///
/// # Errors
///
/// Returns [`SaveRefused`] for invalid data or migrations, foreign ownership, or owner refusal.
pub fn open_saved_data(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    mods: &mut [&mut (dyn HostMod + '_)],
    bytes: Option<&[u8]>,
    packages: &BTreeMap<ModId, Version>,
) -> Result<SavedDataAggregate, SaveRefused> {
    let (declarations, chains) = declarations(machine, environment, mods)?;
    let mut aggregate =
        SavedDataAggregate::open_preserving(bytes, &declarations, packages, |owner| {
            machine.state(owner) == Ok(LifecycleState::Failed)
        })
        .map_err(SaveRefused::Data)?;
    migrate(machine, environment, &mut aggregate, &declarations, &chains)?;
    let order: Vec<ModId> = machine.order().cloned().collect();
    for mod_id in order {
        if machine.state(&mod_id) == Ok(LifecycleState::Failed) {
            continue;
        }
        let Some(declaration) = declarations.iter().find(|value| value.owner() == &mod_id) else {
            continue;
        };
        let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) else {
            continue;
        };
        let envelope = aggregate.envelope(&mod_id);
        let plan = SavedDataPlan::open(declaration, envelope.map(SavedDataEnvelope::data_version))
            .map_err(SaveRefused::Data)?;
        if owner_callback(machine, environment, &mod_id, || {
            host.open_saved_data(envelope, &plan)
        })?
        .is_none()
        {
            let prior =
                SavedDataAggregate::open_preserving(bytes, &declarations, packages, |owner| {
                    machine.state(owner) == Ok(LifecycleState::Failed)
                })
                .map_err(SaveRefused::Data)?;
            if let Some(envelope) = prior.envelope(&mod_id) {
                aggregate.replace(envelope.clone());
            }
        }
    }
    Ok(aggregate)
}

fn migrate(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    aggregate: &mut SavedDataAggregate,
    declarations: &[SavedDataDeclaration],
    chains: &[MigrationChain],
) -> Result<(), SaveRefused> {
    for declaration in declarations {
        let mod_id = declaration.owner();
        if machine.state(mod_id) == Ok(LifecycleState::Failed) {
            continue;
        }
        let Some(envelope) = aggregate.envelope(mod_id) else {
            continue;
        };
        let plan = SavedDataPlan::resolve(declaration, envelope.data_version())
            .map_err(SaveRefused::Data)?;
        if plan.is_current() {
            continue;
        }
        let chain = chains
            .iter()
            .find(|chain| chain.owner() == mod_id)
            .ok_or_else(|| {
                SaveRefused::Data(SavedDataError::MissingMigration {
                    owner: mod_id.clone(),
                    to: declaration.current(),
                })
            })?;
        let Some(payload) = owner_callback(machine, environment, mod_id, || {
            Ok(chain.apply(&plan, envelope.payload()))
        })?
        else {
            continue;
        };
        let payload = payload.map_err(SaveRefused::Data)?;
        aggregate.replace(envelope.migrated(declaration.current(), payload));
    }
    Ok(())
}

fn declarations(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    mods: &[&mut (dyn HostMod + '_)],
) -> Result<(Vec<SavedDataDeclaration>, Vec<MigrationChain>), SaveRefused> {
    let mut declarations = Vec::with_capacity(mods.len());
    let mut chains = Vec::with_capacity(mods.len());
    for host in mods {
        let mod_id = host.id();
        if !loads_here(machine, mod_id) {
            continue;
        }
        if machine.state(mod_id) == Ok(LifecycleState::Failed) {
            continue;
        }
        let Some(declaration) = owner_callback(machine, environment, mod_id, || {
            host.saved_data_declaration()
        })?
        .flatten() else {
            continue;
        };
        if declaration.owner() != mod_id {
            return Err(SaveRefused::ForeignDeclaration {
                mod_id: mod_id.clone(),
                owner: declaration.owner().clone(),
            });
        }
        let Some(chain) = owner_callback(machine, environment, mod_id, || {
            host.saved_data_migrations()
        })?
        else {
            continue;
        };
        if let Some(chain) = chain {
            chain.covers(&declaration).map_err(SaveRefused::Data)?;
            chains.push(chain);
        } else if declaration.versions().count() > 1 {
            return Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: mod_id.clone(),
                to: declaration.current(),
            }));
        }
        declarations.push(declaration);
    }
    Ok((declarations, chains))
}

fn owner_callback<T>(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    mod_id: &ModId,
    call: impl FnOnce() -> Result<T, ravel_host_api::ModError>,
) -> Result<Option<T>, SaveRefused> {
    let state = machine.state(mod_id).expect("loaded owner is tracked");
    match crate::callback(mod_id, state, call) {
        Ok(value) => Ok(Some(value)),
        Err(source) => record_failure(machine, environment, mod_id, source),
    }
}

fn record_failure<T>(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    mod_id: &ModId,
    source: HostError,
) -> Result<Option<T>, SaveRefused> {
    let required = machine
        .entry(mod_id)
        .expect("loaded owner is tracked")
        .requirement
        == Requirement::Required;
    let attribution = FailureAttribution {
        mod_id: Some(mod_id.clone()),
        version: None,
        dependency_chain: Vec::new(),
        state: source.state(),
        kind: None,
        surface: None,
        environment: environment.clone(),
        message: DiagnosticMessage::raw(source.to_string()),
    };
    let recorded = machine.fail(mod_id, attribution);
    if required || recorded.is_err() {
        return Err(SaveRefused::Owner {
            mod_id: mod_id.clone(),
            source,
        });
    }
    Ok(None)
}

fn loads_here(machine: &LifecycleMachine, mod_id: &ModId) -> bool {
    machine
        .entry(mod_id)
        .is_ok_and(|entry| entry.sides.loads_on(machine.side()))
}

/// Why a world's saved data could not be opened or written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveRefused {
    /// The aggregate, a version, or a migration was refused.
    Data(SavedDataError),
    /// A mod declared saved data owned by another identity.
    ForeignDeclaration {
        /// The mod that returned it.
        mod_id: ModId,
        /// The identity the declaration names as its owner.
        owner: ModId,
    },
    /// A mod refused the entry it was handed.
    Owner {
        /// The mod that refused.
        mod_id: ModId,
        /// What it said.
        source: HostError,
    },
    /// A mod produced an envelope owned by another identity.
    ForeignEnvelope {
        /// The mod that produced the envelope.
        mod_id: ModId,
        /// The identity the envelope names as its owner.
        owner: ModId,
    },
}

impl SaveRefused {
    /// Returns the mod this refusal is about, when it is about one.
    #[must_use]
    pub const fn owner(&self) -> Option<&ModId> {
        match self {
            Self::Data(error) => error.owner(),
            Self::ForeignDeclaration { mod_id, .. }
            | Self::ForeignEnvelope { mod_id, .. }
            | Self::Owner { mod_id, .. } => Some(mod_id),
        }
    }
}

impl fmt::Display for SaveRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Data(error) => error.fmt(formatter),
            Self::ForeignDeclaration { mod_id, owner } => write!(
                formatter,
                "`{mod_id}` declared saved data owned by `{owner}`, and a mod owns only its own \
                 world data"
            ),
            Self::ForeignEnvelope { mod_id, owner } => write!(
                formatter,
                "`{mod_id}` produced saved data owned by `{owner}`, and a mod writes only its own \
                 world data"
            ),
            Self::Owner { mod_id, source } => {
                write!(formatter, "`{mod_id}` refused its saved data: {source}")
            }
        }
    }
}

impl std::error::Error for SaveRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Data(error) => Some(error),
            Self::ForeignDeclaration { .. } | Self::ForeignEnvelope { .. } => None,
            Self::Owner { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for SaveRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Owner { source, .. } => source.state(),
            Self::Data(_) | Self::ForeignDeclaration { .. } => LifecycleState::Registered,
            Self::ForeignEnvelope { .. } => LifecycleState::Running,
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Data(error) => error.quotes_untrusted_text(),
            Self::ForeignDeclaration { .. } | Self::ForeignEnvelope { .. } => false,
            Self::Owner { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use ravel_core::{FailureEnvironment, LifecycleState, ModId, Side, StructuredFailure};
    use ravel_host_api::{HostError, HostMod, ModError};
    use ravel_save::{
        MigrationChain, MigrationFailed, SavedDataAggregate, SavedDataDeclaration,
        SavedDataEnvelope, SavedDataError, SavedDataPlan, SavedDataVersion,
    };
    use serde_json::{Value, json};

    use super::{SaveRefused, open_saved_data};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};
    use crate::tests::entry;

    struct Failing {
        id: ModId,
        phase: &'static str,
    }

    impl HostMod for Failing {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
            assert_ne!(self.phase, "declaration", "declaration panic");
            Ok(Some(
                SavedDataDeclaration::new(self.id.clone(), [version(1)]).expect("valid"),
            ))
        }

        fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, ModError> {
            assert_ne!(self.phase, "migrations", "migration declaration panic");
            Ok(None)
        }

        fn open_saved_data(
            &mut self,
            _envelope: Option<&SavedDataEnvelope>,
            _plan: &SavedDataPlan,
        ) -> Result<(), ModError> {
            assert_ne!(self.phase, "open", "opening panic");
            Err(ModError::Unsupported)
        }
    }

    #[test]
    fn saved_data_callbacks_cannot_forge_the_owner_or_unwind() {
        for phase in ["declaration", "migrations", "open", "refusal"] {
            let mut host = Failing {
                id: id("actual"),
                phase,
            };
            let mut machine = machine(vec![entry("actual", &[Side::DedicatedServer])]);
            let environment = environment();
            let refusal = open_saved_data(
                &mut machine,
                &environment,
                &mut [&mut host],
                None,
                &BTreeMap::new(),
            )
            .expect_err("all failures stay within the callback boundary");
            let SaveRefused::Owner { mod_id, source } = refusal else {
                panic!("expected an owner refusal");
            };
            assert_eq!(mod_id, id("actual"));
            assert_eq!(source.mod_id(), &id("actual"));
            assert_eq!(source.state(), LifecycleState::Discovered);
            assert_eq!(machine.state(&id("actual")), Ok(LifecycleState::Failed));
            let failure = machine
                .failure(&id("actual"))
                .expect("tracked")
                .expect("recorded");
            assert_eq!(failure.environment, environment);
            assert_eq!(failure.surface, None);
        }
    }

    #[test]
    fn migration_callback_panics_are_owner_refusals() {
        fn panics(_payload: &Value) -> Result<Value, MigrationFailed> {
            panic!("migration panic")
        }
        let mut host = Storing::new("stored");
        host.chain = Some(
            MigrationChain::new(id("stored"))
                .reaching(version(2), panics)
                .reaching(version(3), to_three),
        );
        let mut aggregate = SavedDataAggregate::fresh();
        aggregate.replace(SavedDataEnvelope::current(
            id("stored"),
            version(1),
            json!({}),
        ));
        let bytes = aggregate.to_bytes().expect("valid aggregate");
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);
        let refusal = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut host],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect_err("migration panic is contained");
        assert!(matches!(refusal, SaveRefused::Owner { mod_id, .. } if mod_id == id("stored")));
    }

    #[test]
    fn failed_owner_envelopes_are_admitted_without_callbacks_or_migration() {
        let mut host = Failing {
            id: id("stored"),
            phase: "declaration",
        };
        let mut owner = entry("stored", &[Side::DedicatedServer]);
        owner.saved_data_versions = BTreeSet::from([version(1), version(2)]);
        owner.requirement = Requirement::Optional;
        let mut machine = machine(vec![owner]);
        machine
            .fail(
                &id("stored"),
                ravel_core::FailureAttribution {
                    mod_id: Some(id("stored")),
                    version: None,
                    dependency_chain: Vec::new(),
                    state: LifecycleState::Discovered,
                    kind: None,
                    surface: None,
                    environment: FailureEnvironment {
                        ravel: "0.1.0".parse().expect("version"),
                        minecraft: "test".to_owned(),
                        platform: "linux".to_owned(),
                        architecture: "x86_64".to_owned(),
                    },
                    message: ravel_core::DiagnosticMessage::raw("startup refusal"),
                },
            )
            .expect("a startup failure");
        let mut prior = SavedDataAggregate::fresh();
        prior.replace(SavedDataEnvelope::current(
            id("stored"),
            version(99),
            json!({"poisonedForActiveMigration": {"unknownFutureShape": [null, "opaque"]}}),
        ));
        let bytes = prior.to_bytes().expect("valid aggregate");
        let admitted = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut host],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect("failed optional owner is carried over unchanged");
        assert_eq!(admitted.to_bytes().expect("unchanged aggregate"), bytes);
    }

    #[test]
    fn optional_callback_failures_preserve_the_original_envelope() {
        for phase in ["declaration", "migrations", "open", "refusal"] {
            let mut host = Failing {
                id: id("stored"),
                phase,
            };
            let mut owner = entry("stored", &[Side::DedicatedServer]);
            owner.requirement = Requirement::Optional;
            let mut machine = machine(vec![owner]);
            let bytes = world(&[("stored", 1, json!({"opaque": [null, "unchanged"]}))]);
            let environment = environment();
            let aggregate = open_saved_data(
                &mut machine,
                &environment,
                &mut [&mut host],
                Some(&bytes),
                &BTreeMap::new(),
            )
            .expect("optional owner does not refuse the world");
            assert_eq!(machine.state(&id("stored")), Ok(LifecycleState::Failed));
            let failure = machine
                .failure(&id("stored"))
                .expect("tracked")
                .expect("recorded");
            assert_eq!(failure.state, LifecycleState::Discovered);
            assert_eq!(failure.environment, environment);
            assert_eq!(failure.surface, None);
            assert_eq!(aggregate.to_bytes().expect("preserved"), bytes);
        }
    }

    #[test]
    fn optional_migration_panics_preserve_the_original_envelope() {
        fn panics(_payload: &Value) -> Result<Value, MigrationFailed> {
            panic!("migration panic")
        }
        let mut host = Storing::new("stored");
        host.chain = Some(
            MigrationChain::new(id("stored"))
                .reaching(version(2), panics)
                .reaching(version(3), to_three),
        );
        let mut owner = entry("stored", &[Side::DedicatedServer]);
        owner.requirement = Requirement::Optional;
        let mut machine = machine(vec![owner]);
        let bytes = world(&[("stored", 1, json!({"opaque": true}))]);
        let aggregate = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut host],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect("optional migration failure does not refuse the world");
        assert_eq!(machine.state(&id("stored")), Ok(LifecycleState::Failed));
        assert_eq!(aggregate.to_bytes().expect("preserved"), bytes);
        assert!(host.read.is_none());
    }

    #[test]
    fn active_migration_and_failed_owner_preservation_share_one_world() {
        let mut active = Storing::new("active");
        active.versions = vec![version(1), version(2)];
        active.chain = Some(MigrationChain::new(id("active")).reaching(version(2), to_two));
        let mut failed = Storing::new("failed").refusing();
        let mut owner = entry("failed", &[Side::DedicatedServer]);
        owner.requirement = Requirement::Optional;
        let mut machine = machine(vec![entry("active", &[Side::DedicatedServer]), owner]);
        let bytes = world(&[
            ("active", 1, json!({"count": 7})),
            ("failed", 1, json!({"count": 19})),
        ]);
        let aggregate = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut active, &mut failed],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect("active owner opens despite optional refusal after migration");
        assert_eq!(active.read, Some(json!({"counts": [7]})));
        assert_eq!(machine.state(&id("failed")), Ok(LifecycleState::Failed));
        let expected = world(&[
            ("active", 2, json!({"counts": [7]})),
            ("failed", 1, json!({"count": 19})),
        ]);
        assert_eq!(aggregate.to_bytes().expect("mixed world"), expected);
    }

    #[test]
    fn required_migration_refusal_does_not_rewrite_the_input_world() {
        fn refuses(_payload: &Value) -> Result<Value, MigrationFailed> {
            Err(MigrationFailed::new("cannot migrate"))
        }
        let mut first = Storing::new("first");
        let mut second = Storing::new("second");
        second.chain = Some(
            MigrationChain::new(id("second"))
                .reaching(version(2), to_two)
                .reaching(version(3), refuses),
        );
        let mut machine = machine(vec![
            entry("first", &[Side::DedicatedServer]),
            entry("second", &[Side::DedicatedServer]),
        ]);
        let bytes = world(&[
            ("first", 1, json!({"count": 7})),
            ("second", 1, json!({"count": 19})),
        ]);
        let refusal = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut first, &mut second],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect_err("later migration refuses the world");
        assert_eq!(refusal.owner(), Some(&id("second")));
        assert!(first.read.is_none() && second.read.is_none());
        assert_eq!(
            bytes,
            world(&[
                ("first", 1, json!({"count": 7})),
                ("second", 1, json!({"count": 19})),
            ])
        );
    }

    #[test]
    fn active_migration_skips_an_already_failed_owners_future_data() {
        let mut active = Storing::new("active");
        active.versions = vec![version(1), version(2)];
        active.chain = Some(MigrationChain::new(id("active")).reaching(version(2), to_two));
        let mut failed = Failing {
            id: id("failed"),
            phase: "declaration",
        };
        let mut owner = entry("failed", &[Side::DedicatedServer]);
        owner.requirement = Requirement::Optional;
        let mut machine = machine(vec![entry("active", &[Side::DedicatedServer]), owner]);
        super::record_failure::<()>(
            &mut machine,
            &environment(),
            &id("failed"),
            HostError::Unsupported {
                mod_id: id("failed"),
                state: LifecycleState::Discovered,
            },
        )
        .expect("optional failure is recorded");
        let bytes = world(&[
            ("active", 1, json!({"count": 7})),
            ("failed", 99, json!({"opaque": [null, "future"]})),
        ]);
        let aggregate = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut active, &mut failed],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect("failed owner metadata is never consulted");
        assert_eq!(
            aggregate.to_bytes().expect("mixed world"),
            world(&[
                ("active", 2, json!({"counts": [7]})),
                ("failed", 99, json!({"opaque": [null, "future"]})),
            ])
        );
    }

    fn environment() -> FailureEnvironment {
        FailureEnvironment {
            ravel: "9.8.7".parse().expect("version"),
            minecraft: "Synthetic26.1.5-test-session".to_owned(),
            platform: "test-platform".to_owned(),
            architecture: "test-architecture".to_owned(),
        }
    }

    fn id(value: &str) -> ModId {
        value.parse().expect("a legal mod id")
    }

    fn version(raw: u32) -> SavedDataVersion {
        SavedDataVersion::new(raw).expect("a non-zero version")
    }

    fn to_two(payload: &Value) -> Result<Value, MigrationFailed> {
        let count = payload
            .get("count")
            .and_then(Value::as_u64)
            .ok_or_else(|| MigrationFailed::new("version 1 held no `count`"))?;
        Ok(json!({ "counts": [count] }))
    }

    fn to_three(payload: &Value) -> Result<Value, MigrationFailed> {
        let counts = payload
            .get("counts")
            .cloned()
            .ok_or_else(|| MigrationFailed::new("version 2 held no `counts`"))?;
        Ok(json!({ "counts": counts, "total": 0 }))
    }

    struct Storing {
        id: ModId,
        versions: Vec<SavedDataVersion>,
        chain: Option<MigrationChain>,
        read: Option<Value>,
        refuses: bool,
        declares_as: Option<ModId>,
        refuses_migrations: bool,
    }

    impl Storing {
        fn new(name: &str) -> Self {
            Self {
                id: id(name),
                versions: vec![version(1), version(2), version(3)],
                chain: Some(
                    MigrationChain::new(id(name))
                        .reaching(version(2), to_two)
                        .reaching(version(3), to_three),
                ),
                read: None,
                refuses: false,
                declares_as: None,
                refuses_migrations: false,
            }
        }

        fn declaring_for(mut self, other: &str) -> Self {
            self.declares_as = Some(id(other));
            self
        }

        fn without_a_link(mut self) -> Self {
            self.chain = Some(MigrationChain::new(self.id.clone()).reaching(version(2), to_two));
            self
        }

        fn without_the_first_link(mut self) -> Self {
            self.chain = Some(MigrationChain::new(self.id.clone()).reaching(version(3), to_three));
            self
        }

        fn refusing(mut self) -> Self {
            self.refuses = true;
            self
        }
    }

    impl HostMod for Storing {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
            let owner = self.declares_as.clone().unwrap_or_else(|| self.id.clone());
            SavedDataDeclaration::new(owner, self.versions.iter().copied())
                .map(Some)
                .map_err(|source| ModError::Refused {
                    detail: source.to_string(),
                })
        }

        fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, ModError> {
            if self.refuses_migrations {
                return Err(ModError::Unsupported);
            }
            Ok(self.chain.clone())
        }

        fn open_saved_data(
            &mut self,
            envelope: Option<&SavedDataEnvelope>,
            _plan: &SavedDataPlan,
        ) -> Result<(), ModError> {
            if self.refuses {
                return Err(ModError::Refused {
                    detail: "this is not a shape this mod wrote".to_owned(),
                });
            }
            self.read = envelope.map(|held| held.payload().clone());
            Ok(())
        }
    }

    #[test]
    fn invalid_saved_data_declaration_is_not_absent() {
        let mut host = Storing::new("invalid_mod");
        host.versions.clear();
        let mut machine = machine(vec![entry("invalid_mod", &[Side::DedicatedServer])]);
        let refusal = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut host],
            None,
            &BTreeMap::new(),
        )
        .expect_err("invalid declarations cannot opt out");
        assert!(matches!(
            refusal,
            SaveRefused::Owner { mod_id, source: HostError::Refused { .. } }
                if mod_id == id("invalid_mod")
        ));
        assert_eq!(host.read, None);
    }

    #[test]
    fn saved_data_migrations_refusal_is_not_absence() {
        let mut host = Storing::new("refusing_mod");
        host.refuses_migrations = true;
        let mut machine = machine(vec![entry("refusing_mod", &[Side::DedicatedServer])]);
        assert_eq!(
            open_saved_data(
                &mut machine,
                &environment(),
                &mut [&mut host],
                None,
                &BTreeMap::new(),
            ),
            Err(SaveRefused::Owner {
                mod_id: id("refusing_mod"),
                source: HostError::Unsupported {
                    mod_id: id("refusing_mod"),
                    state: LifecycleState::Discovered,
                },
            })
        );
        assert_eq!(host.read, None);
    }

    fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
        LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan")
    }

    fn world(owners: &[(&str, u32, Value)]) -> Vec<u8> {
        let mut aggregate = SavedDataAggregate::fresh();
        for (owner, at, payload) in owners {
            aggregate.replace(SavedDataEnvelope::new(
                id(owner),
                version(*at),
                Vec::new(),
                None,
                payload.clone(),
            ));
        }
        aggregate.to_bytes().expect("a bounded aggregate")
    }

    #[test]
    fn a_world_two_versions_behind_reaches_the_current_shape_before_the_mod_reads_it() {
        let bytes = world(&[("stored", 1, json!({ "count": 7 }))]);
        let mut host = Storing::new("stored");
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        let aggregate = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut host],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect("both steps run");

        assert_eq!(host.read, Some(json!({ "counts": [7], "total": 0 })));
        assert_eq!(
            aggregate
                .envelope(&id("stored"))
                .expect("still present")
                .data_version(),
            version(3),
            "the aggregate a caller saves from is current too, so an old envelope \
             cannot be written back"
        );
    }

    #[test]
    fn a_mod_declaring_another_mods_world_data_is_refused_before_the_aggregate_is_opened() {
        let bytes = world(&[("stored", 3, json!({ "counts": [7], "total": 0 }))]);
        let mut host = Storing::new("thief").declaring_for("stored");
        let mut machine = machine(vec![entry("thief", &[Side::DedicatedServer])]);

        let refused = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut host],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect_err("a mod owns only its own world data");

        assert_eq!(
            refused,
            SaveRefused::ForeignDeclaration {
                mod_id: id("thief"),
                owner: id("stored"),
            }
        );
        let text = refused.to_string();
        assert!(text.contains("thief") && text.contains("stored"), "{text}");
        assert!(host.read.is_none(), "the refusal handed the mod nothing");
        assert!(!refused.quotes_untrusted_text());
    }

    #[test]
    fn a_missing_link_refuses_before_any_mod_is_handed_anything() {
        let bytes = world(&[("stored", 1, json!({ "count": 7 }))]);
        let mut host = Storing::new("stored").without_a_link();
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        assert_eq!(
            open_saved_data(
                &mut machine,
                &environment(),
                &mut [&mut host],
                Some(&bytes),
                &BTreeMap::new()
            ),
            Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: id("stored"),
                to: version(3),
            }))
        );
        assert!(host.read.is_none(), "a mod read a world that was refused");
    }

    #[test]
    fn a_missing_link_refuses_a_world_that_would_never_have_crossed_it() {
        // Validate declared coverage even when this world's current version needs no migration.
        let bytes = world(&[("stored", 3, json!({ "counts": [7], "total": 0 }))]);
        let mut host = Storing::new("stored").without_the_first_link();
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        assert_eq!(
            open_saved_data(
                &mut machine,
                &environment(),
                &mut [&mut host],
                Some(&bytes),
                &BTreeMap::new()
            ),
            Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: id("stored"),
                to: version(2),
            }))
        );
        assert!(host.read.is_none(), "a mod read a world that was refused");

        let mut whole = Storing::new("stored");
        open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut whole],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect("the chain covers every declared version");
        assert!(whole.read.is_some());
    }

    #[test]
    fn one_owner_refusing_withholds_the_world_from_every_owner() {
        let bytes = world(&[
            ("alpha", 3, json!({ "counts": [1], "total": 0 })),
            ("zeta", 3, json!({ "counts": [2], "total": 0 })),
        ]);
        let mut alpha = Storing::new("alpha");
        let mut zeta = Storing::new("zeta").refusing();
        let mut machine = machine(vec![
            entry("alpha", &[Side::DedicatedServer]),
            entry("zeta", &[Side::DedicatedServer]),
        ]);

        let refusal = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut alpha, &mut zeta],
            Some(&bytes),
            &BTreeMap::new(),
        )
        .expect_err("`zeta` refuses its own shape");

        assert_eq!(refusal.owner(), Some(&id("zeta")));
        assert!(refusal.to_string().contains("zeta"), "{refusal}");
        assert!(refusal.quotes_untrusted_text());
    }

    #[test]
    fn absence_opens_a_fresh_world_rather_than_refusing_one() {
        let mut host = Storing::new("stored");
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        let aggregate = open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut host],
            None,
            &BTreeMap::new(),
        )
        .expect("absence initializes");
        assert_eq!(aggregate, SavedDataAggregate::fresh());
        assert!(host.read.is_none());
    }

    #[test]
    fn a_mod_that_declares_versions_and_no_chain_at_all_is_refused() {
        let mut host = Storing::new("stored");
        host.chain = None;
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        assert_eq!(
            open_saved_data(
                &mut machine,
                &environment(),
                &mut [&mut host],
                None,
                &BTreeMap::new(),
            ),
            Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: id("stored"),
                to: version(3),
            }))
        );
    }

    #[test]
    fn a_mod_this_side_parks_owns_no_world_data_this_session() {
        let mut parked = Storing::new("parked");
        parked.chain = None;
        let mut host = Storing::new("stored");
        let mut machine = machine(vec![
            entry("parked", &[Side::Client]),
            entry("stored", &[Side::DedicatedServer]),
        ]);

        open_saved_data(
            &mut machine,
            &environment(),
            &mut [&mut parked, &mut host],
            None,
            &BTreeMap::new(),
        )
        .expect("the parked mod takes no part");
        assert!(parked.read.is_none());
    }
}
