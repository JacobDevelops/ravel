use std::fmt;

use ravel_core::{ModId, Side, Version};

/// The order a validated package set loads in, on one side.
///
/// `SPEC.md` §14 requires lifecycle order to be deterministic for an identical
/// package set, and this is where that determinism is produced: a plan is a
/// total order over the mods that load, so the loader has nothing left to
/// decide. The order is the lexicographically smallest topological order of the
/// dependency graph, which makes it a property of the package set alone and not
/// of the order the packages were discovered in.
///
/// A plan is only ever built by [`crate::resolve`], because the order it holds
/// is meaningless unless every declared relationship was checked first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadPlan {
    side: Side,
    order: Vec<ModId>,
    discouraged: Vec<Discouraged>,
}

impl LoadPlan {
    pub(crate) fn new(side: Side, order: Vec<ModId>, discouraged: Vec<Discouraged>) -> Self {
        Self {
            side,
            order,
            discouraged,
        }
    }

    /// Returns the side this plan was resolved for.
    ///
    /// A plan is per-side because a package declares the sides it loads on, and
    /// `SPEC.md` §14 requires that client-only code never loads on a server
    /// side. Two sides of the same installation are two different plans.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the mods that load, in the order they load in.
    ///
    /// A mod that does not declare [`Self::side`] is absent rather than
    /// present-and-skipped: it is not loading, so it has no position.
    #[must_use]
    pub fn order(&self) -> &[ModId] {
        &self.order
    }

    /// Returns the discouraged combinations this package set contains.
    ///
    /// A discouraged relationship is the one dependency kind that reports
    /// without failing, so it is carried on the successful plan rather than in
    /// an error, and a pre-world report notes it rather than refusing on it.
    /// The list is empty for a package set that declares none.
    #[must_use]
    pub fn discouraged(&self) -> &[Discouraged] {
        &self.discouraged
    }
}

/// A discouraged combination that the resolved package set contains.
///
/// `SPEC.md` §3.4 lists discouraged alongside required, optional, and
/// incompatible, and it is the only one of the four that neither fails the
/// graph nor orders it: both mods load and the player is told. It is reported
/// as data rather than as an error because a caller that treats it as a failure
/// would be adding a rule the package model does not have.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Discouraged {
    /// The mod that declared the discouraged relationship.
    pub declaring: ModId,
    /// The mod it discourages.
    pub subject: ModId,
    /// The version of `subject` that is installed.
    pub installed: Version,
}

impl fmt::Display for Discouraged {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            declaring,
            subject,
            installed,
        } = self;
        write!(
            formatter,
            "`{declaring}` discourages `{subject}`, and `{subject}` {installed} is installed"
        )
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ModId, Side, Version};

    use super::{Discouraged, LoadPlan};

    fn id(text: &str) -> ModId {
        text.parse().expect("a valid mod id")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    fn discouragement() -> Discouraged {
        Discouraged {
            declaring: id("example_mod"),
            subject: id("example_rival"),
            installed: version("2.0.0"),
        }
    }

    #[test]
    fn a_plan_reports_the_side_and_the_order_it_was_built_from() {
        let plan = LoadPlan::new(
            Side::DedicatedServer,
            vec![id("example_library"), id("example_mod")],
            Vec::new(),
        );

        assert_eq!(plan.side(), Side::DedicatedServer);
        assert_eq!(plan.order(), [id("example_library"), id("example_mod")]);
        assert!(plan.discouraged().is_empty());
    }

    #[test]
    fn a_plan_carries_every_discouraged_combination_without_failing() {
        let plan = LoadPlan::new(
            Side::Client,
            vec![id("example_mod"), id("example_rival")],
            vec![discouragement()],
        );

        assert_eq!(plan.discouraged(), [discouragement()]);
        assert_eq!(plan.order().len(), 2, "both mods still load");
    }

    #[test]
    fn a_discouragement_renders_both_mods_and_the_installed_version() {
        assert_eq!(
            discouragement().to_string(),
            "`example_mod` discourages `example_rival`, and `example_rival` 2.0.0 is installed"
        );
    }

    #[test]
    fn a_discouragement_names_only_mods_and_a_version() {
        let rendered = discouragement().to_string();
        assert!(!rendered.contains('/'), "{rendered}");
    }
}
