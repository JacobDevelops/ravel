use super::*;
use crate::{__ravel_entry, CONVENTION_NAMESPACE, world};
use ravel_sdk::content::{RegistrationRefused, RegistryKey};
use ravel_sdk::json;
use rvl_321_counterpart::model::Crafted;

struct Provider(rvl_321_counterpart::Entry);

impl ServiceDirectory for Provider {
    fn call(
        &mut self,
        request: &ServiceRequest,
    ) -> Result<ServiceResponse, ravel_sdk::services::ServiceCallFailed> {
        Ok(self
            .0
            .answer_service(request)
            .expect("the actual provider answers its flags"))
    }
}

#[test]
fn flags_are_resolved_from_the_configured_dependency_not_our_table() {
    use ravel_sdk::config::ValueDocument;
    use rvl_321_counterpart::settings as depended;
    for (ground, tools) in [(false, false), (false, true), (true, false), (true, true)] {
        let document = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": rvl_321_counterpart::MOD_ID,
                "values": {
                    (depended::GROUND_CONVERT_BY_MATERIAL): ground,
                    (depended::GEAR_TOP_TOOLS_ENCHANTABLE): tools
                }
            })
            .to_string(),
        )
        .expect("valid settings document");
        let resolved = LayeredConfiguration::resolve(
            depended::declaration().expect("valid declaration"),
            Some(&document),
            None,
        )
        .expect("valid configured flags");
        let mut provider = Provider(rvl_321_counterpart::Entry::new());
        provider
            .0
            .configure(&resolved)
            .expect("flags are published");
        let mut entry = Entry::new();
        for (flag, enabled) in settings::consumed_flags()
            .expect("legal flags")
            .into_iter()
            .zip([tools, ground])
        {
            entry.flags.publish_constant(flag, !enabled);
        }
        let declaration = entry
            .service_declaration()
            .expect("legal services")
            .expect("required flags");
        assert!(
            declaration.requires().any(
                |id| id == &rvl_321_counterpart::Entry::flags_service().expect("legal service")
            )
        );
        entry
            .use_services(&mut provider)
            .expect("actual cross-mod answers");
        assert_eq!(entry.ground_conversion_enabled(), ground);
        assert_eq!(entry.top_tools_enchantable(), tools);
    }
}

#[test]
fn a_packaged_settings_refusal_is_reported_at_constructed() {
    let mut entry = Entry::new();
    entry.settings = Err(ModError::refused("invalid packaged settings"));
    assert!(entry.enter(LifecycleState::Constructed).is_err());
    assert_eq!(entry.entered, []);
}

#[derive(Default)]
struct Accepting {
    claimed: Vec<RegistryKey>,
}

impl Registrar for Accepting {
    fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
        self.claimed.push(key);
        Ok(())
    }

    fn claimed(&self) -> &[RegistryKey] {
        &self.claimed
    }

    fn loads(&self, _mod_id: &ModId) -> bool {
        false
    }
}

#[test]
fn the_declared_identity_is_the_one_the_package_carries() {
    assert_eq!(Entry::declared_id().as_str(), MOD_ID);
    assert_eq!(Entry::new().id().as_str(), MOD_ID);
    assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
}

#[test]
fn a_fresh_entry_registers_all_six_crops_and_has_not_adjusted_them() {
    let entry = Entry::new();
    assert_eq!(entry.crops().len(), crops::COUNT);
    assert!(entry.crops().iter().all(|crop| crop.enabled));
    assert!(entry.crops().iter().all(|crop| crop.foundation.is_none()));
    assert!(!entry.adjusted());
    assert_eq!(entry.tier().value, tier::VALUE);
    assert_eq!(entry.tier().ground, None);
    assert_eq!(
        entry.settings().expect("valid settings"),
        &Settings::shipped().expect("valid defaults")
    );
}

#[test]
fn the_adjust_pass_points_the_tier_at_content_and_every_crop_at_an_anchor() {
    let mut entry = Entry::new();
    entry.adjust().expect("valid settings");
    assert!(entry.adjusted());
    assert!(entry.tier().ground.is_some());
    assert!(entry.tier().material.is_some());
    assert!(!entry.tier().second_drop);
    for (crop, declared) in entry.crops().iter().zip(crops::DECLARED) {
        assert_eq!(
            crop.foundation,
            Some(
                format!("{MOD_ID}:{}", declared.anchor)
                    .parse()
                    .expect("the declared anchor is an identifier")
            )
        );
    }
}

#[test]
fn the_registration_barrier_runs_the_adjust_pass() {
    let mut entry = Entry::new();
    let mut registrar = Accepting::default();
    entry
        .register(&mut registrar)
        .expect("the accepting registrar refuses nothing");
    assert!(
        entry.adjusted(),
        "the adjust pass follows the register pass"
    );
}

#[test]
fn every_block_is_claimed_before_any_item_and_block_items_come_first_among_items() {
    let declaration = Entry::declaration().expect("the packaged declaration is well formed");
    let entries = declaration.entries();
    let first_item = entries
        .iter()
        .position(|key| key.kind() == ContentKind::Item)
        .expect("the entry claims items");
    assert_eq!(first_item, census::BLOCKS);
    let item_paths: Vec<&str> = entries[first_item..]
        .iter()
        .map(|key| key.id().path())
        .collect();
    assert_eq!(item_paths, census::item_paths());
    assert_eq!(
        &item_paths[..census::BLOCK_ITEMS],
        census::block_item_paths().as_slice()
    );
}

