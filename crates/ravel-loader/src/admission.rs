use ravel_core::{ModId, Version};
use ravel_package::{CompatibilityError, PackageManifest};
use ravel_resolve::LoadPlan;

use crate::lifecycle::{LifecycleEntry, Requirement};

/// Admits compatible packages in load order, followed by parked packages in mod-id order.
///
/// # Errors
///
/// Returns the first incompatible manifest's [`CompatibilityError`], including parked packages.
pub fn admitted(
    plan: &LoadPlan,
    manifests: &[PackageManifest],
    ravel: &Version,
    minecraft: &str,
) -> Result<Vec<LifecycleEntry>, CompatibilityError> {
    for manifest in manifests {
        manifest.compatibility.check_ravel(ravel)?;
        manifest.compatibility.check_minecraft(minecraft)?;
    }

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

    Ok(entries)
}

fn find<'a>(manifests: &'a [PackageManifest], id: &ModId) -> Option<&'a PackageManifest> {
    manifests.iter().find(|manifest| manifest.id == *id)
}

// Installed packages remain required even when the resolver omitted them for this side.
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
    use ravel_core::{Side, SideSet, Version};
    use ravel_package::{CompatibilityError, PackageManifest};
    use ravel_resolve::resolve;

    use super::admitted;

    const GAME: &str = "test-version";

    fn ravel() -> Version {
        Version::new(0, 0, 0)
    }

    fn manifest(id: &str, sides: &[&str]) -> PackageManifest {
        PackageManifest::parse(&manifest_document(id, sides)).expect("a legal manifest")
    }

    fn manifest_document(id: &str, sides: &[&str]) -> String {
        let sides = sides
            .iter()
            .map(|side| format!("\"{side}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
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
        )
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
        let entries = admitted(&plan, &manifests, &ravel(), GAME).expect("a compatible set");

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

        let entries = admitted(&plan, &manifests, &ravel(), GAME).expect("a compatible set");
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
            let first = admitted(
                &resolve(&forward, side).expect("a legal set"),
                &forward,
                &ravel(),
                GAME,
            )
            .expect("a compatible set");
            let second = admitted(
                &resolve(&reversed, side).expect("a legal set"),
                &reversed,
                &ravel(),
                GAME,
            )
            .expect("a compatible set");
            assert_eq!(first, second, "{}", side.as_str());
            assert_eq!(first.len(), 3, "every installed package is admitted");
        }
    }

    #[test]
    fn a_mod_built_for_another_game_version_is_refused_and_names_both() {
        let manifests = [manifest("a_mod", &["client"])];
        let plan = resolve(&manifests, Side::Client).expect("it loads on the client");

        let refused = admitted(&plan, &manifests, &ravel(), "another-version")
            .expect_err("the manifest declares a different game");

        let CompatibilityError::Minecraft { declared, running } = refused else {
            panic!("the refusal should be about the game: {refused:?}");
        };
        assert_eq!(declared, [GAME.to_owned()]);
        assert_eq!(running, "another-version");
    }

    #[test]
    fn a_mod_built_for_another_ravel_version_is_refused_before_any_entry_exists() {
        let manifests = [manifest("a_mod", &["client"])];
        let plan = resolve(&manifests, Side::Client).expect("it loads on the client");
        // The declared range is `>=0.0.0`, which no version is below, so the
        // refusal has to come from a range this manifest could fail. Declaring the
        // narrower one here keeps the fixture honest about which half is tested.
        let narrow = PackageManifest::parse(
            &manifest_document("b_mod", &["client"]).replace(">=0.0.0", ">=9.0.0"),
        )
        .expect("a legal manifest");
        let narrowed = [narrow];
        let narrowed_plan = resolve(&narrowed, Side::Client).expect("it loads on the client");

        assert!(
            admitted(&plan, &manifests, &ravel(), GAME).is_ok(),
            "the wide range admits the running version"
        );
        let refused = admitted(&narrowed_plan, &narrowed, &ravel(), GAME)
            .expect_err("the manifest declares a Ravel version that is not running");

        assert!(
            matches!(refused, CompatibilityError::Ravel { .. }),
            "{refused:?}"
        );
    }
}
