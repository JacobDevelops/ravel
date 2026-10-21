//! Section eleven's recipe views, and section 11.1's brewing closure.
//!
//! A recipe view answers two questions: enumerate the recipes this station can
//! run, and given a set of inputs, what would come out. The general station view
//! covers seven of the eight vanilla attachments and the brewing view computes a
//! closure, because brewing has no recipes as documents.
//! The private selection module owns that closure and its simulation; shared
//! recipe types and attachment selection stay here.
//!
//! Section 11.1's cap is the one place this module deliberately parts from the
//! original's shape rather than its arithmetic. The original hard-codes 7500,
//! which section 11.1 calls a disclosed ceiling and a guess at where a legitimate
//! pack ends and a cycle begins. The closure is genuinely unbounded under a
//! badly-behaved mod, so the cap stays and becomes a setting.

use core::fmt;
use std::collections::BTreeSet;

use rvl_210_counterpart::quantity::QuantityKind;

use crate::capabilities;
use crate::forms::CandidateSet;
use crate::kinds::{Ingredient, Stack, Stackable};

mod selection;

pub use selection::{BREWING_POSITIONS, Closure, DEFAULT_CLOSURE_CAP, MAX_CLOSURE_CAP};

/// The width and height of the transient grid every station simulation builds.
///
/// Section eleven records that building a 3×3 grid for every station is a
/// shortcut: a station with one input slot gets an eight-slot grid it never
/// reads. It costs an allocation per simulation and is invisible from outside,
/// so it is kept.
pub const GRID_SIDE: usize = 3;

/// How many positions that grid holds.
pub const GRID_SIZE: usize = GRID_SIDE * GRID_SIDE;

/// The most recipes a converted collection may hold.
pub const MAX_CONVERTED: usize = 100_000;

/// What a recipe view refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeRefused {
    /// The inputs name more than one ingredient kind.
    MixedKinds {
        /// How many they named.
        named: usize,
    },
    /// The inputs are empty, or empty after the optional empty-slot filter.
    NoInput,
    /// The input count is outside the view's declared range.
    Count {
        /// How many were given.
        given: usize,
        /// The fewest admitted.
        least: usize,
        /// The most admitted.
        most: usize,
    },
    /// More inputs were given than the grid holds.
    TooManyInputs {
        /// How many the grid holds.
        limit: usize,
    },
    /// A recipe definition's output is not the one shape output checking needs.
    Output {
        /// What was wrong.
        detail: &'static str,
    },
    /// The closure passed its cap, which section 11.1 names a probable cycle.
    ProbableCycle {
        /// The cap it passed.
        cap: usize,
    },
    /// A converted collection passed [`MAX_CONVERTED`].
    TooManyRecipes {
        /// The published ceiling.
        limit: usize,
    },
    /// A setting named a cap outside the admitted range.
    Cap {
        /// The most admitted.
        limit: usize,
    },
    /// A recipe the vocabulary's converter refused, with
    /// `recipes.abort_on_invalid` switched on.
    InvalidRecipe {
        /// What the refused recipe is called.
        id: String,
    },
}

impl fmt::Display for RecipeRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MixedKinds { named } => write!(
                formatter,
                "a simulation names {named} ingredient kinds and admits one"
            ),
            Self::NoInput => formatter.write_str("a simulation was given nothing to work on"),
            Self::Count { given, least, most } => write!(
                formatter,
                "{given} inputs is outside the {least} to {most} this view admits"
            ),
            Self::TooManyInputs { limit } => {
                write!(formatter, "the grid holds {limit} positions")
            }
            Self::Output { detail } => formatter.write_str(detail),
            Self::ProbableCycle { cap } => write!(
                formatter,
                "the brewing closure passed {cap} recipes, which is probably a cycle"
            ),
            Self::TooManyRecipes { limit } => {
                write!(
                    formatter,
                    "more converted recipes than the {limit} admitted"
                )
            }
            Self::Cap { limit } => write!(formatter, "a closure cap past the {limit} admitted"),
            Self::InvalidRecipe { id } => {
                write!(formatter, "the recipe {id} did not convert")
            }
        }
    }
}

impl std::error::Error for RecipeRefused {}

/// The input-count range a view admits.
///
/// Defect one: the original's brewing predicate is a disjunction of "at least
/// two" and "at most four", which is true for every value, so every input count
/// is admitted including zero and one and a caller relying on the check is not
/// protected. This is the range check the disjunction was meant to be.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Counts {
    /// The fewest inputs admitted.
    pub least: usize,
    /// The most inputs admitted.
    pub most: usize,
}

impl Counts {
    /// The range the brewing view admits.
    pub const BREWING: Self = Self { least: 2, most: 4 };

