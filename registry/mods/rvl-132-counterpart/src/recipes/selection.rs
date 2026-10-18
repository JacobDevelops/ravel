//! Brewing closure computation and ordered-input simulation.

use std::collections::BTreeSet;

use super::{Converted, Counts, Input, RecipeRefused, check_input_kinds};
use crate::forms::{Candidate, CandidateSet};
use crate::kinds::{ConditionSet, Flags, Stack};

/// How many positions a brewing recipe holds.
pub const BREWING_POSITIONS: usize = 4;

/// The cap section 11.1 states, which is this package's default rather than a
/// constant.
pub const DEFAULT_CLOSURE_CAP: usize = 7_500;

/// The largest closure cap a setting may name.
///
/// A cap is a resolved setting rather than a trusted input, and a cap of no
/// ceiling would put the unbounded closure back.
pub const MAX_CLOSURE_CAP: usize = 100_000;

/// The brewing view's closure of section 11.1.
///
/// It does not enumerate the world's recipes, because brewing has none as
/// documents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Closure {
    cap: usize,
}

impl Closure {
    /// A closure bounded by the cap a setting names.
    ///
    /// # Errors
    ///
    /// Returns [`RecipeRefused::Cap`] past [`MAX_CLOSURE_CAP`] and for a cap of zero,
    /// which would refuse a pack with any brewing at all.
    pub const fn new(cap: usize) -> Result<Self, RecipeRefused> {
        if cap == 0 || cap > MAX_CLOSURE_CAP {
            return Err(RecipeRefused::Cap {
                limit: MAX_CLOSURE_CAP,
            });
        }
        Ok(Self { cap })
    }

    /// The closure at the cap section 11.1 states.
    #[must_use]
    pub const fn shipped() -> Self {
        Self {
            cap: DEFAULT_CLOSURE_CAP,
        }
    }

    /// The cap in force.
    #[must_use]
    pub const fn cap(self) -> usize {
        self.cap
    }

    /// Computes the closure.
    ///
    /// Breadth-first from a single water-filled bottle: every collected
    /// ingredient is mixed against every item at the current frontier, an output
    /// is kept when it is non-empty and `keep` accepts it, a result already
    /// present is not added again, and only genuinely new results extend the
    /// frontier.
    ///
    /// # Errors
    ///
    /// Returns [`RecipeRefused::ProbableCycle`] once the accumulated set passes the
    /// cap. The closure is unbounded under a badly-behaved mod, so this is the
    /// bound rather than a diagnostic.
    pub fn compute(
        self,
        base: &Stack,
        ingredients: &[Stack],
        mix: impl Fn(&Stack, &Stack) -> Option<Stack>,
        keep: impl Fn(&Stack, &Stack) -> bool,
    ) -> Result<Vec<Converted>, RecipeRefused> {
        let mut recipes = Vec::new();
        let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
        let mut reached: BTreeSet<String> = BTreeSet::new();
        let mut frontier = vec![base.clone()];
        reached.insert(identify(base));
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for input in &frontier {
                for ingredient in ingredients {
                    let Some(output) = mix(ingredient, input) else {
                        continue;
                    };
                    if output.is_empty() || !keep(input, &output) {
                        continue;
                    }
                    let key = (identify(ingredient), identify(input), identify(&output));
                    if !seen.insert(key) {
                        continue;
                    }
                    if recipes.len() >= self.cap {
                        return Err(RecipeRefused::ProbableCycle { cap: self.cap });
                    }
                    recipes.push(brewing_recipe(ingredient, input, &output));
                    if reached.insert(identify(&output)) {
                        next.push(output);
                    }
                }
            }
            frontier = next;
        }
        Ok(recipes)
    }

    /// Simulates a brew from the given inputs.
    ///
    /// Section eleven: the inputs are mixed in order, each against the running
    /// result, the first position is then blanked, and the three remaining
    /// positions come back. The running result starts as the first input, which
    /// is the only reading under which "each against the running result" has a
    /// first mix to perform (**inferred**).
    ///
    /// The input-count predicate section eleven records as defect one is the
    /// range check rather than the disjunction, so zero and one inputs are
    /// refused here where the original admitted them.
    ///
    /// # Errors
    ///
    /// Returns [`RecipeRefused::MixedKinds`] when the inputs name more than one
    /// ingredient kind, [`RecipeRefused::NoInput`] for no inputs at all and
    /// [`RecipeRefused::Count`] outside [`Counts::BREWING`].
    pub fn simulate(
        &self,
        inputs: &[Input],
        mix: impl Fn(&Stack, &Stack) -> Option<Stack>,
    ) -> Result<Option<Vec<Stack>>, RecipeRefused> {
        check_input_kinds(inputs)?;
        let Some((first, rest)) = inputs.split_first() else {
            return Err(RecipeRefused::NoInput);
        };
        Counts::BREWING.check(inputs.len())?;
        let mut running = first.value.clone();
        for input in rest {
            let Some(mixed) = mix(&input.value, &running) else {
                return Ok(None);
            };
            running = mixed;
        }
        if running.is_empty() {
            return Ok(None);
        }
        let mut positions = vec![running; BREWING_POSITIONS];
        positions[0] = Stack::empty();
        Ok(Some(positions.split_off(1)))
    }
}

