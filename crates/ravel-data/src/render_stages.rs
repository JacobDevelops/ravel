use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ModId, Side, precedence};

/// A published frame stage; declarations currently provide ordering, not drawing.
///
/// Published names use `snake_case` to match the mod-facing stage declaration vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RenderStage {
    /// After the world's solid geometry is drawn.
    AfterTerrain,
    /// After the things moving in the world are drawn.
    AfterEntities,
    /// After the interface over the world is drawn.
    AfterInterface,
}

impl RenderStage {
    /// Every published stage, in the order they are reached in a frame.
    pub const ALL: [Self; 3] = [
        Self::AfterTerrain,
        Self::AfterEntities,
        Self::AfterInterface,
    ];

    /// Returns the stage's published name, which is what a mod writes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AfterTerrain => "after_terrain",
            Self::AfterEntities => "after_entities",
            Self::AfterInterface => "after_interface",
        }
    }

    /// Returns the stage `name` publishes, if any stage does.
    #[must_use]
    pub fn published(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| stage.as_str() == name)
    }

    /// Returns whether `side` reaches this stage.
    ///
    /// Only the client reaches frame stages.
    #[must_use]
    pub const fn happens_on(self, side: Side) -> bool {
        let _ = self;
        matches!(side, Side::Client)
    }
}

impl fmt::Display for RenderStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Per-stage contributors and explicit ordering constraints between mods.
///
/// # Examples
///
/// ```
/// use ravel_core::ModId;
/// use ravel_data::{RenderStage, StageDeclaration};
///
/// let glow: ModId = "glow_mod".parse()?;
/// let outline: ModId = "outline_mod".parse()?;
///
/// let mut stages = StageDeclaration::default();
/// stages.contribute(&glow, RenderStage::AfterTerrain);
/// stages.contribute(&outline, RenderStage::AfterTerrain);
/// // The outline is drawn over the glow, so it goes after it.
/// stages.after(&outline, &glow);
///
/// let order = stages.resolve().expect("a stage without a cycle resolves");
/// assert_eq!(order.at(RenderStage::AfterTerrain), [glow, outline]);
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StageDeclaration {
    contributions: BTreeMap<RenderStage, Vec<ModId>>,
    after: BTreeMap<ModId, BTreeSet<ModId>>,
}

impl StageDeclaration {
    /// Declares that `owner` contributes to the published `stage`.
    ///
    /// ```compile_fail
    /// use ravel_core::ModId;
    /// use ravel_data::StageDeclaration;
    /// let owner: ModId = "sample".parse().expect("a valid owner");
    /// StageDeclaration::default().contribute(&owner, "after_terrain");
    /// ```
    pub fn contribute(&mut self, owner: &ModId, stage: RenderStage) {
        let contributors = self.contributions.entry(stage).or_default();
        if !contributors.contains(owner) {
            contributors.push(owner.clone());
        }
    }

    /// Declares that `later` is entered after `earlier`, at every stage they
    /// share.
    pub fn after(&mut self, later: &ModId, earlier: &ModId) {
        self.after
            .entry(earlier.clone())
            .or_default()
            .insert(later.clone());
    }

    /// Returns the mods contributing to `stage`, in declaration order.
    #[must_use]
    pub fn contributors(&self, stage: RenderStage) -> &[ModId] {
        self.contributions.get(&stage).map_or(&[], Vec::as_slice)
    }

    /// Decides the order at every stage.
    ///
    /// # Errors
    ///
    /// Returns unknown-contributor or circular-ordering errors.
    pub fn resolve(&self) -> Result<StageOrder, StageRefused> {
        let contributing: BTreeSet<&ModId> = self.contributions.values().flatten().collect();
        let strangers: Vec<ModId> = self
            .after
            .iter()
            .flat_map(|(earlier, later)| std::iter::once(earlier).chain(later))
            .filter(|id| !contributing.contains(id))
            .cloned()
            .collect::<BTreeSet<ModId>>()
            .into_iter()
            .collect();
        if !strangers.is_empty() {
            return Err(StageRefused::UnknownContributors {
                participants: strangers,
            });
        }

        let mut ordered = BTreeMap::new();
        for (stage, contributors) in &self.contributions {
            let order = precedence(contributors.iter().cloned(), &self.after).map_err(|cycle| {
                StageRefused::Circular {
                    stage: *stage,
                    // Report participants in contribution order.
                    participants: contributors
                        .iter()
                        .filter(|id| cycle.participants().contains(id))
                        .cloned()
                        .collect(),
                }
            })?;
            ordered.insert(*stage, order);
        }
        Ok(StageOrder { ordered })
    }
}

