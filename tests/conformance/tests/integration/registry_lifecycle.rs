#![forbid(unsafe_code)]

//! Counterparts register through the loader's real barrier and service admission.

use std::collections::BTreeSet;

use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, admitted, drive};
use ravel_package::{DependencyOrder, PackageManifest};
use ravel_resolve::resolve;
use ravel_sdk::content::{ContentKind, RegistryKey};
use ravel_sdk::{LifecycleState, Mod, Side};

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
            let host = constructor(side);
            let (machine, hosts) = drive(
                side,
                &ravel_host_runtime::environment(),
                vec![(lifecycle_entry(manifest), host)],
            )
            .unwrap_or_else(|error| panic!("{} alone on {side:?}: {error}", manifest.id));
            assert_started(&machine, &hosts, std::slice::from_ref(manifest), side);
        }
    }
}

#[test]
fn all_counterparts_register_together_in_manifest_dependency_order_on_each_side() {
    let manifests = manifests();
    // This round tests the shipped packages' declared target, not the selected adapter.
    let mut environment = ravel_host_runtime::environment();
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
        let composed = entries
            .into_iter()
            .map(|entry| {
                let index = manifests
                    .iter()
                    .position(|manifest| manifest.id == entry.mod_id)
                    .expect("admission preserves package identities");
                let host = PACKAGES[index].1(side);
                (entry, host)
            })
            .collect();
        let (machine, hosts) = drive(side, &environment, composed)
            .unwrap_or_else(|error| panic!("all counterparts on {side:?}: {error}"));
        assert_started(&machine, &hosts, &manifests, side);
        assert_dependency_order(&hosts, &manifests, side);
    }
}

fn assert_started(
    machine: &LifecycleMachine,
    hosts: &[Box<dyn Mod>],
    manifests: &[PackageManifest],
    side: Side,
) {
    assert_eq!(hosts.len(), manifests.len());
    assert!(machine.registration_barrier_closed());
    machine
        .world_access()
        .expect("every required active mod is running");
    for host in hosts {
        let manifest = manifests
            .iter()
            .find(|manifest| &manifest.id == host.id())
            .expect("the host identity matches a shipped manifest");
        assert_eq!(
            machine.entry(host.id()).expect("an admitted identity"),
            &lifecycle_entry(manifest)
        );
        assert!(manifest.sides.loads_on(side));
        assert_eq!(
            machine.state(host.id()),
            Ok(LifecycleState::Running),
            "{} on {side:?}",
            host.id()
        );
        assert!(machine.loads(host.id()));
        assert_saved_data(host.as_ref(), manifest);
        let expected = expected_claims(host.as_ref());
        for key in &expected {
            assert_eq!(
                machine.registry_owner(key),
                Some(host.id()),
                "{key} on {side:?}"
            );
        }
        // Every expected key is present; equality also excludes undeclared extras.
        assert_eq!(
            machine.registry_claim_count(host.id()),
            expected.len(),
            "{} on {side:?}",
            host.id()
        );
        assert_census(host.id().as_str(), &expected);
    }
}

fn expected_claims(host: &dyn Mod) -> BTreeSet<RegistryKey> {
    if host.id().as_str() == rvl_060_counterpart::MOD_ID {
        // This support library exposes its command without a pack declaration.
        return std::iter::once(
            "command:rvl_060_counterpart:support_report"
                .parse()
                .expect("a legal command key"),
        )
        .collect();
    }
    host.content_declaration()
        .expect("a packaged declaration")
        .map_or_else(BTreeSet::new, |declaration| {
            declaration.entries().iter().cloned().collect()
        })
}

// Literal censuses preserve the independent expectations of the retired mock tests.
fn assert_census(id: &str, actual: &BTreeSet<RegistryKey>) {
    let counts: &[(ContentKind, usize)] = match id {
        "rvl_002_counterpart" => &[(ContentKind::Block, 26), (ContentKind::Item, 53)],
        "rvl_025_counterpart" => &[
            (ContentKind::Block, 5),
            (ContentKind::Item, 17),
            (ContentKind::Recipe, 12),
            (ContentKind::Command, 2),
        ],
        "rvl_042_counterpart" => &[(ContentKind::Recipe, 5), (ContentKind::Command, 1)],
        "rvl_060_counterpart" => &[(ContentKind::Command, 1)],
        "rvl_086_counterpart" => &[
            (ContentKind::Block, 17),
            (ContentKind::Item, 76),
            (ContentKind::Recipe, 1),
        ],
        "rvl_120_counterpart" => &[(ContentKind::Recipe, 1)],
        "rvl_180_counterpart" => &[(ContentKind::Command, 3)],
        "rvl_210_counterpart" => &[(ContentKind::Command, 7)],
        "rvl_321_counterpart" => &[(ContentKind::Recipe, 8)],
        "rvl_132_counterpart" | "rvl_146_counterpart" | "rvl_287_counterpart" => &[],
        _ => panic!("a counterpart needs an explicit registration census: {id}"),
    };
    assert_eq!(
        actual.len(),
        counts.iter().map(|(_, count)| count).sum::<usize>(),
        "{id}: total claims include no unlisted kinds"
    );
    for (kind, count) in counts {
        assert_eq!(
            actual.iter().filter(|key| key.kind() == *kind).count(),
            *count,
            "{id}: {kind:?}"
        );
    }
}

fn assert_saved_data(host: &dyn Mod, manifest: &PackageManifest) {
    let declaration = host
        .saved_data_declaration()
        .expect("a shipped saved-data declaration");
    let actual: BTreeSet<_> = declaration
        .as_ref()
        .map_or_else(BTreeSet::new, |declaration| {
            declaration.versions().collect()
        });
    let expected: BTreeSet<_> = manifest.saved_data_versions.iter().copied().collect();
    assert_eq!(actual, expected, "{} saved-data versions", host.id());
    if let Some(declaration) = declaration {
        assert_eq!(declaration.owner(), host.id());
    }
}

fn assert_dependency_order(hosts: &[Box<dyn Mod>], manifests: &[PackageManifest], side: Side) {
    for manifest in manifests {
        let index = hosts
            .iter()
            .position(|host| host.id() == &manifest.id)
            .expect("the dependent is loaded");
        for dependency in &manifest.dependencies {
            if dependency.order != Some(DependencyOrder::After)
                || dependency
                    .sides
                    .as_ref()
                    .is_some_and(|sides| !sides.loads_on(side))
            {
                continue;
            }
            let dependency_index = hosts
                .iter()
                .position(|host| host.id() == &dependency.id)
                .expect("the ordered dependency is loaded");
            assert!(
                dependency_index < index,
                "{} must load after {} on {side:?}",
                manifest.id,
                dependency.id
            );
        }
    }
}