#[test]
fn the_declaration_carries_the_forty_eight_tag_documents_too() {
    let declaration = Entry::declaration().expect("the packaged declaration is well formed");
    assert_eq!(declaration.tags().count(), data::TAG_DOCUMENTS);
}

#[test]
fn each_of_the_six_crops_is_crafted_from_a_material_of_its_own() {
    let entry = Entry::new();
    let mut ids: Vec<String> = entry
        .crops()
        .iter()
        .map(|crop| match &crop.crafted {
            Crafted::Entry { id, .. } | Crafted::Tag(id) => id.to_string(),
            Crafted::Nothing => String::new(),
        })
        .collect();
    assert_eq!(ids.len(), crops::COUNT);
    assert!(
        ids.iter().all(|id| !id.is_empty()),
        "every one of the six names a material"
    );
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        crops::COUNT,
        "the five shapes do not collapse to one name"
    );
}

#[test]
fn a_tag_shaped_crop_names_a_tag_this_entry_declares_or_one_nobody_owns() {
    let entry = Entry::new();
    let declaration = Entry::declaration().expect("the packaged declaration is well formed");
    let mut shaped = 0;
    for crop in entry.crops() {
        let Crafted::Tag(id) = &crop.crafted else {
            continue;
        };
        shaped += 1;
        if id.namespace() == Entry::declared_id() {
            let tag = declaration
                .tags()
                .find(|tag| tag.id() == id)
                .expect("a tag this entry declares itself");
            assert_eq!(
                declaration.tag_members(tag).len(),
                crops::OPTIONAL_TAG_MEMBERS,
                "the two optional spellings are the members"
            );
        } else {
            assert!(
                id.namespace().as_str() == CONVENTION_NAMESPACE,
                "a cross-mod tag is named where no mod owns the name"
            );
        }
    }
    assert_eq!(shaped, 2, "the convention tag and this entry's own");
}

#[test]
fn a_short_tag_declaration_is_refused_rather_than_carried_to_the_barrier() {
    let declaration = Entry::declaration().expect("the packaged declaration is well formed");
    let owner = Entry::declared_id().clone();
    let blocks: Vec<RegistryKey> = declaration
        .entries()
        .iter()
        .filter(|key| key.kind() == ContentKind::Block)
        .cloned()
        .collect();
    let mut short = ContentDeclaration::new(owner);
    let refusal = data::declare_tags(&mut short, &blocks, &[]);
    assert!(
        refusal.is_err(),
        "a family with no members to state is a packaging failure"
    );
}

#[test]
fn the_entry_declares_both_its_content_and_its_settings() {
    let entry = Entry::new();
    assert!(
        entry
            .content_declaration()
            .expect("valid content")
            .is_some()
    );
    assert!(
        entry
            .configuration_declaration()
            .expect("valid configuration")
            .is_some()
    );
}

#[test]
fn the_lifecycle_runs_through_and_a_reload_is_the_one_signal_it_counts() {
    let mut entry = Entry::new();
    for state in LifecycleState::ALL {
        entry.enter(state).expect("the entry accepts every step");
    }
    assert_eq!(entry.entered(), LifecycleState::ALL);

    entry
        .observe(GameEvent::ResourcesReloaded)
        .expect("a reload is a signal this entry sees");
    assert_eq!(entry.reloads(), 1);
}

#[test]
fn a_thousand_ticks_leave_the_entry_exactly_as_they_found_it() {
    let mut entry = Entry::new();
    entry.adjust().expect("valid settings");
    let before = format!("{entry:?}");
    for _ in 0..1000 {
        entry
            .observe(GameEvent::ServerTick)
            .expect("a tick is a signal this entry has nothing to do about");
        entry
            .observe(GameEvent::ClientTick)
            .expect("a tick is a signal this entry has nothing to do about");
    }
    assert_eq!(format!("{entry:?}"), before);
    assert_eq!(entry.reloads(), 0);
}

#[test]
fn a_reload_changes_nothing_because_this_entry_owns_nothing_a_reload_rebuilds() {
    let mut entry = Entry::new();
    entry.adjust().expect("valid settings");
    let tier = entry.tier().clone();
    let crops = entry.crops().to_vec();
    for _ in 0..5 {
        entry
            .observe(GameEvent::ResourcesReloaded)
            .expect("a reload is a signal this entry sees");
    }
    assert_eq!(entry.reloads(), 5);
    assert_eq!(entry.tier(), &tier);
    assert_eq!(entry.crops(), crops.as_slice());
    assert_eq!(
        data::OWNS_PERSISTED_STATE,
        entry
            .saved_data_declaration()
            .expect("valid saved data declaration")
            .is_some(),
        "an entry that owns no world data declares none"
    );
}

#[test]
fn this_entry_owns_no_saved_data_and_writes_no_envelope() {
    let entry = Entry::new();
    assert!(
        entry
            .saved_data_declaration()
            .expect("valid saved data declaration")
            .is_none()
    );
    assert!(
        entry
            .save_saved_data()
            .expect("an entry that owns nothing writes nothing")
            .is_none()
    );
}

#[test]
fn a_world_this_entry_deposited_into_carries_its_name() {
    let mut shaping = WorldShaping::default();
    world::shaping(&mut shaping);
    assert_eq!(shaping.mods().len(), 1);
    assert_eq!(shaping.mods()[0].as_str(), MOD_ID);
}
