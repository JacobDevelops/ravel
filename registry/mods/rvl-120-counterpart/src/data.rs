//! The common record every dependent's block carries, from §6.
//!
//! One record shape, registered twice: as an attachment on a block entity and
//! as a data component on an item. The attachment declares the persistence form
//! alone; the item component declares both that and the wire form.
//!
//! Two of the four fields are contracts rather than incidental shapes. The
//! redstone mode **saves** as a short display word that is not the name of its
//! constant, and it **travels** as the constant's ordinal, so the two forms are
//! independent and a counterpart that reorders the constants breaks packets even
//! where its saved words match. The three display words are upstream serialised
//! spellings that the specification deliberately does not carry, so
//! [`Spellings`] reads them from a build-time input and
//! [`Spellings::placeholder`] is what the tests pin the surrounding format with.

use std::fmt;

/// The build-time name the three saved spellings are read from.
pub const SPELLINGS_INPUT: &str = "RVL_120_COUNTERPART_REDSTONE_SPELLINGS";

/// The channel a record carries when no channel has been assigned.
pub const UNASSIGNED_CHANNEL: i32 = -1;

/// What a block does about the redstone around it.
///
/// The ordinal layout is a wire contract in its own right: a packet carries this
/// small integer and neither saved spelling is written, so reordering these
/// three constants changes what every dependent's packets mean.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RedstoneMode {
    /// Redstone is not consulted.
    #[default]
    Ignored,
    /// The block works while the signal is low.
    RequiresLow,
    /// The block works while the signal is high.
    RequiresHigh,
}

impl RedstoneMode {
    /// Every mode, in ordinal order.
    pub const ALL: [Self; 3] = [Self::Ignored, Self::RequiresLow, Self::RequiresHigh];

    /// Returns the ordinal the wire carries.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Ignored => 0,
            Self::RequiresLow => 1,
            Self::RequiresHigh => 2,
        }
    }

    /// Returns the mode an ordinal names, refusing one outside the layout.
    ///
    /// The original's screen adapter indexes its value array with no bounds
    /// check and raises on a mismatched number. This answers `None`, which is
    /// what lets a receiver refuse a packet instead of failing inside it.
    #[must_use]
    pub const fn from_ordinal(ordinal: u8) -> Option<Self> {
        match ordinal {
            0 => Some(Self::Ignored),
            1 => Some(Self::RequiresLow),
            2 => Some(Self::RequiresHigh),
            _ => None,
        }
    }
}

/// The three words the redstone mode is written as in saved data.
///
/// These are compatibility-required serialised spellings that only the pinned
/// artifact carries. Nothing here invents them: a build that means to read the
/// original's saves supplies them, and a build that does not gets
/// [`Spellings::placeholder`], whose words are this package's own and are only
/// ever round-tripped against themselves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spellings {
    ignored: String,
    requires_low: String,
    requires_high: String,
}

