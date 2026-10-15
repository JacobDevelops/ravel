use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ModId, Side, precedence};

/// A named place in the frame that Ravel publishes for mods to contribute to.
///
/// The set is closed, for the same reason [`crate::ServerDataKind`] is not open
/// and [`ravel_core::ContentKind`] is not: a stage nothing publishes is a name
/// no drawing ever reaches, and accepting it would turn a mod author's typo into
/// a contribution that silently never happens.
///
/// **Nothing draws at any of these yet, and this module ships no way to draw.**
/// The screens Ravel owns are declared in data and drawn in Java; see
/// `docs/architecture/overview.md#mechanisms`. Nothing here extends that to a
/// mod's own drawing: what is shipped is the stage name, the order
/// among contributors, and the two refusals, all of which are decidable without
/// a frame. A mod that declares a contribution today is declaring where its
/// drawing would go, and `crates/ravel-sdk/src/render_stages.rs` says so to the
/// author in the same words.
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
    /// Drawing is the client's alone. A server has no frame, so a stage is not
    /// something a server skips — it is something a server never arrives at.
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

/// Which mods contribute to which stages, and in what order among each other.
///
/// Ordering between mods is what [`crate::ContentDeclaration`]'s siblings
/// already solve for named calls between mods; drawing had none of it, so two
/// mods adding to one stage were entered in whatever order they happened to
/// load. This is the same answer for the same question: a mod says which stages
/// it contributes to and which mods it wants to follow, and Ravel decides the
/// order once for the set.
///
/// # Examples
///
/// ```
/// use ravel_core::ModId;
/// use ravel_data::StageDeclaration;
///
/// let glow: ModId = "glow_mod".parse()?;
/// let outline: ModId = "outline_mod".parse()?;
///
/// let mut stages = StageDeclaration::default();
/// stages.contribute(&glow, "after_terrain");
/// stages.contribute(&outline, "after_terrain");
/// // The outline is drawn over the glow, so it goes after it.
/// stages.after(&outline, &glow);
///
/// let order = stages.resolve().expect("a stage without a cycle resolves");
/// assert_eq!(order.at("after_terrain"), [glow, outline]);
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StageDeclaration {
    contributions: BTreeMap<RenderStage, Vec<ModId>>,
    unpublished: BTreeMap<String, Vec<ModId>>,
    after: BTreeMap<ModId, BTreeSet<ModId>>,
}

impl StageDeclaration {
    /// Declares that `owner` contributes to the stage named `stage`.
    ///
    /// A name nothing publishes is kept rather than dropped, and refused by
    /// [`StageDeclaration::resolve`] naming both the name and the mod that wrote
    /// it. A mod that wants the answer at the moment it declares — because it
    /// built the name rather than writing it — calls
    /// [`StageDeclaration::contribute_checked`] instead. Neither route lets an
    /// unpublished name through; they differ only in when they say so.
    pub fn contribute(&mut self, owner: &ModId, stage: &str) {
        let contributors = match RenderStage::published(stage) {
            Some(published) => self.contributions.entry(published).or_default(),
            None => self.unpublished.entry(stage.to_owned()).or_default(),
        };
        if !contributors.contains(owner) {
            contributors.push(owner.clone());
        }
    }

    /// Declares a contribution, refusing an unpublished stage immediately.
    ///
    /// # Errors
    ///
    /// Returns [`StageRefused::UnknownStage`] when no stage publishes `stage`.
    /// Nothing is recorded when it does, so a refused contribution leaves the
    /// declaration exactly as it was.
    pub fn contribute_checked(
        &mut self,
        owner: &ModId,
        stage: &str,
    ) -> Result<RenderStage, StageRefused> {
        let published =
            RenderStage::published(stage).ok_or_else(|| StageRefused::UnknownStage {
                name: stage.to_owned(),
                claimants: vec![owner.clone()],
            })?;
        self.contribute(owner, published.as_str());
        Ok(published)
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
    /// Returns [`StageRefused::UnknownStage`] for a contribution to a stage
    /// nothing publishes, [`StageRefused::UnknownContributors`] for an ordering
    /// against a mod that contributes to no stage, and
    /// [`StageRefused::Circular`] for contributors at one stage that each want
    /// to follow the other. All three are refusals about the set, which is why
    /// they are decided here and not when a frame is drawn.
    pub fn resolve(&self) -> Result<StageOrder, StageRefused> {
        if let Some((name, claimants)) = self.unpublished.first_key_value() {
            return Err(StageRefused::UnknownStage {
                name: name.clone(),
                claimants: claimants.clone(),
            });
        }

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
                    // In the order the mods contributed, because a mod author
                    // reading the refusal is looking for the edge they wrote and
                    // wrote it in this sequence.
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
    /// Returns the mods entered at the stage named `stage`, in order.
    ///
    /// A name nothing publishes, and a published stage nothing contributes to,
    /// both hold nobody. Neither can reach here as a mistake: an unpublished
    /// name is refused before an order exists.
    #[must_use]
    pub fn at(&self, stage: &str) -> &[ModId] {
        RenderStage::published(stage)
            .and_then(|published| self.ordered.get(&published))
            .map_or(&[], Vec::as_slice)
    }

    /// Returns every stage anything contributes to, in frame order.
    pub fn stages(&self) -> impl Iterator<Item = RenderStage> + '_ {
        self.ordered.keys().copied()
    }
}

/// Why a set of stage contributions has no order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StageRefused {
    /// A mod contributes to a name no stage publishes.
    UnknownStage {
        /// The name that was written.
        name: String,
        /// The mods that wrote it, in declaration order.
        claimants: Vec<ModId>,
    },
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
    /// Returns the unpublished name, or `""` when the refusal is not about one.
    #[must_use]
    pub fn unknown_stage(&self) -> &str {
        match self {
            Self::UnknownStage { name, .. } => name,
            Self::UnknownContributors { .. } | Self::Circular { .. } => "",
        }
    }