    /// Any count from one up to the grid's size.
    pub const ANY: Self = Self {
        least: 1,
        most: GRID_SIZE,
    };

    /// Whether a count is admitted.
    #[must_use]
    pub const fn admits(self, given: usize) -> bool {
        given >= self.least && given <= self.most
    }

    /// Holds a count to the range.
    ///
    /// # Errors
    ///
    /// Returns [`RecipeRefused::Count`] outside it.
    pub const fn check(self, given: usize) -> Result<(), RecipeRefused> {
        if self.admits(given) {
            return Ok(());
        }
        Err(RecipeRefused::Count {
            given,
            least: self.least,
            most: self.most,
        })
    }
}

/// One recipe as the vocabulary defines it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Definition {
    /// What the recipe is called.
    pub id: String,
    /// Which recipe kind it belongs to.
    pub kind: String,
    /// Whether it is marked special, which enumeration drops.
    pub special: bool,
    /// The per-slot candidate set for each input position.
    pub inputs: Vec<CandidateSet<Stack>>,
    /// What comes out.
    pub output: Vec<Stack>,
}

/// One recipe after the vocabulary's own converter has been through it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Converted {
    /// What the recipe is called.
    pub id: String,
    /// The per-slot candidate set for each input position.
    pub inputs: Vec<CandidateSet<Stack>>,
    /// What comes out.
    pub output: Vec<Stack>,
}

/// One input a caller hands a simulation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Input {
    /// Which ingredient kind it belongs to.
    pub kind: String,
    /// The instance.
    pub value: Stack,
}

impl Input {
    /// An input of the stackable-item kind.
    #[must_use]
    pub fn stackable(value: Stack) -> Self {
        Self {
            kind: crate::kinds::STACKABLE.to_owned(),
            value,
        }
    }
}

fn admits(set: &CandidateSet<Stack>, value: &Stack, kind: &Stackable) -> bool {
    set.alternatives()
        .any(|candidate| kind.matches(&candidate.value, value, candidate.under.conditions()))
}

/// Whether one converted recipe matches a filled grid.
#[must_use]
pub fn matches(recipe: &Converted, grid: &[Option<Stack>], kind: &Stackable) -> bool {
    let filled: Vec<&Stack> = grid.iter().flatten().collect();
    if filled.len() != recipe.inputs.len() {
        return false;
    }
    recipe
        .inputs
        .iter()
        .zip(filled)
        .all(|(set, value)| admits(set, value, kind))
}

/// The general station view, which covers seven of the eight vanilla
/// attachments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Station {
    kind: String,
    counts: Counts,
    drop_empty: bool,
    check_output: bool,
}

impl Station {
    /// Builds a view over one recipe kind.
    #[must_use]
    pub fn new(kind: &str, counts: Counts, drop_empty: bool, check_output: bool) -> Self {
        Self {
            kind: kind.to_owned(),
            counts,
            drop_empty,
            check_output,
        }
    }

    /// The recipe kind this view enumerates.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Whether empty slots are dropped before matching.
    #[must_use]
    pub const fn drops_empty(&self) -> bool {
        self.drop_empty
    }

    /// Whether the output is checked as well as the input.
    ///
    /// Only the cutting block turns this on, because it is the one station where
    /// one input has several outputs.
    #[must_use]
    pub const fn checks_output(&self) -> bool {
        self.check_output
    }

    /// Enumerates the world's recipes of this kind.
    ///
    /// The whole collection is filtered to this kind, recipes marked special are
    /// dropped, and each survivor is put through the vocabulary's own converter.
    ///
    /// What a conversion failure does is section three's
    /// `recipes.abort_on_invalid`: at its shipped default of `false` the recipe
    /// is skipped, and switched on the first one aborts loading. A view that
    /// always skipped would leave that setting inert.
    ///
    /// # Errors
    ///
    /// Returns [`RecipeRefused::TooManyRecipes`] past [`MAX_CONVERTED`], and
    /// [`RecipeRefused::InvalidRecipe`] for the first refused conversion when
    /// `abort_on_invalid` is on.
    pub fn enumerate(
        &self,
        world: &[Definition],
        convert: impl Fn(&Definition) -> Option<Converted>,
        abort_on_invalid: bool,
    ) -> Result<Vec<Converted>, RecipeRefused> {
        let mut converted = Vec::new();
        for definition in world {
            if definition.kind != self.kind || definition.special {
                continue;
            }
            let Some(recipe) = convert(definition) else {
                if abort_on_invalid {
                    return Err(RecipeRefused::InvalidRecipe {
                        id: definition.id.clone(),
                    });
                }
                continue;
            };
            if converted.len() >= MAX_CONVERTED {
                return Err(RecipeRefused::TooManyRecipes {
                    limit: MAX_CONVERTED,
                });
            }
            converted.push(recipe);
        }
        Ok(converted)
    }

