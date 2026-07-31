use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a legal value for one setting looks like.
///
/// `configuration.schema.json` types `valueSchema` as an open JSON Schema
/// fragment, and no JSON Schema engine is in this workspace's dependency graph:
/// documents are validated in the gate by `check-jsonschema`, a process a
/// running game does not get to spawn. This is the subset Ravel enforces itself,
/// and `deny_unknown_fields` is what keeps it honest — a keyword this crate
/// cannot enforce is refused when the declaration loads rather than ignored, so
/// nobody writes a `pattern` and believes it is being checked. Widening the
/// subset later starts accepting declarations that were refused and breaks
/// nothing already written; ignoring an unknown keyword now could not be
/// withdrawn.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ValueSchema {
    /// The JSON type a value must have.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<ValueKind>,
    /// The exact values permitted, when the setting is a closed choice.
    #[serde(default, rename = "enum", skip_serializing_if = "Option::is_none")]
    pub permitted: Option<Vec<Value>>,
    /// The lowest number permitted, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    /// The highest number permitted, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    /// The fewest characters permitted in a string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    /// The most characters permitted in a string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
}

impl ValueSchema {
    /// Checks one value against every keyword this fragment declares.
    ///
    /// A keyword that does not apply to the value's type constrains nothing,
    /// which is what JSON Schema means by an inapplicable keyword: a `minimum`
    /// standing next to a string says nothing about that string. The `kind`
    /// check is what refuses the string in the first place when the author
    /// wanted a number.
    ///
    /// # Errors
    ///
    /// Returns the first keyword the value breaks, which is the one a screen
    /// shows next to the field the player just edited.
    pub fn check(&self, value: &Value) -> Result<(), ValueError> {
        if let Some(kind) = self.kind
            && !kind.matches(value)
        {
            return Err(ValueError::Kind {
                expected: kind,
                found: type_name(value),
            });
        }

        if let Some(permitted) = &self.permitted
            && !permitted.contains(value)
        {
            return Err(ValueError::NotPermitted {
                permitted: permitted.clone(),
            });
        }

        if let Some(number) = value.as_f64() {
            if let Some(minimum) = self.minimum
                && number < minimum
            {
                return Err(ValueError::Below { minimum });
            }
            if let Some(maximum) = self.maximum
                && number > maximum
            {
                return Err(ValueError::Above { maximum });
            }
        }

        if let Some(text) = value.as_str() {
            let length = text.chars().count();
            if let Some(minimum) = self.min_length
                && length < minimum
            {
                return Err(ValueError::TooShort { minimum, length });
            }
            if let Some(maximum) = self.max_length
                && length > maximum
            {
                return Err(ValueError::TooLong { maximum, length });
            }
        }

        Ok(())
    }
}

/// The JSON types a setting value may declare.
///
/// Scalars only. What an array or object setting looks like on a screen is a
/// widget question ADG-6 leaves to the screen, and refusing one now can be
/// widened later; admitting one before anything draws it could not be withdrawn.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueKind {
    /// A JSON boolean.
    Boolean,
    /// A JSON number written without a fractional part.
    Integer,
    /// Any JSON number.
    Number,
    /// A JSON string.
    String,
}

impl ValueKind {
    /// Every kind, in the order JSON Schema names them.
    pub const ALL: [Self; 4] = [Self::Boolean, Self::Integer, Self::Number, Self::String];

    /// The name this kind is written under, matching its serialized form.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::String => "string",
        }
    }

    /// Returns whether `value` has this type.
    ///
    /// `integer` means a JSON integer literal, which is narrower than JSON
    /// Schema's rule that also admits `2.0`. Every document Ravel writes spells
    /// an integer as one, and the narrow reading is the one that can be widened
    /// later without invalidating anything already accepted.
    fn matches(self, value: &Value) -> bool {
        match self {
            Self::Boolean => value.is_boolean(),
            Self::Integer => value.is_i64() || value.is_u64(),
            Self::Number => value.is_number(),
            Self::String => value.is_string(),
        }
    }
}

/// The JSON type name a value has, so a message can say what arrived.
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Why a value is not one its setting's [`ValueSchema`] permits.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueError {
    /// The value has the wrong JSON type.
    Kind {
        /// The type the setting declared.
        expected: ValueKind,
        /// The type the value actually has.
        found: &'static str,
    },
    /// The setting is a closed choice and the value is not one of them.
    NotPermitted {
        /// The values the setting admits.
        permitted: Vec<Value>,
    },
    /// The number is below the declared `minimum`.
    Below {
        /// The lowest number permitted.
        minimum: f64,
    },
    /// The number is above the declared `maximum`.
    Above {
        /// The highest number permitted.
        maximum: f64,
    },
    /// The string is shorter than the declared `minLength`.
    TooShort {
        /// The fewest characters permitted.
        minimum: usize,
        /// The characters the string actually has.
        length: usize,
    },
    /// The string is longer than the declared `maxLength`.
    TooLong {
        /// The most characters permitted.
        maximum: usize,
        /// The characters the string actually has.
        length: usize,
    },
}

impl fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kind { expected, found } => write!(
                formatter,
                "the setting holds a {}, not a {found}",
                expected.name()
            ),
            Self::NotPermitted { permitted } => {
                let names: Vec<String> = permitted.iter().map(ToString::to_string).collect();
                write!(formatter, "the setting holds one of {}", names.join(", "))
            }
            Self::Below { minimum } => write!(formatter, "the setting holds at least {minimum}"),
            Self::Above { maximum } => write!(formatter, "the setting holds at most {maximum}"),
            Self::TooShort { minimum, length } => write!(
                formatter,
                "the setting holds at least {minimum} characters, and this has {length}"
            ),
            Self::TooLong { maximum, length } => write!(
                formatter,
                "the setting holds at most {maximum} characters, and this has {length}"
            ),
        }
    }
}

