//! Section fourteen's caches, and when they are wrong.
//!
//! The two caches this package keeps are keyed and invalidated differently: the
//! general station view's is keyed by recipe kind and dimension name and cleared
//! when a server stops, and the brewing closure is a process-lifetime value with
//! no key and no clearing at all. Section fourteen's third cache is not kept:
//! nothing files anything in it, and section fourteen says in as many words that
//! its shape is not to be ported.
//!
//! Defect eight is that neither is invalidated on a data-pack reload. On an
//! integrated server a reload that changes a recipe leaves every station view
//! answering from the old collection until the world is left, and the whole
//! brewing list is stale for the life of the process. Section fourteen calls
//! fixing that the single most useful divergence in the document, so
//! [`Caches::invalidate`] clears both at both points.

use core::fmt;

use crate::recipes::Converted;

/// The most station keys the first cache holds.
///
/// The key is a recipe kind and a dimension name, both of which arrive from the
/// running world, so the cache has a ceiling rather than growing with them.
pub const MAX_STATION_KEYS: usize = 512;

/// The two points at which every cache here is invalidated.
///
/// The original clears the station cache at the second point only, and the
/// brewing closure at neither.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Point {
    /// A data-pack reload, which the original does not clear at.
    Reloaded,
    /// A server stopping.
    Stopped,
}

/// What a cache refuses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheRefused {
    /// The station cache holds as many keys as it publishes.
    Full {
        /// The published ceiling.
        limit: usize,
    },
}

impl fmt::Display for CacheRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full { limit } => {
                write!(formatter, "the station cache holds at most {limit} keys")
            }
        }
    }
}

impl std::error::Error for CacheRefused {}

/// One station cache key.
///
/// Recipes are per-server rather than per-dimension in the pinned build, so the
/// dimension makes the cache larger and never wrong; it is kept because a
/// dependent that files under one dimension reads back under the same one.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Key {
    /// The recipe kind.
    pub kind: String,
    /// The dimension the collection was converted for.
    pub dimension: String,
}

impl Key {
    /// Builds a key.
    #[must_use]
    pub fn new(kind: &str, dimension: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            dimension: dimension.to_owned(),
        }
    }
}

/// The caches of section fourteen, held together so that one invalidation
/// reaches both of them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Caches {
    stations: Vec<(Key, Vec<Converted>)>,
    brewing: Option<Vec<Converted>>,
}