fn identify(value: &Stack) -> String {
    let data: Vec<String> = value
        .data
        .iter()
        .map(|(name, held)| format!("{name}={held:?}"))
        .collect();
    format!("{}#{}", value.identity, data.join(","))
}

/// One brewing result as a four-position recipe.
///
/// The ingredient sits in the first position and the input in the other three,
/// producing nothing in the first and the output in the other three.
fn brewing_recipe(ingredient: &Stack, input: &Stack, output: &Stack) -> Converted {
    let mut inputs = Vec::with_capacity(BREWING_POSITIONS);
    inputs.push(one_of(ingredient));
    for _ in 1..BREWING_POSITIONS {
        inputs.push(one_of(input));
    }
    let mut produced = Vec::with_capacity(BREWING_POSITIONS);
    produced.push(Stack::empty());
    for _ in 1..BREWING_POSITIONS {
        produced.push(output.clone());
    }
    Converted {
        id: format!("{}+{}", identify(ingredient), identify(input)),
        inputs,
        output: produced,
    }
}

fn one_of(value: &Stack) -> CandidateSet<Stack> {
    let mut set = CandidateSet::new();
    let under = Flags::slot(Stack::IDENTITY).combine(Flags::slot(Stack::DATA));
    // A fresh set holds nothing, so the one addition is inside the ceiling.
    set.add(Candidate {
        value: value.clone(),
        under,
    })
    .expect("a fresh candidate set has room for one value");
    set
}

#[cfg(test)]
mod tests {
    use super::{Closure, Input, RecipeRefused, Stack};

    fn stack(name: &str) -> Stack {
        Stack::of(name.parse().expect("a valid identity"), 1)
    }

    fn mix(ingredient: &Stack, running: &Stack) -> Option<Stack> {
        match (
            ingredient.identity.to_string().as_str(),
            running.identity.to_string().as_str(),
        ) {
            ("test:wart", "test:water") => Some(stack("test:awkward")),
            ("test:sugar", "test:awkward") => Some(stack("test:sweet")),
            _ => None,
        }
    }

    #[test]
    fn simulation_mixes_in_order_and_returns_three_copies_without_the_ingredient_slot() {
        let inputs =
            ["test:water", "test:wart", "test:sugar"].map(|name| Input::stackable(stack(name)));
        let result = Closure::shipped()
            .simulate(&inputs, mix)
            .expect("three stackable inputs");
        assert_eq!(result, Some(vec![stack("test:sweet"); 3]));
        let reversed = [inputs[0].clone(), inputs[2].clone(), inputs[1].clone()];
        assert_eq!(Closure::shipped().simulate(&reversed, mix), Ok(None));
    }

    #[test]
    fn simulation_refuses_distinct_kinds_before_count_or_mixing_empty_values() {
        let closure = Closure::shipped();
        let inputs = ["item", "fluid", "item", "fluid", "energy"].map(|kind| Input {
            kind: kind.to_owned(),
            value: Stack::empty(),
        });
        for (count, named) in [(2, 2), (4, 2), (5, 3)] {
            assert_eq!(
                closure.simulate(&inputs[..count], |_, _| panic!("mixed kinds must not mix")),
                Err(RecipeRefused::MixedKinds { named })
            );
        }
        assert_eq!(
            closure.simulate(&[], |_, _| panic!("no inputs must not mix")),
            Err(RecipeRefused::NoInput)
        );
        assert_eq!(
            closure.simulate(&inputs[..1], |_, _| panic!("one input must not mix")),
            Err(RecipeRefused::Count {
                given: 1,
                least: 2,
                most: 4,
            })
        );
        let same_kind = vec![inputs[0].clone(); 5];
        assert_eq!(
            closure.simulate(&same_kind, |_, _| panic!("too many inputs must not mix")),
            Err(RecipeRefused::Count {
                given: 5,
                least: 2,
                most: 4,
            })
        );
    }

    #[test]
    fn revisiting_a_reached_stack_records_the_edge_without_reexpanding_it() {
        let mixes = std::cell::Cell::new(0);
        let recipes = Closure::new(2)
            .expect("two recipes fit")
            .compute(
                &stack("test:water"),
                &[stack("test:toggle")],
                |_, input| {
                    mixes.set(mixes.get() + 1);
                    if input.identity == stack("test:water").identity {
                        Some(stack("test:awkward"))
                    } else {
                        Some(stack("test:water"))
                    }
                },
                |_, _| true,
            )
            .expect("a finite two-edge cycle terminates at the exact cap");
        assert_eq!(recipes.len(), 2);
        assert_eq!(mixes.get(), 2, "reached stacks are not expanded again");
        assert_eq!(recipes[0].output[1..], vec![stack("test:awkward"); 3]);
        assert_eq!(recipes[1].output[1..], vec![stack("test:water"); 3]);
    }
}
