use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::identity::ModId;

/// Returns the lexicographically smallest topological order of `participants`.
///
/// Each mod listed under `x` in `after` follows `x`. Edges involving absent
/// participants are ignored.
///
/// # Errors
///
/// Returns [`Cycle`] naming all participants left waiting when ordering stalls.
///
/// # Examples
///
/// ```
/// use std::collections::{BTreeMap, BTreeSet};
///
/// use ravel_core::{ModId, precedence};
///
/// let early: ModId = "base_mod".parse()?;
/// let late: ModId = "skin_mod".parse()?;
///
/// // `skin_mod` follows `base_mod`.
/// let mut after = BTreeMap::new();
/// after.insert(early.clone(), BTreeSet::from([late.clone()]));
///
/// let order = precedence([late.clone(), early.clone()], &after)
///     .expect("an edge without a cycle orders");
/// assert_eq!(order, [early.clone(), late.clone()]);
///
/// let mut circular = after.clone();
/// circular.insert(late.clone(), BTreeSet::from([early.clone()]));
/// let refusal = precedence([early.clone(), late.clone()], &circular)
///     .expect_err("a cycle has no order");
/// assert_eq!(refusal.participants(), [early, late]);
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
pub fn precedence(
    participants: impl IntoIterator<Item = ModId>,
    after: &BTreeMap<ModId, BTreeSet<ModId>>,
) -> Result<Vec<ModId>, Cycle> {
    let participants: BTreeSet<ModId> = participants.into_iter().collect();
    let mut waiting: BTreeMap<&ModId, usize> = participants.iter().map(|id| (id, 0)).collect();

    for (earlier, later) in after {
        if !participants.contains(earlier) {
            continue;
        }
        for follower in later {
            if let Some(count) = waiting.get_mut(follower) {
                *count += 1;
            }
        }
    }

    let mut ready: BTreeSet<&ModId> = waiting
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::with_capacity(participants.len());
    while let Some(next) = ready.pop_first() {
        order.push(next.clone());
        for follower in after.get(next).into_iter().flatten() {
            if let Some(count) = waiting.get_mut(follower) {
                *count -= 1;
                if *count == 0 {
                    ready.insert(follower);
                }
            }
        }
    }

    if order.len() == participants.len() {
        return Ok(order);
    }
    Err(Cycle {
        participants: participants
            .into_iter()
            .filter(|id| !order.contains(id))
            .collect(),
    })
}

/// Participants left waiting when a cycle prevents a complete order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cycle {
    participants: Vec<ModId>,
}

impl Cycle {
    /// Returns all waiting participants in name order.
    #[must_use]
    pub fn participants(&self) -> &[ModId] {
        &self.participants
    }
}

impl fmt::Display for Cycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.participants.iter().map(ModId::as_str).collect();
        write!(
            formatter,
            "`{}` each wait for another, so no order puts every one of them after \
             what it declared it follows",
            names.join("`, `")
        )
    }
}

impl std::error::Error for Cycle {}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::precedence;
    use crate::identity::ModId;

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    fn edges(pairs: [(&str, &str); 2]) -> BTreeMap<ModId, BTreeSet<ModId>> {
        let mut after: BTreeMap<ModId, BTreeSet<ModId>> = BTreeMap::new();
        for (earlier, later) in pairs {
            after.entry(id(earlier)).or_default().insert(id(later));
        }
        after
    }

    fn satisfies(order: &[ModId], after: &BTreeMap<ModId, BTreeSet<ModId>>) -> bool {
        after.iter().all(|(earlier, followers)| {
            followers.iter().all(|follower| {
                match (
                    order.iter().position(|id| id == earlier),
                    order.iter().position(|id| id == follower),
                ) {
                    (Some(first), Some(second)) => first < second,
                    _ => true,
                }
            })
        })
    }

    #[test]
    fn a_declared_edge_is_honoured_whichever_order_the_participants_arrive_in() {
        let after = edges([("a_mod", "b_mod"), ("b_mod", "c_mod")]);
        let forward = precedence([id("a_mod"), id("b_mod"), id("c_mod")], &after)
            .expect("a chain without a cycle orders");
        let backward = precedence([id("c_mod"), id("b_mod"), id("a_mod")], &after)
            .expect("a chain without a cycle orders");

        assert_eq!(forward, backward);
        assert!(satisfies(&forward, &after));
        assert_eq!(forward, [id("a_mod"), id("b_mod"), id("c_mod")]);
    }

    #[test]
    fn mods_with_no_edge_between_them_come_out_in_name_order() {
        let after = BTreeMap::new();
        let order = precedence([id("z_mod"), id("a_mod")], &after).expect("no edge, no cycle");
        assert_eq!(order, [id("a_mod"), id("z_mod")]);
    }

    #[test]
    fn a_cycle_names_every_mod_still_waiting_and_nobody_else() {
        let after = edges([("a_mod", "b_mod"), ("b_mod", "a_mod")]);
        let refusal = precedence([id("a_mod"), id("b_mod"), id("free_mod")], &after)
            .expect_err("a cycle has no order");

        assert_eq!(refusal.participants(), [id("a_mod"), id("b_mod")]);
        assert!(refusal.to_string().contains("a_mod"));
    }

    #[test]
    fn an_edge_naming_a_mod_that_is_not_a_participant_is_ignored_rather_than_counted() {
        let after = edges([("a_mod", "absent_mod"), ("absent_mod", "b_mod")]);
        let order =
            precedence([id("a_mod"), id("b_mod")], &after).expect("an absent mod holds nobody up");

        assert_eq!(order, [id("a_mod"), id("b_mod")]);
    }
}
