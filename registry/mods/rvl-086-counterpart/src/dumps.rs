//! The derived documents this package writes beside its source.
//!
//! Nothing here is a new fact. Each document is a rendering of a table the crate
//! already computes — the registration census of [`crate::census`], the packaged
//! settings declaration of [`crate::settings`], and the surface-by-shape matrix
//! of [`crate::refusal`] — written out so a reader can see what the code decides
//! without running it. The test beside them rebuilds every one and compares it
//! against the bytes on disk, so a table that moves and a document that does not
//! is a failing test rather than a stale file.

use std::fmt::Write as _;

use ravel_sdk::{JsonValue, ModError, json};

use crate::census;
use crate::refusal::Surface;
use crate::repair::Shape;
use crate::settings;

/// Where the registration census is written.
pub const CENSUS_PATH: &str = "generated/census.json";

/// Where the settings table is written.
pub const SETTINGS_PATH: &str = "generated/settings.txt";

/// Where the refusal matrix is written.
pub const REFUSAL_PATH: &str = "generated/refusal.txt";

/// Every generated document, as a path relative to the package root and the
/// bytes that belong at it.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when the packaged settings document does not parse or a
/// census gate is not a legal flag identity.
pub fn documents() -> Result<Vec<(&'static str, String)>, ModError> {
    Ok(vec![
        (CENSUS_PATH, census()?),
        (SETTINGS_PATH, settings()?),
        (REFUSAL_PATH, refusal()),
    ])
}

/// The registration census: what is claimed, in what order, what gates each
/// listed entry, what shape each item is, and what colour each variant carries.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when a gate named in the census tables is not a legal
/// flag identity under this package's own namespace.
pub fn census() -> Result<String, ModError> {
    let claimed: Vec<JsonValue> = census::claims()
        .into_iter()
        .map(|(kind, name)| json!({"kind": kind.as_str(), "name": name}))
        .collect();
    let grouping = census::grouping(crate::FLAG_NAMESPACE).map_err(ModError::from_display)?;
    let listed: Vec<JsonValue> = grouping
        .declared()
        .iter()
        .map(|entry| {
            json!({
                "entry": entry.entry,
                "gates": entry
                    .gates
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let shapes: Vec<JsonValue> = census::item_names()
        .into_iter()
        .zip(census::item_shapes())
        .map(|(name, shape)| {
            json!({
                "item": name,
                "shape": shape.map(Shape::as_str),
            })
        })
        .collect();
    let tints: Vec<JsonValue> = (0..census::TINTED_VARIANTS)
        .filter_map(|variant| {
            let colour = census::tint(variant)?;
            Some(json!({"variant": variant, "packed": colour.pack()}))
        })
        .collect();

    let document = json!({
        "blocks": census::BLOCKS,
        "items": census::ITEMS,
        "claimed": claimed,
        "listed": listed,
        "shapes": shapes,
        "tints": tints,
    });
    Ok(format!("{document}\n"))
}

/// The settings table: every declared setting with the side it applies to, the
/// group it belongs to, its packaged default, and the flags bound to it.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when the packaged document does not parse.
///
/// # Panics
///
/// Panics if formatting into a string fails.
pub fn settings() -> Result<String, ModError> {
    let declaration = settings::declaration().map_err(ModError::from_display)?;
    let mut rendered = String::from("key\tside\tgroup\trestart\tdefault\tflags\n");
    for setting in declaration.settings() {
        let key = setting.key.as_str();
        let group = if settings::DECIDING_KEYS.contains(&key) {
            "deciding"
        } else {
            "presentation"
        };
        let bound: Vec<&str> = settings::FLAG_BINDINGS
            .iter()
            .filter(|(_, feature)| feature.key() == key)
            .map(|(flag, _)| flag.name())
            .collect();
        let bound = if bound.is_empty() {
            "-".to_owned()
        } else {
            bound.join(",")
        };
        let side = setting.side.as_str();
        writeln!(
            rendered,
            "{key}\t{side}\t{group}\t{}\t{}\t{bound}",
            setting.restart_required, setting.default
        )
        .expect("writing into a String cannot fail");
    }
    Ok(rendered)
}

/// The refusal matrix: which of the seven surfaces the nearly-destroyed rule
/// watches for each of the six shapes.
///
/// # Panics
///
/// Panics if formatting into a string fails.
#[must_use]
pub fn refusal() -> String {
    let mut rendered = String::from("surface");
    for shape in Shape::ALL {
        write!(rendered, "\t{}", shape.as_str()).expect("writing into a String cannot fail");
    }
    rendered.push('\n');
    for surface in Surface::ALL {
        write!(rendered, "{}", surface.as_str()).expect("writing into a String cannot fail");
        for shape in Shape::ALL {
            rendered.push('\t');
            rendered.push_str(if surface.watches(shape) {
                "watches"
            } else {
                "-"
            });
        }
        rendered.push('\n');
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::documents;

    fn render_documents() -> Vec<(&'static str, String)> {
        documents().expect("the packaged declarations render")
    }

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let rendered = render_documents();
        assert!(!rendered.is_empty(), "the package renders documents");
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        for (relative, document) in rendered {
            assert!(document.ends_with('\n'), "{relative} must end in a newline");
            let path = root.join(relative);
            if regenerating {
                let parent = path.parent().expect("a document sits in a directory");
                std::fs::create_dir_all(parent).expect("the generated directory is writable");
                std::fs::write(&path, document.as_bytes()).expect("the document is writable");
                continue;
            }
            let on_disk = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} is not on disk: {error}"));
            assert_eq!(
                on_disk, document,
                "{relative} has drifted from the functions that render it"
            );
        }
    }
}
