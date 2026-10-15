//! Recipes, ingredients and menu payloads, from §8.
//!
//! Three related shapes, all thin over the host and all serialisation contracts.
//!
//! **Host-blocked.** Ravel publishes no recipe kind registration, no custom
//! ingredient kind and no screen-opening payload, so none of §8 can be made live
//! at the reviewed public revision. What is here is the portable half: the base
//! recipe shape's four deliberate answers, the ingredient complexity flag and
//! the inversion the loader is told, the namespace-scoped lazy registration, and
//! the optional menu payload.

use std::collections::BTreeMap;

use ravel_sdk::JsonValue;

/// The base shape a codec-described recipe has before a dependent overrides it.
///
/// All four answers are deliberate: this shape is for recipes consumed by
/// machinery rather than by a crafting grid. A dependent that wants a
/// crafting-grid recipe overrides all four, which is what this trait's methods
/// are for.
pub trait RecipeShape {
    /// Returns the recipe kind this belongs to.
    fn kind(&self) -> &str;

    /// Returns whether the recipe declares itself special.
    ///
    /// True, so the crafting book does not attempt to show it.
    fn is_special(&self) -> bool {
        true
    }

    /// Returns what assembling produces.
    ///
    /// Nothing, which is the empty item.
    fn assemble(&self, grid: &Grid) -> Option<String> {
        let _ = grid;
        None
    }

    /// Returns the result item shown before assembly.
    ///
    /// Nothing, for the same reason [`RecipeShape::assemble`] produces nothing.
    fn result(&self) -> Option<String> {
        None
    }

    /// Returns whether this recipe accepts a grid of the given size.
    ///
    /// Any size, because a recipe consumed by machinery has no grid.
    fn accepts(&self, width: usize, height: usize) -> bool {
        let _ = (width, height);
        true
    }
}

/// A grid a recipe is offered.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Grid {
    /// How wide the grid is.
    pub width: usize,
    /// How tall the grid is.
    pub height: usize,
    /// What is in each cell, row by row.
    pub cells: Vec<Option<String>>,
}

/// The base recipe, exactly as §8 describes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseRecipe {
    kind: String,
}

impl BaseRecipe {
    /// Names a recipe kind.
    #[must_use]
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
        }
    }
}

impl RecipeShape for BaseRecipe {
    fn kind(&self) -> &str {
        &self.kind
    }
}

/// A codec-described ingredient.
///
/// *Complex* means the answer depends on more than the candidate's item, and it
/// defaults to true. The loader is told the inverse, because what the loader
/// wants to know is whether the ingredient may be optimised into a plain item
/// test — and a complex one must not be. Getting the inversion wrong is a
/// correctness bug that only shows under a pack that uses the optimisation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ingredient {
    id: String,
    complex: bool,
    examples: Vec<String>,
    admitted: Option<Vec<String>>,
}

impl Ingredient {
    /// Declares an ingredient, complex by default.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            complex: true,
            examples: Vec::new(),
            admitted: None,
        }
    }

    /// Declares the ingredient simple, so the loader may optimise it.
    #[must_use]
    pub fn simple(mut self) -> Self {
        self.complex = false;
        self
    }

    /// Declares the example stacks this ingredient matches, for display.
    ///
    /// §8 lists the test and the example stacks as two things a dependent
    /// supplies, so this changes what a tooltip shows and never what
    /// [`Ingredient::admits_item`] answers. The test is
    /// [`Ingredient::admitting`].
    #[must_use]
    pub fn matching<I, S>(mut self, examples: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.examples = examples.into_iter().map(Into::into).collect();
        self
    }

    /// Returns the ingredient's identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns whether the answer depends on more than the candidate's item.
    #[must_use]
    pub const fn is_complex(&self) -> bool {
        self.complex
    }

    /// Returns what the loader is told, which is the inverse of complexity.
    #[must_use]
    pub const fn may_be_optimised(&self) -> bool {
        !self.complex
    }

    /// Returns the example stacks this matches.
    #[must_use]
    pub fn examples(&self) -> &[String] {
        &self.examples
    }

    /// Declares the item-level test: the items this ingredient admits.
    ///
    /// This is the test §8 says the dependent implements, and it is separate
    /// from [`Ingredient::matching`]: a dependent that listed stacks for a
    /// tooltip has not thereby defined what its ingredient accepts.
    #[must_use]
    pub fn admitting<I, S>(mut self, admitted: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.admitted = Some(admitted.into_iter().map(Into::into).collect());
        self
    }

    /// Returns whether a candidate satisfies this ingredient's item-level test.
    ///
    /// Answers `None` when the dependent declared no item-level test at all,
    /// which is the whole of what a complex ingredient does: its answer depends
    /// on more than the candidate's item, so the caller decides. A declared test
    /// answers `Some`, and that is the part an optimising loader would have
    /// used.
    #[must_use]
    pub fn admits_item(&self, candidate: &str) -> Option<bool> {
        self.admitted
            .as_ref()
            .map(|admitted| admitted.iter().any(|item| item == candidate))
    }
}

