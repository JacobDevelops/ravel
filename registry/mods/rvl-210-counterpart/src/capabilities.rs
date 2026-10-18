//! Section seventeen's largest use: the capability-attachment registry.
//!
//! A dependent registers a constructor that attaches a capability to a block, a
//! block entity, an entity or an item, addressed either by a specific registered
//! type or by a supertype or entity category. Two behaviours are what a
//! dependent breaks on and both are here: the registry refuses registrations
//! after it is baked, raising rather than ignoring them, and it installs its
//! platform event listener lazily on the first registration of each of the four
//! target classes.

use std::collections::BTreeSet;

use core::fmt;

/// The four classes a capability can be attached to.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetClass {
    /// A block.
    Block,
    /// A block entity.
    BlockEntity,
    /// An entity.
    Entity,
    /// An item.
    Item,
}

impl TargetClass {
    /// The name the listener for this class is installed under.
    #[must_use]
    pub const fn listener(self) -> &'static str {
        match self {
            Self::Block => "block_capabilities",
            Self::BlockEntity => "block_entity_capabilities",
            Self::Entity => "entity_capabilities",
            Self::Item => "item_capabilities",
        }
    }
}

/// How a registration names what it attaches to.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Address {
    /// One specific registered type.
    Specific(String),
    /// Every registered type below a supertype, or in an entity category.
    Broad(String),
}

impl Address {
    /// The name written, whichever form it is.
    #[must_use]
    pub fn written(&self) -> &str {
        match self {
            Self::Specific(name) | Self::Broad(name) => name,
        }
    }

    /// Whether this address covers a specific registered type.
    ///
    /// A broad address covers a type whose name it prefixes, which is what a
    /// supertype or an entity category means here.
    #[must_use]
    pub fn covers(&self, name: &str) -> bool {
        match self {
            Self::Specific(held) => held == name,
            Self::Broad(held) => name == held || name.starts_with(&format!("{held}/")),
        }
    }
}

/// One registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    /// What class it attaches to.
    pub class: TargetClass,
    /// What it attaches to inside that class.
    pub address: Address,
    /// Which capability it attaches.
    pub capability: String,
}

/// What the registry refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityRefused {
    /// The registry is baked and takes nothing more.
    AlreadyBaked {
        /// What was offered.
        capability: String,
    },
    /// The registry holds as many attachments as it publishes.
    Full {
        /// The published limit.
        limit: usize,
    },
}

impl fmt::Display for CapabilityRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyBaked { capability } => write!(
                formatter,
                "{capability} was registered after the registry was baked"
            ),
            Self::Full { limit } => {
                write!(formatter, "the registry holds at most {limit} attachments")
            }
        }
    }
}

impl std::error::Error for CapabilityRefused {}

/// The most attachments the registry holds.
pub const MAX_ATTACHMENTS: usize = 4096;

/// The attachment registry.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    attachments: Vec<Attachment>,
    listeners: BTreeSet<TargetClass>,
    baked: bool,
}

