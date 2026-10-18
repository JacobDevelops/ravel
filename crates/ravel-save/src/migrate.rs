use std::collections::BTreeMap;
use std::fmt;

use ravel_core::ModId;
use serde_json::Value;

use crate::error::SavedDataError;
use crate::{SavedDataDeclaration, SavedDataPlan, SavedDataVersion};

/// A non-capturing migration from the preceding data version to the target version.
pub type MigrationStep = fn(&Value) -> Result<Value, MigrationFailed>;

/// A mod-provided migration refusal, treated as untrusted text when reported.
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

/// An owner's migration steps, applied in the order selected by [`SavedDataPlan`].
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
/// chain.check_covers(&declaration)?;
///
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

    /// Declares a step into `to`, replacing any previously declared step.
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

    /// Checks that every declared version after the oldest has a migration step.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::UnknownOwner`] for a different owner or [`SavedDataError::MissingMigration`] for a missing step.
    pub fn check_covers(&self, declaration: &SavedDataDeclaration) -> Result<(), SavedDataError> {
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

    /// Runs the planned steps in order without modifying the original payload.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::UnknownOwner`], [`SavedDataError::MissingMigration`], or [`SavedDataError::MigrationFailed`].
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{MigrationChain, MigrationFailed};
    use crate::error::SavedDataError;
    use crate::tests::{id, version};
    use crate::{SavedDataDeclaration, SavedDataPlan};

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
        route.push(json!(to));
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
        json!({ "route": [at] })
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

        assert_eq!(
            broken.check_covers(&declaration),
            Err(SavedDataError::MissingMigration {
                owner: id("example"),
                to: version(3),
            })
        );
        let plan = SavedDataPlan::open(&declaration, Some(version(1))).expect("a declared version");
        assert_eq!(
            broken.apply(&plan, &stored(1)),
            Err(SavedDataError::MissingMigration {
                owner: id("example"),
                to: version(3),
            })
        );
        chain()
            .check_covers(&declaration)
            .expect("every link declared");
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
        assert_eq!(arrived, stored(1));
        assert!(refusal.to_string().contains("never written"), "{refusal}");
    }

    #[test]
    fn a_chain_belonging_to_another_mod_is_refused_rather_than_run() {
        let declaration = declaration(&[1, 2]);
        let plan = SavedDataPlan::open(&declaration, Some(version(1))).expect("a declared version");
        let other = MigrationChain::new(id("other")).reaching(version(2), to_two);

        assert_eq!(
            other.check_covers(&declaration),
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
}