/// The most namespaces the ingredient registration will hold, and the most
/// ingredients in one namespace.
pub const MAX_INGREDIENTS: usize = 4096;

/// Namespace-scoped ingredient registration.
///
/// The first ingredient registered for a namespace creates that namespace's
/// collection lazily, and every later one joins it. The original's map is not
/// synchronised and is written during load; whether load is single-threaded
/// there is unknown, and was not tested. Here the registration owns itself and a
/// caller that needs it across threads has to say so, which turns the question
/// into the caller's rather than leaving it open.
#[derive(Clone, Debug, Default)]
pub struct Ingredients {
    namespaces: BTreeMap<String, Vec<Ingredient>>,
}

impl Ingredients {
    /// Builds an empty registration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an ingredient under a namespace, creating the namespace's
    /// collection on the first one.
    ///
    /// # Errors
    ///
    /// Returns [`ShapeError::Full`] at [`MAX_INGREDIENTS`] namespaces or
    /// ingredients in one namespace, and [`ShapeError::Duplicate`] for an
    /// identifier already registered under the namespace.
    pub fn register(&mut self, namespace: &str, ingredient: Ingredient) -> Result<(), ShapeError> {
        if !self.namespaces.contains_key(namespace) && self.namespaces.len() >= MAX_INGREDIENTS {
            return Err(ShapeError::Full(MAX_INGREDIENTS));
        }
        let held = self.namespaces.entry(namespace.to_owned()).or_default();
        if held.len() >= MAX_INGREDIENTS {
            return Err(ShapeError::Full(MAX_INGREDIENTS));
        }
        if held.iter().any(|known| known.id() == ingredient.id()) {
            return Err(ShapeError::Duplicate(ingredient.id().to_owned()));
        }
        held.push(ingredient);
        Ok(())
    }

    /// Returns a namespace's ingredients, in registration order.
    #[must_use]
    pub fn in_namespace(&self, namespace: &str) -> &[Ingredient] {
        self.namespaces.get(namespace).map_or(&[], Vec::as_slice)
    }

    /// Returns the namespaces that have a collection.
    #[must_use]
    pub fn namespaces(&self) -> Vec<&str> {
        self.namespaces.keys().map(String::as_str).collect()
    }

    /// Returns whether a namespace has a collection yet.
    #[must_use]
    pub fn has_namespace(&self, namespace: &str) -> bool {
        self.namespaces.contains_key(namespace)
    }
}

/// Why a shape refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapeError {
    /// A collection already holds the ceiling.
    Full(usize),
    /// An identifier is registered twice under one namespace.
    Duplicate(String),
    /// A menu payload's bytes do not decode.
    Undecodable,
}

impl std::fmt::Display for ShapeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(ceiling) => write!(formatter, "a collection holds {ceiling} already"),
            Self::Duplicate(id) => write!(formatter, "`{id}` is registered twice"),
            Self::Undecodable => write!(formatter, "a menu payload does not decode"),
        }
    }
}

impl std::error::Error for ShapeError {}

/// What a screen may be opened with.
///
/// The server writes the payload when opening; the client decodes it into an
/// optional. Decoding may yield nothing and the screen must handle that: an
/// absent payload is the declared state when no serialiser was supplied at all.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuProvider {
    serialised: bool,
    resets_pointer: bool,
}

