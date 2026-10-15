use std::collections::BTreeMap;
use std::fmt;

use ravel_core::ModId;
use serde_json::Value;

use crate::{
    SavedDataAggregate, SavedDataDeclaration, SavedDataError, SavedDataPlan, SavedDataVersion,
};

/// One declared step, from the version below it to the version it reaches.
///
/// A plain function pointer rather than a boxed closure: a migration is a pure
/// rewriting of one payload shape into the next, so it captures nothing, and a
/// pointer keeps [`MigrationChain`] `Clone` and keeps a mod's declaration a value
/// the host can hold beside every other declaration.
pub type MigrationStep = fn(&Value) -> Result<Value, MigrationFailed>;

/// What a migration step said when it could not read the shape it was given.
///
/// A mod's own sentence, in the mod's words, because only the mod knows what its
/// old shape was supposed to hold. It travels as text and is marked untrusted
/// where it is reported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationFailed {
    detail: String,
}

impl MigrationFailed {
    /// Records why a step could not migrate the payload it was handed.
    #[must_use]
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    /// Returns the mod's own account of the failure.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for MigrationFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for MigrationFailed {}

/// Every step one owner declares for carrying an old payload up to the current
/// version.
///
/// `SPEC.md` §14 requires persistent data to be versioned. This crate owns
/// migrations. [`SavedDataPlan`] decides *which* versions a world
/// has to cross; this is what actually crosses them. The two are separate because
/// the decision is about declared versions alone and can be made before any
/// payload is read, which is what lets a world be refused rather than opened
/// half-applied.
///
/// # Examples
///
/// ```
/// use ravel_save::{MigrationChain, MigrationFailed, SavedDataDeclaration, SavedDataPlan};
/// use serde_json::{Value, json};
///
/// fn to_two(payload: &Value) -> Result<Value, MigrationFailed> {
///     let count = payload
///         .get("count")
///         .and_then(Value::as_u64)
///         .ok_or_else(|| MigrationFailed::new("version 1 held no `count`"))?;
///     Ok(json!({ "counts": [count] }))
/// }
///
/// fn to_three(payload: &Value) -> Result<Value, MigrationFailed> {
///     let counts = payload
///         .get("counts")
///         .cloned()
///         .ok_or_else(|| MigrationFailed::new("version 2 held no `counts`"))?;
///     Ok(json!({ "counts": counts, "total": 0 }))
/// }
///
/// let owner = "example".parse()?;
/// let versions = [1, 2, 3].map(|raw| {
///     std::num::NonZero::new(raw).expect("a non-zero version")
/// });
/// let declaration = SavedDataDeclaration::new(owner, versions)?;
/// let chain = MigrationChain::new(declaration.owner().clone())
///     .reaching(versions[1], to_two)
///     .reaching(versions[2], to_three);
///
/// // Every version the declaration crosses has a step.
/// chain.covers(&declaration)?;
///
/// // A world two versions behind arrives at the current shape.
/// let plan = SavedDataPlan::open(&declaration, Some(versions[0]))?;
/// assert_eq!(plan.steps(), [versions[1], versions[2]]);
/// assert_eq!(
///     chain.apply(&plan, &json!({ "count": 7 }))?,
///     json!({ "counts": [7], "total": 0 })
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct MigrationChain {
    owner: ModId,
    steps: BTreeMap<SavedDataVersion, MigrationStep>,
}

impl MigrationChain {
    /// Starts an empty chain for `owner`.
    #[must_use]
    pub const fn new(owner: ModId) -> Self {
        Self {
            owner,
            steps: BTreeMap::new(),
        }
    }

    /// Declares the step that reaches `to` from the version below it.
    ///
    /// A second step declared for the same version replaces the first, because a
    /// version has one shape and therefore one way in.
    #[must_use]
    pub fn reaching(mut self, to: SavedDataVersion, step: MigrationStep) -> Self {
        self.steps.insert(to, step);
        self
    }

    /// Returns the mod whose data this chain migrates.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Returns every version this chain can reach, in ascending order.
    pub fn reaches(&self) -> impl Iterator<Item = SavedDataVersion> + '_ {
        self.steps.keys().copied()
    }

