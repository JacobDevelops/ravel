//! Typed admission and closed vocabularies shared by milestone evidence records.

use std::fmt;

use serde::{Deserialize, Deserializer, de::Error};
use serde_json::Value;

pub(super) fn admit<'a, T: Deserialize<'a>>(value: &'a Value) -> Result<T, String> {
    serde_path_to_error::deserialize(value)
        .map_err(|error| format!("{}: {}", JsonPointer(error.path()), error.inner()))
}

pub(super) fn require_identity(actual: &str, expected: &str, pointer: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{pointer} must be {expected}"))
    }
}

struct JsonPointer<'a>(&'a serde_path_to_error::Path);

impl fmt::Display for JsonPointer<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use serde_path_to_error::Segment;

        if self.0.iter().next().is_none() {
            return formatter.write_str("/");
        }
        for segment in self.0 {
            formatter.write_str("/")?;
            match segment {
                Segment::Seq { index } => write!(formatter, "{index}")?,
                Segment::Map { key } | Segment::Enum { variant: key } => {
                    for character in key.chars() {
                        match character {
                            '~' => formatter.write_str("~0")?,
                            '/' => formatter.write_str("~1")?,
                            other => write!(formatter, "{other}")?,
                        }
                    }
                }
                Segment::Unknown => formatter.write_str("?")?,
            }
        }
        Ok(())
    }
}

pub(super) struct NonemptyString<'a>(pub(super) &'a str);

impl<'de: 'a, 'a> Deserialize<'de> for NonemptyString<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <&str>::deserialize(deserializer)?;
        if value.is_empty() {
            return Err(D::Error::custom("expected a nonempty string"));
        }
        Ok(Self(value))
    }
}

pub(super) struct NonemptyRows<T>(pub(super) Vec<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NonemptyRows<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let rows = Vec::deserialize(deserializer)?;
        if rows.is_empty() {
            return Err(D::Error::custom("expected a nonempty array"));
        }
        Ok(Self(rows))
    }
}

pub(super) fn count_sum(counts: impl IntoIterator<Item = u64>) -> Option<u64> {
    counts.into_iter().try_fold(0_u64, u64::checked_add)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Verdict {
    Pass,
    Fail,
    Provisional,
    NotRequired,
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Provisional => "provisional",
            Self::NotRequired => "not-required",
        })
    }
}

#[derive(Deserialize)]
pub(super) enum UpperBoundOperator {
    #[serde(rename = "less-than-or-equal")]
    LessThanOrEqual,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;
    use serde_json::{Number, Value, json};

    use super::{NonemptyRows, NonemptyString, admit};

    #[test]
    fn admission_preserves_array_indices_and_escapes_json_pointer_segments() {
        let value = json!({"a~/b": [null]});
        let error = admit::<BTreeMap<String, Vec<Number>>>(&value)
            .expect_err("null must not deserialize as a number");
        let reason =
            Number::deserialize(&Value::Null).expect_err("null must not deserialize as a number");
        assert_eq!(error, format!("/a~0~1b/0: {reason}"));
    }

    #[test]
    fn nonempty_collections_and_strings_keep_their_field_path() {
        for (value, expected) in [
            (json!({"rows": []}), "/rows: expected a nonempty array"),
            (json!({"rows": [""]}), "/rows/0: expected a nonempty string"),
        ] {
            let error = admit::<BTreeMap<String, NonemptyRows<NonemptyString<'_>>>>(&value)
                .err()
                .expect("empty collection or string must refuse");
            assert_eq!(error, expected);
        }
        let value = json!(["borrowed"]);
        let rows: NonemptyRows<NonemptyString<'_>> =
            admit(&value).expect("nonempty borrowed strings should admit");
        assert_eq!(
            rows.0[0].0.as_ptr(),
            value[0].as_str().expect("string").as_ptr()
        );
    }

    #[test]
    fn root_admission_errors_keep_the_root_pointer() {
        let error = admit::<NonemptyRows<Number>>(&json!([]))
            .err()
            .expect("an empty root array must refuse");
        assert_eq!(error, "/: expected a nonempty array");
    }
}