impl Default for Spellings {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl Spellings {
    /// The words the tests pin the saved format against.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            ignored: "unheld".to_owned(),
            requires_low: "held_low".to_owned(),
            requires_high: "held_high".to_owned(),
        }
    }

    /// Reads the three words from a comma-separated build-time input.
    ///
    /// # Errors
    ///
    /// Returns [`SpellingsUnavailable`] when the build supplied nothing, or
    /// supplied something that is not three distinct non-empty words. Two modes
    /// sharing a word would make a save unreadable in one direction, so the
    /// distinctness is checked here rather than discovered later.
    pub fn declared() -> Result<Self, SpellingsUnavailable> {
        Self::supplied(option_env!("RVL_120_COUNTERPART_REDSTONE_SPELLINGS"))
    }

    /// Reads the three words a build supplied, or refuses their absence.
    ///
    /// Taking the supplied text as an argument is what makes the absence
    /// testable in a build that legitimately supplies one: a test that asserted
    /// the variable is unset would pass only where it happens to be.
    ///
    /// # Errors
    ///
    /// Returns [`SpellingsUnavailable::NotSupplied`] for nothing at all, and
    /// whatever [`Spellings::parse`] refuses otherwise.
    pub fn supplied(text: Option<&str>) -> Result<Self, SpellingsUnavailable> {
        Self::parse(text.ok_or(SpellingsUnavailable::NotSupplied)?)
    }

    /// Reads the three words from a comma-separated list.
    ///
    /// # Errors
    ///
    /// Returns [`SpellingsUnavailable::Malformed`] for anything that is not
    /// three distinct non-empty words in ordinal order.
    pub fn parse(text: &str) -> Result<Self, SpellingsUnavailable> {
        let words: Vec<&str> = text.split(',').map(str::trim).collect();
        let [ignored, requires_low, requires_high] = words.as_slice() else {
            return Err(SpellingsUnavailable::Malformed);
        };
        if [ignored, requires_low, requires_high]
            .iter()
            .any(|word| word.is_empty())
            || ignored == requires_low
            || ignored == requires_high
            || requires_low == requires_high
        {
            return Err(SpellingsUnavailable::Malformed);
        }
        Ok(Self {
            ignored: (*ignored).to_owned(),
            requires_low: (*requires_low).to_owned(),
            requires_high: (*requires_high).to_owned(),
        })
    }

    /// Returns the word `mode` is saved as.
    #[must_use]
    pub fn word(&self, mode: RedstoneMode) -> &str {
        match mode {
            RedstoneMode::Ignored => &self.ignored,
            RedstoneMode::RequiresLow => &self.requires_low,
            RedstoneMode::RequiresHigh => &self.requires_high,
        }
    }

    /// Returns the mode a saved word names, refusing an unknown one.
    #[must_use]
    pub fn mode(&self, word: &str) -> Option<RedstoneMode> {
        RedstoneMode::ALL
            .into_iter()
            .find(|&mode| self.word(mode) == word)
    }
}

/// Why the build-time spellings did not resolve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellingsUnavailable {
    /// The build named no spellings at all.
    NotSupplied,
    /// The build named something that is not three distinct words.
    Malformed,
}

impl fmt::Display for SpellingsUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupplied => write!(
                formatter,
                "the three saved redstone words are compatibility-required and \
                 nothing supplied `{SPELLINGS_INPUT}` at build time"
            ),
            Self::Malformed => write!(
                formatter,
                "`{SPELLINGS_INPUT}` is not three distinct non-empty words in ordinal order"
            ),
        }
    }
}

impl std::error::Error for SpellingsUnavailable {}

/// A player's stable identity, as the record carries it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Identity(String);

impl Identity {
    /// The most characters an identity read off the wire may carry.
    pub const MAX_LENGTH: usize = 64;

    /// Builds an identity, refusing an empty or over-long one.
    #[must_use]
    pub fn new(text: &str) -> Option<Self> {
        if text.is_empty() || text.len() > Self::MAX_LENGTH {
            return None;
        }
        Some(Self(text.to_owned()))
    }

    /// Returns the identity as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The record every dependent's block and item carries.
///
/// The name is not optional and carries the empty string when unowned; the
/// identity is optional and is absent then. The two can therefore disagree, and
/// a record with a name and no identity is representable and is exactly what the
/// default constructs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockData {
    /// The name of the player who placed it, empty when unowned.
    pub owner_name: String,
    /// That player's stable identity, absent when unowned.
    pub owner: Option<Identity>,
    /// The security channel, [`UNASSIGNED_CHANNEL`] when unassigned.
    pub channel: i32,
    /// What the block does about the redstone around it.
    pub redstone: RedstoneMode,
}

impl Default for BlockData {
    fn default() -> Self {
        Self {
            owner_name: String::new(),
            owner: None,
            channel: UNASSIGNED_CHANNEL,
            redstone: RedstoneMode::Ignored,
        }
    }
}

impl BlockData {
    /// The most characters an owner name read off the wire may carry.
    pub const MAX_NAME: usize = 64;

