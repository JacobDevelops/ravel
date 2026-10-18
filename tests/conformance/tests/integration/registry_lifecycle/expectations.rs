//! Independent registry censuses, saved-data contracts, and dependency-order expectations.

use std::collections::BTreeSet;

use ravel_loader::LifecycleMachine;
use ravel_package::{DependencyOrder, PackageManifest};
use ravel_sdk::content::{ContentKind, RegistryKey};
use ravel_sdk::{LifecycleState, Mod, Side};

use super::lifecycle_entry;

pub(super) fn assert_running<'a>(
    machine: &LifecycleMachine,
    hosts: impl IntoIterator<Item = &'a dyn Mod>,
    side: Side,
) {
    assert!(machine.registration_barrier_closed());
    for host in hosts {
        assert_eq!(
            machine.state(host.id()),
            Ok(LifecycleState::Running),
            "{} on {side:?}",
            host.id()
        );
        assert!(machine.loads(host.id()));
    }
}

pub(super) fn assert_started(
    machine: &LifecycleMachine,
    hosts: &[Box<dyn Mod>],
    manifests: &[PackageManifest],
    side: Side,
) {
    assert_eq!(hosts.len(), manifests.len());
    assert_running(machine, hosts.iter().map(Box::as_ref), side);
    machine
        .world_access(&ravel_observability::PreWorldReport::new(
            ravel_bootstrap_runtime::environment(),
        ))
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

pub(super) fn assert_dependency_order(
    hosts: &[Box<dyn Mod>],
    manifests: &[PackageManifest],
    side: Side,
) {
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