/// The order the mods at each stage are entered in.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StageOrder {
    ordered: BTreeMap<RenderStage, Vec<ModId>>,
}

impl StageOrder {
    /// Returns the mods entered at `stage`, in order.
    ///
    /// An unused stage returns an empty slice.
    #[must_use]
    pub fn at(&self, stage: RenderStage) -> &[ModId] {
        self.ordered.get(&stage).map_or(&[], Vec::as_slice)
    }

    /// Returns every stage anything contributes to, in frame order.
    pub fn stages(&self) -> impl Iterator<Item = RenderStage> + '_ {
        self.ordered.keys().copied()
    }
}

/// Why a set of stage contributions has no order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StageRefused {
    /// An ordering names a mod that contributes to no stage.
    UnknownContributors {
        /// The mods named by an edge and contributing nothing, in name order.
        participants: Vec<ModId>,
    },
    /// Contributors at one stage each want to follow the other.
    Circular {
        /// The stage whose order was refused.
        stage: RenderStage,
        /// The mods in the cycle, in declaration order.
        participants: Vec<ModId>,
    },
}

impl StageRefused {
    /// Returns the mods named by an ordering that contribute to no stage.
    #[must_use]
    pub fn unknown_contributors(&self) -> &[ModId] {
        match self {
            Self::UnknownContributors { participants } => participants,
            Self::Circular { .. } => &[],
        }
    }

    /// Returns the mods in the cycle, in the order they contributed.
    #[must_use]
    pub fn cycle(&self) -> &[ModId] {
        match self {
            Self::Circular { participants, .. } => participants,
            Self::UnknownContributors { .. } => &[],
        }
    }
}

impl fmt::Display for StageRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = |mods: &[ModId]| {
            mods.iter()
                .map(ModId::as_str)
                .collect::<Vec<&str>>()
                .join("`, `")
        };
        match self {
            Self::UnknownContributors { participants } => write!(
                formatter,
                "`{}` is ordered against and contributes to no stage, so the order is against \
                 nothing",
                names(participants)
            ),
            Self::Circular {
                stage,
                participants,
            } => write!(
                formatter,
                "at `{stage}`, `{}` each want to follow another, and a cycle has no order",
                names(participants)
            ),
        }
    }
}

impl std::error::Error for StageRefused {}

#[cfg(test)]
mod tests {
    use ravel_core::{ModId, Side};

    use super::{RenderStage, StageDeclaration, StageOrder, StageRefused};

    fn id(name: &str) -> ModId {
        name.parse().expect("a well formed owner id")
    }

    fn entered_before(
        order: &StageOrder,
        stage: RenderStage,
        earlier: &ModId,
        later: &ModId,
    ) -> bool {
        let at = order.at(stage);
        match (
            at.iter().position(|id| id == earlier),
            at.iter().position(|id| id == later),
        ) {
            (Some(first), Some(second)) => first < second,
            _ => false,
        }
    }

    #[test]
    fn every_published_stage_names_itself_once_and_is_found_by_that_name() {
        for stage in RenderStage::ALL {
            assert_eq!(RenderStage::published(stage.as_str()), Some(stage));
            assert_eq!(stage.to_string(), stage.as_str());
            assert!(stage.happens_on(Side::Client));
            assert!(!stage.happens_on(Side::IntegratedServer));
            assert!(!stage.happens_on(Side::DedicatedServer));
        }
        assert_eq!(RenderStage::published("a_stage_nobody_publishes"), None);
    }