impl Default for MenuProvider {
    fn default() -> Self {
        Self::without_payload()
    }
}

impl MenuProvider {
    /// A provider that supplies no serialiser, so every opening carries nothing.
    #[must_use]
    pub const fn without_payload() -> Self {
        Self {
            serialised: false,
            resets_pointer: true,
        }
    }

    /// A provider that supplies a serialiser.
    #[must_use]
    pub const fn with_payload() -> Self {
        Self {
            serialised: true,
            resets_pointer: true,
        }
    }

    /// Declares that opening does not reset the pointer position.
    #[must_use]
    pub const fn keeping_pointer(mut self) -> Self {
        self.resets_pointer = false;
        self
    }

    /// Returns whether opening resets the pointer position.
    #[must_use]
    pub const fn resets_pointer(&self) -> bool {
        self.resets_pointer
    }

    /// Returns what the server writes when opening.
    #[must_use]
    pub fn write(&self, payload: &JsonValue) -> Option<JsonValue> {
        if self.serialised {
            Some(payload.clone())
        } else {
            None
        }
    }

    /// Returns what the client decodes.
    ///
    /// Nothing when no serialiser was supplied at all, and nothing when the
    /// bytes are not a value.
    #[must_use]
    pub fn read(&self, written: Option<&JsonValue>) -> Option<JsonValue> {
        if !self.serialised {
            return None;
        }
        written.cloned()
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::json;

    use super::{
        BaseRecipe, Grid, Ingredient, Ingredients, MAX_INGREDIENTS, MenuProvider, RecipeShape,
        ShapeError,
    };

    #[test]
    fn the_base_recipe_declares_itself_special_so_the_crafting_book_skips_it() {
        let recipe = BaseRecipe::new("infusion");
        assert!(recipe.is_special());
        assert_eq!(recipe.kind(), "infusion");
    }

    #[test]
    fn the_base_recipe_assembles_nothing_shows_nothing_and_accepts_any_grid() {
        let recipe = BaseRecipe::new("infusion");
        assert_eq!(recipe.assemble(&Grid::default()), None);
        assert_eq!(recipe.result(), None);
        for (width, height) in [(0, 0), (1, 1), (3, 3), (9, 9)] {
            assert!(recipe.accepts(width, height));
        }
    }

    #[test]
    fn a_dependent_overriding_the_four_gets_a_crafting_grid_recipe() {
        struct Grid2x2;
        impl RecipeShape for Grid2x2 {
            fn kind(&self) -> &'static str {
                "small"
            }
            fn is_special(&self) -> bool {
                false
            }
            fn assemble(&self, grid: &Grid) -> Option<String> {
                grid.cells.iter().flatten().next().cloned()
            }
            fn result(&self) -> Option<String> {
                Some("lantern".to_owned())
            }
            fn accepts(&self, width: usize, height: usize) -> bool {
                width * height >= 4
            }
        }
        let recipe = Grid2x2;
        assert!(!recipe.is_special());
        assert_eq!(recipe.result(), Some("lantern".to_owned()));
        assert!(recipe.accepts(2, 2));
        assert!(!recipe.accepts(1, 2));
        let grid = Grid {
            width: 2,
            height: 2,
            cells: vec![None, Some("copper".to_owned()), None, None],
        };
        assert_eq!(recipe.assemble(&grid), Some("copper".to_owned()));
    }

    #[test]
    fn an_ingredient_is_complex_by_default_and_the_loader_is_told_the_inverse() {
        let ingredient = Ingredient::new("any_copper");
        assert!(ingredient.is_complex());
        assert!(
            !ingredient.may_be_optimised(),
            "a complex ingredient must not be optimised into a plain item test"
        );
        let simple = Ingredient::new("one_item").simple();
        assert!(!simple.is_complex());
        assert!(simple.may_be_optimised());
    }

    #[test]
    fn an_ingredient_names_the_stacks_it_matches() {
        let ingredient = Ingredient::new("any_copper").matching(["copper", "raw_copper"]);
        assert_eq!(ingredient.examples(), ["copper", "raw_copper"]);
        assert_eq!(ingredient.id(), "any_copper");
        assert_eq!(
            ingredient.admits_item("copper"),
            None,
            "listing stacks for a tooltip does not define what the ingredient accepts"
        );
    }

