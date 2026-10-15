//! The one crafting recipe kind, from §13.
//!
//! A shaped recipe that carries chosen item data from an ingredient onto its
//! result. Declaratively it is an ordinary shaped recipe nested whole under a
//! single field, so a pack author writes the familiar shape and changes only the
//! kind.
//!
//! **Host-blocked.** Ravel claims a recipe kind as a registry entry and decides
//! nothing about what one does, so the matching and the assembly are host work
//! and no document is serialised here. What is portable is the rule the assembly
//! applies, and that is what [`assemble`] is.

use std::collections::BTreeMap;

/// The field the nested shaped recipe is written under.
pub const NESTED_FIELD: &str = "recipe";

/// The crafting-book category the kind is filed in.
///
/// The original marks this an unresolved guess, and it is carried as one rather
/// than promoted to a decision.
pub const CATEGORY: &str = "misc";

/// One ingredient in the grid, and what it carries.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Ingredient {
    /// The data kinds this ingredient's item declares it preserves.
    pub preserves: Vec<String>,
    /// The data the ingredient actually carries, by kind.
    pub carries: BTreeMap<String, String>,
}

impl Ingredient {
    /// Builds an ingredient that preserves the named kinds and carries values.
    #[must_use]
    pub fn new(preserves: &[&str], carries: &[(&str, &str)]) -> Self {
        Self {
            preserves: preserves.iter().map(|kind| (*kind).to_owned()).collect(),
            carries: carries
                .iter()
                .map(|(kind, value)| ((*kind).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }
}

/// Assembles the result, carrying chosen data from the grid onto it.
///
/// `base` is what the nested shaped recipe already gave the result. Every
/// ingredient in the grid is examined, and one whose item declares a set of data
/// kinds to preserve contributes each of those it actually carries.
///
/// Two consequences the original neither prevents nor documents are kept here,
/// because a dependent's pack author can see both: **all** matching ingredients
/// contribute, in grid order, and a later one **overwrites** an earlier one for
/// the same kind, so a recipe with two such ingredients is order-dependent; and
/// data an ingredient declares but does not carry is **skipped rather than
/// cleared**, so the result keeps whatever the base recipe gave it.
#[must_use]
pub fn assemble(base: &BTreeMap<String, String>, grid: &[Ingredient]) -> BTreeMap<String, String> {
    let mut result = base.clone();
    for ingredient in grid {
        for kind in &ingredient.preserves {
            if let Some(value) = ingredient.carries.get(kind) {
                result.insert(kind.clone(), value.clone());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(kind, value)| ((*kind).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn an_ingredient_contributes_every_declared_kind_it_actually_carries() {
        let grid = vec![Ingredient::new(
            &["energy", "inventory"],
            &[("energy", "full"), ("inventory", "two")],
        )];
        assert_eq!(
            assemble(&base(&[]), &grid),
            base(&[("energy", "full"), ("inventory", "two")])
        );
    }

    #[test]
    fn a_kind_an_ingredient_declares_but_does_not_carry_is_skipped_and_not_cleared() {
        let grid = vec![Ingredient::new(&["energy"], &[])];
        assert_eq!(
            assemble(&base(&[("energy", "from_the_base")]), &grid),
            base(&[("energy", "from_the_base")]),
            "absent data leaves what the base recipe gave the result"
        );
    }

    #[test]
    fn a_kind_an_ingredient_carries_but_does_not_declare_is_not_contributed() {
        let grid = vec![Ingredient::new(&[], &[("energy", "full")])];
        assert_eq!(assemble(&base(&[]), &grid), base(&[]));
    }

    #[test]
    fn every_matching_ingredient_contributes_rather_than_only_the_first() {
        let grid = vec![
            Ingredient::new(&["energy"], &[("energy", "full")]),
            Ingredient::new(&["inventory"], &[("inventory", "two")]),
        ];
        assert_eq!(
            assemble(&base(&[]), &grid),
            base(&[("energy", "full"), ("inventory", "two")])
        );
    }

    #[test]
    fn two_ingredients_carrying_the_same_kind_make_the_recipe_order_dependent() {
        let first = Ingredient::new(&["energy"], &[("energy", "first")]);
        let second = Ingredient::new(&["energy"], &[("energy", "second")]);

        assert_eq!(
            assemble(&base(&[]), &[first.clone(), second.clone()]),
            base(&[("energy", "second")]),
            "a later ingredient overwrites an earlier one"
        );
        assert_eq!(
            assemble(&base(&[]), &[second, first]),
            base(&[("energy", "first")]),
            "and the reverse grid order gives the other answer"
        );
    }

    #[test]
    fn an_empty_grid_leaves_the_base_result_exactly_as_it_was() {
        let given = base(&[("energy", "full")]);
        assert_eq!(assemble(&given, &[]), given);
    }
}
