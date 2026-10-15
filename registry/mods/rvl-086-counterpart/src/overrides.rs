//! The repair-material override table of section seven.
//!
//! An entry is written as a tool selector, an equals sign, and a material
//! selector that may carry an `@` and a multiplier. A selector is an entry
//! identifier or the `tag:` form, told apart by counting colon-separated fields
//! exactly as [`crate::matching`] does.
//!
//! Three departures from the original, each declared in `NOTES.md` and each
//! tested here:
//!
//! - selectors are kept as written and matched when a repair asks, so a data
//!   reload and a late registration both reach this table. The original resolves
//!   every identifier once at startup and is deaf to both afterwards;
//! - a selector carrying no colon takes the supplied default namespace instead
//!   of being refused as a syntax error;
//! - a multiplier that is not above zero is refused when the table is read. The
//!   original compares a value against zero for exact equality, so a negative
//!   multiplier survives that test and subtracts from the running repair, which
//!   is a configuration-driven way to hand a player back a more damaged tool.
//!
//! Order is preserved and is significant: a lookup returns the first entry whose
//! two selectors both match.

use std::fmt;

use crate::matching::Expression;

/// The multiplier an entry that names none carries.
pub const DEFAULT_MULTIPLIER: f64 = 1.0;

/// The separator between the two selectors.
pub const PAIR_SEPARATOR: char = '=';

/// The separator that introduces a multiplier.
pub const MULTIPLIER_SEPARATOR: char = '@';

/// The most entries one table will hold.
///
/// A table is written by hand into a configuration file, so its length is
/// untrusted and a lookup is a linear scan. The ceiling is far above any table a
/// pack writes and far below a scan anyone notices; a pack that genuinely needs
/// more raises it here.
pub const MAXIMUM_ENTRIES: usize = 4096;

/// Why one written entry was not read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Rejected {
    /// The entry does not hold exactly one pair separator.
    NotAPair,
    /// One of the two selectors is not a shape the language admits.
    UnreadableSelector,
    /// The multiplier is not a number.
    UnreadableMultiplier,
    /// The multiplier is a number that is not above zero.
    NonPositiveMultiplier,
    /// The table already holds as many entries as it will hold.
    TableFull,
}

impl fmt::Display for Rejected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self {
            Self::NotAPair => {
                "an override is a tool selector, an equals sign and a material selector"
            }
            Self::UnreadableSelector => "a selector is an entry identifier or the tag form",
            Self::UnreadableMultiplier => "the multiplier after the separator is not a number",
            Self::NonPositiveMultiplier => {
                "a multiplier at or below zero would damage the tool it repairs"
            }
            Self::TableFull => "the table already holds as many overrides as it will hold",
        };
        formatter.write_str(said)
    }
}

impl std::error::Error for Rejected {}

/// One row: which tool, which material, and what one of it is worth.
#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    /// The tool this row speaks about.
    pub tool: Expression,
    /// The material this row speaks about.
    pub material: Expression,
    /// What one of that material is worth, above zero.
    pub multiplier: f64,
}

/// One written entry that was skipped, and why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Skipped {
    /// The line as it was written.
    pub written: String,
    /// What was wrong with it.
    pub reason: Rejected,
}

/// The whole table, in the order it was written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    rows: Vec<Override>,
    skipped: Vec<Skipped>,
}

impl Table {
    /// Reads a comma-separated setting into a table.
    ///
    /// A written entry that cannot be read is skipped and recorded; the rest of
    /// the table still loads, which is what keeps one mistyped line from
    /// stopping a launch. `default_namespace` is what a selector carrying no
    /// colon is completed with; it is supplied rather than written here because
    /// the host's own namespace is not this package's to name.
    #[must_use]
    pub fn parse(setting: &str, default_namespace: &str) -> Self {
        let mut table = Self::default();
        for written in setting.split(',').map(str::trim) {
            if written.is_empty() {
                continue;
            }
            match Self::read_entry(written, default_namespace) {
                Ok(row) if table.rows.len() < MAXIMUM_ENTRIES => table.rows.push(row),
                Ok(_) => table.skipped.push(Skipped {
                    written: written.to_owned(),
                    reason: Rejected::TableFull,
                }),
                Err(reason) => table.skipped.push(Skipped {
                    written: written.to_owned(),
                    reason,
                }),
            }
        }
        table
    }