impl std::error::Error for ValueError {}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{ValueError, ValueKind, ValueSchema};

    fn schema(fragment: &str) -> ValueSchema {
        serde_json::from_str(fragment).expect("a fragment inside the enforced subset")
    }

    #[test]
    fn an_empty_fragment_constrains_nothing() {
        let anything = ValueSchema::default();
        for value in [json!(true), json!(7), json!("text"), Value::Null] {
            assert_eq!(anything.check(&value), Ok(()));
        }
    }

    #[test]
    fn every_kind_accepts_its_own_type_and_refuses_the_others() {
        let sample = [
            (ValueKind::Boolean, json!(true)),
            (ValueKind::Integer, json!(7)),
            (ValueKind::Number, json!(1.5)),
            (ValueKind::String, json!("text")),
        ];
        for (kind, value) in &sample {
            let declared = schema(&format!("{{\"type\":\"{}\"}}", kind.name()));
            assert_eq!(declared.check(value), Ok(()), "{kind:?}");
            for (other, candidate) in &sample {
                if other != kind && !(*kind == ValueKind::Number && *other == ValueKind::Integer) {
                    assert!(declared.check(candidate).is_err(), "{kind:?} vs {other:?}");
                }
            }
        }
    }

    #[test]
    fn an_integer_is_a_number_and_a_fractional_number_is_not_an_integer() {
        assert_eq!(schema(r#"{"type":"number"}"#).check(&json!(7)), Ok(()));
        assert_eq!(
            schema(r#"{"type":"integer"}"#).check(&json!(1.5)),
            Err(ValueError::Kind {
                expected: ValueKind::Integer,
                found: "number",
            })
        );
    }

    #[test]
    fn a_refusal_names_the_type_that_arrived() {
        let declared = schema(r#"{"type":"string"}"#);
        for (value, found) in [
            (Value::Null, "null"),
            (json!(true), "boolean"),
            (json!(1), "number"),
            (json!([1]), "array"),
            (json!({"a": 1}), "object"),
        ] {
            assert_eq!(
                declared.check(&value),
                Err(ValueError::Kind {
                    expected: ValueKind::String,
                    found,
                })
            );
        }
    }

    #[test]
    fn a_closed_choice_admits_only_what_it_lists() {
        let declared = schema(r#"{"type":"string","enum":["low","high"]}"#);
        assert_eq!(declared.check(&json!("low")), Ok(()));

        let refused = declared.check(&json!("medium")).expect_err("not listed");
        assert_eq!(
            refused,
            ValueError::NotPermitted {
                permitted: vec![json!("low"), json!("high")],
            }
        );
        assert!(refused.to_string().contains("\"high\""), "{refused}");
    }

    #[test]
    fn a_bound_applies_to_the_kind_it_bounds_and_to_nothing_else() {
        let bounded = schema(r#"{"minimum":1.0,"maximum":10.0}"#);
        assert_eq!(bounded.check(&json!(5)), Ok(()));
        assert_eq!(
            bounded.check(&json!(0)),
            Err(ValueError::Below { minimum: 1.0 })
        );
        assert_eq!(
            bounded.check(&json!(11)),
            Err(ValueError::Above { maximum: 10.0 })
        );
        // A numeric bound standing next to a string constrains nothing.
        assert_eq!(bounded.check(&json!("text")), Ok(()));

        let sized = schema(r#"{"minLength":2,"maxLength":4}"#);
        assert_eq!(sized.check(&json!("abc")), Ok(()));
        assert_eq!(
            sized.check(&json!("a")),
            Err(ValueError::TooShort {
                minimum: 2,
                length: 1,
            })
        );
        assert_eq!(
            sized.check(&json!("abcde")),
            Err(ValueError::TooLong {
                maximum: 4,
                length: 5,
            })
        );
        assert_eq!(sized.check(&json!(3)), Ok(()));
    }

    #[test]
    fn a_string_is_measured_in_characters_rather_than_bytes() {
        let sized = schema(r#"{"maxLength":2}"#);
        assert_eq!(sized.check(&json!("é€")), Ok(()));
        assert!(sized.check(&json!("é€a")).is_err());
    }

    #[test]
    fn a_keyword_this_crate_cannot_enforce_is_refused_rather_than_ignored() {
        // Accepting `pattern` and never applying it would tell an author their
        // value is checked when nothing checks it.
        let refused = serde_json::from_str::<ValueSchema>(r#"{"type":"string","pattern":"^a"}"#)
            .expect_err("pattern is outside the enforced subset");
        assert!(refused.to_string().contains("pattern"), "{refused}");

        let composite = serde_json::from_str::<ValueSchema>(r#"{"type":"array"}"#)
            .expect_err("only scalars are enforced");
        assert!(composite.to_string().contains("array"), "{composite}");
    }

    #[test]
    fn a_fragment_renders_back_to_the_keywords_it_declared() {
        let declared = schema(r#"{"type":"integer","minimum":0.0}"#);
        let rendered = serde_json::to_string(&declared).expect("a fragment serializes");
        assert_eq!(rendered, r#"{"type":"integer","minimum":0.0}"#);
        assert_eq!(schema(&rendered), declared);
    }

    #[test]
    fn every_kind_names_itself_the_way_it_serializes() {
        for kind in ValueKind::ALL {
            let rendered = serde_json::to_string(&kind).expect("a kind serializes");
            assert_eq!(rendered, format!("\"{}\"", kind.name()));
            assert_eq!(
                serde_json::from_str::<ValueKind>(&rendered).expect("a kind deserializes"),
                kind
            );
        }
    }
}
