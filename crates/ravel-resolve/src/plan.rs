use std::fmt;

use ravel_core::{ModId, Side, Version};

/// A side's validated load order, with lexicographic tie-breaking between ready mods.
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
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns loading mods in order; mods absent from this side are excluded.
    #[must_use]
    pub fn order(&self) -> &[ModId] {
        &self.order
    }

    /// Returns discouraged combinations that were reported without failing resolution.
    #[must_use]
    pub fn discouraged(&self) -> &[Discouraged] {
        &self.discouraged
    }
}

/// A discouraged combination whose mods both load.
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
    use ravel_core::{ModId, Version};

    use super::Discouraged;

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
    fn a_discouragement_renders_both_mods_and_the_installed_version() {
        assert_eq!(
            discouragement().to_string(),
            "`example_mod` discourages `example_rival`, and `example_rival` 2.0.0 is installed"
        );
    }
}
