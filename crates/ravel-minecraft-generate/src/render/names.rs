//! Renders the Java names and layout constants used by hand-written adapters.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::GenerateError;
use crate::inputs::{self, Adapter, Inventory, Lookup, Reflection};
use crate::mapping::Mapping;

/// Renders the game names the agent jar's hand-written adapters resolve.
pub(crate) fn game_names(adapter: &Adapter, surface: &Reflection) -> String {
    let mut text = String::with_capacity(surface.classes.len() * 160 + 1024);
    writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
     package dev.ravel.jvm;\n\n\
     /**\n\
     \x20* Game classes the hand-written adapters resolve by name.\n\
     \x20*\n\
     \x20* <p>Every value is the name the JVM links, and every comment is the\n\
     \x20* Mojang name the reviewed surface declared it under. The two are the\n\
     \x20* same under an unobfuscated version and differ under an obfuscated one,\n\
     \x20* which is why no source spells either of them by hand.\n\
     \x20*/\n\
     final class GameNames {{\n",
        adapter.minecraft
    )
    .expect("writing to a String cannot fail");
    for reflected in &surface.classes {
        writeln!(
            text,
            "    /** {}. */\n\
         \x20   static final String {} = \"{}\";\n",
            reflected.class, reflected.name, reflected.runtime
        )
        .expect("writing to a String cannot fail");
    }
    writeln!(
        text,
        "    private GameNames() {{\n\
     \x20   }}\n\
     }}"
    )
    .expect("writing to a String cannot fail");
    text
}

/// Renders the linked members the agent jar's hand-written adapters resolve.
///
/// Values pair linked names and descriptors with `:`; argument-count lookups emit no constant.
///
/// # Errors
///
/// Returns an error for invalid lookups, unresolved members, or duplicate constants.
pub(crate) fn game_members(
    adapter: &Adapter,
    surface: &Reflection,
    mapping: &Mapping,
) -> Result<String, GenerateError> {
    let mut text = String::with_capacity(surface.classes.len() * 200 + 1024);
    writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
     package dev.ravel.jvm;\n\n\
     /**\n\
     \x20* Members of the game classes the hand-written adapters resolve by name.\n\
     \x20*\n\
     \x20* <p>Every value is the linked name and the linked descriptor the JVM\n\
     \x20* declares, separated by a colon, and every comment is the reviewed\n\
     \x20* member the surface declared it under. The descriptor is carried\n\
     \x20* because a name alone does not pick an overload, and under an\n\
     \x20* obfuscated version it does not even pick a method.\n\
     \x20*/\n\
     final class GameMembers {{\n",
        adapter.minecraft
    )
    .expect("writing to a String cannot fail");
    let mut written = BTreeSet::new();
    for reflected in &surface.classes {
        let owner = reflected.class.replace('.', "/");
        for member in &reflected.members {
            let lookup = &member.lookup;
            let Lookup::Descriptor(descriptor) = lookup else {
                continue;
            };
            let (name, linked) = mapping.runtime_member(&owner, &member.name, descriptor)?;
            let constant = member_constant(&reflected.name, &member.name);
            if !written.insert(constant.clone()) {
                return Err(GenerateError::Drift(format!(
                    "{constant} would be written down twice, so one of the two members it names \
                     would be unreachable"
                )));
            }
            writeln!(
                text,
                "    /** {}#{}. */\n\
             \x20   static final String {constant} = \"{name}:{linked}\";\n",
                reflected.class, member.name
            )
            .expect("writing to a String cannot fail");
        }
    }
    writeln!(
        text,
        "    private GameMembers() {{\n\
     \x20   }}\n\
     }}"
    )
    .expect("writing to a String cannot fail");
    Ok(text)
}

/// The constant one reviewed member of one class role is written down under.
fn member_constant(role: &str, member: &str) -> String {
    if member == "<init>" {
        return format!("{role}_NEW");
    }
    let mut text = String::with_capacity(role.len() + member.len() + 8);
    text.push_str(role);
    text.push('_');
    let mut previous = '_';
    for character in member.chars() {
        if character.is_ascii_uppercase() && previous.is_ascii_lowercase() {
            text.push('_');
        }
        text.push(character.to_ascii_uppercase());
        previous = character;
    }
    text
}

/// Renders the title-menu placement this version's column cannot supply.
///
/// Void-returning seams supply no row, so affected title menus need explicit placement.
pub(crate) fn menu_layout(adapter: &Adapter, inventory: &Inventory) -> String {
    let row = inventory
        .hooks
        .iter()
        .find(|hook| hook.id == inputs::TITLE_MENU_SEAM)
        .and_then(|hook| hook.title_menu_row);
    let mut text = String::with_capacity(1024);
    writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
     package dev.ravel.jvm;\n\n\
     /**\n\
     \x20* Where Ravel's own row goes on this version's title menu.\n\
     \x20*\n\
     \x20* <p>Measured down from the top the column builder is handed. It is\n\
     \x20* declared rather than derived because a column builder that returns\n\
     \x20* nothing says nothing about what is free below it, and a title screen\n\
     \x20* that places its own lower row at a constant reserves nothing when a\n\
     \x20* row is added to the column.\n\
     \x20*\n\
     \x20* <p>{} is what this version declares.\n\
     \x20*/\n\
     final class MenuLayout {{\n\n\
     \x20   /**\n\
     \x20    * What a version whose column returns its own row declares.\n\
     \x20    *\n\
     \x20    * <p>Out of the range of any row, because a row may be negative: a\n\
     \x20    * version whose menu has no space below the column has Ravel's row\n\
     \x20    * above it, and an ordinary sentinel would be a legal offset.\n\
     \x20    */\n\
     \x20   static final int DERIVED = Integer.MIN_VALUE;\n\n\
     \x20   /**\n\
     \x20    * The row, or DERIVED where this version's column hands one back.\n\
     \x20    */\n\
     \x20   static final int RAVEL_ROW = {};\n\n\
     \x20   private MenuLayout() {{\n\
     \x20   }}\n\
     }}",
        adapter.minecraft,
        row.map_or_else(
            || "No row, because this version's column hands one back".to_owned(),
            |row| format!("A row {row} pixels below the column top")
        ),
        row.map_or_else(|| "DERIVED".to_owned(), |row| row.to_string())
    )
    .expect("writing to a String cannot fail");
    text
}

/// Renders the screen class names the agent jar loads reflectively.
pub(crate) fn screen_names(adapter: &Adapter, names: &[(String, String)]) -> String {
    let mut text = String::with_capacity(names.len() * 120 + 768);
    writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
     package dev.ravel.jvm;\n\n\
     /**\n\
     \x20* Ravel's own screen classes, as the version they were generated for\n\
     \x20* names them.\n\
     \x20*\n\
     \x20* <p>A screen extends a game class, so it is generated rather than\n\
     \x20* written and it is emitted beside the names that version links. Under a\n\
     \x20* version whose game is in the unnamed package the screens are there too,\n\
     \x20* which is why no source spells one of these by hand.\n\
     \x20*/\n\
     final class ScreenNames {{\n",
        adapter.minecraft
    )
    .expect("writing to a String cannot fail");
    for (constant, binary) in names {
        let simple = binary.rsplit('.').next().unwrap_or(binary);
        writeln!(
            text,
            "    /** Ravel's {simple}, where this version's names put it. */\n\
         \x20   static final String {constant} = \"{binary}\";\n"
        )
        .expect("writing to a String cannot fail");
    }
    writeln!(
        text,
        "    private ScreenNames() {{\n\
     \x20   }}\n\
     }}"
    )
    .expect("writing to a String cannot fail");
    text
}
