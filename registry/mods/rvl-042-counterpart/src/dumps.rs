//! The documents this package renders out of its own functions.
//!
//! Nothing here decides anything. Each function walks a domain the crate already
//! fixes and writes down what the crate's own functions answer, so a dependent
//! reading a table is reading the code rather than a transcription of it. The
//! rendered bytes are kept beside the source under `generated/`, and the
//! regeneration test in this module is what stops the two drifting apart.
//!
//! Every document ends with a trailing newline and is deterministic: the domains
//! are ranges and the declaration's own order, neither of which depends on a
//! hash, a clock or a filesystem.

use std::fmt::Write;

use ravel_sdk::Side;
use ravel_sdk::config::DeclarationError;
use ravel_sdk::{JsonValue, json};

use crate::arithmetic::{half_extent, radial_positions, sweep_positions, weapon_radius};
use crate::reusable::{Reusable, UsesLeft};
use crate::settings;

/// The largest declared side length the geometry table walks.
const LARGEST_SIDE: u32 = 13;

/// The largest declared use count the reusable table walks.
const LARGEST_USES: u32 = 8;

/// The largest durability-preserving level the reusable table walks.
const LARGEST_LEVEL: u32 = 4;

/// The origin every rendered position count is measured from.
///
/// Both position functions translate by the origin, so the count is the same
/// wherever it is measured; the origin is fixed so the table is.
const ORIGIN: (i32, i32, i32) = (0, 0, 0);

/// Every document this package renders, as a relative path and its contents.
///
/// # Errors
///
/// Returns whatever [`settings::declaration`] refuses, which for the packaged
/// document is a packaging failure rather than a runtime one.
pub fn documents() -> Result<Vec<(&'static str, String)>, DeclarationError> {
    Ok(vec![
        ("generated/geometry.txt", geometry_table()),
        ("generated/reusable.txt", reusable_table()),
        ("generated/settings.json", settings_document()?),
    ])
}

/// Renders the cube and square a declared side length reaches.
///
/// One row per declared side length, so the truncating half-extent and the areas
/// that follow from it are readable side by side: an even side behaves as the
/// odd number below it, and both rows say so.
#[must_use]
pub fn geometry_table() -> String {
    let mut rendered = String::from("side\textent\tsweep\tradial\tweapon\n");
    for side in 1..=LARGEST_SIDE {
        let extent = half_extent(side);
        let radius = i32::try_from(extent).unwrap_or(i32::MAX);
        let _ = writeln!(
            rendered,
            "{side}\t{extent}\t{}\t{}\t{:.6}",
            sweep_positions(ORIGIN, radius).len(),
            radial_positions(ORIGIN, radius).len(),
            weapon_radius(extent),
        );
    }
    rendered
}

/// Renders what a declared use count leaves a player to read.
///
/// One row per declared use count: whether it is ever spent, the durability one
/// below it, what a fresh entry and a spent one read as, and the probability a
/// use is charged at each durability-preserving level.
#[must_use]
pub fn reusable_table() -> String {
    let mut rendered = String::from("uses\tunlimited\tdurability\tfresh\tspent");
    for level in 0..=LARGEST_LEVEL {
        let _ = write!(rendered, "\tcharge_{level}");
    }
    rendered.push('\n');

    for uses in 0..=LARGEST_USES {
        let reusable = Reusable::new(uses);
        let maximum = reusable.durability();
        let _ = write!(
            rendered,
            "{uses}\t{}\t{}\t{}\t{}",
            reusable.unlimited(),
            maximum.map_or_else(|| "none".to_owned(), |value| value.to_string()),
            left(reusable.uses_left(0)),
            left(reusable.uses_left(maximum.unwrap_or(0))),
        );
        for level in 0..=LARGEST_LEVEL {
            let _ = write!(rendered, "\t{:.6}", reusable.charge_probability(level));
        }
        rendered.push('\n');
    }
    rendered
}

fn left(uses_left: UsesLeft) -> String {
    match uses_left {
        UsesLeft::Unlimited => "unlimited".to_owned(),
        UsesLeft::Last => "last".to_owned(),
        UsesLeft::Remaining(count) => format!("remaining_{count}"),
    }
}

/// Renders the schedule the packaged settings declaration resolves to.
///
/// The sides each setting is resolved on are computed through
/// [`settings::resolved_on`] rather than restated, so a side edited in the
/// packaged document moves this document with it.
///
/// # Errors
///
/// Returns whatever [`settings::declaration`] refuses.
pub fn settings_document() -> Result<String, DeclarationError> {
    let declaration = settings::declaration()?;
    let settings: Vec<JsonValue> = declaration
        .settings
        .iter()
        .map(|setting| {
            let resolved: Vec<&str> = Side::ALL
                .into_iter()
                .filter(|side| settings::resolved_on(setting.key.as_str(), *side))
                .map(Side::name)
                .collect();
            json!({
                "key": setting.key.as_str(),
                "side": setting.side,
                "default": setting.default,
                "restartRequired": setting.restart_required,
                "packOverride": setting.pack_override,
                "overrulesPlayer": setting.pack_override.overrules_player(),
                "obligesPack": setting.pack_override.obliges_pack(),
                "resolvedOn": resolved,
            })
        })
        .collect();

    let document = json!({
        "owner": declaration.owner.as_str(),
        "travelsToClients": settings::travels_to_clients(),
        "settings": settings,
    });
    Ok(format!("{document}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let rendered = documents().expect("the packaged declaration parses");
        assert!(
            !rendered.is_empty(),
            "the package renders at least one document"
        );
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();

        for (relative, document) in rendered {
            assert!(
                document.ends_with('\n'),
                "{relative} ends with a trailing newline"
            );

            let path = root.join(relative);
            if regenerating {
                let parent = path.parent().expect("a document sits in a directory");
                std::fs::create_dir_all(parent).expect("the directory is writable");
                std::fs::write(&path, document.as_bytes()).expect("the document is writable");
                continue;
            }

            let on_disk = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} is missing from the tree: {error}"));
            assert_eq!(
                on_disk, document,
                "{relative} on disk has drifted from what the crate renders"
            );
        }
    }
}