impl Caches {
    /// Two empty caches.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stations: Vec::new(),
            brewing: None,
        }
    }

    /// The collection filed under one station key, computing and filing it when
    /// absent.
    ///
    /// # Errors
    ///
    /// Returns [`CacheRefused::Full`] past [`MAX_STATION_KEYS`].
    pub fn station(
        &mut self,
        key: &Key,
        convert: impl FnOnce() -> Vec<Converted>,
    ) -> Result<&[Converted], CacheRefused> {
        if let Some(index) = self.stations.iter().position(|(held, _)| held == key) {
            return Ok(&self.stations[index].1);
        }
        if self.stations.len() >= MAX_STATION_KEYS {
            return Err(CacheRefused::Full {
                limit: MAX_STATION_KEYS,
            });
        }
        let converted = convert();
        self.stations.push((key.clone(), converted));
        Ok(&self.stations[self.stations.len() - 1].1)
    }

    /// The brewing closure, computing and holding it when absent.
    ///
    /// # Errors
    ///
    /// Returns whatever the caller's own derivation refuses, which for section
    /// 11.1 is the probable-cycle refusal at the cap.
    pub fn brewing<E>(
        &mut self,
        derive: impl FnOnce() -> Result<Vec<Converted>, E>,
    ) -> Result<&[Converted], E> {
        Ok(match &mut self.brewing {
            Some(held) => held,
            vacant @ None => vacant.insert(derive()?),
        })
    }

    /// How many station keys are filed.
    #[must_use]
    pub fn station_count(&self) -> usize {
        self.stations.len()
    }

    /// Whether the brewing closure has been derived.
    #[must_use]
    pub const fn has_brewing(&self) -> bool {
        self.brewing.is_some()
    }

    /// Whether both caches are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stations.is_empty() && self.brewing.is_none()
    }

    /// Empties both.
    ///
    /// Both points empty everything. The original clears the station cache when
    /// a server stops and the brewing closure never, which is defect eight.
    pub fn invalidate(&mut self, _at: Point) {
        self.stations.clear();
        self.brewing = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converted(id: &str) -> Vec<Converted> {
        vec![Converted {
            id: id.to_owned(),
            inputs: Vec::new(),
            output: Vec::new(),
        }]
    }

    #[test]
    fn a_station_collection_is_converted_once_per_key() {
        let mut caches = Caches::new();
        let key = Key::new("minecraft:smelting", "minecraft:overworld");
        let mut conversions = 0;
        for _ in 0..3 {
            caches
                .station(&key, || {
                    conversions += 1;
                    converted("a")
                })
                .expect("room for one key");
        }
        assert_eq!(conversions, 1);
        assert_eq!(caches.station_count(), 1);
    }

    #[test]
    fn the_dimension_is_part_of_the_station_key() {
        let mut caches = Caches::new();
        caches
            .station(
                &Key::new("minecraft:smelting", "minecraft:overworld"),
                || converted("a"),
            )
            .expect("room");
        caches
            .station(
                &Key::new("minecraft:smelting", "minecraft:the_nether"),
                || converted("b"),
            )
            .expect("room");
        assert_eq!(caches.station_count(), 2);
    }

    #[test]
    fn the_station_cache_has_an_enforced_ceiling() {
        let mut caches = Caches::new();
        for index in 0..MAX_STATION_KEYS {
            caches
                .station(
                    &Key::new("minecraft:smelting", &format!("d{index}")),
                    Vec::new,
                )
                .expect("inside the ceiling");
        }
        assert_eq!(
            caches
                .station(&Key::new("minecraft:smelting", "one_too_many"), Vec::new)
                .err(),
            Some(CacheRefused::Full {
                limit: MAX_STATION_KEYS
            })
        );
        assert_eq!(caches.station_count(), MAX_STATION_KEYS);
    }

    #[test]
    fn the_brewing_closure_is_derived_once() {
        let mut caches = Caches::new();
        let mut derivations = 0;
        for _ in 0..3 {
            caches
                .brewing(|| {
                    derivations += 1;
                    Ok::<Vec<Converted>, ()>(converted("brew"))
                })
                .expect("a small table");
        }
        assert_eq!(derivations, 1);
        assert!(caches.has_brewing());
    }

    #[test]
    fn a_derivation_that_refuses_holds_nothing_and_is_tried_again() {
        let mut caches = Caches::new();
        assert_eq!(
            caches.brewing(|| Err::<Vec<Converted>, &str>("a probable cycle")),
            Err("a probable cycle")
        );
        assert!(!caches.has_brewing());
        assert!(
            caches
                .brewing(|| Ok::<Vec<Converted>, &str>(converted("brew")))
                .is_ok()
        );
        assert!(caches.has_brewing());
    }

    #[test]
    fn a_reload_invalidates_both_where_the_original_invalidates_neither() {
        let mut caches = Caches::new();
        caches
            .station(
                &Key::new("minecraft:smelting", "minecraft:overworld"),
                || converted("a"),
            )
            .expect("room");
        caches
            .brewing(|| Ok::<Vec<Converted>, ()>(converted("brew")))
            .expect("a small table");
        assert!(!caches.is_empty());
        caches.invalidate(Point::Reloaded);
        assert!(caches.is_empty());
        assert_eq!(caches.station_count(), 0);
        assert!(!caches.has_brewing());
    }

    #[test]
    fn a_shutdown_invalidates_both_as_well() {
        let mut caches = Caches::new();
        caches
            .brewing(|| Ok::<Vec<Converted>, ()>(converted("brew")))
            .expect("a small table");
        caches.invalidate(Point::Stopped);
        assert!(caches.is_empty());
    }

    #[test]
    fn a_fresh_set_of_caches_is_empty() {
        let caches = Caches::new();
        assert!(caches.is_empty());
        assert_eq!(caches.station_count(), 0);
        assert!(!caches.has_brewing());
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            CacheRefused::Full { limit: 4 }
                .to_string()
                .contains("at most 4")
        );
    }
}
