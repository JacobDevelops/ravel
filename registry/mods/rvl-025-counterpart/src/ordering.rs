//! The cross-block ordering service of §14, which is what dependents rely on
//! most quietly.
//!
//! Block entities in different mods that must act in a fixed order relative to
//! one another enqueue themselves against a dimension and a rank, and everything
//! queued for a dimension runs in rank order after that dimension's server tick.
//! Two properties are load-bearing and §14 says a counterpart must keep them:
//! the queue is per dimension, so ordering is not global; and each rank's list
//! is cleared as it is run, so work enqueued during the drain of an earlier rank
//! is picked up by a later rank in the same tick while work enqueued during the
//! last rank waits for the next one.
//!
//! The five ranks are a contract this entry publishes and does not itself use.
//! Upstream they are documented only as a comment naming five device kinds, none
//! of which is in this entry, so they are numbered here and named for their
//! position rather than for a device.
//!
//! Two ceilings bound the structure rather than one. A rank of a dimension holds
//! at most [`MAX_QUEUED`], and the *number of dimensions* is bounded by
//! [`MAX_DIMENSIONS`], because nothing else would bound it: a dimension name is a
//! caller's string, and a per-dimension map with no ceiling on its keys grows
//! with what callers name rather than with what a world holds. A dimension that
//! empties is dropped at the drain, so the map tracks what is queued rather than
//! what has ever been named.

use std::collections::BTreeMap;

/// How many ranks the drain walks.
pub const RANKS: usize = 5;

/// The most dimensions the service holds queues for at once.
///
/// A dimension name arrives from a caller, so the key space is untrusted input
/// in the same way a rank's length is. A dimension that holds nothing is dropped
/// when it is drained, so this bounds what is queued rather than what has ever
/// been enqueued.
pub const MAX_DIMENSIONS: usize = 256;

/// The most entries one rank of one dimension holds.
///
/// A dependent enqueues from a block entity, so the queue grows with the world
/// rather than with anything this entry controls. The ceiling is a declared
/// number to raise rather than a rule to rewrite; past it an enqueue is refused
/// and says so instead of the queue growing until the allocator ends the
/// process.
pub const MAX_QUEUED: usize = 65_536;

/// What running one entry answers with: the rank and caller of each re-enqueue.
///
/// A run hands back what it wants queued next rather than reaching into the
/// service, so the drain owns every insertion and can refuse one at the ceiling.
pub type Reenqueued = Vec<(u8, String)>;

/// One thing waiting to be run, as the service knows it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Entry {
    /// Who enqueued, named by the caller.
    pub who: String,
    /// Which rank it asked for.
    pub rank: u8,
    /// The counter value at the moment it was enqueued.
    pub stamp: u64,
}

/// The per-dimension, per-rank queues and the counter beside them.
#[derive(Clone, Debug, Default)]
pub struct Ordering {
    queues: BTreeMap<String, Vec<Vec<Entry>>>,
    counter: u64,
}

impl Ordering {
    /// Builds an empty service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the monotonic counter.
    ///
    /// A caller compares this against the stamp it last enqueued under to tell
    /// whether it has already enqueued itself this tick.
    #[must_use]
    pub const fn stamp(&self) -> u64 {
        self.counter
    }

    /// Enqueues a caller against a dimension and a rank.
    ///
    /// # Errors
    ///
    /// Returns [`OrderingRefused::Rank`] for a rank at or past [`RANKS`] and
    /// [`OrderingRefused::Full`] at [`MAX_QUEUED`].
    pub fn enqueue(
        &mut self,
        dimension: &str,
        rank: u8,
        who: &str,
    ) -> Result<Entry, OrderingRefused> {
        if usize::from(rank) >= RANKS {
            return Err(OrderingRefused::Rank { asked: rank });
        }
        if !self.queues.contains_key(dimension) && self.queues.len() >= MAX_DIMENSIONS {
            return Err(OrderingRefused::Dimensions {
                limit: MAX_DIMENSIONS,
            });
        }
        let ranks = self
            .queues
            .entry(dimension.to_owned())
            .or_insert_with(|| vec![Vec::new(); RANKS]);
        let queue = &mut ranks[usize::from(rank)];
        if queue.len() >= MAX_QUEUED {
            return Err(OrderingRefused::Full { limit: MAX_QUEUED });
        }
        let entry = Entry {
            who: who.to_owned(),
            rank,
            stamp: self.counter,
        };
        queue.push(entry.clone());
        Ok(entry)
    }