    /// Refuses a chain that cannot carry a world from the oldest version
    /// `declaration` admits to the version this build writes.
    ///
    /// This is checkable before any world is opened, and that is the point: a
    /// build shipped with a link missing is a build that would refuse somebody's
    /// world, and it is better found by its own tests than by their save.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::UnknownOwner`] when the chain and the
    /// declaration belong to different mods, and
    /// [`SavedDataError::MissingMigration`] for the lowest declared version above
    /// the oldest that has no step.
    pub fn covers(&self, declaration: &SavedDataDeclaration) -> Result<(), SavedDataError> {
        if declaration.owner() != &self.owner {
            return Err(SavedDataError::UnknownOwner {
                owner: declaration.owner().clone(),
            });
        }
        for version in declaration.versions().skip(1) {
            if !self.steps.contains_key(&version) {
                return Err(SavedDataError::MissingMigration {
                    owner: self.owner.clone(),
                    to: version,
                });
            }
        }
        Ok(())
    }

    /// Runs every step `plan` names, in order, and returns the current payload.
    ///
    /// `payload` is borrowed and a new value is returned, so a chain that refuses
    /// partway leaves the caller holding exactly what it had. There is no
    /// half-migrated payload to discard, because none was ever produced: this is
    /// the payload-level half of the rule that a refusal withholds the world
    /// rather than opening it part-applied.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::UnknownOwner`] when the plan is another mod's,
    /// [`SavedDataError::MissingMigration`] for a step the plan crosses and this
    /// chain does not declare, and [`SavedDataError::MigrationFailed`] carrying
    /// the mod's own account when a step could not read what it was given.
    pub fn apply(&self, plan: &SavedDataPlan, payload: &Value) -> Result<Value, SavedDataError> {
        if plan.owner() != &self.owner {
            return Err(SavedDataError::UnknownOwner {
                owner: plan.owner().clone(),
            });
        }
        let mut migrated = payload.clone();
        for to in plan.steps().iter().copied() {
            let step = self
                .steps
                .get(&to)
                .ok_or_else(|| SavedDataError::MissingMigration {
                    owner: self.owner.clone(),
                    to,
                })?;
            migrated = step(&migrated).map_err(|failed| SavedDataError::MigrationFailed {
                owner: self.owner.clone(),
                to,
                detail: failed.detail,
            })?;
        }
        Ok(migrated)
    }
}

impl SavedDataAggregate {
    /// Brings every owner's payload up to the version this build writes.
    ///
    /// Every owner or none. The migrated envelopes are built beside the stored
    /// ones and only replace them once every chain has succeeded, so an aggregate
    /// whose third owner refuses is the aggregate that arrived rather than one
    /// holding two migrated owners and one stale. A refusal withholds the world
    /// and stages nothing.
    ///
    /// An owner already at the current version needs no chain. An owner that has
    /// versions to cross and no chain is refused, because a declared version with
    /// no way into it is a build that cannot read the world it admitted.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::UnknownOwner`] for stored data whose owner is
    /// not installed, and otherwise whatever [`MigrationChain::apply`] refuses.
    /// Nothing is replaced on any error.
    pub fn migrate(
        &mut self,
        declarations: &[SavedDataDeclaration],
        chains: &[MigrationChain],
    ) -> Result<Vec<SavedDataPlan>, SavedDataError> {
        let mut migrated = Vec::with_capacity(self.owners.len());
        let mut plans = Vec::with_capacity(self.owners.len());

        for envelope in &self.owners {
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.owner() == envelope.owner())
                .ok_or_else(|| SavedDataError::UnknownOwner {
                    owner: envelope.owner().clone(),
                })?;
            let plan = SavedDataPlan::resolve(declaration, envelope.data_version())?;
            if plan.is_current() {
                migrated.push(envelope.clone());
                plans.push(plan);
                continue;
            }

            let chain = chains
                .iter()
                .find(|chain| chain.owner() == envelope.owner())
                .ok_or_else(|| SavedDataError::MissingMigration {
                    owner: envelope.owner().clone(),
                    to: declaration.current(),
                })?;
            let payload = chain.apply(&plan, envelope.payload())?;
            migrated.push(envelope.migrated(declaration.current(), payload));
            plans.push(plan);
        }