    fn read_entry(written: &str, default_namespace: &str) -> Result<Override, Rejected> {
        let mut halves = written.split(PAIR_SEPARATOR);
        let (Some(left), Some(right), None) = (halves.next(), halves.next(), halves.next()) else {
            return Err(Rejected::NotAPair);
        };
        let (material_text, multiplier) = match right.split_once(MULTIPLIER_SEPARATOR) {
            None => (right.trim(), DEFAULT_MULTIPLIER),
            Some((material, written_multiplier)) => {
                let read = written_multiplier
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| Rejected::UnreadableMultiplier)?;
                if !read.is_finite() || read <= 0.0 {
                    return Err(Rejected::NonPositiveMultiplier);
                }
                (material.trim(), read)
            }
        };
        let tool = selector(left.trim(), default_namespace)?;
        let material = selector(material_text, default_namespace)?;
        Ok(Override {
            tool,
            material,
            multiplier,
        })
    }

    /// Returns the rows this table holds, in written order.
    #[must_use]
    pub fn rows(&self) -> &[Override] {
        &self.rows
    }

    /// Returns every written entry that was skipped, and why.
    ///
    /// A caller logs these once. They are the whole of what the original
    /// reports, and reporting them is what makes a mistyped override findable.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    /// Returns the multiplier for a pair, or nothing when no row matches both.
    ///
    /// The scan stops at the first row whose two selectors both match, so the
    /// order a pack wrote decides which of two overlapping rows wins.
    #[must_use]
    pub fn multiplier(
        &self,
        tool: &str,
        material: &str,
        tool_tagged: &dyn Fn(&str) -> bool,
        material_tagged: &dyn Fn(&str) -> bool,
    ) -> Option<f64> {
        self.rows
            .iter()
            .find(|row| {
                row.tool.matches(tool, tool_tagged)
                    && row.material.matches(material, material_tagged)
            })
            .map(|row| row.multiplier)
    }

    /// Returns whether any row speaks about this tool at all.
    ///
    /// This is the tool-level test the override-mode branch of section six uses,
    /// and it deliberately ignores the material: listing a tool once is what
    /// replaces its default materials entirely.
    #[must_use]
    pub fn names_tool(&self, tool: &str, tool_tagged: &dyn Fn(&str) -> bool) -> bool {
        self.rows
            .iter()
            .any(|row| row.tool.matches(tool, tool_tagged))
    }
}

