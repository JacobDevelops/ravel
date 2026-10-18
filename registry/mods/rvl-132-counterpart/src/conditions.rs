use super::*;
use crate::MOD_ID;
use crate::capabilities::{RECIPES, VANILLA_ATTACHMENTS};
use crate::kinds::REGISTERED;

#[test]
fn shipped_settings_failure_is_refused_at_constructed() {
    let original = ModError::refused("invalid shipped settings");
    let mut entry = Entry::new();
    entry.settings = Err(original.clone());
    assert_eq!(entry.enter(LifecycleState::Constructed), Err(original));
    assert_eq!(entry.entered(), []);
}

#[test]
fn wrapped_refusals_expose_their_typed_sources() {
    use std::error::Error;
    let kinds = EntryRefused::Kinds(kinds::NotRegistered::Full { limit: 2 });
    assert!(
        kinds
            .source()
            .expect("kind source")
            .is::<kinds::NotRegistered>()
    );
    let bundle = EntryRefused::Bundle(capabilities::NotInstalled::NoTarget);
    assert!(
        bundle
            .source()
            .expect("bundle source")
            .is::<capabilities::NotInstalled>()
    );
    let settings = EntryRefused::Settings(settings::SettingsRefused::ListTooLong { limit: 4 });
    assert!(
        settings
            .source()
            .expect("settings source")
            .is::<settings::SettingsRefused>()
    );
    let serialisers = EntryRefused::Serialisers(forms::TagError::Shape { expected: "a tag" });
    assert!(
        serialisers
            .source()
            .expect("serialiser source")
            .is::<forms::TagError>()
    );
    assert!(
        EntryRefused::OutOfOrder {
            wanted: Step::SetUp,
            reached: Step::Constructed,
        }
        .source()
        .is_none()
    );
}

fn run_to_registered() -> Entry {
    let mut entry = Entry::new();
    entry
        .enter(LifecycleState::Constructed)
        .expect("a fresh entry");
    entry
        .enter(LifecycleState::Registered)
        .expect("a fresh entry");
    entry
}

#[test]
fn the_entry_claims_nothing_at_the_registration_barrier() {
    let declared = Entry::declaration();
    assert_eq!(declared.entries().len(), 0);
    assert_eq!(Entry::declared_id().as_str(), MOD_ID);
}

#[test]
fn construction_installs_the_two_serialisers_and_nothing_else() {
    let mut entry = Entry::new();
    assert!(entry.serialisers().is_empty());
    entry
        .enter(LifecycleState::Constructed)
        .expect("a fresh entry");
    assert_eq!(entry.serialisers().len(), 2);
    assert_eq!(entry.step(), Step::SerialisersInstalled);
    assert!(entry.kinds().is_empty());
    assert_eq!(entry.adapters().len(), 0);
}

#[test]
fn registering_the_kinds_before_the_serialisers_is_refused() {
    let mut entry = Entry::new();
    assert_eq!(
        entry.register_kinds(),
        Err(EntryRefused::OutOfOrder {
            wanted: Step::KindsRegistered,
            reached: Step::Constructed
        })
    );
    assert!(entry.kinds().is_empty());
}

#[test]
fn asking_for_the_adapter_table_before_common_setup_is_refused() {
    let mut entry = Entry::new();
    entry.install_serialisers().expect("a fresh table");
    entry.register_kinds().expect("a fresh registry");
    assert_eq!(
        entry.install_adapters(),
        Err(EntryRefused::OutOfOrder {
            wanted: Step::AdaptersInstalled,
            reached: Step::KindsRegistered
        })
    );
    assert_eq!(entry.adapters().len(), 0);
}

#[test]
fn registration_registers_three_kinds_wires_them_and_installs_twelve_adapters() {
    let entry = run_to_registered();
    assert_eq!(entry.kinds().len(), 3);
    let names: Vec<&str> = entry
        .kinds()
        .entries()
        .map(|held| held.name.as_str())
        .collect();
    assert_eq!(names, REGISTERED);
    assert_eq!(entry.kinds().subscribed_count(), 3);
    assert_eq!(entry.adapters().len(), 12);
    assert_eq!(entry.step(), Step::AdaptersInstalled);
}