    /// Builds the transient grid a simulation runs against.
    ///
    /// # Errors
    ///
    /// Returns [`RecipeRefused::MixedKinds`] when the inputs name more than one
    /// ingredient kind, [`RecipeRefused::NoInput`] when nothing is left after the
    /// optional empty-slot filter, [`RecipeRefused::Count`] outside the declared
    /// range and [`RecipeRefused::TooManyInputs`] past the grid's size.
    pub fn grid(&self, inputs: &[Input]) -> Result<Vec<Option<Stack>>, RecipeRefused> {
        check_input_kinds(inputs)?;
        let filled: Vec<&Stack> = inputs
            .iter()
            .map(|input| &input.value)
            .filter(|value| !self.drop_empty || !value.is_empty())
            .collect();
        if filled.is_empty() {
            return Err(RecipeRefused::NoInput);
        }
        self.counts.check(filled.len())?;
        if filled.len() > GRID_SIZE {
            return Err(RecipeRefused::TooManyInputs { limit: GRID_SIZE });
        }
        let mut grid = vec![None; GRID_SIZE];
        for (position, value) in filled.into_iter().enumerate() {
            grid[position] = Some(value.clone());
        }
        Ok(grid)
    }

    /// Simulates from a set of inputs and answers what would come out.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Station::grid`] refuses.
    pub fn simulate(
        &self,
        inputs: &[Input],
        recipes: &[Converted],
        kind: &Stackable,
    ) -> Result<Option<Vec<Stack>>, RecipeRefused> {
        let grid = self.grid(inputs)?;
        Ok(recipes
            .iter()
            .find(|recipe| matches(recipe, &grid, kind))
            .map(|recipe| recipe.output.clone()))
    }

    /// Simulates from a recipe definition rather than from loose inputs.
    ///
    /// With output checking on, the definition's output must name exactly one
    /// ingredient kind and exactly one instance, and the scan then looks for a
    /// recipe matching both input and output.
    ///
    /// # Errors
    ///
    /// Returns [`RecipeRefused::Output`] when output checking is on and the
    /// definition's output is not that one shape.
    pub fn simulate_from(
        &self,
        definition: &Definition,
        recipes: &[Converted],
        kind: &Stackable,
    ) -> Result<Option<Vec<Stack>>, RecipeRefused> {
        if self.check_output {
            if definition.output.len() != 1 {
                return Err(RecipeRefused::Output {
                    detail: "output checking needs a definition naming exactly one instance",
                });
            }
            let wanted = &definition.output[0];
            return Ok(recipes
                .iter()
                .find(|recipe| {
                    recipe.inputs == definition.inputs
                        && recipe.output.len() == 1
                        && kind.matches(&recipe.output[0], wanted, kind.exact_conditions())
                })
                .map(|recipe| recipe.output.clone()));
        }
        Ok(recipes
            .iter()
            .find(|recipe| recipe.inputs == definition.inputs)
            .map(|recipe| recipe.output.clone()))
    }
}

fn check_input_kinds(inputs: &[Input]) -> Result<(), RecipeRefused> {
    let kinds: BTreeSet<&str> = inputs.iter().map(|input| input.kind.as_str()).collect();
    if kinds.len() > 1 {
        return Err(RecipeRefused::MixedKinds { named: kinds.len() });
    }
    Ok(())
}

/// Which view one recipe-view attachment runs.
///
/// Section nine attaches the recipe view to eight game objects and section
/// eleven has each of them answer from one of two places: the brewing station
/// answers from a derived closure and the other seven from the world's own
/// recipe collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum View {
    /// The general station view, over one recipe kind.
    Station(Station),
    /// The brewing view, which derives its recipes rather than reading them.
    Brewing(Closure),
}

/// One recipe-view attachment bound to the view behind it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bound {
    /// The game object the capability is attached to.
    pub target: &'static str,
    /// The view that answers for it.
    pub view: View,
}

