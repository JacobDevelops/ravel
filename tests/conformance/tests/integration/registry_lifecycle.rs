#![forbid(unsafe_code)]

//! Drives shipped counterparts with required dependencies and together through real admission.

use ravel_loader::{
    LifecycleEntry, LifecycleMachine, LoadedMods, Requirement, admitted, start_all,
};
use ravel_package::PackageManifest;
use ravel_resolve::resolve;
use ravel_sdk::{Mod, Side};

mod expectations;
mod packets;

use super::repository_root;
use expectations::{assert_dependency_order, assert_started};

type Constructor = fn(Side) -> Box<dyn Mod>;

const PACKAGES: [(&str, Constructor); 12] = [
    (
        include_str!("../../../../registry/mods/rvl-002-counterpart/package.json"),
        rvl_002_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-025-counterpart/package.json"),
        rvl_025_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-042-counterpart/package.json"),
        rvl_042_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-060-counterpart/package.json"),
        rvl_060_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-086-counterpart/package.json"),
        rvl_086_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-120-counterpart/package.json"),
        rvl_120_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-132-counterpart/package.json"),
        rvl_132_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-146-counterpart/package.json"),
        rvl_146_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-180-counterpart/package.json"),
        rvl_180_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-210-counterpart/package.json"),
        rvl_210_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-287-counterpart/package.json"),
        rvl_287_counterpart::__ravel_entry,
    ),
    (
        include_str!("../../../../registry/mods/rvl-321-counterpart/package.json"),
        rvl_321_counterpart::__ravel_entry,
    ),
];

const SIDES: [Side; 3] = [Side::Client, Side::IntegratedServer, Side::DedicatedServer];

fn manifests() -> Vec<PackageManifest> {
    PACKAGES
        .iter()
        .map(|(document, _)| PackageManifest::parse(document).expect("a shipped manifest"))
        .collect()
}

fn lifecycle_entry(manifest: &PackageManifest) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: manifest.id.clone(),
        sides: manifest.sides.clone(),
        requirement: Requirement::Required,
        saved_data_versions: manifest.saved_data_versions.iter().copied().collect(),
    }
}

fn required_manifests(
    root: &PackageManifest,
    manifests: &[PackageManifest],
    side: Side,
) -> Vec<PackageManifest> {
    let mut selected = std::collections::BTreeSet::new();
    let mut pending = vec![root];
    while let Some(manifest) = pending.pop() {
        if !selected.insert(manifest.id.clone()) {
            continue;
        }
        for dependency in &manifest.dependencies {
            if dependency.relationship != ravel_package::DependencyRelationship::Required
                || dependency
                    .sides
                    .as_ref()
                    .is_some_and(|sides| !sides.loads_on(side))
            {
                continue;
            }
            pending.push(
                manifests
                    .iter()
                    .find(|candidate| candidate.id == dependency.id)
                    .expect("every mandatory dependency is shipped"),
            );
        }
    }
    manifests
        .iter()
        .filter(|manifest| selected.contains(&manifest.id))
        .cloned()
        .collect()
}

fn construct(
    manifest: &PackageManifest,
    manifests: &[PackageManifest],
    side: Side,
) -> Box<dyn Mod> {
    let index = manifests
        .iter()
        .position(|candidate| candidate.id == manifest.id)
        .expect("every selected manifest has a constructor");
    PACKAGES[index].1(side)
}

#[test]
fn every_discovered_counterpart_is_driven_through_lifecycle_admission() {
    let directory = repository_root().join("registry/mods");
    let discovered: std::collections::BTreeSet<_> = std::fs::read_dir(directory)
        .expect("the registry directory is readable")
        .map(|entry| {
            entry
                .expect("a readable registry entry")
                .path()
                .join("package.json")
        })
        .filter(|path| path.is_file())
        .map(|path| {
            let document = std::fs::read_to_string(path).expect("a readable shipped manifest");
            PackageManifest::parse(&document)
                .expect("a shipped manifest")
                .id
        })
        .collect();
    let driven: std::collections::BTreeSet<_> = manifests()
        .into_iter()
        .map(|manifest| manifest.id)
        .collect();
    assert_eq!(
        discovered, driven,
        "every shipped counterpart must enter PACKAGES"
    );
}

