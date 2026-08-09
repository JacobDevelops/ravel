use ravel_core::ModId;
use ravel_package::PackageManifest;
use ravel_resolve::LoadPlan;

use crate::lifecycle::{LifecycleEntry, Requirement};

/// Turns a validated package set and its resolved plan into lifecycle entries.
///
/// This is the one place a manifest's `sides` declaration becomes the thing the
/// lifecycle reads. It is production code on purpose: before this existed the
/// mapping lived in `ravel-testkit`, so the only consumer of the field a mod
/// author writes was test support, and replacing that one line with "every side"
/// left the whole workspace green.
///
/// # Ordering
///
/// The plan's mods come first, in load order, because that order is
/// `SPEC.md` §14's determinism requirement and the lifecycle honours the
/// sequence it is given. Every installed package the plan does not order follows,
/// in mod-id order, so the result is a property of the package set alone.
///
/// # Why a package the plan excluded is still admitted
///
/// [`ravel_resolve::resolve`] drops a package that does not load on the side
/// being resolved, so the plan holds only mods that run. Admitting nothing else
/// would make an installed client-only mod *absent* on a server rather than
/// **parked**, and the two are different claims: a player who installed it
/// should see it listed and marked as not loading here — which is what
/// [`crate::ModSurface`] draws — and the lifecycle should refuse to run its code
/// rather than never having heard of it. Parking is also the enforcement
/// `SPEC.md` §14 asks for at the load boundary, and it can only fire for a mod
/// the machine holds.
///
/// A manifest the plan names but the set does not contain is skipped, because a
/// plan is only ever built from a package set and an identity in one and not the
/// other is not a state this can reach.
#[must_use]
pub fn admitted(plan: &LoadPlan, manifests: &[PackageManifest]) -> Vec<LifecycleEntry> {
    let mut entries: Vec<LifecycleEntry> = plan
        .order()
        .iter()
        .filter_map(|id| find(manifests, id))
        .map(entry)
        .collect();

    let mut parked: Vec<&PackageManifest> = manifests
        .iter()
        .filter(|manifest| !plan.order().contains(&manifest.id))
        .collect();
    parked.sort_by(|left, right| left.id.cmp(&right.id));
    entries.extend(parked.into_iter().map(entry));

    entries
}

fn find<'a>(manifests: &'a [PackageManifest], id: &ModId) -> Option<&'a PackageManifest> {
    manifests.iter().find(|manifest| manifest.id == *id)
}

/// Reads the fields the lifecycle needs off one manifest.
///
/// Every mod a plan orders is [`Requirement::Required`]: the resolver has
/// already dropped an optional dependency that was absent, so everything that
/// reached a validated package set is installed and expected to load, and
/// treating any of it as droppable would grant world access to a set that lost a
/// mod the player installed.
fn entry(manifest: &PackageManifest) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: manifest.id.clone(),
        sides: manifest.sides.clone(),
        requirement: Requirement::Required,
        saved_data_versions: manifest.saved_data_versions.iter().copied().collect(),
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{Side, SideSet};
    use ravel_package::PackageManifest;
    use ravel_resolve::resolve;

    use super::admitted;

    /// A manifest declaring `sides`, written as the document a package ships.
    ///
    /// Parsing the text rather than building the value is what makes this a test
    /// about a manifest: the `sides` list travels the same route an author's
    /// does, through the schema and into the parsed type.
    fn manifest(id: &str, sides: &[&str]) -> PackageManifest {
        let sides = sides
            .iter()
            .map(|side| format!("\"{side}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let document = format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{id}",
              "version": "0.0.0",
              "compatibility": {{ "ravel": ">=0.0.0", "minecraft": ["test-version"] }},
              "sides": [{sides}],
              "authors": [{{ "name": "Ravel" }}],
              "license": "MIT",
              "source": {{
                "repository": "https://example.invalid/r",
                "files": [{{
                  "path": "src/lib.rs",
                  "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
                }}]
              }},
              "provenance": {{
                "record": "provenance.json",
                "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
              }}
            }}"#
        );
        PackageManifest::parse(&document).expect("a legal manifest")
    }

    fn sides(entries: &[super::LifecycleEntry], id: &str) -> SideSet {
        entries
            .iter()
            .find(|entry| entry.mod_id.as_str() == id)
            .expect("the set admitted it")
            .sides
            .clone()
    }

    #[test]
    fn every_entry_carries_the_side_declaration_its_manifest_made() {
        let manifests = [
            manifest("client_side", &["client"]),
            manifest(
                "every_side",
                &["client", "integrated-server", "dedicated-server"],
            ),
        ];
        let plan = resolve(&manifests, Side::Client).expect("both load on the client");
        let entries = admitted(&plan, &manifests);

        assert_eq!(
            sides(&entries, "client_side"),
            SideSet::new([Side::Client]).expect("one side"),
            "a narrower declaration was widened on the way in"
        );
        assert_eq!(
            sides(&entries, "every_side"),
            SideSet::new(Side::ALL).expect("three sides")
        );
    }

    #[test]
    fn a_package_the_plan_excluded_is_admitted_and_keeps_its_narrower_declaration() {
        let manifests = [
            manifest("client_side", &["client"]),
            manifest(
                "every_side",
                &["client", "integrated-server", "dedicated-server"],
            ),
        ];
        let plan = resolve(&manifests, Side::DedicatedServer).expect("one loads here");
        assert_eq!(
            plan.order().len(),
            1,
            "the resolver drops the client-only one"
        );

        let entries = admitted(&plan, &manifests);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.mod_id.as_str())
                .collect::<Vec<_>>(),
            ["every_side", "client_side"],
            "the plan's order comes first and the parked package follows it"
        );
        assert!(
            !sides(&entries, "client_side").loads_on(Side::DedicatedServer),
            "the parked entry would run here"
        );
    }

    #[test]
    fn the_same_package_set_is_admitted_the_same_way_every_time() {
        let forward = [
            manifest("a_mod", &["client"]),
            manifest("b_mod", &["dedicated-server"]),
            manifest("c_mod", &["client", "dedicated-server"]),
        ];
        let reversed = [forward[2].clone(), forward[1].clone(), forward[0].clone()];

        for side in Side::ALL {
            let first = admitted(&resolve(&forward, side).expect("a legal set"), &forward);
            let second = admitted(&resolve(&reversed, side).expect("a legal set"), &reversed);
            assert_eq!(first, second, "{}", side.name());
            assert_eq!(first.len(), 3, "every installed package is admitted");
        }
    }
}