#[test]
fn a_dependents_kind_registered_before_common_setup_is_wired_too() {
    let mut entry = Entry::new();
    entry.install_serialisers().expect("a fresh table");
    entry.register_kinds().expect("a fresh registry");
    entry
        .kinds_mut()
        .register("other:gas", &kinds::Portion::CATEGORIES, false)
        .expect("a dependent's kind");
    assert_eq!(entry.common_setup(), Ok(4));
    assert!(
        entry
            .kinds()
            .entry("other:gas")
            .expect("registered")
            .subscribed
    );
}

#[test]
fn registration_installs_the_vanilla_bundle_and_bakes_the_registry() {
    let entry = run_to_registered();
    assert_eq!(entry.attachments().attachments().len(), VANILLA_ATTACHMENTS);
    assert!(entry.attachments().baked());
}

#[test]
fn every_recipe_view_attachment_the_entry_installs_has_a_view_behind_it() {
    let entry = run_to_registered();
    let views = entry.recipe_views().expect("valid settings");
    let attached: Vec<&str> = entry
        .attachments()
        .attachments()
        .iter()
        .filter(|attachment| attachment.capability == RECIPES)
        .map(|attachment| attachment.address.written())
        .collect();
    assert_eq!(attached.len(), views.len());
    for bound in &views {
        assert!(attached.contains(&bound.target), "{}", bound.target);
    }
}

#[test]
fn the_entrys_brewing_view_carries_the_resolved_cap() {
    let mut entry = Entry::new();
    let changed = settings::DOCUMENT.replace("\"default\": 7500", "\"default\": 42");
    let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
    let resolved =
        LayeredConfiguration::resolve(declared, None, None).expect("the changed defaults resolve");
    entry.settings = Ok(Settings::read(&resolved).expect("a valid cap"));
    let views = entry.recipe_views().expect("valid settings");
    let brewing = views
        .iter()
        .find_map(|bound| match &bound.view {
            recipes::View::Brewing(closure) => Some(closure),
            recipes::View::Station(_) => None,
        })
        .expect("a brewing view");
    assert_eq!(brewing.cap(), 42);
}

#[test]
fn a_bundle_failure_is_recorded_and_loading_carries_on_at_the_shipped_default() {
    // Section three: with `compatibility.abort_on_failure` off, a bundle
    // failure is logged and loading continues. Ravel publishes no logger, so
    // the record is readable instead.
    let mut entry = run_to_registered();
    assert!(entry.attachments().baked());
    assert_eq!(entry.load_compatibility(None), Ok(0));
    assert_eq!(entry.bundle_failures().len(), 1);
}

#[test]
fn the_abort_setting_turns_a_bundle_failure_into_a_refusal() {
    let mut entry = run_to_registered();
    entry
        .settings
        .as_mut()
        .expect("valid settings")
        .abort_on_bundle_failure = true;
    assert!(matches!(
        entry.load_compatibility(None),
        Err(EntryRefused::Bundle(_))
    ));
    assert_eq!(entry.bundle_failures().len(), 0);
}

#[test]
fn the_third_party_bundle_adds_one_attachment_when_a_target_is_named() {
    let mut entry = Entry::new();
    assert_eq!(entry.load_compatibility(Some("other:station")), Ok(47));
}

fn configuration_ignoring(names: &str) -> LayeredConfiguration {
    let document =
        settings::DOCUMENT.replace("\"default\": \"\"", &format!("\"default\": \"{names}\""));
    let declared = ConfigurationDeclaration::parse(&document).expect("a legal declaration");
    LayeredConfiguration::resolve(declared, None, None).expect("legal defaults")
}

#[test]
fn initial_comparator_uses_shipped_ignored_components() {
    let settings = Settings::read(&configuration_ignoring("wear")).expect("a one-name list");
    let entry = Entry::with_settings(Ok(settings));
    assert_eq!(entry.comparator().ignored().collect::<Vec<_>>(), ["wear"]);
}

