//! The interface sprite set, and the one atlas document that stitches it.
//!
//! §8 of the specification counts twenty-one interface images in one asset
//! namespace — five pointers, six widget parts and ten frame parts — and one
//! atlas document declaring a single directory source stitched under a prefix.
//! The images were listed and counted from the archive's table of contents and
//! none was opened, so what is observed is the set, the grouping and the naming
//! shape; that the widget and frame images are scaled rather than drawn at a
//! fixed size is marked inferred there and is marked inferred here.
//!
//! **No image is shipped, and nothing blocks one.** §11 item 3 asks for an
//! equivalent set drawn fresh, `policies/source-access.json` permits shipping
//! art rewritten with the originals as reference, and Ravel needs no seam for a
//! `.png` under this package's own assets. The set is simply undone. So this
//! module names the paths a drawing would land on and generates the atlas
//! document that would stitch them, and no bytes sit behind any of the
//! twenty-one names: [`atlas_document`] declares a directory source over a
//! directory holding no image, and a dependent addressing a stitched sprite by
//! name today resolves to nothing. `NOTES.md` says so under what is not
//! provided, and it is the one item of §11's first five this package lacks.

use ravel_sdk::resources::{RelativePath, RelativePathError};

use crate::MOD_ID;

/// The directory under the namespace's textures the atlas stitches.
pub const ATLAS_SOURCE: &str = "gui";

/// The prefix the stitched sprites are addressed under.
pub const ATLAS_PREFIX: &str = "gui/";

/// The five pointer sprites.
pub const POINTERS: [&str; 5] = [
    "pointer_resize_vertical",
    "pointer_resize_horizontal",
    "pointer_resize_diagonal_falling",
    "pointer_resize_diagonal_rising",
    "pointer_drag",
];

/// The six widget sprites.
pub const WIDGETS: [&str; 6] = [
    "energy_bar_empty",
    "energy_bar_full",
    "slot_small",
    "slot_large",
    "progress_arrow_empty",
    "progress_arrow_full",
];

/// The ten frame sprites: two panel backgrounds and eight button states.
pub const FRAMES: [&str; 10] = [
    "panel_bordered",
    "panel_borderless",
    "button_bordered_idle",
    "button_bordered_pressed",
    "button_bordered_highlighted",
    "button_bordered_disabled",
    "button_borderless_idle",
    "button_borderless_pressed",
    "button_borderless_highlighted",
    "button_borderless_disabled",
];

/// How many sprites the set holds.
pub const SPRITE_COUNT: usize = POINTERS.len() + WIDGETS.len() + FRAMES.len();

/// Returns every sprite name, pointers then widgets then frames.
#[must_use]
pub fn names() -> Vec<&'static str> {
    POINTERS
        .iter()
        .chain(WIDGETS.iter())
        .chain(FRAMES.iter())
        .copied()
        .collect()
}

/// Returns whether a drawing of `name` is scaled rather than drawn at its own
/// size.
///
/// **Inferred.** §8 reads the widget and frame images as nine-slice shaped from
/// their naming and from the atlas's dynamic grouping, and says plainly that no
/// image was opened to confirm it. A pointer is a cursor and is not scaled.
#[must_use]
pub fn scales(name: &str) -> bool {
    WIDGETS.contains(&name) || FRAMES.contains(&name)
}

/// Returns the path the atlas document lands on.
///
/// # Errors
///
/// Returns [`RelativePathError`] when the derived path is illegal, which for
/// these constants would be a packaging failure.
pub fn atlas_path() -> Result<RelativePath, RelativePathError> {
    RelativePath::try_from(format!("assets/{MOD_ID}/atlases/{ATLAS_SOURCE}.json"))
}

/// Returns the path one sprite's image would land on.
///
/// # Errors
///
/// Returns [`RelativePathError`] when the derived path is illegal.
pub fn sprite_path(name: &str) -> Result<RelativePath, RelativePathError> {
    RelativePath::try_from(format!(
        "assets/{MOD_ID}/textures/{ATLAS_SOURCE}/{name}.png"
    ))
}

