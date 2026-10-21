//! The fluid registration path and its property vocabulary, from §9.
//!
//! **Host-blocked.** Ravel publishes no fluid registration and no client fluid
//! rendering surface, so nothing here is made live. What is recorded is the
//! property vocabulary as a declared shape, because that vocabulary is what a
//! dependent writes and what a later Ravel revision would have to accept.
//!
//! The whole area is marked experimental in the original.
//!
//! One rule is portable and is implemented rather than recorded: the client-side
//! property record is looked up lazily by identifier the first time the client
//! needs it and then cached per fluid kind, so a record registered after the
//! first look is never seen. That is a real observable consequence and
//! [`FluidCache::resolve`] reproduces it.

use std::collections::BTreeMap;

/// The named sound actions a fluid may carry a sound for.
///
/// The names are what the property record keys its sound map by. They are named
/// for what each does; nothing here is a carried spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum SoundAction {
    /// Taking the fluid out of the world.
    Collect,
    /// Putting the fluid into the world.
    Place,
    /// Moving through the fluid.
    Travel,
}

impl SoundAction {
    /// Returns the action's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Collect => "collect",
            Self::Place => "place",
            Self::Travel => "travel",
        }
    }
}

/// The carrier of a fluid's own attributes.
///
/// Every field is the host's. Entity interaction attributes are grouped together;
/// the remaining attributes describe the fluid itself. Its display key is derived
/// from the fluid's identifier by the host's own rule, which is the host's and
/// is not invented here.
#[derive(Clone, Debug, PartialEq)]
pub struct FluidProperties {
    /// What the fluid converts into.
    pub conversion: Option<String>,
    /// How entities interact with the fluid.
    pub entity: EntityProperties,
    /// Whether it puts out fire.
    pub extinguishing: bool,
    /// Whether it hydrates what it touches.
    pub hydration: bool,
    /// How dense it is.
    pub density: i32,
    /// How much it scales a fall.
    pub fall_distance_scaling: f32,
    /// How much light it gives off.
    pub light_level: i32,
    /// How much it scales motion.
    pub motion_scale: f64,
    /// The path types it produces.
    pub path_types: Vec<String>,
    /// How rare it is.
    pub rarity: String,
    /// How hot it is.
    pub temperature: i32,
    /// How viscous it is.
    pub viscosity: i32,
    /// A sound per named action.
    pub sounds: BTreeMap<SoundAction, String>,
}

/// The fluid's entity interaction attributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityProperties {
    /// Whether an entity in it drowns.
    pub drowning: bool,
    /// Whether it pushes entities along.
    pub entity_pushing: bool,
    /// Whether an entity can swim in it.
    pub swimming: bool,
}

impl Default for FluidProperties {
    fn default() -> Self {
        Self {
            conversion: None,
            entity: EntityProperties {
                drowning: true,
                entity_pushing: true,
                swimming: true,
            },
            extinguishing: false,
            hydration: false,
            density: 1000,
            fall_distance_scaling: 0.0,
            light_level: 0,
            motion_scale: 1.0,
            path_types: Vec::new(),
            rarity: "common".to_owned(),
            temperature: 300,
            viscosity: 1000,
            sounds: BTreeMap::new(),
        }
    }
}

impl FluidProperties {
    /// Returns the sound a named action plays, when one is declared.
    #[must_use]
    pub fn sound(&self, action: SoundAction) -> Option<&str> {
        self.sounds.get(&action).map(String::as_str)
    }

    /// Declares a sound for an action, replacing any already there.
    pub fn with_sound(&mut self, action: SoundAction, sound: &str) -> &mut Self {
        self.sounds.insert(action, sound.to_owned());
        self
    }
}

/// The companion client-side record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientProperties {
    /// The textures the fluid is drawn with.
    pub textures: Vec<String>,
    /// The overlay drawn over it.
    pub overlay: Option<String>,
    /// The tint applied to the textures.
    pub tint: Option<crate::colour::Colour>,
    /// A custom drawing path, when the dependent supplies one.
    pub custom_render: Option<String>,
    /// How the fog's colour is changed.
    pub fog_colour: Option<crate::colour::Colour>,
    /// How the fog's range is changed.
    pub fog_range: Option<(f32, f32)>,
}

/// The most fluids a client cache will hold.
pub const MAX_FLUIDS: usize = 1024;

/// Why the fluid surface refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FluidError {
    /// The cache already holds [`MAX_FLUIDS`].
    Full(usize),
    /// A record is registered twice under one identifier.
    Duplicate(String),
}

impl std::fmt::Display for FluidError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(ceiling) => write!(formatter, "the fluid cache holds {ceiling} already"),
            Self::Duplicate(id) => write!(formatter, "`{id}` is registered twice"),
        }
    }
}

impl std::error::Error for FluidError {}

/// The lazy per-fluid cache the client resolves records through.
///
/// The lookup happening lazily is the part worth having: a record registered
/// after the first look is never seen, and a dependent has to be able to observe
/// that rather than discover it. Registration itself is host-blocked; what is
/// here is the resolution rule.
#[derive(Clone, Debug, Default)]
pub struct FluidCache {
    registered: BTreeMap<String, ClientProperties>,
    resolved: BTreeMap<String, Option<ClientProperties>>,
}