    /// Records who placed the block, when the server setting says to.
    ///
    /// A record made with the setting off keeps the unowned default, which is
    /// what makes the security channel meaningless in that configuration. The
    /// name is bounded because it reaches this record from a player profile.
    #[must_use]
    pub fn placed_by(name: &str, owner: Option<Identity>, record_owner: bool) -> Self {
        if !record_owner {
            return Self::default();
        }
        let mut bounded = name.to_owned();
        while bounded.len() > Self::MAX_NAME {
            bounded.pop();
        }
        Self {
            owner_name: bounded,
            owner,
            ..Self::default()
        }
    }

    /// Returns whether the record names an owner at all.
    #[must_use]
    pub fn is_owned(&self) -> bool {
        !self.owner_name.is_empty() || self.owner.is_some()
    }

    /// Writes the record in its saved form.
    ///
    /// # Errors
    ///
    /// Returns [`DataRefused::Field`] for an owner name longer than
    /// [`BlockData::from_saved`] admits. The bound belongs on both sides: the
    /// name field is public, so a caller can hold one this record could not read
    /// back, and the refusal belongs at the write rather than at the next load.
    pub fn to_saved(&self, spellings: &Spellings) -> Result<ravel_sdk::JsonValue, DataRefused> {
        if self.owner_name.len() > Self::MAX_NAME {
            return Err(DataRefused::Field { field: "name" });
        }
        let mut saved = ravel_sdk::json!({
            "name": self.owner_name,
            "channel": self.channel,
            "redstone": spellings.word(self.redstone),
        });
        if let Some(owner) = &self.owner {
            saved["owner"] = ravel_sdk::JsonValue::from(owner.as_str());
        }
        Ok(saved)
    }

    /// Reads a record back from its saved form.
    ///
    /// # Errors
    ///
    /// Returns [`DataRefused`] when a field is missing, is the wrong shape, is
    /// longer than the record admits, or names a redstone word the build does
    /// not know. Saved data is read back from a file a pack or a player may have
    /// edited, so every field is checked rather than defaulted.
    pub fn from_saved(
        saved: &ravel_sdk::JsonValue,
        spellings: &Spellings,
    ) -> Result<Self, DataRefused> {
        let name = saved
            .get("name")
            .and_then(ravel_sdk::JsonValue::as_str)
            .ok_or(DataRefused::Field { field: "name" })?;
        if name.len() > Self::MAX_NAME {
            return Err(DataRefused::Field { field: "name" });
        }
        let owner = match saved.get("owner") {
            None => None,
            Some(value) => {
                let text = value
                    .as_str()
                    .ok_or(DataRefused::Field { field: "owner" })?;
                Some(Identity::new(text).ok_or(DataRefused::Field { field: "owner" })?)
            }
        };
        let channel = saved
            .get("channel")
            .and_then(ravel_sdk::JsonValue::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or(DataRefused::Field { field: "channel" })?;
        let word = saved
            .get("redstone")
            .and_then(ravel_sdk::JsonValue::as_str)
            .ok_or(DataRefused::Field { field: "redstone" })?;
        let redstone = spellings.mode(word).ok_or_else(|| DataRefused::Redstone {
            word: word.to_owned(),
        })?;
        Ok(Self {
            owner_name: name.to_owned(),
            owner,
            channel,
            redstone,
        })
    }

    /// Writes the record in its wire form.
    ///
    /// The redstone mode goes out as its ordinal and neither saved word is
    /// written, which is the second contract of §6.
    #[must_use]
    pub fn to_wire(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_text(&mut bytes, &self.owner_name);
        match &self.owner {
            None => bytes.push(0),
            Some(owner) => {
                bytes.push(1);
                write_text(&mut bytes, owner.as_str());
            }
        }
        bytes.extend_from_slice(&self.channel.to_be_bytes());
        bytes.push(self.redstone.ordinal());
        bytes
    }

    /// Reads a record back from its wire form.
    ///
    /// # Errors
    ///
    /// Returns [`DataRefused`] for a buffer that ends inside a field, announces
    /// a length it does not carry, carries text that is not UTF-8, or carries a
    /// redstone ordinal outside the three the layout declares.
    pub fn from_wire(bytes: &[u8]) -> Result<Self, DataRefused> {
        let mut reader = Reader { bytes, read: 0 };
        let owner_name = reader.text(Self::MAX_NAME)?;
        let owner = match reader.byte()? {
            0 => None,
            1 => {
                let text = reader.text(Identity::MAX_LENGTH)?;
                Some(Identity::new(&text).ok_or(DataRefused::Field { field: "owner" })?)
            }
            _ => return Err(DataRefused::Field { field: "owner" }),
        };
        let channel = reader.integer()?;
        let ordinal = reader.byte()?;
        let redstone =
            RedstoneMode::from_ordinal(ordinal).ok_or(DataRefused::Ordinal { ordinal })?;
        if reader.read != bytes.len() {
            return Err(DataRefused::Trailing);
        }
        Ok(Self {
            owner_name,
            owner,
            channel,
            redstone,
        })
    }
}

/// Why a record was not read back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataRefused {
    /// A field is missing, the wrong shape, or longer than the record admits.
    Field {
        /// Which field.
        field: &'static str,
    },
    /// The buffer ended inside a field.
    Truncated,
    /// The buffer carried bytes after the record ended.
    Trailing,
    /// The redstone ordinal is outside the three the layout declares.
    Ordinal {
        /// What the buffer carried.
        ordinal: u8,
    },
    /// The saved redstone word is not one this build knows.
    Redstone {
        /// What the document carried.
        word: String,
    },
}

