use std::fmt;

use ravel_core::{ModId, ModIdError};
use serde::{Deserialize, Serialize};

const SHAPING_FORMAT_VERSION: u8 = 1;

/// The deduplicated world-shaping mods, in declaration order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorldShaping {
    mods: Vec<ModId>,
}

impl WorldShaping {
    /// Declares a world-shaping mod, ignoring duplicate declarations.
    pub fn declare(&mut self, shaper: ModId) {
        if !self.mods.contains(&shaper) {
            self.mods.push(shaper);
        }
    }

    /// Returns the shaping mods, in declaration order.
    #[must_use]
    pub fn mods(&self) -> &[ModId] {
        &self.mods
    }

    /// Returns whether nothing shapes the world.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mods.is_empty()
    }
}

/// The creation-time shaping set; admission refuses both added and removed mods.
///
/// # Examples
///
/// ```
/// use ravel_core::ModId;
/// use ravel_save::{ShapingRecord, WorldShaping};
///
/// let terrain: ModId = "terrain_mod".parse()?;
/// let caves: ModId = "cave_mod".parse()?;
///
/// let mut at_creation = WorldShaping::default();
/// at_creation.declare(terrain.clone());
/// at_creation.declare(caves.clone());
/// let written = ShapingRecord::at_creation(&at_creation);
///
/// written.admit(&at_creation).expect("what made the world may open it");
///
/// let mut without_caves = WorldShaping::default();
/// without_caves.declare(terrain);
/// let refusal = written
///     .admit(&without_caves)
///     .expect_err("a world is refused when what shaped it is gone");
/// assert_eq!(refusal.missing(), [caves]);
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapingRecord {
    mods: Vec<ModId>,
}

/// The record as it is written, and the only shape [`ShapingRecord::read`] takes.
#[derive(Deserialize, Serialize)]
struct StoredShaping {
    format: u8,
    shaping: Vec<String>,
}

impl ShapingRecord {
    /// Records the shaping set a world is being made with.
    #[must_use]
    pub fn at_creation(shaping: &WorldShaping) -> Self {
        Self {
            mods: shaping.mods().to_vec(),
        }
    }

    /// Returns the mods the record names, in the order it names them.
    #[must_use]
    pub fn mods(&self) -> &[ModId] {
        &self.mods
    }

    /// Serializes the record deterministically in declaration order.
    ///
    /// # Panics
    ///
    /// Panics if the format number and mod-id strings cannot be serialized.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&StoredShaping {
            format: SHAPING_FORMAT_VERSION,
            shaping: self.mods.iter().map(|id| id.as_str().to_owned()).collect(),
        })
        .expect("a number and a list of strings always serialize")
    }

    /// Reads a record a world stored.
    ///
    /// # Errors
    ///
    /// Returns [`ShapingError::Unreadable`] for invalid records or [`ShapingError::IllegalOwner`] for invalid mod ids.
    pub fn read(bytes: &[u8]) -> Result<Self, ShapingError> {
        let stored: StoredShaping =
            serde_json::from_slice(bytes).map_err(|source| ShapingError::Unreadable {
                detail: source.to_string(),
            })?;
        if stored.format != SHAPING_FORMAT_VERSION {
            return Err(ShapingError::Unreadable {
                detail: format!(
                    "the record is format {}, and this build writes and reads format {}",
                    stored.format, SHAPING_FORMAT_VERSION
                ),
            });
        }

        let mut mods = Vec::with_capacity(stored.shaping.len());
        for candidate in stored.shaping {
            let id = candidate
                .parse::<ModId>()
                .map_err(|source| ShapingError::IllegalOwner {
                    candidate: candidate.clone(),
                    source,
                })?;
            mods.push(id);
        }
        Ok(Self { mods })
    }

    /// Answers whether `now` may open the world this record came from.
    ///
    /// # Errors
    ///
    /// Returns [`ShapingMismatch`] with all removed and newly added shaping mods.
    pub fn admit(&self, now: &WorldShaping) -> Result<(), ShapingMismatch> {
        let missing: Vec<ModId> = self
            .mods
            .iter()
            .filter(|id| !now.mods().contains(id))
            .cloned()
            .collect();
        let arrived: Vec<ModId> = now
            .mods()
            .iter()
            .filter(|id| !self.mods.contains(id))
            .cloned()
            .collect();

        if missing.is_empty() && arrived.is_empty() {
            return Ok(());
        }
        Err(ShapingMismatch { missing, arrived })
    }
}

/// Why bytes a world stored are not a shaping record this build can read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapingError {
    /// The bytes are not a shaping record this build can read.
    Unreadable {
        /// What made them unreadable, in the reader's words.
        detail: String,
    },
    /// The record's shape is right and a name inside it is not a mod id.
    IllegalOwner {
        /// The name that is not a mod id.
        candidate: String,
        /// Why it is not one.
        source: ModIdError,
    },
}

impl ShapingError {
    /// Returns whether this build cannot read the bytes as a shaping record.
    #[must_use]
    pub const fn is_unreadable(&self) -> bool {
        matches!(self, Self::Unreadable { .. })
    }
}

impl fmt::Display for ShapingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { detail } => write!(
                formatter,
                "this build cannot read these bytes as a world shaping record: {detail}"
            ),
            Self::IllegalOwner { candidate, source } => write!(
                formatter,
                "a shaping record names `{candidate}`, which is not a mod id: {source}"
            ),
        }
    }
}

impl std::error::Error for ShapingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { .. } => None,
            Self::IllegalOwner { source, .. } => Some(source),
        }
    }
}