fn selector(text: &str, default_namespace: &str) -> Result<Expression, Rejected> {
    if text.is_empty() {
        return Err(Rejected::UnreadableSelector);
    }
    let completed = if text.contains(':') {
        text.to_owned()
    } else {
        format!("{default_namespace}:{text}")
    };
    match Expression::parse(&completed) {
        // A whole namespace is the one shape the table refuses, and it is refused
        // however it is spelled: `tag:*` names the namespace `tag` rather than a
        // tag, so admitting it on the prefix would admit the shape by spelling.
        Expression::Nothing | Expression::Namespace(_) => Err(Rejected::UnreadableSelector),
        readable @ (Expression::Tag(_) | Expression::Entry(_)) => Ok(readable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST: &str = "host";

    fn untagged(_tag: &str) -> bool {
        false
    }

    #[test]
    fn a_selector_naming_a_whole_namespace_is_refused_however_it_is_spelled() {
        for written in ["example:*", "tag:*", "*"] {
            let table = Table::parse(&format!("{written}=example:ingot"), HOST);
            assert_eq!(table.rows().len(), 0, "{written} names a namespace");
            assert_eq!(table.skipped().len(), 1);
        }
    }

    #[test]
    fn a_pair_without_a_multiplier_is_worth_one() {
        let table = Table::parse("example:pick=example:ingot", HOST);
        assert_eq!(table.rows().len(), 1);
        assert_eq!(
            table.multiplier("example:pick", "example:ingot", &untagged, &untagged),
            Some(DEFAULT_MULTIPLIER)
        );
    }

    #[test]
    fn a_multiplier_is_read_off_the_material_selector() {
        let table = Table::parse("example:pick=example:ingot@0.5", HOST);
        assert_eq!(
            table.multiplier("example:pick", "example:ingot", &untagged, &untagged),
            Some(0.5)
        );
    }

    #[test]
    fn either_selector_may_be_a_tag_and_the_caller_answers_membership() {
        let table = Table::parse("tag:example:picks=tag:example:ingots@2", HOST);
        assert_eq!(
            table.multiplier("example:pick", "example:ingot", &|_| true, &|_| true),
            Some(2.0)
        );
        assert_eq!(
            table.multiplier("example:pick", "example:ingot", &|_| true, &untagged),
            None
        );
    }

    #[test]
    fn the_scan_returns_the_first_row_that_matches_both_selectors() {
        let table = Table::parse(
            "example:pick=example:ingot@3, example:pick=example:ingot@9",
            HOST,
        );
        assert_eq!(table.rows().len(), 2);
        assert_eq!(
            table.multiplier("example:pick", "example:ingot", &untagged, &untagged),
            Some(3.0)
        );
    }

    #[test]
    fn the_tool_level_test_ignores_the_material_entirely() {
        let table = Table::parse("example:pick=example:ingot", HOST);
        assert!(table.names_tool("example:pick", &untagged));
        assert!(!table.names_tool("example:shovel", &untagged));
        assert_eq!(
            table.multiplier("example:pick", "example:gem", &untagged, &untagged),
            None
        );
    }

    #[test]
    fn an_unreadable_entry_is_skipped_and_the_rest_of_the_table_still_loads() {
        let table = Table::parse(
            "not a pair, example:pick=example:ingot, a=b=c, =example:ingot",
            HOST,
        );
        assert_eq!(table.rows().len(), 1);
        assert_eq!(table.skipped().len(), 3);
        assert_eq!(table.skipped()[1].reason, Rejected::NotAPair);
    }

    #[test]
    fn a_multiplier_that_is_not_a_number_drops_the_entry_rather_than_the_table() {
        let table = Table::parse("example:pick=example:ingot@later", HOST);
        assert_eq!(table.rows().len(), 0);
        assert_eq!(table.skipped()[0].reason, Rejected::UnreadableMultiplier);
    }

    #[test]
    fn a_multiplier_at_or_below_zero_is_refused_when_the_table_is_read() {
        for written in [
            "example:pick=example:ingot@0",
            "example:pick=example:ingot@-4",
            "example:pick=example:ingot@-0.0001",
        ] {
            let table = Table::parse(written, HOST);
            assert_eq!(table.rows().len(), 0, "{written} is refused");
            assert_eq!(table.skipped()[0].reason, Rejected::NonPositiveMultiplier);
        }
    }

    #[test]
    fn a_multiplier_that_is_not_finite_is_refused_too() {
        for written in [
            "example:pick=example:ingot@inf",
            "example:pick=example:ingot@NaN",
        ] {
            let table = Table::parse(written, HOST);
            assert_eq!(table.rows().len(), 0, "{written} is refused");
        }
    }

    #[test]
    fn a_selector_carrying_no_colon_takes_the_supplied_namespace() {
        let table = Table::parse("pick=ingot", HOST);
        assert_eq!(table.rows().len(), 1);
        assert_eq!(
            table.multiplier("host:pick", "host:ingot", &untagged, &untagged),
            Some(DEFAULT_MULTIPLIER)
        );
    }

    #[test]
    fn a_namespace_wildcard_is_not_a_selector_this_table_admits() {
        let table = Table::parse("example:*=example:ingot", HOST);
        assert_eq!(table.rows().len(), 0);
        assert_eq!(table.skipped()[0].reason, Rejected::UnreadableSelector);
    }

    #[test]
    fn a_table_longer_than_the_ceiling_stops_growing_and_records_every_surplus() {
        let written = (0..MAXIMUM_ENTRIES + 3)
            .map(|index| format!("example:tool{index}=example:ingot"))
            .collect::<Vec<_>>()
            .join(",");
        let table = Table::parse(&written, HOST);
        assert_eq!(table.rows().len(), MAXIMUM_ENTRIES);
        assert_eq!(table.skipped().len(), 3);
        assert_eq!(table.skipped()[0].reason, Rejected::TableFull);
    }

    #[test]
    fn every_rejection_says_what_was_wrong_in_words() {
        for reason in [
            Rejected::NotAPair,
            Rejected::UnreadableSelector,
            Rejected::UnreadableMultiplier,
            Rejected::NonPositiveMultiplier,
            Rejected::TableFull,
        ] {
            assert_ne!(reason.to_string(), "");
        }
    }
}