    #[test]
    fn contributors_to_one_stage_are_entered_in_declared_order() {
        let early = id("sample");
        let late = id("probe");

        let mut stages = StageDeclaration::default();
        stages.contribute(&early, RenderStage::AfterTerrain);
        stages.contribute(&late, RenderStage::AfterTerrain);
        stages.after(&late, &early);

        let order = stages.resolve().expect("a stage without a cycle resolves");

        assert_eq!(
            order.at(RenderStage::AfterTerrain),
            [early.clone(), late.clone()]
        );
        assert!(entered_before(
            &order,
            RenderStage::AfterTerrain,
            &early,
            &late
        ));
        assert_eq!(
            order.stages().collect::<Vec<_>>(),
            [RenderStage::AfterTerrain]
        );
    }

    #[test]
    fn the_declared_edge_decides_the_order_rather_than_the_order_of_declaration() {
        let first = id("sample");
        let second = id("probe");

        let mut stages = StageDeclaration::default();
        stages.contribute(&first, RenderStage::AfterTerrain);
        stages.contribute(&second, RenderStage::AfterTerrain);
        // The mod declared first is the one asked to follow.
        stages.after(&first, &second);

        let order = stages.resolve().expect("a stage without a cycle resolves");
        assert_eq!(
            order.at(RenderStage::AfterTerrain),
            [second.clone(), first.clone()]
        );
        assert!(entered_before(
            &order,
            RenderStage::AfterTerrain,
            &second,
            &first
        ));
    }

    #[test]
    fn an_order_at_one_stage_says_nothing_about_another() {
        let owner = id("sample");
        let mut stages = StageDeclaration::default();
        stages.contribute(&owner, RenderStage::AfterTerrain);

        let order = stages.resolve().expect("one contributor resolves");
        assert_eq!(order.at(RenderStage::AfterTerrain), [owner]);
        assert_eq!(order.at(RenderStage::AfterEntities), []);
    }

    #[test]
    fn a_cycle_between_contributors_is_refused() {
        let first = id("sample");
        let second = id("probe");

        let mut stages = StageDeclaration::default();
        stages.contribute(&first, RenderStage::AfterTerrain);
        stages.contribute(&second, RenderStage::AfterTerrain);
        stages.after(&first, &second);
        stages.after(&second, &first);

        let refusal = stages
            .resolve()
            .expect_err("a cycle has no order and is not given one");

        assert_eq!(refusal.cycle(), [first.clone(), second.clone()]);
        assert!(refusal.to_string().contains("after_terrain"));
        assert!(matches!(refusal, StageRefused::Circular { .. }));
    }

    #[test]
    fn text_is_admitted_before_a_typed_contribution() {
        assert_eq!(RenderStage::published("a_stage_nobody_publishes"), None);
        let owner = id("sample");
        let mut stages = StageDeclaration::default();
        let stage = RenderStage::published("after_entities").expect("a published stage");
        stages.contribute(&owner, stage);
        assert_eq!(stage, RenderStage::AfterEntities);
        assert_eq!(stages.contributors(RenderStage::AfterEntities), [owner]);
    }

    #[test]
    fn an_ordering_against_a_mod_that_contributes_nothing_is_refused() {
        let owner = id("sample");
        let absent = id("probe");

        let mut stages = StageDeclaration::default();
        stages.contribute(&owner, RenderStage::AfterTerrain);
        stages.after(&owner, &absent);

        let refusal = stages
            .resolve()
            .expect_err("an order against nothing is refused rather than dropped");

        assert_eq!(refusal.unknown_contributors(), [absent]);
        assert_eq!(refusal.cycle(), []);
    }

    #[test]
    fn contributing_twice_to_one_stage_contributes_once() {
        let owner = id("sample");
        let mut stages = StageDeclaration::default();
        stages.contribute(&owner, RenderStage::AfterTerrain);
        stages.contribute(&owner, RenderStage::AfterTerrain);

        assert_eq!(stages.contributors(RenderStage::AfterTerrain), [owner]);
    }
}