    /// Returns how many entries one dimension holds across every rank.
    #[must_use]
    pub fn queued(&self, dimension: &str) -> usize {
        self.queues
            .get(dimension)
            .map_or(0, |ranks| ranks.iter().map(Vec::len).sum())
    }

    /// Drains one dimension after its server tick, running each entry.
    ///
    /// Rank zero first. Each rank's list is taken before it is run, so an entry
    /// the run enqueues into a later rank is picked up in this same drain and
    /// one enqueued into the rank being run, or into an earlier one, waits for
    /// the next tick. Whatever `run` answers with is enqueued in this dimension
    /// exactly as a direct call would be. A drain never stops partway, because
    /// the remaining ranks would go unrun, so a re-enqueue the service refuses
    /// is reported in [`Drained::refused`] rather than raised or dropped.
    pub fn drain_with(
        &mut self,
        dimension: &str,
        run: &mut dyn FnMut(&Entry) -> Reenqueued,
    ) -> Drained {
        self.counter = self.counter.wrapping_add(1);
        self.run_ranks(dimension, run)
    }

    /// Drains one dimension without running anything, answering in rank order.
    pub fn drain(&mut self, dimension: &str) -> Drained {
        self.drain_with(dimension, &mut |_| Vec::new())
    }

    /// Drains every dimension that holds anything, for one server tick.
    ///
    /// §14 puts the drain after a server-side *level* tick and Ravel's event set
    /// carries no dimension with its server tick, so the tick drains every
    /// dimension rather than one named dimension. Draining one would leave every
    /// other dimension's queue to grow until its ceiling refused, which is a loss
    /// no caller can see. The counter advances once for the tick, not once per
    /// dimension, because it is what a caller compares against to tell whether it
    /// has already enqueued itself *this tick*.
    ///
    /// A dimension left empty is dropped, so the key space tracks what is queued.
    pub fn drain_all_with(
        &mut self,
        run: &mut dyn FnMut(&str, &Entry) -> Reenqueued,
    ) -> Vec<(String, Drained)> {
        self.counter = self.counter.wrapping_add(1);
        let dimensions: Vec<String> = self.queues.keys().cloned().collect();
        let mut drained = Vec::new();
        for dimension in dimensions {
            let outcome = self.run_ranks(&dimension, &mut |entry| run(&dimension, entry));
            let empty = self
                .queues
                .get(&dimension)
                .is_none_or(|ranks| ranks.iter().all(Vec::is_empty));
            if empty {
                self.queues.remove(&dimension);
            }
            drained.push((dimension, outcome));
        }
        drained
    }

    /// Drains every dimension without running anything.
    pub fn drain_all(&mut self) -> Vec<(String, Drained)> {
        self.drain_all_with(&mut |_, _| Vec::new())
    }

    fn run_ranks(&mut self, dimension: &str, run: &mut dyn FnMut(&Entry) -> Reenqueued) -> Drained {
        let mut drained = Drained::default();
        for rank in 0..RANKS {
            let taken = match self.queues.get_mut(dimension) {
                Some(ranks) => std::mem::take(&mut ranks[rank]),
                None => Vec::new(),
            };
            for entry in taken {
                let enqueued = run(&entry);
                drained.ran.push(entry);
                for (rank, who) in enqueued {
                    if let Err(refusal) = self.enqueue(dimension, rank, &who) {
                        drained.refused.push((rank, who, refusal));
                    }
                }
            }
        }
        drained
    }