/// Returns the path of every sprite in the set, in [`names`] order.
///
/// # Errors
///
/// Returns [`RelativePathError`] when a derived path is illegal.
pub fn sprite_paths() -> Result<Vec<RelativePath>, RelativePathError> {
    names().into_iter().map(sprite_path).collect()
}

/// Returns the atlas document, which declares one directory source.
///
/// One source, as §8 observes: the directory the sprites sit in, stitched under
/// a prefix, so a dependent addresses a sprite by its stitched name rather than
/// by a path.
///
/// The directory holds no image today, so this document stitches an empty set.
/// It is the shape a drawn set would land in rather than a working atlas, and
/// this module's own documentation says which.
#[must_use]
pub fn atlas_document() -> Vec<u8> {
    format!(
        "{{\n  \"sources\": [\n    {{\n      \"type\": \"directory\",\n      \"source\": \"{ATLAS_SOURCE}\",\n      \"prefix\": \"{ATLAS_PREFIX}\"\n    }}\n  ]\n}}\n"
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::{
        ATLAS_PREFIX, ATLAS_SOURCE, FRAMES, POINTERS, SPRITE_COUNT, WIDGETS, atlas_document,
        atlas_path, names, scales, sprite_paths,
    };
    use crate::MOD_ID;
    use std::collections::BTreeSet;

    #[test]
    fn twenty_one_sprites_are_named_across_the_three_groups() {
        assert_eq!(POINTERS.len(), 5);
        assert_eq!(WIDGETS.len(), 6);
        assert_eq!(FRAMES.len(), 10);
        assert_eq!(SPRITE_COUNT, 21);
        assert_eq!(names().len(), 21);
        let distinct: BTreeSet<&str> = names().into_iter().collect();
        assert_eq!(distinct.len(), 21);
    }

    #[test]
    fn eight_of_the_ten_frame_sprites_are_button_states_across_two_treatments() {
        let buttons: Vec<&&str> = FRAMES
            .iter()
            .filter(|name| name.starts_with("button_"))
            .collect();
        assert_eq!(buttons.len(), 8);
        for treatment in ["bordered", "borderless"] {
            for state in ["idle", "pressed", "highlighted", "disabled"] {
                let wanted = format!("button_{treatment}_{state}");
                assert!(
                    FRAMES.contains(&wanted.as_str()),
                    "{wanted} is one of the eight states"
                );
            }
        }
        assert_eq!(
            FRAMES
                .iter()
                .filter(|name| name.starts_with("panel_"))
                .count(),
            2
        );
    }

    #[test]
    fn the_sixteen_widget_and_frame_sprites_scale_and_the_five_pointers_do_not() {
        assert_eq!(names().into_iter().filter(|name| scales(name)).count(), 16);
        for pointer in POINTERS {
            assert!(!scales(pointer));
        }
    }

    #[test]
    fn every_sprite_lands_in_the_stitched_directory_under_this_packages_namespace() {
        let paths = sprite_paths().expect("the constants spell legal paths");
        assert_eq!(paths.len(), 21);
        for path in &paths {
            let spelled = path.as_str();
            assert!(spelled.starts_with(&format!("assets/{MOD_ID}/textures/{ATLAS_SOURCE}/")));
            assert_eq!(
                std::path::Path::new(spelled).extension(),
                Some(std::ffi::OsStr::new("png"))
            );
        }
        let distinct: BTreeSet<&str> = paths
            .iter()
            .map(ravel_sdk::resources::RelativePath::as_str)
            .collect();
        assert_eq!(distinct.len(), 21);
    }

    #[test]
    fn the_atlas_declares_one_source_and_stitches_it_under_the_prefix() {
        let document = String::from_utf8(atlas_document()).expect("the document is text");
        assert_eq!(document.matches("\"type\"").count(), 1);
        assert!(document.contains(&format!("\"source\": \"{ATLAS_SOURCE}\"")));
        assert!(document.contains(&format!("\"prefix\": \"{ATLAS_PREFIX}\"")));
        assert_eq!(
            atlas_path().expect("a legal path").as_str(),
            format!("assets/{MOD_ID}/atlases/{ATLAS_SOURCE}.json")
        );
    }
}