impl Registry {
    /// An empty registry that has installed no listener.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            attachments: Vec::new(),
            listeners: BTreeSet::new(),
            baked: false,
        }
    }

    /// Registers one constructor.
    ///
    /// The listener for a target class is installed the first time that class is
    /// registered against and not before, which is why a dependent that
    /// registers nothing for entities costs nothing on the entity path.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityRefused::AlreadyBaked`] after baking, which the registry raises
    /// rather than ignoring, and [`CapabilityRefused::Full`] past the published limit.
    pub fn register(
        &mut self,
        class: TargetClass,
        address: Address,
        capability: &str,
    ) -> Result<(), CapabilityRefused> {
        if self.baked {
            return Err(CapabilityRefused::AlreadyBaked {
                capability: capability.to_owned(),
            });
        }
        if self.attachments.len() >= MAX_ATTACHMENTS {
            return Err(CapabilityRefused::Full {
                limit: MAX_ATTACHMENTS,
            });
        }
        self.listeners.insert(class);
        self.attachments.push(Attachment {
            class,
            address,
            capability: capability.to_owned(),
        });
        Ok(())
    }

    /// Bakes the registry, after which it takes nothing more.
    pub const fn bake(&mut self) {
        self.baked = true;
    }

    /// Whether the registry is baked.
    #[must_use]
    pub const fn baked(&self) -> bool {
        self.baked
    }

    /// Whether the listener for a class has been installed.
    #[must_use]
    pub fn listener_installed(&self, class: TargetClass) -> bool {
        self.listeners.contains(&class)
    }

    /// Every listener installed, in class order.
    #[must_use]
    pub fn listeners(&self) -> Vec<&'static str> {
        self.listeners
            .iter()
            .map(|class| class.listener())
            .collect()
    }

    /// Every registration, in registration order.
    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// The capabilities attached to one registered type, in registration order.
    #[must_use]
    pub fn attached(&self, class: TargetClass, name: &str) -> Vec<&str> {
        self.attachments
            .iter()
            .filter(|attachment| attachment.class == class && attachment.address.covers(name))
            .map(|attachment| attachment.capability.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listener_is_installed_on_the_first_registration_of_its_class_and_not_before() {
        let mut registry = Registry::new();
        assert_eq!(registry.listeners().len(), 0);
        for class in [
            TargetClass::Block,
            TargetClass::BlockEntity,
            TargetClass::Entity,
            TargetClass::Item,
        ] {
            assert!(!registry.listener_installed(class));
        }
        registry
            .register(
                TargetClass::Item,
                Address::Specific("one".to_owned()),
                "store",
            )
            .expect("the registry is open");
        assert!(registry.listener_installed(TargetClass::Item));
        assert!(!registry.listener_installed(TargetClass::Entity));
        assert_eq!(registry.listeners(), vec!["item_capabilities"]);
    }

    #[test]
    fn each_of_the_four_classes_installs_its_own_listener_once() {
        let mut registry = Registry::new();
        for class in [
            TargetClass::Block,
            TargetClass::BlockEntity,
            TargetClass::Entity,
            TargetClass::Item,
        ] {
            for name in ["one", "two"] {
                registry
                    .register(class, Address::Specific(name.to_owned()), "store")
                    .expect("the registry is open");
            }
        }
        assert_eq!(registry.listeners().len(), 4);
        let named = vec![
            "block_capabilities",
            "block_entity_capabilities",
            "entity_capabilities",
            "item_capabilities",
        ];
        let mut installed = registry.listeners();
        installed.sort_unstable();
        assert_eq!(installed, named);
        assert_eq!(registry.attachments().len(), 8);
    }

    #[test]
    fn the_registry_raises_after_baking_rather_than_ignoring_the_registration() {
        let mut registry = Registry::new();
        registry.bake();
        assert!(registry.baked());
        assert_eq!(
            registry.register(
                TargetClass::Block,
                Address::Specific("one".to_owned()),
                "store"
            ),
            Err(CapabilityRefused::AlreadyBaked {
                capability: "store".to_owned()
            })
        );
        assert_eq!(
            registry.attachments().len(),
            0,
            "a refused registration is not held"
        );
        assert!(
            !registry.listener_installed(TargetClass::Block),
            "a refused registration installs no listener"
        );
    }

    #[test]
    fn a_specific_address_covers_that_type_and_nothing_else() {
        let address = Address::Specific("machines/press".to_owned());
        assert!(address.covers("machines/press"));
        assert!(!address.covers("machines/pressure"));
        assert!(!address.covers("machines"));
        assert_eq!(address.written(), "machines/press");
    }

    #[test]
    fn a_broad_address_covers_everything_below_the_name_it_writes() {
        let address = Address::Broad("machines".to_owned());
        assert!(address.covers("machines"));
        assert!(address.covers("machines/press"));
        assert!(
            !address.covers("machinesomething"),
            "a prefix is not a supertype"
        );
    }

    #[test]
    fn what_is_attached_to_a_type_is_every_matching_registration_in_order() {
        let mut registry = Registry::new();
        registry
            .register(
                TargetClass::Block,
                Address::Broad("machines".to_owned()),
                "energy",
            )
            .expect("open");
        registry
            .register(
                TargetClass::Block,
                Address::Specific("machines/press".to_owned()),
                "fluid",
            )
            .expect("open");
        registry
            .register(
                TargetClass::Item,
                Address::Broad("machines".to_owned()),
                "never",
            )
            .expect("open");
        assert_eq!(
            registry.attached(TargetClass::Block, "machines/press"),
            vec!["energy", "fluid"]
        );
        assert_eq!(
            registry.attached(TargetClass::Block, "machines/oven"),
            vec!["energy"]
        );
        assert_eq!(
            registry
                .attached(TargetClass::Entity, "machines/press")
                .len(),
            0
        );
    }

    #[test]
    fn the_registry_refuses_more_attachments_than_it_publishes() {
        let mut registry = Registry::new();
        for held in 0..MAX_ATTACHMENTS {
            registry
                .register(
                    TargetClass::Item,
                    Address::Specific(format!("one{held:05}")),
                    "store",
                )
                .expect("inside the limit");
        }
        assert_eq!(
            registry.register(
                TargetClass::Item,
                Address::Specific("last".to_owned()),
                "store"
            ),
            Err(CapabilityRefused::Full {
                limit: MAX_ATTACHMENTS
            })
        );
    }
}
