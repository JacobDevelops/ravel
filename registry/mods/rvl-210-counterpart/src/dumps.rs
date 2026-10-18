//! Section fourteen: the registry-dumping service.
//!
//! A service collects producers that can each serialise one registry to a
//! document, and a command writes them all out. Seven producers ship and
//! dependents add their own. The purpose is external tooling, so what a pack
//! author depends on is that the set is nameable and that each producer writes
//! a document; the manual engine of the same section is Ravel-drawn if it is
//! built at all and is not here.
//!
//! The section's own service dumps a running pack's registries, which needs a
//! game. What this module can dump without one is this entry's own tables, and
//! the second half of the file does exactly that: [`documents`] renders the
//! declaration census, the command tree and the settings framework out of the
//! functions that already derive them, so a document that disagrees with the
//! crate has drifted rather than stated the same rule twice.
//!
//! Every list below is in an order the crate already fixes -- declaration
//! order, registration order, or the vocabulary's own order -- so nothing here
//! sorts, because a sort would hide the orderings sections 5.1 and 15 make
//! observable.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ravel_sdk::{JsonValue, Side, json};

use crate::{commands, declarations, settings};

/// The producer that writes plain crafting recipes.
pub const CRAFTING: &str = "crafting_recipes";
/// The producer that writes smelting recipes.
pub const SMELTING: &str = "smelting_recipes";
/// The producer that writes blasting recipes.
pub const BLASTING: &str = "blasting_recipes";
/// The producer that writes smoking recipes.
pub const SMOKING: &str = "smoking_recipes";
/// The producer that writes campfire cooking recipes.
pub const CAMPFIRE: &str = "campfire_recipes";
/// The producer that writes the translated display names of every item.
pub const ITEM_NAMES: &str = "item_names";
/// The producer that writes the translated display names of every fluid.
pub const FLUID_NAMES: &str = "fluid_names";

/// The seven producers that ship, in the order the service collects them.
///
/// The plain crafting one, the four cooking variants, and the translated display
/// names of every item and of every fluid.
pub const SHIPPED: [&str; 7] = [
    CRAFTING,
    SMELTING,
    BLASTING,
    SMOKING,
    CAMPFIRE,
    ITEM_NAMES,
    FLUID_NAMES,
];

/// The service that collects producers.
#[derive(Clone, Debug, Default)]
pub struct Producers {
    names: Vec<String>,
}

impl Producers {
    /// A service with nothing collected.
    #[must_use]
    pub const fn new() -> Self {
        Self { names: Vec::new() }
    }

    /// The service holding the seven that ship.
    #[must_use]
    pub fn shipped() -> Self {
        let mut producers = Self::new();
        for name in SHIPPED {
            producers.collect(name);
        }
        producers
    }

    /// Collects one producer. A name already collected is not collected twice.
    ///
    /// Answers whether it was added, so a dependent registering the same
    /// producer twice can see that the second call did nothing.
    pub fn collect(&mut self, name: &str) -> bool {
        if self.names.iter().any(|held| held == name) {
            return false;
        }
        self.names.push(name.to_owned());
        true
    }

    /// Every producer, in the order it was collected.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.names.iter().map(String::as_str).collect()
    }

    /// How many producers are collected.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether nothing is collected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Writes every producer out, which is what the command does.
    ///
    /// `serialise` answers the document one producer writes. A producer that
    /// answers nothing stays absent in the result rather than being substituted
    /// with an empty registry: section fourteen's purpose is external tooling,
    /// and a registry that failed to serialise and one that genuinely holds
    /// nothing are different facts about a pack. Every producer the service
    /// holds is named either way, so the set is still the set.
    #[must_use]
    pub fn write_all(
        &self,
        serialise: &dyn Fn(&str) -> Option<JsonValue>,
    ) -> BTreeMap<String, Option<JsonValue>> {
        self.names
            .iter()
            .map(|name| (name.clone(), serialise(name)))
            .collect()
    }
}

/// Where the declaration census is written, relative to the package root.
pub const CENSUS_PATH: &str = "generated/census.json";

/// Where the command table is written, relative to the package root.
pub const COMMANDS_PATH: &str = "generated/commands.txt";

/// Where the settings document is written, relative to the package root.
pub const SETTINGS_PATH: &str = "generated/settings.json";

/// Every rendered document, each as its relative path and its bytes.
#[must_use]
pub fn documents() -> Vec<(&'static str, String)> {
    vec![
        (CENSUS_PATH, census_document()),
        (COMMANDS_PATH, commands_table()),
        (SETTINGS_PATH, settings_document()),
    ]
}

