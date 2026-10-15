use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure, Version};
use ravel_host_api::{HostError, HostMod};
use ravel_save::{
    MigrationChain, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope, SavedDataError,
    SavedDataPlan,
};

use crate::lifecycle::LifecycleMachine;

/// Validates and migrates all saved data before handing entries to owners in load order.
///
/// Parked mods are skipped. The returned aggregate is current; mods must still honor
/// [`SavedDataPlan::steps`] on paths that do not migrate before delivery.
/// Any refusal withholds the aggregate and must prevent the world from opening.
///
/// # Errors
///
/// Returns [`SaveRefused`] for invalid data or migrations, foreign ownership, or owner refusal.
pub fn open_saved_data(
    machine: &LifecycleMachine,
    mods: &mut [&mut dyn HostMod],
    bytes: Option<&[u8]>,
    packages: &BTreeMap<ModId, Version>,
) -> Result<SavedDataAggregate, SaveRefused> {
    let mut declarations: Vec<SavedDataDeclaration> = Vec::new();
    let mut chains: Vec<MigrationChain> = Vec::new();
    for host in mods.iter() {
        if !loads_here(machine, host.id()) {
            continue;
        }
        let Some(declaration) = host.saved_data_declaration() else {
            continue;
        };
        if declaration.owner() != host.id() {
            return Err(SaveRefused::ForeignDeclaration {
                mod_id: host.id().clone(),
                owner: declaration.owner().clone(),
            });
        }
        if let Some(chain) = host.saved_data_migrations() {
            chain.covers(&declaration).map_err(SaveRefused::Data)?;
            chains.push(chain);
        } else if declaration.versions().count() > 1 {
            return Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: declaration.owner().clone(),
                to: declaration.current(),
            }));
        }
        declarations.push(declaration);
    }

    let mut aggregate =
        SavedDataAggregate::open(bytes, &declarations, packages).map_err(SaveRefused::Data)?;
    aggregate
        .migrate(&declarations, &chains)
        .map_err(SaveRefused::Data)?;

    for mod_id in machine.order().cloned().collect::<Vec<ModId>>() {
        if !loads_here(machine, &mod_id) {
            continue;
        }
        let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) else {
            continue;
        };
        let Some(declaration) = host.saved_data_declaration() else {
            continue;
        };
        let envelope = aggregate.envelope(&mod_id);
        let plan = SavedDataPlan::open(&declaration, envelope.map(SavedDataEnvelope::data_version))
            .map_err(SaveRefused::Data)?;
        host.open_saved_data(envelope, &plan)
            .map_err(|source| SaveRefused::Owner {
                mod_id: source.mod_id().clone(),
                source,
            })?;
    }

    Ok(aggregate)
}

fn loads_here(machine: &LifecycleMachine, mod_id: &ModId) -> bool {
    machine
        .entry(mod_id)
        .is_ok_and(|entry| entry.sides.loads_on(machine.side()))
}

/// Why no world's saved data was opened.
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
}

impl SaveRefused {
    /// Returns the mod this refusal is about, when it is about one.
    #[must_use]
    pub const fn owner(&self) -> Option<&ModId> {
        match self {
            Self::Data(error) => error.owner(),
            Self::ForeignDeclaration { mod_id, .. } | Self::Owner { mod_id, .. } => Some(mod_id),
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
            Self::ForeignDeclaration { .. } => None,
            Self::Owner { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for SaveRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Registered
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Data(error) => error.quotes_untrusted_text(),
            Self::ForeignDeclaration { .. } => false,
            Self::Owner { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use ravel_core::{LifecycleState, ModId, Side, SideSet, StructuredFailure};
    use ravel_host_api::{HostError, HostMod};
    use ravel_save::{
        MigrationChain, MigrationFailed, SavedDataAggregate, SavedDataDeclaration,
        SavedDataEnvelope, SavedDataError, SavedDataPlan, SavedDataVersion,
    };
    use serde_json::{Value, json};

    use super::{SaveRefused, open_saved_data};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

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

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
            let owner = self.declares_as.clone().unwrap_or_else(|| self.id.clone());
            SavedDataDeclaration::new(owner, self.versions.iter().copied()).ok()
        }

        fn saved_data_migrations(&self) -> Option<MigrationChain> {
            self.chain.clone()
        }

        fn open_saved_data(
            &mut self,
            envelope: Option<&SavedDataEnvelope>,
            _plan: &SavedDataPlan,
        ) -> Result<(), HostError> {
            if self.refuses {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Registered,
                    detail: "this is not a shape this mod wrote".to_owned(),
                });
            }
            self.read = envelope.map(|held| held.payload().clone());
            Ok(())
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
        let machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        let aggregate = open_saved_data(&machine, &mut [&mut host], Some(&bytes), &BTreeMap::new())
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
        let machine = machine(vec![entry("thief", &[Side::DedicatedServer])]);

        let refused = open_saved_data(&machine, &mut [&mut host], Some(&bytes), &BTreeMap::new())
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
        let machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        assert_eq!(
            open_saved_data(&machine, &mut [&mut host], Some(&bytes), &BTreeMap::new()),
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
        let machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        assert_eq!(
            open_saved_data(&machine, &mut [&mut host], Some(&bytes), &BTreeMap::new()),
            Err(SaveRefused::Data(SavedDataError::MissingMigration {
                owner: id("stored"),
                to: version(2),
            }))
        );
        assert!(host.read.is_none(), "a mod read a world that was refused");

        let mut whole = Storing::new("stored");
        open_saved_data(&machine, &mut [&mut whole], Some(&bytes), &BTreeMap::new())
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
        let machine = machine(vec![
            entry("alpha", &[Side::DedicatedServer]),
            entry("zeta", &[Side::DedicatedServer]),
        ]);

        let refusal = open_saved_data(
            &machine,
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
        let machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        let aggregate = open_saved_data(&machine, &mut [&mut host], None, &BTreeMap::new())
            .expect("absence initializes");
        assert_eq!(aggregate, SavedDataAggregate::fresh());
        assert!(host.read.is_none());
    }

    #[test]
    fn a_mod_that_declares_versions_and_no_chain_at_all_is_refused() {
        let mut host = Storing::new("stored");
        host.chain = None;
        let machine = machine(vec![entry("stored", &[Side::DedicatedServer])]);

        assert_eq!(
            open_saved_data(&machine, &mut [&mut host], None, &BTreeMap::new()),
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
        let machine = machine(vec![
            entry("parked", &[Side::Client]),
            entry("stored", &[Side::DedicatedServer]),
        ]);

        open_saved_data(
            &machine,
            &mut [&mut parked, &mut host],
            None,
            &BTreeMap::new(),
        )
        .expect("the parked mod takes no part");
        assert!(parked.read.is_none());
    }
}
