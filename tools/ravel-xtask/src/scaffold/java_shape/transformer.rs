//! Injected transformer instruction contracts.

use std::fs;
use std::path::Path;

use ravel_minecraft_generate::Kind;

use super::{AGENT_JAR_SOURCES, TRANSFORMER_SHAPE_RECORD};
use crate::hooks::HookInventory;
use crate::text::read_text;
use crate::version_boundary::VersionAdapters;

fn uses_modern_bootstrap(adapters: &VersionAdapters, inventory: &HookInventory) -> bool {
    adapters
        .get(&inventory.version)
        .is_some_and(|facts| !facts.legacy_adapter)
}

#[derive(Clone, Copy)]
enum Parameter {
    At(usize),
    Last,
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Reference,
    Boolean,
    Int,
    Void,
}

struct Seam {
    id: &'static str,
    receiver: bool,
    loads: &'static [(Parameter, Shape)],
    returns: &'static [Shape],
}

const TRANSFORMER_SEAMS: [Seam; 5] = [
    Seam {
        id: "world.open-persistence",
        receiver: true,
        loads: &[
            (Parameter::At(0), Shape::Reference),
            (Parameter::At(2), Shape::Reference),
            (Parameter::Last, Shape::Boolean),
        ],
        returns: &[Shape::Void],
    },
    Seam {
        id: "world.create-pack-repository",
        receiver: false,
        loads: &[(Parameter::At(0), Shape::Reference)],
        returns: &[],
    },
    Seam {
        id: "screen.create-world-init",
        receiver: true,
        loads: &[],
        returns: &[],
    },
    Seam {
        id: "world.save-persistence",
        receiver: true,
        loads: &[],
        returns: &[],
    },
    Seam {
        id: "screen.title-menu-options",
        receiver: true,
        loads: &[
            (Parameter::At(0), Shape::Int),
            (Parameter::At(1), Shape::Int),
        ],
        returns: &[Shape::Int, Shape::Void],
    },
];

const MENU_ROW_CONSTANT: &str = "MenuLayout.RAVEL_ROW";

const MENU_COLUMN_BRIDGE: &str = "addTitleMenuColumn";

pub(crate) fn verify_menu_row_placement(root: &Path, violations: &mut Vec<String>) {
    let path = root
        .join(AGENT_JAR_SOURCES)
        .join("dev/ravel/jvm/Transformer.java");
    let source = match read_text(root, &path) {
        Ok(source) => source,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(at) = source.find(MENU_COLUMN_BRIDGE) else {
        violations.push(format!(
            "Transformer.java declares no {MENU_COLUMN_BRIDGE}, so the column shape that hands \
             back no row has no bridge and this check has no subject"
        ));
        return;
    };
    let body = &source[at..];
    let end = body.find("\n    }").map_or(body.len(), |end| end + 1);
    let body = &body[..end];
    if !body.contains(MENU_ROW_CONSTANT) {
        violations.push(format!(
            "Transformer.java's {MENU_COLUMN_BRIDGE} does not place from {MENU_ROW_CONSTANT}; a \
             row worked out from the column's own base and spacing is one version's layout \
             asserted over the rest, and it draws Ravel's row on top of the game's own options"
        ));
    }
}

pub(crate) fn verify_transformer_shape(
    root: &Path,
    inventories: &[HookInventory],
    adapters: &VersionAdapters,
    violations: &mut Vec<String>,
) {
    let inventories: Vec<_> = inventories
        .iter()
        .filter(|inventory| uses_modern_bootstrap(adapters, inventory))
        .collect();
    if inventories.is_empty() {
        return;
    }
    let mut held = Vec::new();
    let mut named = 0;
    for inventory in inventories {
        let before = violations.len();
        let mut seams = 0;
        for seam in &TRANSFORMER_SEAMS {
            let Some((kind, descriptor)) = declared_hook(inventory, seam.id) else {
                continue;
            };
            seams += 1;
            named += 1;
            verify_seam_shape(inventory, seam, kind, descriptor, violations);
        }
        if violations.len() == before && seams > 0 {
            held.push(format!(
                "held: Minecraft {} against {seams} transformer seams",
                inventory.version
            ));
        }
    }

    if named == 0 {
        violations.push(format!(
            "no tracked version declares a seam {AGENT_JAR_SOURCES}'s transformer injects into, \
             so its parameter positions and return kinds were checked against nothing"
        ));
    }

    let record = root.join(TRANSFORMER_SHAPE_RECORD);
    let written = record
        .parent()
        .map_or_else(|| Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&record, held.join("\n") + "\n"));
    if let Err(error) = written {
        violations.push(format!(
            "cannot record the verified transformer shapes in \
             {TRANSFORMER_SHAPE_RECORD}: {error}"
        ));
    }
}

fn verify_seam_shape(
    inventory: &HookInventory,
    seam: &Seam,
    kind: Kind,
    descriptor: &str,
    violations: &mut Vec<String>,
) {
    let path = &inventory.path;
    let id = seam.id;
    if seam.receiver && kind != Kind::InstanceMethod {
        violations.push(format!(
            "{path} declares {id} as a {kind} and the transformer loads its receiver; local zero \
             is the first argument on a static method, so the injected call would hand the \
             adapter an argument in place of the object the seam is about",
            kind = kind.as_str()
        ));
    }

    let Ok(parameters) = ravel_minecraft_generate::descriptor_parameters(descriptor) else {
        violations.push(format!(
            "{path} declares {id} at `{descriptor}`, which is not a method descriptor, so the \
             transformer's slots are derived from nothing"
        ));
        return;
    };

    for (position, shape) in seam.loads {
        let (index, declared) = match position {
            Parameter::At(index) => (*index, parameters.get(*index)),
            Parameter::Last => (parameters.len().saturating_sub(1), parameters.last()),
        };
        let Some(declared) = declared else {
            violations.push(format!(
                "{path} declares {id} with {} parameters and the transformer loads parameter \
                 {index}; a load past the declared list is a local the method never received",
                parameters.len()
            ));
            continue;
        };
        if !shape.matches(declared) {
            violations.push(format!(
                "{path} declares parameter {index} of {id} as `{declared}` and the transformer \
                 loads {} there; the load and the local disagree, which the class file verifier \
                 refuses at load time rather than anything here refusing at build time",
                shape.instruction()
            ));
        }
    }

    if !seam.returns.is_empty() {
        let returned = descriptor
            .split_once(')')
            .map_or(descriptor, |(_, returned)| returned);
        if !seam.returns.iter().any(|shape| shape.matches(returned)) {
            violations.push(format!(
                "{path} declares {id} returning `{returned}` and the transformer injects at its \
                 return for {}; a sequence written for another return kind either matches no \
                 return instruction and is silently never applied, or leaves the stack the \
                 verifier refuses",
                seam.returns
                    .iter()
                    .copied()
                    .map(Shape::instruction)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        }
    }
}

pub(super) fn declared_hook<'a>(inventory: &'a HookInventory, id: &str) -> Option<(Kind, &'a str)> {
    let hook = inventory.hooks.iter().find(|hook| hook.id == id)?;
    Some((hook.kind, &hook.descriptor))
}

impl Shape {
    fn matches(self, declared: &str) -> bool {
        match self {
            Self::Reference => declared.starts_with('L') || declared.starts_with('['),
            Self::Boolean => declared == "Z",
            Self::Int => declared == "I",
            Self::Void => declared == "V",
        }
    }

    fn instruction(self) -> &'static str {
        match self {
            Self::Reference => "a reference",
            Self::Boolean => "a boolean",
            Self::Int => "an int",
            Self::Void => "nothing",
        }
    }
}
