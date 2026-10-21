use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{
    FailureEnvironment, FailureKind, LifecycleState, ModId, StructuredFailure, Version,
};
use ravel_host_api::{HostError, Mod};
use ravel_save::{
    MigrationChain, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope, SavedDataError,
    SavedDataPlan,
};

use crate::lifecycle::{LifecycleError, LifecycleMachine};
use crate::loaded::LoadedMods;

/// Snapshots lifecycle members in load order after checking save and world access.
///
/// Failed mods retain their prior envelopes without callbacks. Healthy mods may return no
/// envelope. Any snapshot failure refuses the snapshot and records its owner's failure.
///
/// # Errors
///
/// Returns [`SaveRefused::Lifecycle`] for denied lifecycle access,
/// [`SaveRefused::Mod`] for callback failure, or [`SaveRefused::ForeignEnvelope`]
/// when an instance attempts to write another owner's data.
pub fn produce_saved_data<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    prior: Option<&SavedDataAggregate>,
    environment: &FailureEnvironment,
) -> Result<SavedDataAggregate, SaveRefused> {
    let result = snapshot(loaded, prior);
    let (machine, _) = loaded.parts();
    attribute_snapshot_refusal(machine, environment, result)
}

fn snapshot<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    prior: Option<&SavedDataAggregate>,
) -> Result<SavedDataAggregate, SaveRefused> {
    let (machine, mods) = loaded.parts();
    machine.save_access().map_err(SaveRefused::Lifecycle)?;
    machine
        .lifecycle_world_access()
        .map_err(SaveRefused::Lifecycle)?;
    let mut aggregate = SavedDataAggregate::fresh();
    for (entry, state) in machine.tracked_entries() {
        let mod_id = &entry.mod_id;
        if !entry.sides.loads_on(machine.side())
            || !entry.sides.loads_on(machine.side().world_side())
        {
            continue;
        }
        if state == LifecycleState::Failed {
            if let Some(envelope) = prior.and_then(|aggregate| aggregate.envelope(mod_id)) {
                aggregate.replace(envelope.clone());
            }
            continue;
        }
        let instance = crate::loaded::instance_mut(mods, mod_id);
        let saved =
            crate::callback(mod_id, state, || instance.save_saved_data()).map_err(|source| {
                SaveRefused::Mod {
                    mod_id: mod_id.clone(),
                    source,
                }
            })?;
        if let Some(envelope) = saved {
            if envelope.owner() != mod_id {
                return Err(SaveRefused::ForeignEnvelope {
                    mod_id: mod_id.clone(),
                    owner: envelope.owner().clone(),
                });
            }
            aggregate.replace(envelope);
        }
    }
    Ok(aggregate)
}

