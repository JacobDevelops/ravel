//! The tables this library decides, rendered as documents under `generated/`.
//!
//! Three of this package's rules are tables rather than prose: §6's selection
//! of one of five texture slots from three booleans, §8's per-face in-plane
//! basis and the eight samples it places, and §10's slot vocabulary as each of
//! §5's eight kinds admits it. Every one of them is decided in code here and
//! nowhere else, so the readable form of them is derived from the same
//! functions the library runs rather than typed out beside them.
//!
//! Nothing here decides anything. [`documents`] renders what
//! [`crate::quadrant`], [`crate::neighbourhood`], [`crate::slots`] and
//! [`crate::kinds`] already answer, and the test below fails if the tree under
//! `generated/` stops agreeing with them.

use std::fmt::Write as _;

use ravel_sdk::{JsonValue, json};

use crate::kinds::BUILT_IN;
use crate::neighbourhood::{Face, Offset};
use crate::quadrant::select;
use crate::slots::{CELL_BASE, PARTICLE_KEY, PARTICLE_SLOT, Slot};

/// The path the quadrant selection table is written to.
pub const QUADRANT_PATH: &str = "generated/quadrant.txt";

/// The path the face basis and sample table is written to.
pub const NEIGHBOURHOOD_PATH: &str = "generated/neighbourhood.json";

/// The path the per-kind slot vocabulary is written to.
pub const SLOTS_PATH: &str = "generated/slots.json";

/// Every generated document, as a package-relative path and its whole content.
///
/// The order is fixed and every document ends with a trailing newline, so the
/// rendered tree is a function of the crate alone.
#[must_use]
pub fn documents() -> Vec<(&'static str, String)> {
    vec![
        (QUADRANT_PATH, quadrant_table()),
        (NEIGHBOURHOOD_PATH, neighbourhood_document()),
        (SLOTS_PATH, slots_document()),
    ]
}

/// Renders §6's selection rule over all eight input combinations.
///
/// Every row is [`select`] answering, and the two right-hand columns are the
/// answer's own name and number, so a changed rule changes the table.
#[must_use]
pub fn quadrant_table() -> String {
    let mut rendered = String::from("first\tsecond\tdiagonal\tslot\tnumber\n");
    for first in [false, true] {
        for second in [false, true] {
            for diagonal in [false, true] {
                let slot = select(first, second, diagonal);
                let _ = writeln!(
                    rendered,
                    "{first}\t{second}\t{diagonal}\t{}\t{}",
                    slot.declared_name(),
                    slot.slot_number()
                );
            }
        }
    }
    rendered
}

/// Renders §8's in-plane basis and the eight offsets it places per face.
///
/// The faces run in [`Face::ALL`] order, which is the order §10 assigns
/// per-face indices in, and each face's eight samples run in the index order
/// [`Face::orthogonal`] and [`Face::diagonal`] define.
#[must_use]
pub fn neighbourhood_document() -> String {
    let faces: Vec<JsonValue> = Face::ALL
        .into_iter()
        .map(|face| {
            json!({
                "face": face.declared_name(),
                "axis": face.axis().declared_name(),
                "vertical": face.is_vertical(),
                "opposite": face.opposite().declared_name(),
                "normal": offset(face.normal()),
                "right": offset(face.right()),
                "up": offset(face.up()),
                "mirrorsColumn": face.mirrors_column(),
                "mirrorsRow": face.mirrors_row(),
                "orthogonal": (0..4).map(|i| offset(face.orthogonal(i))).collect::<Vec<_>>(),
                "diagonal": (0..4).map(|i| offset(face.diagonal(i))).collect::<Vec<_>>(),
            })
        })
        .collect();
    format!("{}\n", json!({ "faces": faces }))
}

/// Renders §10's slot vocabulary as each of §5's eight kinds admits it.
///
/// The kinds run in [`BUILT_IN`] registration order, which is the order
/// [`crate::kinds::Registry::claim`] resolves a contested identifier in, and
/// each kind's slots run in the order [`crate::kinds::Behaviour::slots`] returns them.
#[must_use]
pub fn slots_document() -> String {
    let kinds: Vec<JsonValue> = BUILT_IN
        .into_iter()
        .map(|(name, behaviour)| {
            json!({
                "kind": name,
                "behaviour": format!("{behaviour:?}"),
                "readsCondition": behaviour.reads_condition(),
                "slots": behaviour
                    .slots()
                    .into_iter()
                    .map(|slot| json!({ "name": slot_name(slot), "number": slot.number() }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    format!(
        "{}\n",
        json!({
            "particle": { "name": PARTICLE_KEY, "number": PARTICLE_SLOT },
            "cellBase": CELL_BASE,
            "kinds": kinds,
        })
    )
}

fn offset(offset: Offset) -> JsonValue {
    json!([offset.x, offset.y, offset.z])
}

/// The name a declaration spells `slot` by, for the slots that have one.
///
/// A cell is addressed by number rather than by name, so it has none.
fn slot_name(slot: Slot) -> Option<&'static str> {
    match slot {
        Slot::Particle => Some(PARTICLE_KEY),
        Slot::Quadrant(quadrant) => Some(quadrant.declared_name()),
        Slot::Column(column) => Some(column.declared_name()),
        Slot::Edge(edge) => Some(edge.declared_name()),
        Slot::Cell(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::documents;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        for (relative, rendered) in documents() {
            let path = root.join(relative);
            if regenerating {
                let directory = path.parent().expect("a document sits in a directory");
                fs::create_dir_all(directory).expect("the generated directory is writable");
                fs::write(&path, rendered.as_bytes()).expect("the document is writable");
                continue;
            }
            let on_disk = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} is not on disk: {error}"));
            assert_eq!(
                on_disk, rendered,
                "{relative} has drifted from the functions it is rendered from"
            );
            assert!(
                rendered.ends_with('\n'),
                "{relative} does not end with a newline"
            );
        }
    }
}