/// The eight recipe-view attachments of section nine, each bound to its view.
///
/// Section eleven states two things about the general view's flags and leaves
/// the rest to the station: only the cutting block checks its output (**read**),
/// and the view is built with a predicate on the input count and a drop-empty
/// flag whose per-station values the specification does not give. The crafting
/// table is the one station handed a grid with empty positions in it, so it is
/// the one that drops them, and each count range is the station's own slot count
/// (**inferred** — recorded in `NOTES.md` as inferred rather than read).
///
/// `closure` is the brewing closure section three's cap setting puts in force,
/// so a resolved cap reaches the attachment rather than stopping at the setting.
#[must_use]
pub fn bound(closure: Closure) -> Vec<Bound> {
    let single = Counts { least: 1, most: 1 };
    let mut table = vec![Bound {
        target: capabilities::BREWING,
        view: View::Brewing(closure),
    }];
    for (target, kind) in [
        (capabilities::SMELTING[0], "minecraft:smelting"),
        (capabilities::SMELTING[1], "minecraft:blasting"),
        (capabilities::SMELTING[2], "minecraft:smoking"),
        (capabilities::CAMPFIRE, "minecraft:campfire_cooking"),
    ] {
        table.push(Bound {
            target,
            view: View::Station(Station::new(kind, single, false, false)),
        });
    }
    table.push(Bound {
        target: capabilities::CRAFTING,
        view: View::Station(Station::new(
            "minecraft:crafting",
            Counts {
                least: 1,
                most: GRID_SIZE,
            },
            true,
            false,
        )),
    });
    table.push(Bound {
        target: capabilities::CUTTING,
        view: View::Station(Station::new("minecraft:stonecutting", single, false, true)),
    });
    table.push(Bound {
        target: capabilities::UPGRADE,
        view: View::Station(Station::new(
            "minecraft:smithing_transform",
            Counts { least: 2, most: 3 },
            false,
            false,
        )),
    });
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::Candidate;
    use crate::kinds::{ConditionSet, Flags};

    fn item_id(name: &str) -> ravel_sdk::content::ContentId {
        if name.contains(':') {
            name.parse().expect("a valid item identity")
        } else {
            format!("minecraft:{name}")
                .parse()
                .expect("a valid item identity")
        }
    }

    fn set_of(names: &[&str]) -> CandidateSet<Stack> {
        let mut set = CandidateSet::new();
        for name in names {
            set.add(Candidate {
                value: Stack::of(item_id(name), 1),
                under: Flags::slot(Stack::IDENTITY),
            })
            .expect("inside the ceiling");
        }
        set
    }

    fn definition(
        id: &str,
        kind: &str,
        special: bool,
        inputs: &[&str],
        output: &str,
    ) -> Definition {
        Definition {
            id: id.to_owned(),
            kind: kind.to_owned(),
            special,
            inputs: inputs.iter().map(|name| set_of(&[name])).collect(),
            output: vec![Stack::of(item_id(output), 1)],
        }
    }

    fn convert(definition: &Definition) -> Option<Converted> {
        (!definition.id.starts_with("bad")).then(|| Converted {
            id: definition.id.clone(),
            inputs: definition.inputs.clone(),
            output: definition.output.clone(),
        })
    }

    #[test]
    fn an_invalid_recipe_aborts_enumeration_when_the_setting_says_so() {
        // Section three's `recipes.abort_on_invalid`. Skipping either way would
        // leave the setting inert, which is what the declaration promises not to
        // be.
        let world = vec![
            definition("good", "minecraft:smelting", false, &["a"], "b"),
            definition("bad", "minecraft:smelting", false, &["c"], "d"),
        ];
        assert_eq!(
            smelting().enumerate(&world, convert, true),
            Err(RecipeRefused::InvalidRecipe {
                id: "bad".to_owned()
            })
        );
        assert_eq!(
            smelting()
                .enumerate(&world, convert, false)
                .expect("a skipping enumeration")
                .len(),
            1
        );
    }

    #[test]
    fn the_brewing_view_mixes_in_order_blanks_the_first_and_answers_three() {
        let closure = Closure::shipped();
        let mix = |ingredient: &Stack, running: &Stack| {
            Some(crate::stack(
                format!(
                    "minecraft:{}_{}",
                    running.identity.path(),
                    ingredient.identity.path()
                ),
                1,
            ))
        };
        let inputs = vec![
            Input::stackable(Stack::of(
                "minecraft:water".parse().expect("a valid content identity"),
                1,
            )),
            Input::stackable(Stack::of(
                "minecraft:nether_wart"
                    .parse()
                    .expect("a valid content identity"),
                1,
            )),
            Input::stackable(Stack::of(
                "minecraft:glowstone"
                    .parse()
                    .expect("a valid content identity"),
                1,
            )),
        ];
        let answered = closure
            .simulate(&inputs, mix)
            .expect("three inputs")
            .expect("a mix that answers");
        assert_eq!(answered.len(), BREWING_POSITIONS - 1);
        for position in &answered {
            assert_eq!(
                position.identity.to_string(),
                "minecraft:water_nether_wart_glowstone"
            );
        }
    }

    #[test]
    fn the_brewing_view_refuses_the_counts_the_disjunction_admitted() {
        let closure = Closure::shipped();
        let mix = |_: &Stack, running: &Stack| Some(running.clone());
        assert_eq!(closure.simulate(&[], mix), Err(RecipeRefused::NoInput));
        assert_eq!(
            closure.simulate(
                &[Input::stackable(Stack::of(
                    "minecraft:water".parse().expect("a valid content identity"),
                    1
                ))],
                mix
            ),
            Err(RecipeRefused::Count {
                given: 1,
                least: 2,
                most: 4
            })
        );
    }

    #[test]
    fn a_brew_that_does_not_mix_answers_nothing_rather_than_the_input() {
        let closure = Closure::shipped();
        let inputs = vec![
            Input::stackable(Stack::of(
                "minecraft:water".parse().expect("a valid content identity"),
                1,
            )),
            Input::stackable(Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                1,
            )),
        ];
        assert_eq!(closure.simulate(&inputs, |_, _| None), Ok(None));
    }

    #[test]
    fn every_recipe_view_attachment_names_the_view_behind_it() {
        // Section nine attaches the recipe view to eight game objects and
        // section eleven names three views; an attachment with no view behind it
        // is a capability name and nothing else.
        let table = bound(Closure::shipped());
        assert_eq!(table.len(), 8);
        let targets: Vec<&str> = table.iter().map(|entry| entry.target).collect();
        for expected in [
            capabilities::BREWING,
            capabilities::CAMPFIRE,
            capabilities::CRAFTING,
            capabilities::CUTTING,
            capabilities::UPGRADE,
        ] {
            assert!(targets.contains(&expected));
        }
        for station in capabilities::SMELTING {
            assert!(targets.contains(&station));
        }
        let brewing: Vec<&Bound> = table
            .iter()
            .filter(|entry| matches!(entry.view, View::Brewing(_)))
            .collect();
        assert_eq!(brewing.len(), 1);
        assert_eq!(brewing[0].target, capabilities::BREWING);
    }

    #[test]
    fn only_the_cutting_block_checks_its_output_and_only_crafting_drops_empties() {
        let table = bound(Closure::shipped());
        for entry in &table {
            let View::Station(station) = &entry.view else {
                continue;
            };
            assert_eq!(
                station.checks_output(),
                entry.target == capabilities::CUTTING,
                "output checking on {}",
                entry.target
            );
            assert_eq!(
                station.drops_empty(),
                entry.target == capabilities::CRAFTING,
                "empty-slot dropping on {}",
                entry.target
            );
        }
    }

    #[test]
    fn the_bound_brewing_view_carries_the_cap_the_setting_resolved() {
        let closure = Closure::new(12).expect("inside the ceiling");
        let table = bound(closure);
        let View::Brewing(carried) = &table[0].view else {
            panic!("the brewing station is the brewing view's");
        };
        assert_eq!(carried.cap(), 12);
    }

    fn smelting() -> Station {
        Station::new("minecraft:smelting", Counts::ANY, true, false)
    }

    #[test]
    fn enumeration_keeps_only_this_kinds_ordinary_convertible_recipes() {
        let world = vec![
            definition(
                "a",
                "minecraft:smelting",
                false,
                &["minecraft:iron_ore"],
                "minecraft:iron_ingot",
            ),
            definition(
                "b",
                "minecraft:crafting",
                false,
                &["minecraft:stick"],
                "minecraft:torch",
            ),
            definition(
                "c",
                "minecraft:smelting",
                true,
                &["minecraft:gold_ore"],
                "minecraft:gold_ingot",
            ),
            definition(
                "bad",
                "minecraft:smelting",
                false,
                &["minecraft:sand"],
                "minecraft:glass",
            ),
        ];
        let enumerated = smelting()
            .enumerate(&world, convert, false)
            .expect("a small world");
        let names: Vec<&str> = enumerated.iter().map(|recipe| recipe.id.as_str()).collect();
        assert_eq!(names, vec!["a"]);
    }

    #[test]
    fn enumeration_of_an_empty_world_answers_nothing_rather_than_refusing() {
        assert_eq!(smelting().enumerate(&[], convert, false), Ok(Vec::new()));
    }

    #[test]
    fn the_grid_is_always_three_by_three_and_filled_in_order() {
        let station = smelting();
        let grid = station
            .grid(&[Input::stackable(crate::stack("minecraft:iron_ore", 1))])
            .expect("one input");
        assert_eq!(grid.len(), GRID_SIZE);
        assert_eq!(
            grid[0].as_ref().map(|held| held.identity.to_string()),
            Some("minecraft:iron_ore".to_owned())
        );
        assert!(grid[1..].iter().all(Option::is_none));
    }

    #[test]
    fn inputs_naming_more_than_one_ingredient_kind_are_refused() {
        let station = smelting();
        let inputs = vec![
            Input::stackable(Stack::of(
                "minecraft:iron_ore"
                    .parse()
                    .expect("a valid content identity"),
                1,
            )),
            Input {
                kind: crate::kinds::PORTION.to_owned(),
                value: Stack::of(
                    "minecraft:water".parse().expect("a valid content identity"),
                    1,
                ),
            },
        ];
        assert_eq!(
            station.grid(&inputs),
            Err(RecipeRefused::MixedKinds { named: 2 })
        );
    }

    #[test]
    fn empty_inputs_are_refused_after_the_optional_filter() {
        let station = smelting();
        assert_eq!(station.grid(&[]), Err(RecipeRefused::NoInput));
        assert_eq!(
            station.grid(&[Input::stackable(Stack::empty())]),
            Err(RecipeRefused::NoInput)
        );
    }

    #[test]
    fn a_view_that_does_not_drop_empty_slots_keeps_them() {
        let station = Station::new("minecraft:smelting", Counts::ANY, false, false);
        assert!(!station.drops_empty());
        let grid = station
            .grid(&[Input::stackable(Stack::empty())])
            .expect("one position, empty");
        assert_eq!(grid[0], Some(Stack::empty()));
    }

    #[test]
    fn a_count_outside_the_declared_range_is_refused() {
        // Defect one: the original's predicate admits every count, so this is
        // the check a caller was relying on and did not have.
        let station = Station::new("brewing", Counts::BREWING, true, false);
        assert_eq!(
            station.grid(&[Input::stackable(Stack::of(
                "minecraft:sugar".parse().expect("a valid content identity"),
                1
            ))]),
            Err(RecipeRefused::Count {
                given: 1,
                least: 2,
                most: 4
            })
        );
        assert!(Counts::BREWING.admits(2));
        assert!(Counts::BREWING.admits(4));
        assert!(!Counts::BREWING.admits(0));
        assert!(!Counts::BREWING.admits(1));
        assert!(!Counts::BREWING.admits(5));
    }

    #[test]
    fn a_disjunction_of_the_two_halves_would_have_admitted_everything() {
        // The shape of the defect, held so that a later "simplification" back to
        // it fails rather than passes: the disjunction admits every count, and
        // the range check has to disagree with it somewhere.
        let disjunction =
            |count: usize| count >= Counts::BREWING.least || count <= Counts::BREWING.most;
        for count in 0..8usize {
            assert!(disjunction(count));
        }
        for count in [0usize, 1] {
            assert!(disjunction(count));
            assert!(!Counts::BREWING.admits(count));
        }
        assert!(disjunction(9));
        assert!(!Counts::BREWING.admits(9));
    }

    #[test]
    fn more_inputs_than_the_grid_holds_are_refused() {
        let station = Station::new(
            "minecraft:crafting",
            Counts { least: 1, most: 20 },
            true,
            false,
        );
        let inputs: Vec<Input> = (0..=GRID_SIZE)
            .map(|index| Input::stackable(crate::stack(format!("minecraft:k{index}"), 1)))
            .collect();
        assert_eq!(
            station.grid(&inputs),
            Err(RecipeRefused::TooManyInputs { limit: GRID_SIZE })
        );
    }

    #[test]
    fn a_simulation_answers_the_matching_recipes_result() {
        let station = smelting();
        let world = vec![definition(
            "a",
            "minecraft:smelting",
            false,
            &["minecraft:iron_ore"],
            "minecraft:iron_ingot",
        )];
        let recipes = station
            .enumerate(&world, convert, false)
            .expect("a small world");
        let kind = Stackable::default();
        let produced = station
            .simulate(
                &[Input::stackable(crate::stack("minecraft:iron_ore", 1))],
                &recipes,
                &kind,
            )
            .expect("one input");
        assert_eq!(
            produced.map(|output| output[0].identity.to_string()),
            Some("minecraft:iron_ingot".to_owned())
        );
    }

    #[test]
    fn a_simulation_with_no_matching_recipe_answers_nothing() {
        let station = smelting();
        let kind = Stackable::default();
        assert_eq!(
            station.simulate(
                &[Input::stackable(Stack::of(
                    "minecraft:stone".parse().expect("a valid content identity"),
                    1
                ))],
                &[],
                &kind
            ),
            Ok(None)
        );
    }

    #[test]
    fn output_checking_needs_exactly_one_named_instance() {
        let station = Station::new("minecraft:cutting", Counts::ANY, true, true);
        assert!(station.checks_output());
        let mut definition = definition(
            "a",
            "minecraft:cutting",
            false,
            &["minecraft:stone"],
            "minecraft:stone_bricks",
        );
        definition
            .output
            .push(crate::stack("minecraft:stone_stairs", 1));
        let kind = Stackable::default();
        assert!(matches!(
            station.simulate_from(&definition, &[], &kind),
            Err(RecipeRefused::Output { .. })
        ));
    }

    #[test]
    fn output_checking_picks_the_recipe_matching_both_input_and_output() {
        let station = Station::new("minecraft:cutting", Counts::ANY, true, true);
        let bricks = definition(
            "a",
            "minecraft:cutting",
            false,
            &["minecraft:stone"],
            "minecraft:stone_bricks",
        );
        let stairs = definition(
            "b",
            "minecraft:cutting",
            false,
            &["minecraft:stone"],
            "minecraft:stone_stairs",
        );
        let world = vec![bricks, stairs.clone()];
        let recipes = station
            .enumerate(&world, convert, false)
            .expect("a small world");
        let kind = Stackable::default();
        assert_eq!(
            station
                .simulate_from(&stairs, &recipes, &kind)
                .expect("one named instance")
                .map(|output| output[0].identity.to_string()),
            Some("minecraft:stone_stairs".to_owned())
        );
    }

    #[test]
    fn without_output_checking_the_first_recipe_on_the_input_wins() {
        let station = smelting();
        let one = definition(
            "a",
            "minecraft:smelting",
            false,
            &["minecraft:stone"],
            "minecraft:smooth_stone",
        );
        let two = definition(
            "b",
            "minecraft:smelting",
            false,
            &["minecraft:stone"],
            "minecraft:glass",
        );
        let recipes = station
            .enumerate(&[one.clone(), two], convert, false)
            .expect("a small world");
        let kind = Stackable::default();
        assert_eq!(
            station
                .simulate_from(&one, &recipes, &kind)
                .expect("no output checking")
                .map(|output| output[0].identity.to_string()),
            Some("minecraft:smooth_stone".to_owned())
        );
    }

    // The two brewing tables, as a pair of functions a Ravel hook would supply.
    fn mix(ingredient: &Stack, input: &Stack) -> Option<Stack> {
        match (
            ingredient.identity.to_string().as_str(),
            input.identity.to_string().as_str(),
        ) {
            ("minecraft:nether_wart", "minecraft:water_bottle") => {
                Some(crate::stack("minecraft:awkward_potion", 1))
            }
            ("minecraft:sugar", "minecraft:awkward_potion") => {
                Some(crate::stack("minecraft:swiftness_potion", 1))
            }
            ("minecraft:gunpowder", "minecraft:swiftness_potion") => {
                Some(crate::stack("minecraft:splash_swiftness", 1))
            }
            _ => None,
        }
    }

    fn keep(input: &Stack, output: &Stack) -> bool {
        output.identity != input.identity && output.identity != item_id("minecraft:water_bottle")
    }

    fn ingredients() -> Vec<Stack> {
        vec![
            Stack::of(
                "minecraft:nether_wart"
                    .parse()
                    .expect("a valid content identity"),
                1,
            ),
            Stack::of(
                "minecraft:sugar".parse().expect("a valid content identity"),
                1,
            ),
            Stack::of(
                "minecraft:gunpowder"
                    .parse()
                    .expect("a valid content identity"),
                1,
            ),
        ]
    }

    #[test]
    fn the_brewing_closure_walks_out_from_the_water_bottle() {
        let closure = Closure::shipped();
        assert_eq!(closure.cap(), DEFAULT_CLOSURE_CAP);
        let recipes = closure
            .compute(
                &crate::stack("minecraft:water_bottle", 1),
                &ingredients(),
                mix,
                keep,
            )
            .expect("a small table");
        assert_eq!(recipes.len(), 3);
    }

    #[test]
    fn each_kept_result_is_a_four_position_recipe_producing_nothing_in_the_first() {
        let recipes = Closure::shipped()
            .compute(
                &crate::stack("minecraft:water_bottle", 1),
                &ingredients(),
                mix,
                keep,
            )
            .expect("a small table");
        let first = &recipes[0];
        assert_eq!(first.inputs.len(), BREWING_POSITIONS);
        assert_eq!(first.output.len(), BREWING_POSITIONS);
        assert!(first.output[0].is_empty());
        assert!(first.output[1..].iter().all(|held| !held.is_empty()));
        assert_eq!(first.inputs[0].len(), 1);
    }

    #[test]
    fn a_result_already_present_is_not_added_again() {
        let repeated: Vec<Stack> = (0..4)
            .map(|_| crate::stack("minecraft:nether_wart", 1))
            .collect();
        let recipes = Closure::shipped()
            .compute(
                &crate::stack("minecraft:water_bottle", 1),
                &repeated,
                mix,
                keep,
            )
            .expect("a small table");
        assert_eq!(recipes.len(), 1);
    }

    #[test]
    fn an_output_that_is_empty_or_refused_by_the_keep_rule_is_dropped() {
        let recipes = Closure::shipped()
            .compute(
                &crate::stack("minecraft:water_bottle", 1),
                &ingredients(),
                |_, _| Some(Stack::empty()),
                keep,
            )
            .expect("a small table");
        assert_eq!(recipes.len(), 0);
        let refused = Closure::shipped()
            .compute(
                &crate::stack("minecraft:water_bottle", 1),
                &ingredients(),
                mix,
                |_, _| false,
            )
            .expect("a small table");
        assert_eq!(refused.len(), 0);
    }

    #[test]
    fn a_cycle_is_refused_at_the_cap_rather_than_running_forever() {
        // Every mix yields a brand new item, so the frontier never empties.
        let closure = Closure::new(500).expect("inside the ceiling");
        let refusal = closure
            .compute(
                &crate::stack("minecraft:water_bottle", 1),
                &[crate::stack("minecraft:sugar", 1)],
                |_, input| Some(crate::stack(format!("{}x", input.identity), 1)),
                |input, output| output.identity != input.identity,
            )
            .expect_err("an unbounded closure");
        assert_eq!(refusal, RecipeRefused::ProbableCycle { cap: 500 });
    }

    #[test]
    fn the_cap_is_a_setting_and_is_itself_bounded() {
        assert_eq!(Closure::new(1).map(Closure::cap), Ok(1));
        assert_eq!(
            Closure::new(MAX_CLOSURE_CAP).map(Closure::cap),
            Ok(MAX_CLOSURE_CAP)
        );
        assert_eq!(
            Closure::new(0),
            Err(RecipeRefused::Cap {
                limit: MAX_CLOSURE_CAP
            })
        );
        assert_eq!(
            Closure::new(MAX_CLOSURE_CAP + 1),
            Err(RecipeRefused::Cap {
                limit: MAX_CLOSURE_CAP
            })
        );
    }

    #[test]
    fn a_brewing_recipes_candidate_sets_match_the_ingredient_and_the_input() {
        let recipes = Closure::shipped()
            .compute(
                &crate::stack("minecraft:water_bottle", 1),
                &ingredients(),
                mix,
                keep,
            )
            .expect("a small table");
        let kind = Stackable::default();
        let first = recipes
            .iter()
            .find(|recipe| recipe.id.starts_with("minecraft:nether_wart"))
            .expect("the first step");
        assert!(admits(
            &first.inputs[0],
            &Stack::of(
                "minecraft:nether_wart"
                    .parse()
                    .expect("a valid content identity"),
                1
            ),
            &kind
        ));
        assert!(admits(
            &first.inputs[1],
            &Stack::of(
                "minecraft:water_bottle"
                    .parse()
                    .expect("a valid content identity"),
                1
            ),
            &kind
        ));
        assert!(!admits(
            &first.inputs[1],
            &Stack::of(
                "minecraft:sugar".parse().expect("a valid content identity"),
                1
            ),
            &kind
        ));
    }

    #[test]
    fn a_grid_matches_only_a_recipe_of_the_same_length() {
        let kind = Stackable::default();
        let recipe = Converted {
            id: "a".to_owned(),
            inputs: vec![set_of(&["minecraft:stone"]), set_of(&["minecraft:stone"])],
            output: vec![Stack::of(
                "minecraft:stone_bricks"
                    .parse()
                    .expect("a valid content identity"),
                1,
            )],
        };
        let mut grid = vec![None; GRID_SIZE];
        grid[0] = Some(crate::stack("minecraft:stone", 1));
        assert!(!matches(&recipe, &grid, &kind));
        grid[1] = Some(crate::stack("minecraft:stone", 1));
        assert!(matches(&recipe, &grid, &kind));
    }

    #[test]
    fn a_candidate_set_admitting_several_alternatives_matches_any_of_them() {
        let kind = Stackable::default();
        let set = set_of(&["minecraft:oak_log", "minecraft:birch_log"]);
        assert!(admits(
            &set,
            &Stack::of(
                "minecraft:birch_log"
                    .parse()
                    .expect("a valid content identity"),
                1
            ),
            &kind
        ));
        assert!(!admits(
            &set,
            &Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                1
            ),
            &kind
        ));
        assert!(Flags::slot(Stack::IDENTITY).test(Flags::slot(Stack::IDENTITY)));
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            RecipeRefused::NoInput
                .to_string()
                .contains("nothing to work on")
        );
        assert!(
            RecipeRefused::ProbableCycle { cap: 7 }
                .to_string()
                .contains("probably a cycle")
        );
        assert!(
            RecipeRefused::MixedKinds { named: 2 }
                .to_string()
                .contains("admits one")
        );
        assert!(
            RecipeRefused::TooManyInputs { limit: 9 }
                .to_string()
                .contains("9 positions")
        );
        assert!(
            RecipeRefused::Output { detail: "wrong" }
                .to_string()
                .contains("wrong")
        );
        assert!(
            RecipeRefused::TooManyRecipes { limit: 4 }
                .to_string()
                .contains("than the 4")
        );
    }
}