/// Why a world is not opened by the set of mods now installed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapingMismatch {
    missing: Vec<ModId>,
    arrived: Vec<ModId>,
}

impl ShapingMismatch {
    /// Returns the mods that shaped the world and are no longer supplied.
    #[must_use]
    pub fn missing(&self) -> &[ModId] {
        &self.missing
    }

    /// Returns the mods that would shape the world now and did not at creation.
    #[must_use]
    pub fn arrived(&self) -> &[ModId] {
        &self.arrived
    }
}

impl fmt::Display for ShapingMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("this world is not opened by the mods now installed")?;
        if !self.missing.is_empty() {
            write!(
                formatter,
                "; {} shaped it and is gone",
                ModId::display_list(&self.missing)
            )?;
        }
        if !self.arrived.is_empty() {
            write!(
                formatter,
                "; {} would shape it now and did not then",
                ModId::display_list(&self.arrived)
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ShapingMismatch {}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{ShapingError, ShapingRecord, WorldShaping};

    use crate::tests::id;

    fn shaping(names: &[&str]) -> WorldShaping {
        let mut shaping = WorldShaping::default();
        for name in names {
            shaping.declare(id(name));
        }
        shaping
    }

    #[test]
    fn the_shaping_set_written_at_creation_is_read_back_equal() {
        let written = ShapingRecord::at_creation(&shaping(&["sample", "probe"]));
        let read = ShapingRecord::read(&written.to_bytes()).expect("what was written reads back");

        assert_eq!(read.mods(), [id("sample"), id("probe")]);
        assert_eq!(read, written);
    }

    // Pin the stored format independently of the matching reader.
    #[test]
    fn the_written_bytes_are_the_document_this_test_spells_out() {
        let written = ShapingRecord::at_creation(&shaping(&["sample", "probe"]));

        assert_eq!(
            String::from_utf8(written.to_bytes()).expect("the record is UTF-8"),
            r#"{"format":1,"shaping":["sample","probe"]}"#
        );

        let document: Value =
            serde_json::from_slice(&written.to_bytes()).expect("the record is a JSON document");
        assert_eq!(document["format"], 1);
        assert_eq!(document["shaping"][0], "sample");
        assert_eq!(document["shaping"][1], "probe");
        assert_eq!(
            document["shaping"]
                .as_array()
                .expect("the shaping set is an array")
                .len(),
            2
        );
    }

    #[test]
    fn a_record_the_capability_did_not_write_is_refused() {
        let refusal = ShapingRecord::read(b"not a shaping record")
            .expect_err("bytes from nowhere are refused rather than guessed at");

        assert!(refusal.is_unreadable());
        assert!(
            refusal
                .to_string()
                .starts_with("this build cannot read these bytes as a world shaping record:")
        );
    }

    #[test]
    fn a_document_of_another_format_number_is_refused_rather_than_read() {
        let refusal = ShapingRecord::read(br#"{"format":2,"shaping":[]}"#)
            .expect_err("a format this build does not write is not read");

        assert!(refusal.is_unreadable());
        assert!(refusal.to_string().contains('2'));
        assert!(
            refusal
                .to_string()
                .starts_with("this build cannot read these bytes as a world shaping record:")
        );
    }

    #[test]
    fn a_record_naming_something_that_is_not_a_mod_id_is_refused_and_names_it() {
        let refusal = ShapingRecord::read(br#"{"format":1,"shaping":["Not An Id"]}"#)
            .expect_err("a name that is not a mod id is refused");

        assert!(!refusal.is_unreadable());
        assert!(matches!(refusal, ShapingError::IllegalOwner { .. }));
        assert!(refusal.to_string().contains("Not An Id"));
    }

    #[test]
    fn an_empty_shaping_set_round_trips_as_an_empty_set() {
        let written = ShapingRecord::at_creation(&WorldShaping::default());
        assert_eq!(
            String::from_utf8(written.to_bytes()).expect("the record is UTF-8"),
            r#"{"format":1,"shaping":[]}"#
        );
        assert_eq!(
            ShapingRecord::read(&written.to_bytes())
                .expect("an empty record reads back")
                .mods(),
            []
        );
    }

    #[test]
    fn a_world_is_refused_when_a_shaping_mod_is_gone() {
        let written = ShapingRecord::at_creation(&shaping(&["sample", "probe"]));

        let refusal = written
            .admit(&shaping(&["sample"]))
            .expect_err("a world is refused when what shaped it is gone");

        assert_eq!(refusal.missing(), [id("probe")]);
        assert_eq!(refusal.arrived(), []);
        assert!(refusal.to_string().contains("probe"));
    }

    #[test]
    fn a_shaping_mod_added_after_creation_is_refused_too() {
        let written = ShapingRecord::at_creation(&shaping(&["sample"]));

        let refusal = written
            .admit(&shaping(&["sample", "probe"]))
            .expect_err("a world is refused when something new would shape it");

        assert_eq!(refusal.arrived(), [id("probe")]);
        assert_eq!(refusal.missing(), []);
    }

    #[test]
    fn the_same_set_in_another_order_still_opens_the_world() {
        let written = ShapingRecord::at_creation(&shaping(&["sample", "probe"]));
        written
            .admit(&shaping(&["probe", "sample"]))
            .expect("the set is what shapes a world, not the order it resolved in");
    }

    #[test]
    fn declaring_one_mod_twice_declares_it_once() {
        let mut shaping = WorldShaping::default();
        shaping.declare(id("sample"));
        shaping.declare(id("sample"));

        assert_eq!(shaping.mods(), [id("sample")]);
        assert!(!shaping.is_empty());
    }
}