#[test]
fn unavailable_initial_settings_or_comparator_refuse_at_constructed() {
    let original = ModError::refused("invalid shipped declaration");
    let mut unavailable = Entry::with_settings(Err(original.clone()));
    assert_eq!(
        unavailable.enter(LifecycleState::Constructed),
        Err(original)
    );
    assert_eq!(unavailable.entered(), []);

    let names: Vec<String> = (0..=data::MAX_IGNORED)
        .map(|index| format!("k{index}"))
        .collect();
    let invalid =
        Settings::read(&configuration_ignoring(&names.join(","))).map_err(ModError::from_display);
    let original = invalid
        .as_ref()
        .expect_err("too many ignored kinds")
        .clone();
    let mut unavailable = Entry::with_settings(invalid);
    assert!(unavailable.comparator().ignored().next().is_none());
    assert_eq!(
        unavailable.enter(LifecycleState::Constructed),
        Err(original)
    );
    assert_eq!(unavailable.entered(), []);
}

#[test]
fn a_resolved_ignore_list_rebuilds_the_comparator_the_kinds_are_built_against() {
    let mut entry = Entry::new();
    assert!(entry.comparator().ignored().next().is_none());
    entry
        .configure(&configuration_ignoring("wear"))
        .expect("a valid ignored list");
    assert_eq!(
        entry.comparator().ignored().collect::<Vec<&str>>(),
        ["wear"]
    );
    assert_eq!(
        entry
            .stackable()
            .comparator()
            .ignored()
            .collect::<Vec<&str>>(),
        ["wear"]
    );
    assert_eq!(
        entry.fluid().comparator().ignored().collect::<Vec<&str>>(),
        ["wear"]
    );
    assert_eq!(entry.scalar(), Scalar);
}

#[test]
fn a_resource_reload_invalidates_every_cache() {
    let mut entry = run_to_registered();
    entry
        .caches_mut()
        .brewing(|| Ok::<Vec<recipes::Converted>, ()>(Vec::new()))
        .expect("a small table");
    assert!(entry.caches().brewed());
    entry
        .observe(GameEvent::ResourcesReloaded)
        .expect("an event");
    assert!(entry.caches().is_empty());
}

#[test]
fn a_tick_invalidates_nothing() {
    let mut entry = run_to_registered();
    entry
        .caches_mut()
        .brewing(|| Ok::<Vec<recipes::Converted>, ()>(Vec::new()))
        .expect("a small table");
    for event in [GameEvent::ServerTick, GameEvent::ClientTick] {
        entry.observe(event).expect("an event");
    }
    assert!(entry.caches().brewed());
}

#[test]
fn a_server_stopping_invalidates_every_cache() {
    let mut entry = run_to_registered();
    entry
        .caches_mut()
        .brewing(|| Ok::<Vec<recipes::Converted>, ()>(Vec::new()))
        .expect("a small table");
    entry
        .enter(LifecycleState::Stopping)
        .expect("a running entry");
    assert!(entry.caches().is_empty());
}

#[test]
fn the_entry_declares_its_settings_document() {
    let entry = Entry::new();
    let declared = entry
        .configuration_declaration()
        .expect("valid settings")
        .expect("a packaged document");
    assert_eq!(declared.settings().len(), settings::KEYS.len());
}

#[test]
fn the_lifecycle_steps_taken_are_kept_in_order() {
    let entry = run_to_registered();
    assert_eq!(
        entry.entered(),
        [LifecycleState::Constructed, LifecycleState::Registered]
    );
}

#[test]
fn refusals_say_what_was_wrong() {
    assert!(
        EntryRefused::OutOfOrder {
            wanted: Step::SetUp,
            reached: Step::Constructed
        }
        .to_string()
        .contains("out of the order")
    );
    assert!(
        EntryRefused::Settings(settings::SettingsRefused::ListTooLong { limit: 4 })
            .to_string()
            .contains("longer than 4")
    );
    assert!(
        EntryRefused::Kinds(kinds::NotRegistered::Full { limit: 2 })
            .to_string()
            .contains("at most 2")
    );
    assert!(
        EntryRefused::Bundle(capabilities::NotInstalled::NoTarget)
            .to_string()
            .contains("naming a target")
    );
    assert!(
        EntryRefused::Serialisers(forms::TagError::Shape { expected: "a tag" })
            .to_string()
            .contains("a tag")
    );
}