    /// Empties every queue, which is what a server starting does.
    ///
    /// The counter is not reset with them: it is the value callers compare
    /// against, and restarting it would make a stale stamp look current.
    pub fn clear(&mut self) {
        self.queues.clear();
    }

    /// Returns how many dimensions hold anything.
    #[must_use]
    pub fn dimensions(&self) -> usize {
        self.queues
            .values()
            .filter(|ranks| ranks.iter().any(|queue| !queue.is_empty()))
            .count()
    }

    /// Returns how many dimensions the service is holding queues for.
    ///
    /// The figure [`MAX_DIMENSIONS`] bounds, which is not the same as
    /// [`Ordering::dimensions`]: a dimension keeps its empty ranks until it is
    /// drained.
    #[must_use]
    pub fn tracked_dimensions(&self) -> usize {
        self.queues.len()
    }

    /// Returns the dimensions holding anything, in name order.
    #[must_use]
    pub fn queued_dimensions(&self) -> Vec<&str> {
        self.queues
            .iter()
            .filter(|(_, ranks)| ranks.iter().any(|queue| !queue.is_empty()))
            .map(|(dimension, _)| dimension.as_str())
            .collect()
    }
}

/// What one dimension's drain ran, and what it could not take back.
///
/// The refused half exists because a drain cannot stop partway without leaving
/// later ranks unrun, so a re-enqueue the service refuses has to be reported
/// somewhere rather than discarded. A caller that ignores it is choosing to, and
/// one that does not can see the entry that was lost and why.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Drained {
    /// What ran, in rank order.
    pub ran: Vec<Entry>,
    /// What a run asked to enqueue and the service refused.
    pub refused: Vec<(u8, String, OrderingRefused)>,
}

/// Why the service refused an enqueue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderingRefused {
    /// The rank asked for is not one of the five.
    Rank {
        /// What was asked for.
        asked: u8,
    },
    /// That rank of that dimension already holds as many as it may.
    Full {
        /// The ceiling that was reached.
        limit: usize,
    },
    /// The service already holds queues for as many dimensions as it may.
    Dimensions {
        /// The ceiling that was reached.
        limit: usize,
    },
}

impl std::fmt::Display for OrderingRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rank { asked } => {
                write!(
                    formatter,
                    "rank {asked} is not one of the {RANKS} published"
                )
            }
            Self::Full { limit } => {
                write!(formatter, "one rank of one dimension holds at most {limit}")
            }
            Self::Dimensions { limit } => {
                write!(
                    formatter,
                    "the service queues for at most {limit} dimensions"
                )
            }
        }
    }
}

