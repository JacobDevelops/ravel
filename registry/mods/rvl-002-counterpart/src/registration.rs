//! Loader-driven entry, configuration, registration, and dependency flags.

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, ContentKind, Registrar, claim_all};
use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;
use rvl_042_counterpart::flags::FlagTable;
use rvl_321_counterpart::model::{Crop, Tier};

use crate::{MOD_ID, Settings, census, crops, data, settings, tier};

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    declaration: Result<ContentDeclaration, ModError>,
    settings: Result<Settings, ModError>,
    flags: FlagTable,
    ground_conversion_enabled: bool,
    top_tools_enchantable: bool,
    tier: Tier,
    crops: Vec<Crop>,
    adjusted: bool,
    reloads: u32,
    entered: Vec<LifecycleState>,
}

impl Default for Entry {
    fn default() -> Self {
        Self::new()
    }
}

impl Entry {
    /// Builds the entry holding its shipped defaults, its tier and its six
    /// crops, all six registered and none of them adjusted yet.
    #[must_use]
    pub fn new() -> Self {
        let tier = tier::registered(
            format!("{MOD_ID}:{}", tier::PATH)
                .parse()
                .expect("the declared tier has a legal identity"),
        );
        let crops = crops::DECLARED
            .iter()
            .map(|declared| {
                declared.crop(
                    ContentId::new(Self::declared_id().clone(), declared.path)
                        .expect("the declared crop has a legal path"),
                    &tier.id,
                    declared
                        .material_id(MOD_ID)
                        .parse()
                        .expect("a legal material identity"),
                )
            })
            .collect();
        Self {
            declaration: Self::declaration(),
            settings: Settings::shipped(),
            flags: FlagTable::default(),
            ground_conversion_enabled: false,
            top_tools_enchantable: false,
            tier,
            crops,
            adjusted: false,
            reloads: 0,
            entered: Vec::new(),
        }
    }

    /// The values in force.
    ///
    /// # Errors
    /// Returns the packaged settings refusal if construction could not resolve them.
    pub fn settings(&self) -> Result<&Settings, ModError> {
        self.settings.as_ref().map_err(Clone::clone)
    }

    /// The flag table this entry published into.
    #[must_use]
    pub const fn flags(&self) -> &FlagTable {
        &self.flags
    }

    /// The dependency's ground conversion flag, resolved through its service.
    #[must_use]
    pub const fn ground_conversion_enabled(&self) -> bool {
        self.ground_conversion_enabled
    }

    /// The dependency's enchantability flag, resolved through its service.
    #[must_use]
    pub const fn top_tools_enchantable(&self) -> bool {
        self.top_tools_enchantable
    }

    /// The sixth tier.
    #[must_use]
    pub const fn tier(&self) -> &Tier {
        &self.tier
    }

    /// The six crops, all registered whatever state they are in.
    #[must_use]
    pub fn crops(&self) -> &[Crop] {
        &self.crops
    }

    /// How many reloads this entry has seen.
    #[must_use]
    pub const fn reloads(&self) -> u32 {
        self.reloads
    }

    /// The lifecycle steps this entry has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Whether the adjust pass has run.
    #[must_use]
    pub const fn adjusted(&self) -> bool {
        self.adjusted
    }

    /// The adjust pass: point the tier at content, copy the accelerant switch,
    /// and point every crop at its anchor block.
    ///
    /// The pointing is unconditional and happens for all six crops including the
    /// ones no companion mod enables, so an unobtainable crop still has a valid
    /// anchor.
    ///
    /// # Errors
    /// Returns the settings refusal before changing tiers or crops.
    pub fn adjust(&mut self) -> Result<(), ModError> {
        let fertilisable = self.settings()?.crops_fertilisable;
        tier::adjust(
            &mut self.tier,
            ContentId::new(Self::declared_id().clone(), "ground").expect("a legal ground path"),
            ContentId::new(Self::declared_id().clone(), "material").expect("a legal material path"),
            fertilisable,
        );
        for (crop, declared) in self.crops.iter_mut().zip(crops::DECLARED) {
            crop.foundation = Some(
                ContentId::new(Self::declared_id().clone(), declared.anchor)
                    .expect("a legal foundation path"),
            );
        }
        self.adjusted = true;
        Ok(())
    }

    /// What this entry claims at the registration barrier.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared name is not a legal content
    /// path, which for these constants is a packaging failure.
    /// Every block is claimed before any item, and the twenty block items are
    /// claimed before the thirty-three plain ones, so the block items occupy the
    /// earliest positions in this namespace's item registry. The tag
    /// memberships of section sixteen are declared in the same place, because
    /// the declaration is the one statement both the barrier and the generated
    /// pack are derived from.
    pub fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        let mut blocks = Vec::with_capacity(census::BLOCKS);
        for path in census::block_paths() {
            blocks.push(
                declaration
                    .entry(ContentKind::Block, &path)
                    .map_err(ModError::from_display)?,
            );
        }
        let mut items = Vec::with_capacity(census::ITEMS);
        for path in census::item_paths() {
            items.push(
                declaration
                    .entry(ContentKind::Item, &path)
                    .map_err(ModError::from_display)?,
            );
        }
        let tags = data::declare_tags(&mut declaration, &blocks, &items)?;
        if tags != data::TAG_DOCUMENTS {
            return Err(ModError::refused(format!(
                "{tags} of the {} tag documents were declared",
                data::TAG_DOCUMENTS
            )));
        }
        Ok(declaration)
    }
}

#[declare_mod(id = "rvl_002_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if state == LifecycleState::Constructed {
            self.declaration.as_ref().map_err(Clone::clone)?;
            self.settings()?;
        }
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        self.declaration
            .as_ref()
            .map(std::borrow::Cow::Borrowed)
            .map(Some)
            .map_err(Clone::clone)
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        let declaration = self.declaration.as_ref().map_err(Clone::clone)?;
        claim_all(declaration, registrar)?;
        self.adjust()?;
        Ok(())
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        settings::declaration()
            .map(Some)
            .map_err(ModError::from_display)
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        let settings = Settings::read(resolved).map_err(ModError::from_display)?;
        settings::publish(&mut self.flags, &settings).map_err(ModError::from_display)?;
        self.settings = Ok(settings);
        Ok(())
    }

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        let mut declaration = ServiceDeclaration::new(Self::declared_id().clone());
        declaration
            .require(rvl_321_counterpart::Entry::flags_service()?)
            .map_err(ModError::from_display)?;
        Ok(Some(declaration))
    }

    fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
        let mut values = [false; 2];
        for (value, flag) in values
            .iter_mut()
            .zip(settings::consumed_flags().map_err(ModError::from_display)?)
        {
            let response = directory
                .call(&ServiceRequest::new(
                    rvl_321_counterpart::Entry::flags_service()?,
                    ravel_sdk::json!({"flag": flag.to_string()}),
                ))
                .map_err(ModError::from_display)?;
            *value = response
                .value()
                .as_bool()
                .ok_or_else(|| ModError::refused("the flag service did not answer a boolean"))?;
        }
        [self.top_tools_enchantable, self.ground_conversion_enabled] = values;
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        match event {
            GameEvent::ResourcesReloaded => self.reloads = self.reloads.saturating_add(1),
            GameEvent::ServerTick | GameEvent::ClientTick => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod conditions;