impl fmt::Display for DataRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field { field } => write!(
                formatter,
                "`{field}` is missing or is not what it should be"
            ),
            Self::Truncated => formatter.write_str("the record ended inside a field"),
            Self::Trailing => {
                formatter.write_str("the record ended with bytes nothing accounted for")
            }
            Self::Ordinal { ordinal } => write!(
                formatter,
                "{ordinal} names no redstone mode; the layout declares three"
            ),
            Self::Redstone { word } => write!(
                formatter,
                "`{word}` is not a redstone word this build knows"
            ),
        }
    }
}

impl std::error::Error for DataRefused {}

fn write_text(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&crate::arithmetic::signed_len(text.len()).to_be_bytes());
    bytes.extend_from_slice(text.as_bytes());
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    read: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], DataRefused> {
        let end = self.read.checked_add(count).ok_or(DataRefused::Truncated)?;
        if end > self.bytes.len() {
            return Err(DataRefused::Truncated);
        }
        let taken = &self.bytes[self.read..end];
        self.read = end;
        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8, DataRefused> {
        Ok(self.take(1)?[0])
    }

    fn integer(&mut self) -> Result<i32, DataRefused> {
        let bytes = self.take(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn text(&mut self, limit: usize) -> Result<String, DataRefused> {
        let announced = self.integer()?;
        let length = usize::try_from(announced).map_err(|_| DataRefused::Truncated)?;
        if length > limit {
            return Err(DataRefused::Field { field: "text" });
        }
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| DataRefused::Field { field: "text" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_record_is_unowned_unassigned_and_ignores_redstone() {
        let data = BlockData::default();
        assert_eq!(data.owner_name, "");
        assert_eq!(data.owner, None);
        assert_eq!(data.channel, -1);
        assert_eq!(data.redstone, RedstoneMode::Ignored);
        assert!(!data.is_owned());
    }

    #[test]
    fn a_name_without_an_identity_is_representable_and_is_what_the_default_shape_admits() {
        let data = BlockData {
            owner_name: "somebody".to_owned(),
            ..BlockData::default()
        };
        assert!(data.is_owned());
        assert_eq!(data.owner, None);
    }

    #[test]
    fn the_three_ordinals_are_zero_one_and_two_in_that_order() {
        assert_eq!(RedstoneMode::Ignored.ordinal(), 0);
        assert_eq!(RedstoneMode::RequiresLow.ordinal(), 1);
        assert_eq!(RedstoneMode::RequiresHigh.ordinal(), 2);
        for mode in RedstoneMode::ALL {
            assert_eq!(RedstoneMode::from_ordinal(mode.ordinal()), Some(mode));
        }
        assert_eq!(RedstoneMode::from_ordinal(3), None);
        assert_eq!(RedstoneMode::from_ordinal(255), None);
    }

    #[test]
    fn the_placeholder_spellings_are_three_distinct_words_and_are_not_the_constant_names() {
        let spellings = Spellings::placeholder();
        let words: Vec<&str> = RedstoneMode::ALL
            .into_iter()
            .map(|mode| spellings.word(mode))
            .collect();
        assert_eq!(words.len(), 3);
        for name in ["Ignored", "RequiresLow", "RequiresHigh"] {
            assert!(
                !words.contains(&name),
                "a saved word is not a constant name"
            );
        }
        for mode in RedstoneMode::ALL {
            assert_eq!(spellings.mode(spellings.word(mode)), Some(mode));
        }
        assert_eq!(spellings.mode("nothing_spells_this"), None);
    }

    #[test]
    fn supplied_spellings_are_three_distinct_non_empty_words_or_they_are_refused() {
        assert_eq!(
            Spellings::parse("a,b,c")
                .expect("three words")
                .word(RedstoneMode::RequiresLow),
            "b"
        );
        for text in ["", "a", "a,b", "a,b,c,d", "a,,c", "a,b,a"] {
            assert_eq!(
                Spellings::parse(text),
                Err(SpellingsUnavailable::Malformed),
                "`{text}` is not three distinct words"
            );
        }
    }

    #[test]
    fn the_saved_spellings_are_absent_rather_than_guessed_when_the_build_supplies_none() {
        assert_eq!(
            Spellings::supplied(None),
            Err(SpellingsUnavailable::NotSupplied)
        );
        assert_eq!(
            Spellings::supplied(Some("a,b,c")),
            Spellings::parse("a,b,c")
        );
        assert_eq!(
            Spellings::declared(),
            Spellings::supplied(option_env!("RVL_120_COUNTERPART_REDSTONE_SPELLINGS")),
            "what this build supplies is what the package reads, whatever that is"
        );
    }

    #[test]
    fn an_owner_name_longer_than_a_load_admits_is_refused_at_the_save() {
        let spellings = Spellings::placeholder();
        let data = BlockData {
            owner_name: "n".repeat(BlockData::MAX_NAME + 1),
            ..BlockData::default()
        };
        assert_eq!(
            data.to_saved(&spellings),
            Err(DataRefused::Field { field: "name" }),
            "the writer is refused rather than the next load"
        );
    }

    #[test]
    fn a_record_round_trips_through_its_saved_form_including_the_unowned_case() {
        let spellings = Spellings::placeholder();
        for data in [
            BlockData::default(),
            BlockData {
                owner_name: "somebody".to_owned(),
                owner: Identity::new("0123-4567"),
                channel: 7,
                redstone: RedstoneMode::RequiresHigh,
            },
        ] {
            let saved = data
                .to_saved(&spellings)
                .expect("an ordinary record writes");
            assert_eq!(
                BlockData::from_saved(&saved, &spellings).expect("what was written reads back"),
                data
            );
        }
    }

    #[test]
    fn an_unowned_record_writes_no_placeholder_identity_in_either_form() {
        let spellings = Spellings::placeholder();
        let saved = BlockData::default()
            .to_saved(&spellings)
            .expect("the unowned default writes");
        assert!(saved.get("owner").is_none(), "absence is absence");
        assert_eq!(saved["name"], "");

        let wire = BlockData::default().to_wire();
        assert_eq!(
            BlockData::from_wire(&wire).expect("what was written reads back"),
            BlockData::default()
        );
    }

    #[test]
    fn the_saved_form_writes_the_display_word_and_the_wire_form_writes_the_ordinal() {
        let spellings = Spellings::placeholder();
        let data = BlockData {
            redstone: RedstoneMode::RequiresLow,
            ..BlockData::default()
        };
        assert_eq!(
            data.to_saved(&spellings)
                .expect("an ordinary record writes")["redstone"],
            "held_low"
        );
        assert_eq!(
            *data
                .to_wire()
                .last()
                .expect("the wire ends with the ordinal"),
            1
        );
    }

    #[test]
    fn a_saved_document_naming_an_unknown_redstone_word_is_refused() {
        let spellings = Spellings::placeholder();
        let saved = ravel_sdk::json!({
            "name": "", "channel": -1, "redstone": "RequiresLow"
        });
        assert!(matches!(
            BlockData::from_saved(&saved, &spellings),
            Err(DataRefused::Redstone { .. })
        ));
    }

    #[test]
    fn a_saved_document_missing_a_field_or_holding_the_wrong_shape_is_refused() {
        let spellings = Spellings::placeholder();
        for saved in [
            ravel_sdk::json!({ "channel": 0, "redstone": "unheld" }),
            ravel_sdk::json!({ "name": 4, "channel": 0, "redstone": "unheld" }),
            ravel_sdk::json!({ "name": "", "redstone": "unheld" }),
            ravel_sdk::json!({ "name": "", "channel": 1e18, "redstone": "unheld" }),
            ravel_sdk::json!({ "name": "", "channel": 0 }),
            ravel_sdk::json!({ "name": "", "channel": 0, "redstone": "unheld", "owner": 7 }),
        ] {
            assert!(
                BlockData::from_saved(&saved, &spellings).is_err(),
                "{saved} must not read back"
            );
        }
    }

    #[test]
    fn a_wire_record_round_trips_and_refuses_every_way_it_can_be_wrong() {
        let data = BlockData {
            owner_name: "somebody".to_owned(),
            owner: Identity::new("0123"),
            channel: -9,
            redstone: RedstoneMode::RequiresHigh,
        };
        let wire = data.to_wire();
        assert_eq!(BlockData::from_wire(&wire).expect("a legal record"), data);

        assert_eq!(
            BlockData::from_wire(&wire[..wire.len() - 1]),
            Err(DataRefused::Truncated)
        );

        let mut trailing = wire.clone();
        trailing.push(0);
        assert_eq!(BlockData::from_wire(&trailing), Err(DataRefused::Trailing));

        let mut bad_ordinal = wire;
        let last = bad_ordinal.len() - 1;
        bad_ordinal[last] = 9;
        assert_eq!(
            BlockData::from_wire(&bad_ordinal),
            Err(DataRefused::Ordinal { ordinal: 9 })
        );
    }

    #[test]
    fn a_wire_record_announcing_a_length_it_does_not_carry_is_refused_before_it_allocates() {
        let mut lying = BlockData::default().to_wire();
        lying[0..4].copy_from_slice(&i32::MAX.to_be_bytes());
        assert!(BlockData::from_wire(&lying).is_err());

        let mut negative = BlockData::default().to_wire();
        negative[0..4].copy_from_slice(&(-1_i32).to_be_bytes());
        assert_eq!(BlockData::from_wire(&negative), Err(DataRefused::Truncated));
    }

    #[test]
    fn an_over_long_name_or_identity_is_refused_rather_than_stored() {
        let long = "n".repeat(BlockData::MAX_NAME + 1);
        let mut wire = Vec::new();
        write_text(&mut wire, &long);
        wire.push(0);
        wire.extend_from_slice(&0_i32.to_be_bytes());
        wire.push(0);
        assert!(BlockData::from_wire(&wire).is_err());

        assert_eq!(Identity::new(""), None);
        assert_eq!(Identity::new(&"i".repeat(Identity::MAX_LENGTH + 1)), None);
    }

    #[test]
    fn ownership_is_recorded_only_when_the_server_setting_says_to() {
        let owner = Identity::new("0123");
        let recorded = BlockData::placed_by("somebody", owner.clone(), true);
        assert_eq!(recorded.owner_name, "somebody");
        assert_eq!(recorded.owner, owner);

        let unrecorded = BlockData::placed_by("somebody", owner, false);
        assert_eq!(unrecorded, BlockData::default());
        assert!(!unrecorded.is_owned());
    }

    #[test]
    fn a_placing_player_name_is_bounded_rather_than_stored_whole() {
        let long = "n".repeat(BlockData::MAX_NAME * 2);
        let recorded = BlockData::placed_by(&long, None, true);
        assert_eq!(recorded.owner_name.len(), BlockData::MAX_NAME);
    }
}