/// Validates and migrates all saved data before handing entries to owners in load order.
/// Declaration and migration callbacks follow lifecycle order even when services reorder slots.
///
/// Owners not loaded on the world-executing side are excluded from declarations;
/// stored envelopes for those parked owners refuse the world. Failed owners' prior
/// envelopes remain opaque without consulting metadata, invoking their code, or
/// migrating their payloads. Active owners receive current payloads after loader-owned
/// migration. Optional owner contract failures
/// preserve prior data; required owner failures and invalid world data prevent world opening.
/// Callback failures are attributed using the supplied session `environment`, including
/// its Minecraft and Ravel versions; persistence failures have no failure surface.
/// World compatibility refusals, such as newer stored data or unmet package requirements,
/// leave owners healthy so another compatible world can be opened in the same session.
///
/// # Errors
///
/// Returns [`SaveRefused`] for invalid data or migrations, foreign ownership, or owner refusal.
pub fn open_saved_data<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    bytes: Option<&[u8]>,
    packages: &BTreeMap<ModId, Version>,
    environment: &FailureEnvironment,
) -> Result<SavedDataAggregate, SaveRefused> {
    let (machine, mods) = loaded.parts();
    let (declarations, chains) = collect_declarations(machine, environment, mods)?;
    let mut aggregate =
        SavedDataAggregate::open_preserving(bytes, &declarations, packages, |owner| {
            machine.has_failed(owner)
        })
        .map_err(SaveRefused::Data)?;
    let mut originals = migrate(machine, environment, &mut aggregate, &declarations, &chains)?;
    for declaration in &declarations {
        let mod_id = declaration.owner();
        if !machine.loads(mod_id) {
            continue;
        }
        let instance = crate::loaded::instance_mut(mods, mod_id);
        let envelope = aggregate.envelope(mod_id);
        if owner_callback(machine, environment, mod_id, || {
            instance.open_saved_data(envelope)
        })?
        .is_none()
            && let Some(index) = originals
                .iter()
                .position(|envelope| envelope.owner() == mod_id)
        {
            aggregate.replace(originals.swap_remove(index));
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
) -> Result<Vec<SavedDataEnvelope>, SaveRefused> {
    let mut originals = Vec::new();
    for declaration in declarations {
        let mod_id = declaration.owner();
        if machine.has_failed(mod_id) {
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
        let Some(chain) = chains.iter().find(|chain| chain.owner() == mod_id) else {
            isolate_refusal(
                machine,
                environment,
                mod_id,
                SaveRefused::Data(SavedDataError::MissingMigration {
                    owner: mod_id.clone(),
                    to: declaration.current(),
                }),
            )?;
            continue;
        };
        let Some(payload) = owner_callback(machine, environment, mod_id, || {
            Ok(chain.apply(&plan, envelope.payload()))
        })?
        else {
            continue;
        };
        let payload = match payload {
            Ok(payload) => payload,
            Err(error) => {
                isolate_refusal(machine, environment, mod_id, SaveRefused::Data(error))?;
                continue;
            }
        };
        let migrated = envelope.migrated(declaration.current(), payload);
        if machine
            .entry(mod_id)
            .expect("declaring owner is tracked")
            .requirement
            == crate::Requirement::Optional
        {
            originals.push(envelope.clone());
        }
        aggregate.replace(migrated);
    }
    Ok(originals)
}

fn collect_declarations<H: std::ops::DerefMut<Target: Mod>>(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    mods: &[H],
) -> Result<(Vec<SavedDataDeclaration>, Vec<MigrationChain>), SaveRefused> {
    let mut declarations = Vec::with_capacity(mods.len());
    let mut chains = Vec::with_capacity(mods.len());
    let order: Vec<_> = machine
        .world_eligible()
        .map(|(entry, _)| entry.mod_id.clone())
        .collect();
    for mod_id in &order {
        let instance = crate::loaded::instance(mods, mod_id);
        let Some(declaration) = owner_callback(machine, environment, mod_id, || {
            instance.saved_data_declaration()
        })?
        .flatten() else {
            continue;
        };
        if declaration.owner() != mod_id {
            isolate_refusal(
                machine,
                environment,
                mod_id,
                SaveRefused::ForeignDeclaration {
                    mod_id: mod_id.clone(),
                    owner: declaration.owner().clone(),
                },
            )?;
            continue;
        }
        let Some(chain) = owner_callback(machine, environment, mod_id, || {
            instance.saved_data_migrations()
        })?
        else {
            continue;
        };
        if let Some(chain) = chain {
            if let Err(error) = chain.check_covers(&declaration) {
                isolate_refusal(machine, environment, mod_id, SaveRefused::Data(error))?;
                continue;
            }
            chains.push(chain);
        } else if declaration.versions().count() > 1 {
            isolate_refusal(
                machine,
                environment,
                mod_id,
                SaveRefused::Data(SavedDataError::MissingMigration {
                    owner: mod_id.clone(),
                    to: declaration.current(),
                }),
            )?;
            continue;
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
        Err(source) => {
            crate::lifecycle::isolate(
                machine,
                environment,
                source,
                SaveRefused::Lifecycle,
                |source| SaveRefused::Mod {
                    mod_id: mod_id.clone(),
                    source,
                },
            )?;
            Ok(None)
        }
    }
}

fn isolate_refusal(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    mod_id: &ModId,
    refused: SaveRefused,
) -> Result<(), SaveRefused> {
    crate::lifecycle::isolate_loader_refusal(
        machine,
        environment,
        mod_id,
        refused,
        SaveRefused::Lifecycle,
    )
}

fn attribute_snapshot_refusal<T>(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    result: Result<T, SaveRefused>,
) -> Result<T, SaveRefused> {
    if let Err(refused) = &result
        && let Some(owner) = refused.mod_id()
        && !machine.has_failed(owner)
    {
        let source = match refused {
            SaveRefused::Mod { source, .. } => source.clone(),
            SaveRefused::ForeignEnvelope { .. } => {
                crate::owner_refusal(machine, owner, refused).map_err(SaveRefused::Lifecycle)?
            }
            SaveRefused::Lifecycle(_)
            | SaveRefused::Data(_)
            | SaveRefused::ForeignDeclaration { .. } => return result,
        };
        machine
            .fail_callback(&source, environment, None)
            .map_err(SaveRefused::Lifecycle)?;
    }
    result
}

/// Why world saved data could not be opened or snapshotted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveRefused {
    /// Lifecycle access was denied or a supplied host is not tracked.
    Lifecycle(LifecycleError),
    /// The aggregate, a version, or a migration was refused.
    Data(SavedDataError),
    /// A mod declared saved data owned by another identity.
    ForeignDeclaration {
        /// The mod that returned it.
        mod_id: ModId,
        /// The identity the declaration names as its owner.
        owner: ModId,
    },
    /// A mod's saved-data callback (declaration, migration, open, or snapshot) failed.
    Mod {
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
    pub fn mod_id(&self) -> Option<&ModId> {
        match self {
            Self::Lifecycle(error) => error.mod_id(),
            Self::Data(error) => error.owner(),
            Self::ForeignDeclaration { mod_id, .. }
            | Self::ForeignEnvelope { mod_id, .. }
            | Self::Mod { mod_id, .. } => Some(mod_id),
        }
    }
}

impl fmt::Display for SaveRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
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
            Self::Mod { mod_id, source } => {
                write!(
                    formatter,
                    "`{mod_id}` failed a saved-data callback: {source}"
                )
            }
        }
    }
}

impl std::error::Error for SaveRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::Data(error) => Some(error),
            Self::ForeignDeclaration { .. } | Self::ForeignEnvelope { .. } => None,
            Self::Mod { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for SaveRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Lifecycle(error) => error.failure_kind(),
            Self::Mod { source, .. } => source.failure_kind(),
            Self::Data(_) | Self::ForeignDeclaration { .. } | Self::ForeignEnvelope { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Lifecycle(error) => error.lifecycle_state(),
            Self::Mod { source, .. } => source.lifecycle_state(),
            Self::Data(_) | Self::ForeignDeclaration { .. } => LifecycleState::Registered,
            Self::ForeignEnvelope { .. } => LifecycleState::Running,
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Lifecycle(error) => error.quotes_untrusted_text(),
            Self::Data(error) => error.quotes_untrusted_text(),
            Self::ForeignDeclaration { .. } | Self::ForeignEnvelope { .. } => false,
            Self::Mod { source, .. } => source.quotes_untrusted_text(),
        }
    }
}