#[test]
fn every_counterpart_registers_with_only_its_required_dependencies_on_each_side() {
    let manifests = manifests();
    let mut environment = ravel_bootstrap_runtime::environment();
    environment.minecraft = "1.21.1".to_owned();
    for side in SIDES {
        for manifest in &manifests {
            let selected = required_manifests(manifest, &manifests, side);
            let plan = resolve(&selected, side).expect("mandatory dependencies resolve");
            let entries = admitted(&plan, &selected, &environment.ravel, &environment.minecraft)
                .expect("the counterpart and its dependencies are compatible");
            let mut mods: Vec<_> = selected
                .iter()
                .map(|manifest| construct(manifest, &manifests, side))
                .collect();
            let mut machine = LifecycleMachine::new(side, entries)
                .expect("the counterpart declares a legal lifecycle");
            start_all(
                &mut LoadedMods::new(&mut machine, &mut mods)
                    .expect("every active member has a mod value"),
                &environment,
            )
            .unwrap_or_else(|error| {
                panic!("{} with dependencies on {side:?}: {error}", manifest.id)
            });
            assert_started(&machine, &mods, &selected, side);
            assert_dependency_order(&mods, &selected, side);
        }
    }
}

#[test]
fn all_counterparts_register_together_in_manifest_dependency_order_on_each_side() {
    let manifests = manifests();
    // This round tests the shipped packages' declared target, not the selected adapter.
    let mut environment = ravel_bootstrap_runtime::environment();
    environment.minecraft = "1.21.1".to_owned();
    for side in SIDES {
        let plan = resolve(&manifests, side).expect("the shipped dependency graph resolves");
        let entries = admitted(
            &plan,
            &manifests,
            &environment.ravel,
            &environment.minecraft,
        )
        .expect("the shipped counterparts are compatible");
        let mut mods: Vec<Box<dyn Mod>> = entries
            .iter()
            .map(|entry| {
                let index = manifests
                    .iter()
                    .position(|manifest| manifest.id == entry.mod_id)
                    .expect("admission preserves package identities");
                PACKAGES[index].1(side)
            })
            .collect();
        // Supplied storage order must not override the manifest's resolved dependency order.
        mods.reverse();
        let mut machine = LifecycleMachine::new(side, entries).expect("a legal admitted lifecycle");
        start_all(
            &mut LoadedMods::new(&mut machine, &mut mods)
                .expect("every active member has a mod value"),
            &environment,
        )
        .unwrap_or_else(|error| panic!("all counterparts on {side:?}: {error}"));
        assert_started(&machine, &mods, &manifests, side);
        assert_dependency_order(&mods, &manifests, side);
    }
}

#[test]
fn configured_provider_flags_reach_the_leaf_through_real_loader_startup() {
    use ravel_sdk::config::ValueDocument;
    use rvl_321_counterpart::settings;

    let manifests = manifests();
    let leaf_manifest = &manifests[0];
    let environment = ravel_bootstrap_runtime::environment();
    for side in SIDES {
        for (ground, tools) in [(false, false), (false, true), (true, false), (true, true)] {
            let selected = required_manifests(leaf_manifest, &manifests, side);
            let plan = resolve(&selected, side).expect("the required graph resolves");
            let entries = plan
                .order()
                .iter()
                .map(|id| {
                    lifecycle_entry(
                        selected
                            .iter()
                            .find(|manifest| &manifest.id == id)
                            .expect("resolved members are selected"),
                    )
                })
                .collect();
            let mut machine = LifecycleMachine::new(side, entries).expect("legal lifecycle");
            let mut shared = rvl_042_counterpart::Entry::new();
            let mut provider = rvl_321_counterpart::Entry::new();
            let mut leaf = rvl_002_counterpart::Entry::new();
            let document = ValueDocument::parse(
                &ravel_sdk::json!({
                    "schemaVersion": 1,
                    "owner": rvl_321_counterpart::MOD_ID,
                    "values": {
                        (settings::GROUND_CONVERT_BY_MATERIAL): ground,
                        (settings::GEAR_TOP_TOOLS_ENCHANTABLE): tools
                    }
                })
                .to_string(),
            )
            .expect("legal launch overrides");
            {
                let mut mods: [&mut dyn Mod; 3] = [&mut leaf, &mut provider, &mut shared];
                let mut loaded = LoadedMods::new(&mut machine, &mut mods).expect("complete set");
                ravel_loader::open_configuration(
                    &mut loaded,
                    &std::iter::once((document.owner.clone(), document)).collect(),
                    |_| Ok(None),
                    &environment,
                )
                .expect("the loader applies the provider's launch configuration");
                start_all(&mut loaded, &environment).expect("configured services start");
            }
            let hosts: [&dyn Mod; 3] = [&leaf, &provider, &shared];
            expectations::assert_running(&machine, hosts, side);
            assert_eq!(leaf.ground_conversion_enabled(), ground);
            assert_eq!(leaf.top_tools_enchantable(), tools);
        }
    }
}