/// The declaration vocabulary and what this entry declares through it.
///
/// The registration list is [`declarations::Declarations::registration_order`]
/// rather than the declaration list re-typed, so the rule that a kind finishes
/// before the next begins is what produces the order here: the sixteen entries
/// are declared interleaved and come out grouped. The per-side companion lists
/// are [`declarations::Declarations::companions_on`], so the declaration-time
/// side split is visible as two different lists rather than asserted in prose.
#[must_use]
pub fn census_document() -> String {
    let declared = declarations::own_declarations();
    let entries: Vec<JsonValue> = declared
        .declared()
        .iter()
        .map(|declaration| {
            json!({
                "kind": declaration.kind().as_str(),
                "name": declaration.name(),
                "disableable": declaration.disableable(),
                "companion": declaration.companion(),
            })
        })
        .collect();
    let registered: Vec<JsonValue> = declared
        .registration_order()
        .into_iter()
        .map(|declaration| {
            json!({
                "kind": declaration.kind().as_str(),
                "name": declaration.name(),
            })
        })
        .collect();
    let companions: BTreeMap<&str, Vec<&str>> = Side::ALL
        .into_iter()
        .map(|side| (side.as_str(), declared.companions_on(side)))
        .collect();
    let document = json!({
        "kinds": declarations::DeclarationKind::ALL.map(declarations::DeclarationKind::as_str).as_slice(),
        "kindCount": declarations::DeclarationKind::ALL.len(),
        "declared": entries,
        "declaredCount": declared.declared().len(),
        "registrationOrder": registered,
        "registeredCount": registered.len(),
        "companions": companions,
        "claimedKinds": declarations::CLAIMED_KINDS
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<&str>>(),
    });
    format!("{document}\n")
}

/// The seven subcommands as a tab-separated table.
///
/// The arity column comes from [`commands::SUBCOMMANDS`] and the inherited column is
/// membership of [`commands::inherited`], so the two facts a dependent observes
/// -- that it gets two subcommands it never declared, and how many arguments
/// each name takes -- are computed rather than restated.
#[must_use]
pub fn commands_table() -> String {
    let inherited = commands::inherited();
    let mut rendered = String::from("subcommand\tarity\tinherited\n");
    for (name, _, arity) in commands::SUBCOMMANDS {
        let inherited = inherited.contains(&name);
        writeln!(rendered, "{name}\t{arity}\t{inherited}")
            .expect("writing to a String cannot fail");
    }
    rendered
}

/// This entry's two settings as the framework sees them.
///
/// The `resolvedBy` list per setting is [`settings::Scope::resolved_by`] run
/// over every side, and the per-document lists are
/// [`settings::Framework::document`], so the scope rule appears as the sides and
/// documents it actually produces.
#[must_use]
pub fn settings_document() -> String {
    let framework = settings::own_framework();
    let declared: Vec<JsonValue> = framework
        .declared()
        .iter()
        .map(|declared| {
            let sides: Vec<&str> = Side::ALL
                .into_iter()
                .filter(|side| declared.scope().resolved_by(*side))
                .map(Side::as_str)
                .collect();
            json!({
                "property": declared.property(),
                "category": declared.category(),
                "comment": declared.comment(),
                "document": declared.document(),
                "changeable": declared.changeable(),
                "valueKind": declared.value().kind(),
                "value": declared.value().to_string(),
                "resolvedBy": sides,
            })
        })
        .collect();
    let documents: BTreeMap<&str, Vec<&str>> = [
        settings::Scope::Client,
        settings::Scope::Server,
        settings::Scope::Common,
    ]
    .into_iter()
    .map(|scope| {
        (
            scope.document(),
            framework
                .document(scope)
                .into_iter()
                .map(settings::Declared::property)
                .collect(),
        )
    })
    .collect();
    let document = json!({
        "grouping": settings::OWN_GROUPING,
        "keys": settings::OWN_KEYS,
        "properties": framework.properties(),
        "declared": declared,
        "documents": documents,
    });
    format!("{document}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_producers_ship_and_each_is_named_once() {
        let mut sorted = SHIPPED.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 7);
        assert_eq!(Producers::shipped().len(), 7);
    }

    #[test]
    fn a_dependent_adds_its_own_and_the_order_of_collection_is_kept() {
        let mut producers = Producers::shipped();
        assert!(producers.collect("mine"));
        assert_eq!(producers.len(), 8);
        assert_eq!(producers.names().last(), Some(&"mine"));
        assert_eq!(producers.names().first(), Some(&CRAFTING));
    }

    #[test]
    fn a_producer_collected_twice_is_held_once() {
        let mut producers = Producers::shipped();
        assert!(!producers.collect(CRAFTING));
        assert_eq!(producers.len(), 7);
    }

    #[test]
    fn writing_them_all_out_yields_one_document_for_every_producer() {
        let producers = Producers::shipped();
        let written = producers.write_all(&|name| Some(json!({ "registry": name })));
        assert_eq!(written.len(), 7);
        assert_eq!(written[CRAFTING], Some(json!({ "registry": CRAFTING })));
    }

    #[test]
    fn a_producer_that_writes_nothing_is_named_with_nothing_rather_than_an_empty_registry() {
        let producers = Producers::shipped();
        let written = producers.write_all(&|name| (name != FLUID_NAMES).then(|| json!({})));
        assert_eq!(
            written.len(),
            7,
            "every producer the service holds is named"
        );
        assert_eq!(
            written[FLUID_NAMES], None,
            "an empty registry here would reach a pack author's tooling as a \
             registry that genuinely holds nothing"
        );
        assert_eq!(written[CRAFTING], Some(json!({})));
    }

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let rendered = documents();
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