#[cfg(test)]
mod save_data_tests;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use ravel_core::{FailureEnvironment, LifecycleState, ModId, Side, StructuredFailure};
    use ravel_host_api::{HostError, Mod, ModError};
    use ravel_save::{
        MigrationChain, MigrationFailed, SavedDataAggregate, SavedDataDeclaration,
        SavedDataEnvelope, SavedDataError, SavedDataVersion,
    };
    use serde_json::{Value, json};

    use super::{SaveRefused, open_saved_data};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};
    use crate::loaded::LoadedMods;
    use crate::tests::entry;

    #[test]
    fn lifecycle_refusals_preserve_the_owner_including_halted_worlds() {
        let owner = id("stored");
        for error in [
            crate::LifecycleError::WorldAccessBlocked {
                mod_id: owner.clone(),
                state: LifecycleState::Registered,
            },
            crate::LifecycleError::WorldAccessHalted {
                mod_id: owner.clone(),
            },
            crate::LifecycleError::PartialStateRefused {
                mod_id: owner.clone(),
            },
        ] {
            assert_eq!(SaveRefused::Lifecycle(error).mod_id(), Some(&owner));
        }
        assert_eq!(
            SaveRefused::Lifecycle(crate::LifecycleError::AttributionRequired).mod_id(),
            None
        );
    }

    struct WorldProbe {
        id: ModId,
        calls: std::cell::Cell<usize>,
    }

    impl Mod for WorldProbe {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
            self.calls.set(self.calls.get() + 1);
            Ok(Some(
                SavedDataDeclaration::new(self.id.clone(), [version(1)]).expect("valid"),
            ))
        }

        fn open_saved_data(&mut self, _: Option<&SavedDataEnvelope>) -> Result<(), ModError> {
            self.calls.set(self.calls.get() + 1);
            Ok(())
        }

        fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
            self.calls.set(self.calls.get() + 1);
            Ok(None)
        }
    }

    #[test]
    fn missing_migration_is_attributed_where_migration_stops() {
        for requirement in [Requirement::Required, Requirement::Optional] {
            let owner = id("stored");
            let mut entry = entry("stored", &[Side::DedicatedServer]);
            entry.requirement = requirement;
            let mut machine =
                LifecycleMachine::new(Side::DedicatedServer, vec![entry]).expect("valid plan");
            crate::tests::advance_through(&mut machine, LifecycleState::Running);
            let declaration = SavedDataDeclaration::new(owner.clone(), [version(1), version(2)])
                .expect("valid declaration");
            let original =
                SavedDataEnvelope::current(owner.clone(), version(1), json!({"kept": 1}));
            let mut aggregate = SavedDataAggregate::fresh();
            aggregate.replace(original.clone());
            let environment = environment();
            let result = super::migrate(
                &mut machine,
                &environment,
                &mut aggregate,
                &[declaration],
                &[],
            );
            if requirement == Requirement::Required {
                assert_eq!(
                    result,
                    Err(SaveRefused::Data(SavedDataError::MissingMigration {
                        owner: owner.clone(),
                        to: version(2),
                    }))
                );
            } else {
                assert_eq!(result, Ok(Vec::new()));
            }
            assert!(machine.has_failed(&owner));
            assert_eq!(aggregate.envelope(&owner), Some(&original));
            let attribution = machine
                .failure(&owner)
                .expect("tracked")
                .expect("attributed");
            assert_eq!(attribution.environment, environment);
            assert_eq!(attribution.state, LifecycleState::Running);
        }
    }

    #[test]
    fn client_world_persistence_uses_only_started_world_side_owners() {
        let mut client = WorldProbe {
            id: id("client"),
            calls: std::cell::Cell::new(0),
        };
        let mut shared = WorldProbe {
            id: id("shared"),
            calls: std::cell::Cell::new(0),
        };
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("client", &[Side::Client]),
                entry("shared", &[Side::Client, Side::IntegratedServer]),
                entry("server", &[Side::IntegratedServer]),
            ],
        )
        .expect("valid plan");
        crate::tests::advance_through(&mut machine, LifecycleState::Running);
        let mut mods: [&mut dyn Mod; 2] = [&mut client, &mut shared];
        let mut loaded = LoadedMods::new(&mut machine, &mut mods).expect("started owners paired");
        let aggregate = open_saved_data(&mut loaded, None, &BTreeMap::new(), &environment())
            .expect("fresh world");
        super::produce_saved_data(&mut loaded, Some(&aggregate), &environment()).expect("snapshot");
        assert_eq!(client.calls.get(), 0);
        assert_eq!(shared.calls.get(), 3);
        assert_eq!(machine.state(&id("server")), Ok(LifecycleState::Resolved));
    }

    struct Failing {
        id: ModId,
        phase: &'static str,
    }

    impl Mod for Failing {
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("complete host set"),
                None,
                &BTreeMap::new(),
                &environment,
            )
            .expect_err("all failures stay within the callback boundary");
            let SaveRefused::Mod { mod_id, source } = refusal else {
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
        )
        .expect_err("migration panic is contained");
        assert!(matches!(refusal, SaveRefused::Mod { mod_id, .. } if mod_id == id("stored")));
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("complete host set"),
                Some(&bytes),
                &BTreeMap::new(),
                &environment,
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
    fn optional_saved_data_failure_after_world_mutation_refuses_the_world() {
        for phase in ["declaration", "migrations", "open", "refusal"] {
            let mut instance = Failing {
                id: id("stored"),
                phase,
            };
            let mut owner = entry("stored", &[Side::DedicatedServer]);
            owner.requirement = Requirement::Optional;
            let mut machine = machine(vec![owner]);
            crate::tests::advance_through(&mut machine, LifecycleState::Running);
            machine.record_world_mutation().expect("running world");
            let bytes = world(&[("stored", 1, json!({"opaque": true}))]);
            let refused = open_saved_data(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut instance])
                    .expect("complete instance set"),
                Some(&bytes),
                &BTreeMap::new(),
                &environment(),
            )
            .expect_err("optional failure cannot leave a mutated world running");
            assert!(matches!(refused, SaveRefused::Mod { mod_id, .. } if mod_id == id("stored")));
            assert_eq!(machine.halted_by(), Some(&id("stored")));
            assert!(machine.lifecycle_world_access().is_err());
        }
    }

    #[test]
    fn optional_migration_failures_preserve_the_original_envelope() {
        fn panics(_payload: &Value) -> Result<Value, MigrationFailed> {
            panic!("migration panic")
        }
        fn refuses(_payload: &Value) -> Result<Value, MigrationFailed> {
            Err(MigrationFailed::new("cannot migrate"))
        }
        let steps: [ravel_save::MigrationStep; 2] = [panics, refuses];
        for step in steps {
            let mut host = Storing::new("stored");
            host.chain = Some(
                MigrationChain::new(id("stored"))
                    .reaching(version(2), to_two)
                    .reaching(version(3), step),
            );
            let mut owner = entry("stored", &[Side::DedicatedServer]);
            owner.requirement = Requirement::Optional;
            let mut machine = machine(vec![owner]);
            let bytes = world(&[("stored", 1, json!({"count": 7}))]);
            let aggregate = open_saved_data(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("complete host set"),
                Some(&bytes),
                &BTreeMap::new(),
                &environment(),
            )
            .expect("optional migration failure does not refuse the world");
            assert_eq!(machine.state(&id("stored")), Ok(LifecycleState::Failed));
            assert_eq!(aggregate.to_bytes().expect("preserved"), bytes);
            assert!(host.read.is_none());
            let failure = machine
                .failure(&id("stored"))
                .expect("tracked")
                .expect("recorded");
            assert_eq!(failure.environment, environment());
        }
    }

    #[test]
    fn optional_invalid_declarations_preserve_data_and_leave_other_owners_healthy() {
        for fault in ["foreign", "missing", "absent"] {
            let mut active = Storing::new("active");
            let mut failed = match fault {
                "foreign" => Storing::new("failed").declaring_for("active"),
                "missing" => Storing::new("failed").without_a_link(),
                _ => Storing::new("failed"),
            };
            if fault == "absent" {
                failed.chain = None;
            }
            let mut optional = entry("failed", &[Side::DedicatedServer]);
            optional.requirement = Requirement::Optional;
            let mut machine = machine(vec![entry("active", &[Side::DedicatedServer]), optional]);
            let bytes = world(&[
                ("active", 3, json!({"counts": [7], "total": 0})),
                ("failed", 99, json!({"opaque": [null, "future"]})),
            ]);
            let aggregate = open_saved_data(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut active, &mut failed])
                    .expect("complete hosts"),
                Some(&bytes),
                &BTreeMap::new(),
                &environment(),
            )
            .expect("optional invalid declaration isolates its owner");
            assert_eq!(aggregate.to_bytes().expect("preserved"), bytes);
            assert!(active.read.is_some());
            assert!(failed.read.is_none());
            assert_eq!(machine.state(&id("failed")), Ok(LifecycleState::Failed));
            assert_eq!(machine.state(&id("active")), Ok(LifecycleState::Discovered));
            let failure = machine
                .failure(&id("failed"))
                .expect("tracked")
                .expect("recorded");
            assert_eq!(failure.mod_id, Some(id("failed")));
            assert_eq!(failure.environment, environment());
        }
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut active, &mut failed])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut first, &mut second])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
        )
        .expect_err("later migration refuses the world");
        assert_eq!(refusal.mod_id(), Some(&id("second")));
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
        machine
            .fail_callback_must_stop(
                &HostError::Unsupported {
                    mod_id: id("failed"),
                    state: LifecycleState::Discovered,
                },
                &environment(),
            )
            .expect("optional failure is recorded");
        let bytes = world(&[
            ("active", 1, json!({"count": 7})),
            ("failed", 99, json!({"opaque": [null, "future"]})),
        ]);
        let aggregate = open_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut active, &mut failed])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
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

    impl Mod for Storing {
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
    fn a_declared_owner_without_a_writer_refuses_and_halts_later_snapshots() {
        let mut host = Storing::new("stored");
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);
        crate::tests::advance_through(&mut machine, LifecycleState::Running);
        machine.record_world_mutation().expect("world opened");
        let mut prior = SavedDataAggregate::fresh();
        prior.replace(SavedDataEnvelope::current(
            id("stored"),
            version(3),
            json!({"durable": true}),
        ));
        let refusal = super::produce_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("paired owner"),
            Some(&prior),
            &environment(),
        )
        .expect_err("a declared owner must implement its writer");
        assert!(matches!(
            refusal,
            SaveRefused::Mod {
                mod_id, source: HostError::Unsupported { .. },
            } if mod_id == id("stored")
        ));
        assert_eq!(machine.state(&id("stored")), Ok(LifecycleState::Failed));
        assert_eq!(machine.halted_by(), Some(&id("stored")));
        let failure = machine
            .failure(&id("stored"))
            .expect("tracked")
            .expect("attributed");
        assert_eq!(failure.mod_id, Some(id("stored")));
        assert_eq!(failure.environment, environment());
        assert_eq!(
            prior
                .envelope(&id("stored"))
                .expect("prior retained")
                .payload(),
            &json!({"durable": true}),
        );
        let later = super::produce_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("failed owner remains paired"),
            Some(&prior),
            &environment(),
        )
        .expect_err("halted snapshots cannot write partial state");
        assert!(matches!(
            later,
            SaveRefused::Lifecycle(crate::LifecycleError::PartialStateRefused { mod_id })
                if mod_id == id("stored")
        ));
    }

    #[test]
    fn a_foreign_migration_chain_is_isolated_as_the_declaring_owners_failure() {
        for requirement in [Requirement::Required, Requirement::Optional] {
            let mut declaring = Storing::new("declaring");
            declaring.chain = Some(MigrationChain::new(id("other")));
            let mut other = Storing::new("other");
            let mut planned = entry("declaring", &[Side::DedicatedServer]);
            planned.requirement = requirement;
            let mut machine = machine(vec![planned, entry("other", &[Side::DedicatedServer])]);
            let result = open_saved_data(
                &mut LoadedMods::<&mut dyn Mod>::new(
                    &mut machine,
                    &mut [&mut declaring, &mut other],
                )
                .expect("paired owners"),
                None,
                &BTreeMap::new(),
                &environment(),
            );
            match requirement {
                Requirement::Required => assert_eq!(
                    result,
                    Err(SaveRefused::Data(SavedDataError::MismatchedChain {
                        declared: id("declaring"),
                        chain: id("other"),
                    })),
                ),
                Requirement::Optional => assert!(result.is_ok(), "optional owner isolated"),
            }
            assert_eq!(machine.state(&id("declaring")), Ok(LifecycleState::Failed));
            assert_eq!(machine.state(&id("other")), Ok(LifecycleState::Discovered));
            let failure = machine
                .failure(&id("declaring"))
                .expect("tracked")
                .expect("attributed");
            assert_eq!(failure.mod_id, Some(id("declaring")));
            assert_eq!(failure.environment, environment());
            assert!(machine.failure(&id("other")).expect("tracked").is_none());
            assert!(
                declaring.read.is_none(),
                "invalid owner never receives saved data"
            );
        }
    }

    #[test]
    fn invalid_saved_data_declaration_is_not_absent() {
        let mut host = Storing::new("invalid_mod");
        host.versions.clear();
        let mut machine = machine(vec![entry("invalid_mod", &[Side::DedicatedServer])]);
        let refusal = open_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            None,
            &BTreeMap::new(),
            &environment(),
        )
        .expect_err("invalid declarations cannot opt out");
        assert!(matches!(
            refusal,
            SaveRefused::Mod { mod_id, source: HostError::OperationRefused { .. } }
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("complete host set"),
                None,
                &BTreeMap::new(),
                &environment(),
            ),
            Err(SaveRefused::Mod {
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
            aggregate.replace(SavedDataEnvelope::current(
                id(owner),
                version(*at),
                payload.clone(),
            ));
        }
        aggregate.to_bytes().expect("a bounded aggregate")
    }

    #[test]
    fn incompatible_world_inputs_do_not_quarantine_an_owner_or_block_retry() {
        for requirement in [Requirement::Required, Requirement::Optional] {
            for future in [true, false] {
                let mut host = Storing::new("stored");
                let mut planned = entry("stored", &[Side::DedicatedServer]);
                planned.requirement = requirement;
                let mut machine = machine(vec![planned]);
                let environment = environment();
                let compatible = world(&[("stored", 3, json!({"compatible": true}))]);
                let refused_bytes = incompatible_world(future);
                let mut hosts: [&mut dyn Mod; 1] = [&mut host];
                let mut loaded = LoadedMods::new(&mut machine, &mut hosts).expect("paired host");
                crate::start_all(&mut loaded, &environment).expect("owner starts");
                open_saved_data(
                    &mut loaded,
                    Some(&compatible),
                    &BTreeMap::new(),
                    &environment,
                )
                .expect("first compatible world opens");
                let refused = open_saved_data(
                    &mut loaded,
                    Some(&refused_bytes),
                    &BTreeMap::new(),
                    &environment,
                )
                .expect_err("incompatible input is refused");
                assert!(
                    matches!(
                        refused,
                        SaveRefused::Data(SavedDataError::FutureVersion { .. }) if future
                    ) || matches!(
                        refused,
                        SaveRefused::Data(SavedDataError::MissingPackage { .. }) if !future
                    )
                );
                open_saved_data(
                    &mut loaded,
                    Some(&compatible),
                    &BTreeMap::new(),
                    &environment,
                )
                .expect("same running owner can retry a compatible world");
                assert_eq!(machine.state(&id("stored")), Ok(LifecycleState::Running));
                assert!(
                    machine
                        .failure(&id("stored"))
                        .expect("tracked owner")
                        .is_none()
                );
                assert_eq!(host.read, Some(json!({"compatible": true})));
            }
        }
    }

    fn incompatible_world(future: bool) -> Vec<u8> {
        let mut aggregate = SavedDataAggregate::fresh();
        let required = if future {
            Vec::new()
        } else {
            vec![ravel_save::RequiredPackage {
                id: id("missing_package"),
                version: "1.0.0".parse().expect("valid version"),
            }]
        };
        aggregate.replace(
            SavedDataEnvelope::new(
                id("stored"),
                version(if future { 4 } else { 3 }),
                required,
                None,
                json!({}),
            )
            .expect("valid envelope metadata"),
        );
        aggregate.to_bytes().expect("valid world input")
    }

    #[test]
    fn a_world_two_versions_behind_reaches_the_current_shape_before_the_mod_reads_it() {
        let bytes = world(&[("stored", 1, json!({ "count": 7 }))]);
        let mut host = Storing::new("stored");
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        let aggregate = open_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("complete host set"),
                Some(&bytes),
                &BTreeMap::new(),
                &environment(),
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("complete host set"),
                Some(&bytes),
                &BTreeMap::new(),
                &environment(),
            ),
            Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: id("stored"),
                to: version(2),
            }))
        );
        assert!(host.read.is_none(), "a mod read a world that was refused");
        assert_eq!(machine.state(&id("stored")), Ok(LifecycleState::Failed));

        let mut whole = Storing::new("stored");
        // Repairing a mod's declaration starts a new session; the broken owner stays failed.
        let mut repaired_session = LifecycleMachine::new(
            Side::DedicatedServer,
            vec![entry("stored", &[Side::DedicatedServer])],
        )
        .expect("a legal plan");
        open_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut repaired_session, &mut [&mut whole])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
        )
        .expect("the chain covers every declared version");
        assert_eq!(whole.read, Some(json!({ "counts": [7], "total": 0 })));
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut alpha, &mut zeta])
                .expect("complete host set"),
            Some(&bytes),
            &BTreeMap::new(),
            &environment(),
        )
        .expect_err("`zeta` refuses its own shape");

        assert_eq!(refusal.mod_id(), Some(&id("zeta")));
        assert!(refusal.to_string().contains("zeta"), "{refusal}");
        assert!(refusal.quotes_untrusted_text());
    }

    #[test]
    fn absence_opens_a_fresh_world_rather_than_refusing_one() {
        let mut host = Storing::new("stored");
        let mut machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        let aggregate = open_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            None,
            &BTreeMap::new(),
            &environment(),
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("complete host set"),
                None,
                &BTreeMap::new(),
                &environment(),
            ),
            Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: id("stored"),
                to: version(3),
            }))
        );
    }

    #[test]
    fn declarations_follow_lifecycle_order_after_service_slot_reordering() {
        struct Refusing {
            id: ModId,
            calls: std::sync::Arc<std::sync::Mutex<Vec<ModId>>>,
        }

        impl Mod for Refusing {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
                self.calls.lock().expect("test log").push(self.id.clone());
                Err(ModError::Unsupported)
            }
        }

        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut alpha = Refusing {
            id: id("alpha"),
            calls: std::sync::Arc::clone(&calls),
        };
        let mut zeta = Refusing {
            id: id("zeta"),
            calls: std::sync::Arc::clone(&calls),
        };
        let mut machine = machine(vec![
            entry("alpha", &[Side::DedicatedServer]),
            entry("zeta", &[Side::DedicatedServer]),
        ]);
        let refused = open_saved_data(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut zeta, &mut alpha])
                .expect("all lifecycle hosts are supplied"),
            None,
            &BTreeMap::new(),
            &environment(),
        )
        .expect_err("the first lifecycle owner refuses admission");
        assert!(matches!(refused, SaveRefused::Mod { mod_id, .. } if mod_id == id("alpha")));
        assert_eq!(*calls.lock().expect("test log"), vec![id("alpha")]);
        assert_eq!(machine.state(&id("alpha")), Ok(LifecycleState::Failed));
        assert_eq!(machine.state(&id("zeta")), Ok(LifecycleState::Discovered));
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut parked, &mut host])
                .expect("complete host set"),
            None,
            &BTreeMap::new(),
            &environment(),
        )
        .expect("the parked mod takes no part");
        assert!(parked.read.is_none());
    }
}