impl std::error::Error for OrderingRefused {}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(drained: &Drained) -> Vec<&str> {
        drained.ran.iter().map(|entry| entry.who.as_str()).collect()
    }

    #[test]
    fn everything_queued_for_a_dimension_runs_in_rank_order() {
        let mut ordering = Ordering::new();
        for (rank, who) in [(4, "last"), (0, "first"), (2, "middle")] {
            ordering
                .enqueue("counterpart:deep", rank, who)
                .expect("a legal rank");
        }

        let ran = ordering.drain("counterpart:deep");
        assert_eq!(names(&ran), ["first", "middle", "last"]);
        assert_eq!(ran.refused.len(), 0);
        assert_eq!(ordering.queued("counterpart:deep"), 0);
    }

    #[test]
    fn two_entries_in_one_rank_keep_the_order_they_were_enqueued_in() {
        let mut ordering = Ordering::new();
        for who in ["first", "second", "third"] {
            ordering
                .enqueue("counterpart:deep", 1, who)
                .expect("a legal rank");
        }
        assert_eq!(
            names(&ordering.drain("counterpart:deep")),
            ["first", "second", "third"]
        );
    }

    #[test]
    fn the_queue_is_per_dimension_so_ordering_is_not_global() {
        let mut ordering = Ordering::new();
        ordering
            .enqueue("counterpart:deep", 0, "deep")
            .expect("a legal rank");
        ordering
            .enqueue("counterpart:high", 0, "high")
            .expect("a legal rank");
        assert_eq!(ordering.dimensions(), 2);

        assert_eq!(names(&ordering.drain("counterpart:deep")), ["deep"]);
        assert_eq!(
            ordering.queued("counterpart:high"),
            1,
            "draining one dimension leaves the other alone"
        );
    }

    #[test]
    fn work_enqueued_during_an_earlier_rank_is_picked_up_by_a_later_one() {
        let mut ordering = Ordering::new();
        ordering
            .enqueue("counterpart:deep", 0, "first")
            .expect("a legal rank");

        let ran = ordering.drain_with("counterpart:deep", &mut |entry| {
            if entry.who == "first" {
                vec![(3, "added_during_rank_zero".to_owned())]
            } else {
                Vec::new()
            }
        });

        assert_eq!(names(&ran), ["first", "added_during_rank_zero"]);
        assert_eq!(ordering.queued("counterpart:deep"), 0);
    }

    #[test]
    fn work_enqueued_during_the_last_rank_waits_for_the_next_tick() {
        let mut ordering = Ordering::new();
        ordering
            .enqueue("counterpart:deep", 4, "last")
            .expect("a legal rank");

        let ran = ordering.drain_with("counterpart:deep", &mut |entry| {
            if entry.who == "last" {
                vec![(0, "added_during_the_last_rank".to_owned())]
            } else {
                Vec::new()
            }
        });

        assert_eq!(names(&ran), ["last"]);
        assert_eq!(ordering.queued("counterpart:deep"), 1);
        assert_eq!(
            names(&ordering.drain("counterpart:deep")),
            ["added_during_the_last_rank"]
        );
    }

    #[test]
    fn work_enqueued_into_the_rank_being_run_waits_rather_than_looping() {
        let mut ordering = Ordering::new();
        ordering
            .enqueue("counterpart:deep", 2, "self_feeding")
            .expect("a legal rank");

        let ran = ordering.drain_with("counterpart:deep", &mut |_| vec![(2, "again".to_owned())]);

        assert_eq!(names(&ran), ["self_feeding"]);
        assert_eq!(
            ordering.queued("counterpart:deep"),
            1,
            "each rank's list is taken before it runs, so the drain terminates"
        );
    }

    #[test]
    fn the_counter_advances_once_per_drain_and_a_caller_reads_its_own_stamp() {
        let mut ordering = Ordering::new();
        let first = ordering
            .enqueue("counterpart:deep", 0, "first")
            .expect("a legal rank");
        assert_eq!(first.stamp, ordering.stamp());

        ordering.drain("counterpart:deep");
        assert_eq!(ordering.stamp(), first.stamp + 1);
        assert!(
            first.stamp < ordering.stamp(),
            "a caller compares its stamp against the counter to tell whether it \
             has already enqueued itself this tick"
        );
    }

    #[test]
    fn a_rank_outside_the_five_is_refused() {
        let mut ordering = Ordering::new();
        for rank in 0..RANKS {
            ordering
                .enqueue(
                    "counterpart:deep",
                    u8::try_from(rank).expect("under five"),
                    "who",
                )
                .expect("a legal rank");
        }
        assert_eq!(
            ordering.enqueue("counterpart:deep", 5, "who"),
            Err(OrderingRefused::Rank { asked: 5 })
        );
        assert_eq!(
            ordering.enqueue("counterpart:deep", u8::MAX, "who"),
            Err(OrderingRefused::Rank { asked: u8::MAX })
        );
    }

    #[test]
    fn a_rank_that_reached_its_ceiling_refuses_rather_than_growing() {
        let mut ordering = Ordering::new();
        for index in 0..MAX_QUEUED {
            ordering
                .enqueue("counterpart:deep", 0, &format!("who{index}"))
                .expect("a legal rank");
        }
        assert_eq!(
            ordering.enqueue("counterpart:deep", 0, "one_too_many"),
            Err(OrderingRefused::Full { limit: MAX_QUEUED })
        );
        assert!(
            ordering
                .enqueue("counterpart:deep", 1, "another_rank")
                .is_ok(),
            "the ceiling is per rank, not per dimension"
        );
    }

    #[test]
    fn a_server_tick_drains_every_dimension_and_not_only_the_first() {
        let mut ordering = Ordering::new();
        for dimension in ["counterpart:deep", "counterpart:high", "counterpart:outer"] {
            ordering
                .enqueue(dimension, 0, "dependent")
                .expect("a legal rank");
        }

        let drained = ordering.drain_all();
        assert_eq!(drained.len(), 3);
        for (_, outcome) in &drained {
            assert_eq!(names(outcome), ["dependent"]);
        }
        assert_eq!(ordering.dimensions(), 0);
        assert_eq!(
            ordering.stamp(),
            1,
            "the counter advances once for the tick rather than once per dimension"
        );
    }

    #[test]
    fn a_dimension_that_is_never_named_by_a_drain_is_still_emptied_by_the_tick() {
        let mut ordering = Ordering::new();
        ordering
            .enqueue("counterpart:elsewhere", 3, "dependent")
            .expect("a legal rank");

        for _ in 0..1000 {
            ordering.drain_all();
        }
        assert_eq!(
            ordering.queued("counterpart:elsewhere"),
            0,
            "a queue no tick reaches is a silent loss, which is what draining every \
             dimension exists to prevent"
        );
    }

    #[test]
    fn an_emptied_dimension_is_dropped_so_the_key_space_does_not_grow() {
        let mut ordering = Ordering::new();
        for index in 0..MAX_DIMENSIONS {
            ordering
                .enqueue(&format!("counterpart:d{index}"), 0, "dependent")
                .expect("a legal rank");
        }
        assert_eq!(ordering.tracked_dimensions(), MAX_DIMENSIONS);
        assert_eq!(
            ordering.enqueue("counterpart:one_too_many", 0, "dependent"),
            Err(OrderingRefused::Dimensions {
                limit: MAX_DIMENSIONS
            })
        );

        ordering.drain_all();
        assert_eq!(ordering.tracked_dimensions(), 0);
        assert!(
            ordering
                .enqueue("counterpart:one_too_many", 0, "dependent")
                .is_ok(),
            "the ceiling bounds what is queued, not what has ever been named"
        );
    }

    #[test]
    fn a_re_enqueue_the_service_refuses_is_reported_rather_than_dropped() {
        let mut ordering = Ordering::new();
        ordering
            .enqueue("counterpart:deep", 0, "first")
            .expect("a legal rank");

        let past_the_last = u8::try_from(RANKS).expect("five fits in a byte");
        let drained = ordering.drain_with("counterpart:deep", &mut |_| {
            vec![(past_the_last, "impossible_rank".to_owned())]
        });
        assert_eq!(names(&drained), ["first"]);
        assert_eq!(
            drained.refused,
            [(
                past_the_last,
                "impossible_rank".to_owned(),
                OrderingRefused::Rank {
                    asked: past_the_last
                }
            )]
        );
    }

    #[test]
    fn a_server_starting_empties_every_queue_and_leaves_the_counter_alone() {
        let mut ordering = Ordering::new();
        ordering
            .enqueue("counterpart:deep", 0, "first")
            .expect("a legal rank");
        ordering.drain("counterpart:deep");
        ordering
            .enqueue("counterpart:high", 1, "second")
            .expect("a legal rank");

        let before = ordering.stamp();
        ordering.clear();

        assert_eq!(ordering.dimensions(), 0);
        assert_eq!(ordering.queued("counterpart:high"), 0);
        assert_eq!(
            ordering.stamp(),
            before,
            "restarting the counter would make a stale stamp look current"
        );
    }
}