    /// Returns the mods named by an ordering that contribute to no stage.
    #[must_use]
    pub fn unknown_contributors(&self) -> &[ModId] {
        match self {
            Self::UnknownContributors { participants } => participants,
            Self::UnknownStage { .. } | Self::Circular { .. } => &[],
        }
    }

    /// Returns the mods in the cycle, in the order they contributed.
    #[must_use]
    pub fn cycle(&self) -> &[ModId] {
        match self {
            Self::Circular { participants, .. } => participants,
            Self::UnknownStage { .. } | Self::UnknownContributors { .. } => &[],
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
            Self::UnknownStage { name, claimants } => write!(
                formatter,
                "`{}` contributes to `{name}`, which is not a stage Ravel publishes, so nothing \
                 would ever draw there",
                names(claimants)
            ),
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

    /// Reads an order back positionally rather than trusting the sequence that
    /// produced it.
    fn entered_before(order: &StageOrder, stage: &str, earlier: &ModId, later: &ModId) -> bool {
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
        stages.contribute(&early, "after_terrain");
        stages.contribute(&late, "after_terrain");
        stages.after(&late, &early);

        let order = stages.resolve().expect("a stage without a cycle resolves");

        assert_eq!(order.at("after_terrain"), [early.clone(), late.clone()]);
        assert!(entered_before(&order, "after_terrain", &early, &late));
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
        stages.contribute(&first, "after_terrain");
        stages.contribute(&second, "after_terrain");
        // The mod declared first is the one asked to follow.
        stages.after(&first, &second);

        let order = stages.resolve().expect("a stage without a cycle resolves");
        assert_eq!(order.at("after_terrain"), [second.clone(), first.clone()]);
        assert!(entered_before(&order, "after_terrain", &second, &first));
    }

    #[test]
    fn an_order_at_one_stage_says_nothing_about_another() {
        let owner = id("sample");
        let mut stages = StageDeclaration::default();
        stages.contribute(&owner, "after_terrain");

        let order = stages.resolve().expect("one contributor resolves");
        assert_eq!(order.at("after_terrain"), [owner]);
        assert!(order.at("after_entities").is_empty());
        assert!(order.at("a_stage_nobody_publishes").is_empty());
    }

    #[test]
    fn a_cycle_between_contributors_is_refused() {
        let first = id("sample");
        let second = id("probe");

        let mut stages = StageDeclaration::default();
        stages.contribute(&first, "after_terrain");
        stages.contribute(&second, "after_terrain");
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
    fn a_contribution_to_an_unpublished_stage_is_refused() {
        let owner = id("sample");
        let mut stages = StageDeclaration::default();

        let refusal = stages
            .contribute_checked(&owner, "a_stage_nobody_publishes")
            .expect_err("a stage nothing publishes cannot be contributed to");

        assert_eq!(refusal.unknown_stage(), "a_stage_nobody_publishes");
        // Nothing was recorded, so the declaration still resolves.
        assert!(
            stages
                .resolve()
                .expect("nothing was declared")
                .at("after_terrain")
                .is_empty()
        );
    }

    #[test]
    fn a_contribution_to_an_unpublished_stage_is_refused_at_the_barrier_too() {
        let owner = id("sample");
        let mut stages = StageDeclaration::default();
        stages.contribute(&owner, "a_stage_nobody_publishes");

        let refusal = stages
            .resolve()
            .expect_err("an unpublished name is not quietly dropped");

        assert_eq!(refusal.unknown_stage(), "a_stage_nobody_publishes");
        assert!(refusal.to_string().contains("sample"));
    }

    #[test]
    fn a_published_stage_is_taken_by_the_checked_route_as_well() {
        let owner = id("sample");
        let mut stages = StageDeclaration::default();

        let stage = stages
            .contribute_checked(&owner, "after_entities")
            .expect("a published stage is contributed to");

        assert_eq!(stage, RenderStage::AfterEntities);
        assert_eq!(stages.contributors(RenderStage::AfterEntities), [owner]);
    }

    #[test]
    fn an_ordering_against_a_mod_that_contributes_nothing_is_refused() {
        let owner = id("sample");
        let absent = id("probe");

        let mut stages = StageDeclaration::default();
        stages.contribute(&owner, "after_terrain");
        stages.after(&owner, &absent);

        let refusal = stages
            .resolve()
            .expect_err("an order against nothing is refused rather than dropped");

        assert_eq!(refusal.unknown_contributors(), [absent]);
        assert!(refusal.cycle().is_empty());
        assert_eq!(refusal.unknown_stage(), "");
    }

    #[test]
    fn contributing_twice_to_one_stage_contributes_once() {
        let owner = id("sample");
        let mut stages = StageDeclaration::default();
        stages.contribute(&owner, "after_terrain");
        stages.contribute(&owner, "after_terrain");

        assert_eq!(stages.contributors(RenderStage::AfterTerrain), [owner]);
    }
}
