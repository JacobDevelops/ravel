//! Renders the Rust target table and version-specific pack format constants.

use std::fmt::Write as _;

use crate::inputs::{Adapter, Inventory, Spelling};
use crate::resolve::Resolved;

use super::shape::constant_name;

/// Renders the Rust target table the adapter includes.
pub(crate) fn bindings(adapter: &Adapter, inventory: &Inventory, resolved: &[Resolved]) -> String {
    let mut text = String::with_capacity(resolved.len() * 320 + 768);
    writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
     //\n\
     // Source of names: {}. Regenerate by building this crate.\n\
     \n\
     use ravel_minecraft::{{HookId, HookKind, HookTarget, PackFormat, PackFormatSpelling}};\n\
     \n\
     /// The Minecraft version every target below was resolved against.\n\
     pub const MINECRAFT_VERSION: &str = \"{}\";\n\
     \n\
     /// Whether the declared adapter capabilities select the legacy game surface.\n\
     pub const LEGACY_ADAPTER: bool = {};\n",
        adapter.minecraft,
        adapter.mapping_source.as_str(),
        adapter.minecraft,
        adapter.is_legacy()
    )
    .expect("writing to a String cannot fail");

    bindings_formats(&mut text, adapter);

    writeln!(
        text,
        "/// Capabilities this version reviewed and deliberately does not carry.\n\
     ///\n\
     /// A caller naming one of these is naming a decision rather than a typo.\n\
     pub const NON_GOALS: [HookId; {}] = [{}];\n",
        inventory.non_goals.len(),
        inventory
            .non_goals
            .iter()
            .map(|non_goal| format!("HookId::new(\"{}\")", non_goal.id))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("writing to a String cannot fail");

    for (hook, target) in inventory.hooks.iter().zip(resolved) {
        writeln!(
            text,
            "/// {}\n\
         pub const {}: HookTarget = HookTarget::new(\n\
         \x20   HookId::new(\"{}\"),\n\
         \x20   \"{}\",\n\
         \x20   \"{}\",\n\
         \x20   \"{}\",\n\
         \x20   HookKind::{},\n\
         );\n",
            hook.capability,
            constant_name(&hook.id),
            target.id,
            target.runtime_class,
            target.runtime_member,
            target.runtime_descriptor,
            hook.kind.variant()
        )
        .expect("writing to a String cannot fail");
    }

    writeln!(
        text,
        "/// Every target the reviewed inventory pinned, in inventory order.\n\
     pub static TARGETS: [HookTarget; {}] = [",
        resolved.len()
    )
    .expect("writing to a String cannot fail");
    for target in resolved {
        writeln!(text, "    {},", constant_name(&target.id))
            .expect("writing to a String cannot fail");
    }
    writeln!(text, "];").expect("writing to a String cannot fail");
    text
}

fn bindings_formats(text: &mut String, adapter: &Adapter) {
    let format = &adapter.pack_format;
    writeln!(
        text,
        "/// What a resource pack this version reads declares as its format.\n\
         pub const RESOURCE_PACK_FORMAT: PackFormat = PackFormat::new({}, {});\n\
         \n\
         /// What a data pack this version reads declares as its format.\n\
         pub const DATA_PACK_FORMAT: PackFormat = PackFormat::new({}, {});\n\
         \n\
         /// The shape this version's own packs write their format in.\n\
         pub const PACK_FORMAT_SPELLING: PackFormatSpelling = PackFormatSpelling::{};\n",
        format.resource,
        format.resource_minor,
        format.data,
        format.data_minor,
        match format.spelling {
            Spelling::PackFormat => "PackFormat",
            Spelling::MinMaxFormat => "MinMaxFormat",
        }
    )
    .expect("writing to a String cannot fail");
}

#[cfg(test)]
mod tests {
    use crate::inputs::Inventory;
    use crate::test_fixtures::legacy_adapter;

    #[test]
    fn adapter_selection_follows_capabilities_not_the_version_label() {
        let mut adapter = legacy_adapter();
        let inventory = Inventory {
            schema_version: 1,
            minecraft: adapter.minecraft.clone(),
            hooks: Vec::new(),
            non_goals: Vec::new(),
        };
        let legacy = super::bindings(&adapter, &inventory, &[]);
        assert!(legacy.contains("pub const LEGACY_ADAPTER: bool = true;"));
        adapter.capabilities.clear();
        let modern = super::bindings(&adapter, &inventory, &[]);
        assert!(modern.contains("pub const LEGACY_ADAPTER: bool = false;"));
    }
}