    #[test]
    fn the_item_level_test_is_the_dependents_own_and_is_not_the_example_list() {
        let ingredient = Ingredient::new("any_copper")
            .matching(["copper", "raw_copper"])
            .admitting(["copper"]);
        assert_eq!(ingredient.admits_item("copper"), Some(true));
        assert_eq!(
            ingredient.admits_item("raw_copper"),
            Some(false),
            "an example stack is shown, not thereby admitted"
        );
        assert_eq!(ingredient.admits_item("iron"), Some(false));
        assert_eq!(
            Ingredient::new("dependents_own").admits_item("anything"),
            None,
            "an ingredient with no declared test leaves the answer to its caller"
        );
    }

    #[test]
    fn the_first_ingredient_for_a_namespace_creates_its_collection_and_later_ones_join_it() {
        let mut registration = Ingredients::new();
        assert!(!registration.has_namespace("example"));
        registration
            .register("example", Ingredient::new("first"))
            .expect("a fresh identifier");
        assert!(registration.has_namespace("example"));
        registration
            .register("example", Ingredient::new("second"))
            .expect("a fresh identifier");
        assert_eq!(registration.in_namespace("example").len(), 2);
        assert_eq!(registration.namespaces(), ["example"]);
    }

    #[test]
    fn two_namespaces_keep_their_own_collections() {
        let mut registration = Ingredients::new();
        registration
            .register("one", Ingredient::new("shared_name"))
            .expect("a fresh identifier");
        registration
            .register("two", Ingredient::new("shared_name"))
            .expect("the same name in another namespace is another ingredient");
        assert_eq!(registration.in_namespace("one").len(), 1);
        assert_eq!(registration.in_namespace("two").len(), 1);
        assert_eq!(registration.namespaces(), ["one", "two"]);
    }

    #[test]
    fn a_namespace_nothing_registered_into_is_empty_rather_than_absent() {
        let registration = Ingredients::new();
        assert_eq!(registration.in_namespace("never").len(), 0);
        assert!(!registration.has_namespace("never"));
    }

    #[test]
    fn one_identifier_is_registered_once_in_a_namespace() {
        let mut registration = Ingredients::new();
        registration
            .register("example", Ingredient::new("once"))
            .expect("a fresh identifier");
        assert_eq!(
            registration.register("example", Ingredient::new("once")),
            Err(ShapeError::Duplicate("once".to_owned()))
        );
    }

    #[test]
    fn a_namespace_past_the_ceiling_is_refused() {
        let mut registration = Ingredients::new();
        for index in 0..MAX_INGREDIENTS {
            registration
                .register("example", Ingredient::new(&format!("i{index}")))
                .expect("a fresh identifier");
        }
        assert_eq!(
            registration.register("example", Ingredient::new("one_more")),
            Err(ShapeError::Full(MAX_INGREDIENTS))
        );
    }

    #[test]
    fn a_provider_that_supplies_no_serialiser_carries_nothing_either_way() {
        let provider = MenuProvider::without_payload();
        assert_eq!(provider.write(&json!({ "stock": 1 })), None);
        assert_eq!(
            provider.read(Some(&json!({ "stock": 1 }))),
            None,
            "an absent payload is the declared state when no serialiser was supplied"
        );
    }

    #[test]
    fn a_provider_that_supplies_a_serialiser_round_trips_and_may_still_yield_nothing() {
        let provider = MenuProvider::with_payload();
        let payload = json!({ "stock": 12 });
        let written = provider.write(&payload).expect("a serialiser was supplied");
        assert_eq!(provider.read(Some(&written)), Some(payload));
        assert_eq!(
            provider.read(None),
            None,
            "decoding may yield nothing and the screen must handle that"
        );
    }

    #[test]
    fn opening_resets_the_pointer_by_default_and_a_provider_may_say_otherwise() {
        assert!(MenuProvider::default().resets_pointer());
        assert!(MenuProvider::with_payload().resets_pointer());
        assert!(
            !MenuProvider::with_payload()
                .keeping_pointer()
                .resets_pointer()
        );
    }
}