impl FluidCache {
    /// Builds an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a client-side record under a fluid identifier.
    ///
    /// # Errors
    ///
    /// Returns [`FluidError::Full`] at [`MAX_FLUIDS`] and
    /// [`FluidError::Duplicate`] for an identifier already registered.
    pub fn register(
        &mut self,
        fluid: &str,
        properties: ClientProperties,
    ) -> Result<(), FluidError> {
        if self.registered.len() >= MAX_FLUIDS {
            return Err(FluidError::Full(MAX_FLUIDS));
        }
        if self.registered.contains_key(fluid) {
            return Err(FluidError::Duplicate(fluid.to_owned()));
        }
        self.registered.insert(fluid.to_owned(), properties);
        Ok(())
    }

    /// Returns the record for a fluid, resolving it on the first ask.
    ///
    /// The answer is cached whether or not there was one, so a record registered
    /// afterwards is never seen.
    ///
    /// The cache is bounded by [`MAX_FLUIDS`] like the registrations are: an
    /// identifier arrives from a dependent's draw call, so a caller asking for
    /// names nothing registered would otherwise grow this without limit. Past
    /// the ceiling a lookup reads through and remembers nothing. The upgrade
    /// path is a larger constant, or an eviction rule.
    pub fn resolve(&mut self, fluid: &str) -> Option<&ClientProperties> {
        if !self.resolved.contains_key(fluid) && self.resolved.len() < MAX_FLUIDS {
            let found = self.registered.get(fluid).cloned();
            self.resolved.insert(fluid.to_owned(), found);
        }
        if self.resolved.contains_key(fluid) {
            self.resolved.get(fluid).and_then(Option::as_ref)
        } else {
            self.registered.get(fluid)
        }
    }

    /// Returns how many lookups the cache remembers.
    #[must_use]
    pub fn resolved_count(&self) -> usize {
        self.resolved.len()
    }

    /// Returns whether a fluid has been looked up yet.
    #[must_use]
    pub fn is_resolved(&self, fluid: &str) -> bool {
        self.resolved.contains_key(fluid)
    }

    /// Returns how many records are registered.
    #[must_use]
    pub fn registered_count(&self) -> usize {
        self.registered.len()
    }

    /// Returns whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registered.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientProperties, FluidCache, FluidError, FluidProperties, SoundAction};

    #[test]
    fn the_property_record_carries_the_whole_attribute_vocabulary() {
        let mut properties = FluidProperties {
            path_types: vec!["water".to_owned()],
            ..FluidProperties::default()
        };
        properties.with_sound(SoundAction::Collect, "example:fill");
        assert_eq!(properties.path_types, ["water"]);
        assert_eq!(properties.sound(SoundAction::Collect), Some("example:fill"));
        assert_eq!(properties.sound(SoundAction::Place), None);
        assert_eq!(SoundAction::Travel.as_str(), "travel");
        assert!(properties.entity.drowning);
        assert!(properties.entity.swimming);
        assert!(!properties.extinguishing);
        assert_eq!(properties.rarity, "common");
    }

    #[test]
    fn a_record_registered_after_the_first_look_is_never_seen() {
        let mut cache = FluidCache::new();
        assert_eq!(cache.registered_count(), 0);
        assert!(cache.resolve("molten_copper").is_none());
        assert!(cache.is_resolved("molten_copper"));
        cache
            .register("molten_copper", ClientProperties::default())
            .expect("a fresh identifier");
        assert_eq!(cache.registered_count(), 1);
        assert!(
            cache.resolve("molten_copper").is_none(),
            "the lazy lookup already answered, and the answer is what is cached"
        );
    }

    #[test]
    fn a_record_registered_before_the_first_look_is_resolved_and_then_held() {
        let mut cache = FluidCache::new();
        let properties = ClientProperties {
            textures: vec!["still".to_owned()],
            ..ClientProperties::default()
        };
        cache
            .register("molten_copper", properties.clone())
            .expect("a fresh identifier");
        assert_eq!(cache.resolve("molten_copper"), Some(&properties));
        assert_eq!(cache.resolve("molten_copper"), Some(&properties));
    }

    #[test]
    fn a_record_is_registered_once_and_the_cache_is_bounded() {
        let mut cache = FluidCache::new();
        cache
            .register("once", ClientProperties::default())
            .expect("a fresh identifier");
        assert_eq!(
            cache.register("once", ClientProperties::default()),
            Err(FluidError::Duplicate("once".to_owned()))
        );
        for index in 1..super::MAX_FLUIDS {
            cache
                .register(&format!("f{index}"), ClientProperties::default())
                .expect("a fresh identifier");
        }
        assert_eq!(
            cache.register("one_more", ClientProperties::default()),
            Err(FluidError::Full(super::MAX_FLUIDS))
        );
    }

    #[test]
    fn looking_up_names_nothing_registered_does_not_grow_the_cache_without_limit() {
        let mut cache = FluidCache::new();
        for index in 0..super::MAX_FLUIDS * 2 {
            assert_eq!(cache.resolve(&format!("absent{index}")), None);
        }
        assert_eq!(cache.registered_count(), 0, "nothing was ever registered");
        assert_eq!(
            cache.resolved_count(),
            super::MAX_FLUIDS,
            "an absent name is remembered up to the ceiling and no further"
        );

        cache
            .register("late", ClientProperties::default())
            .expect("a fresh identifier");
        assert!(
            cache.resolve("late").is_some(),
            "past the ceiling a lookup reads through rather than answering from a full cache"
        );
        assert!(!cache.is_resolved("late"));
    }
}