        self.owners = migrated;
        Ok(plans)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use ravel_core::ModId;
    use serde_json::{Value, json};

    use super::{MigrationChain, MigrationFailed};
    use crate::{
        SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataPlan,
        SavedDataVersion,
    };

    fn id(value: &str) -> ModId {
        value.parse().expect("a legal mod id")
    }

    fn version(raw: u32) -> SavedDataVersion {
        NonZero::new(raw).expect("a non-zero version")
    }

    /// Each step appends its own version, so the payload records the route it
    /// took rather than only where it ended.
    fn noting(to: u32) -> Value {
        json!(to)
    }

    fn to_two(payload: &Value) -> Result<Value, MigrationFailed> {
        append(payload, 2)
    }

    fn to_three(payload: &Value) -> Result<Value, MigrationFailed> {
        append(payload, 3)
    }

    fn to_four(payload: &Value) -> Result<Value, MigrationFailed> {
        append(payload, 4)
    }

    fn append(payload: &Value, to: u32) -> Result<Value, MigrationFailed> {
        let mut route = payload
            .get("route")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| MigrationFailed::new(format!("no `route` to carry into {to}")))?;
        route.push(noting(to));
        Ok(json!({ "route": route }))
    }

    fn refusing(_payload: &Value) -> Result<Value, MigrationFailed> {
        Err(MigrationFailed::new("this shape was never written"))
    }

    fn declaration(versions: &[u32]) -> SavedDataDeclaration {
        SavedDataDeclaration::new(id("example"), versions.iter().copied().map(version))
            .expect("a non-empty declaration")
    }

    fn chain() -> MigrationChain {
        MigrationChain::new(id("example"))
            .reaching(version(2), to_two)
            .reaching(version(3), to_three)
            .reaching(version(4), to_four)
    }

    fn stored(at: u32) -> Value {
        json!({ "route": [noting(at)] })
    }

    #[test]
    fn a_world_three_versions_behind_crosses_every_step_in_order() {
        let declaration = declaration(&[1, 2, 3, 4]);
        let plan = SavedDataPlan::open(&declaration, Some(version(1))).expect("a declared version");
        assert_eq!(plan.steps(), [version(2), version(3), version(4)]);

        assert_eq!(
            chain()
                .apply(&plan, &stored(1))
                .expect("every step declared"),
            json!({ "route": [1, 2, 3, 4] }),
            "the payload does not record the route it took"
        );
    }

    #[test]
    fn a_world_already_current_is_handed_back_untouched() {
        let declaration = declaration(&[1, 2, 3, 4]);
        let plan =
            SavedDataPlan::open(&declaration, Some(version(4))).expect("the current version");
        assert!(plan.is_current());
        assert_eq!(
            chain().apply(&plan, &stored(4)).expect("no step to run"),
            stored(4)
        );
    }

    #[test]
    fn a_chain_missing_a_link_is_refused_before_a_world_is_opened_and_when_one_is() {
        let declaration = declaration(&[1, 2, 3, 4]);
        let broken = MigrationChain::new(id("example"))
            .reaching(version(2), to_two)
            .reaching(version(4), to_four);

        // Found without a world: the version with no way into it is named.
        assert_eq!(
            broken.covers(&declaration),
            Err(SavedDataError::MissingMigration {
                owner: id("example"),
                to: version(3),
            })
        );
        // And with one, at the step the payload actually reached.
        let plan = SavedDataPlan::open(&declaration, Some(version(1))).expect("a declared version");
        assert_eq!(
            broken.apply(&plan, &stored(1)),
            Err(SavedDataError::MissingMigration {
                owner: id("example"),
                to: version(3),
            })
        );
        // The whole chain does cover it, so the refusals above are about the gap.
        chain().covers(&declaration).expect("every link declared");
    }

    #[test]
    fn a_step_that_refuses_leaves_the_payload_exactly_as_it_arrived() {
        let declaration = declaration(&[1, 2, 3]);
        let plan = SavedDataPlan::open(&declaration, Some(version(1))).expect("a declared version");
        let partial = MigrationChain::new(id("example"))
            .reaching(version(2), to_two)
            .reaching(version(3), refusing);

        let arrived = stored(1);
        let refusal = partial
            .apply(&plan, &arrived)
            .expect_err("the second step refuses");
        assert_eq!(
            refusal,
            SavedDataError::MigrationFailed {
                owner: id("example"),
                to: version(3),
                detail: "this shape was never written".to_owned(),
            }
        );
        // The first step did run and produced a value, and none of it survived:
        // the caller holds what it had, so there is nothing half-migrated to
        // mistake for state.
        assert_eq!(arrived, stored(1));
        assert!(refusal.to_string().contains("never written"), "{refusal}");
    }

    #[test]
    fn a_chain_belonging_to_another_mod_is_refused_rather_than_run() {
        let declaration = declaration(&[1, 2]);
        let plan = SavedDataPlan::open(&declaration, Some(version(1))).expect("a declared version");
        let other = MigrationChain::new(id("other")).reaching(version(2), to_two);

        assert_eq!(
            other.covers(&declaration),
            Err(SavedDataError::UnknownOwner {
                owner: id("example"),
            })
        );
        assert_eq!(
            other.apply(&plan, &stored(1)),
            Err(SavedDataError::UnknownOwner {
                owner: id("example"),
            })
        );
    }

    fn aggregate(entries: &[(&str, u32)]) -> SavedDataAggregate {
        let mut aggregate = SavedDataAggregate::fresh();
        for (owner, at) in entries {
            aggregate.replace(SavedDataEnvelope::new(
                id(owner),
                version(*at),
                Vec::new(),
                None,
                stored(*at),
            ));
        }
        aggregate
    }

    #[test]
    fn every_owner_reaches_the_current_version_or_none_of_them_moves() {
        let declarations = [
            SavedDataDeclaration::new(id("first"), [version(1), version(2)]).expect("versions"),
            SavedDataDeclaration::new(id("second"), [version(1), version(2)]).expect("versions"),
        ];
        let chains = [
            MigrationChain::new(id("first")).reaching(version(2), to_two),
            MigrationChain::new(id("second")).reaching(version(2), to_two),
        ];

        let mut both = aggregate(&[("first", 1), ("second", 1)]);
        both.migrate(&declarations, &chains)
            .expect("both chains succeed");
        for owner in ["first", "second"] {
            let envelope = both.envelope(&id(owner)).expect("still present");
            assert_eq!(envelope.data_version(), version(2));
            assert_eq!(envelope.payload(), &json!({ "route": [1, 2] }));
        }

        // Now the second owner refuses. The first one's chain still succeeds, so
        // an implementation that replaced as it went would leave it migrated.
        let refusing_chains = [
            MigrationChain::new(id("first")).reaching(version(2), to_two),
            MigrationChain::new(id("second")).reaching(version(2), refusing),
        ];
        let arrived = aggregate(&[("first", 1), ("second", 1)]);
        let mut attempted = arrived.clone();
        let refusal = attempted
            .migrate(&declarations, &refusing_chains)
            .expect_err("the second owner refuses");
        assert_eq!(refusal.owner(), Some(&id("second")));
        assert_eq!(
            attempted, arrived,
            "one owner was migrated while the world was refused"
        );
    }

    #[test]
    fn an_owner_with_versions_to_cross_and_no_chain_at_all_is_refused() {
        let declarations =
            [SavedDataDeclaration::new(id("first"), [version(1), version(2)]).expect("versions")];
        let arrived = aggregate(&[("first", 1)]);
        let mut attempted = arrived.clone();

        assert_eq!(
            attempted.migrate(&declarations, &[]),
            Err(SavedDataError::MissingMigration {
                owner: id("first"),
                to: version(2),
            })
        );
        assert_eq!(attempted, arrived);
    }
}
