#![forbid(unsafe_code)]

//! Drives shipped counterparts alone and together through real lifecycle admission.

use ravel_loader::{
    LifecycleEntry, LifecycleMachine, LoadedMods, Requirement, admitted, drive_loaded,
};
use ravel_package::PackageManifest;
use ravel_resolve::resolve;
use ravel_sdk::{Mod, Side};

mod expectations;

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

#[test]
fn every_counterpart_registers_alone_on_each_side() {
    let manifests = manifests();
    for side in SIDES {
        for (manifest, (_, constructor)) in manifests.iter().zip(PACKAGES) {
            // Isolate lifecycle admission from dependency resolution in this round.
            let mut mods = vec![constructor(side)];
            let mut machine = LifecycleMachine::new(side, vec![lifecycle_entry(manifest)])
                .expect("the counterpart declares a legal lifecycle");
            drive_loaded(
                &mut LoadedMods::new(&mut machine, &mut mods)
                    .expect("every active member has a mod value"),
                &ravel_bootstrap_runtime::environment(),
            )
            .unwrap_or_else(|error| panic!("{} alone on {side:?}: {error}", manifest.id));
            assert_started(&machine, &mods, std::slice::from_ref(manifest), side);
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
        drive_loaded(
            &mut LoadedMods::new(&mut machine, &mut mods)
                .expect("every active member has a mod value"),
            &environment,
        )
        .unwrap_or_else(|error| panic!("all counterparts on {side:?}: {error}"));
        assert_started(&machine, &mods, &manifests, side);
        assert_dependency_order(&mods, &manifests, side);
    }
}
