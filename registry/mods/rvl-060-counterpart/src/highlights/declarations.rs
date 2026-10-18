//! Descriptor spellings, declared packs and their side-independent contribution.

use std::collections::BTreeMap;

use ravel_sdk::{JsonValue, Side};

/// Why the spellings could not be built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellingsUnavailable {
    /// The build supplied nothing.
    NotSupplied,
    /// A spelling is empty.
    Empty,
    /// Two spellings that have to differ are the same.
    NotDistinct,
}

impl std::fmt::Display for SpellingsUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSupplied => write!(formatter, "the build supplied no pack spellings"),
            Self::Empty => write!(formatter, "a pack spelling is empty"),
            Self::NotDistinct => write!(formatter, "two pack spellings collide"),
        }
    }
}

impl std::error::Error for SpellingsUnavailable {}

/// The three compatibility-required spellings a writer outside this package
/// produces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spellings {
    descriptor_key: String,
    directory: String,
    lines_key: String,
}

impl Default for Spellings {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl Spellings {
    /// The spellings the tests pin the surrounding shapes with.
    ///
    /// These are this package's own words. A build that means to accept an
    /// existing dependent's descriptor, or an existing pack's documents,
    /// supplies the originals.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            descriptor_key: "declared_packs".to_owned(),
            directory: "outlines".to_owned(),
            lines_key: "segments".to_owned(),
        }
    }

    /// Reads the spellings from build-time inputs.
    ///
    /// # Errors
    ///
    /// Returns [`SpellingsUnavailable::NotSupplied`] when the build supplied
    /// nothing, and what [`Spellings::parse`] returns otherwise.
    pub fn declared() -> Result<Self, SpellingsUnavailable> {
        let descriptor_key = option_env!("RVL_060_COUNTERPART_PACK_DESCRIPTOR_KEY")
            .ok_or(SpellingsUnavailable::NotSupplied)?;
        let directory = option_env!("RVL_060_COUNTERPART_HIGHLIGHT_DIRECTORY")
            .ok_or(SpellingsUnavailable::NotSupplied)?;
        let lines_key = option_env!("RVL_060_COUNTERPART_HIGHLIGHT_LINES_KEY")
            .ok_or(SpellingsUnavailable::NotSupplied)?;
        Self::parse(descriptor_key, directory, lines_key)
    }

    /// Reads the three spellings.
    ///
    /// # Errors
    ///
    /// Returns [`SpellingsUnavailable::Empty`] for an empty spelling and
    /// [`SpellingsUnavailable::NotDistinct`] when the directory and the lines
    /// key are the same, which would make a document's own name a field name.
    pub fn parse(
        descriptor_key: &str,
        directory: &str,
        lines_key: &str,
    ) -> Result<Self, SpellingsUnavailable> {
        let trimmed = [descriptor_key.trim(), directory.trim(), lines_key.trim()];
        if trimmed.iter().any(|value| value.is_empty()) {
            return Err(SpellingsUnavailable::Empty);
        }
        if trimmed[1] == trimmed[2] {
            return Err(SpellingsUnavailable::NotDistinct);
        }
        Ok(Self {
            descriptor_key: trimmed[0].to_owned(),
            directory: trimmed[1].to_owned(),
            lines_key: trimmed[2].to_owned(),
        })
    }

    /// Returns the descriptor key a dependent writes its declarations under.
    #[must_use]
    pub fn descriptor_key(&self) -> &str {
        &self.descriptor_key
    }

    /// Returns the resource directory highlight documents load from.
    #[must_use]
    pub fn directory(&self) -> &str {
        &self.directory
    }

    /// Returns the key that tells the two document shapes apart.
    #[must_use]
    pub fn lines_key(&self) -> &str {
        &self.lines_key
    }
}

/// One pack a dependent declares in its own descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Declaration {
    /// The declared name.
    pub name: String,
    /// The declared description, when the record carries one.
    pub description: Option<String>,
    /// Whether the pack is always on and hidden.
    pub required: bool,
}

/// The most packs one dependent may declare.
pub const MAX_DECLARED_PACKS: usize = 64;

/// The fixed directory a declared pack is looked for inside the declaring file.
pub const PACK_ROOT: &str = "resourcepacks";

impl Declaration {
    /// Reads one declaration, which is either a bare name or a record.
    ///
    /// # Errors
    ///
    /// Returns [`PackError::Malformed`] for anything that is neither a text
    /// value nor a record naming a name, and for a required flag or description
    /// of the wrong shape.
    pub fn decode(document: &JsonValue) -> Result<Self, PackError> {
        if let Some(name) = document.as_str() {
            return Self::named(name);
        }
        let object = document.as_object().ok_or_else(|| {
            PackError::Malformed("a declaration is neither a name nor a record".to_owned())
        })?;
        let name = object
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| PackError::Malformed("a declaration record names no pack".to_owned()))?;
        let description = match object.get("description") {
            None => None,
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or_else(|| PackError::Malformed("a description is not text".to_owned()))?
                    .to_owned(),
            ),
        };
        let required = match object.get("required") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| {
                PackError::Malformed("a required flag is not a boolean".to_owned())
            })?,
        };
        let mut declaration = Self::named(name)?;
        declaration.description = description;
        declaration.required = required;
        Ok(declaration)
    }

    /// Declares a pack by its checked name, without a description or required flag.
    pub fn named(name: &str) -> Result<Self, PackError> {
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(PackError::Malformed(format!("`{name}` is not a pack name")));
        }
        Ok(Self {
            name: name.to_owned(),
            description: None,
            required: false,
        })
    }

    /// Returns the path the pack is looked for at inside the declaring file.
    #[must_use]
    pub fn path(&self) -> String {
        format!("{PACK_ROOT}/{}", self.name)
    }

    /// Returns the translation key a description falls back to.
    #[must_use]
    pub fn description_key(&self, namespace: &str) -> String {
        format!("pack.{namespace}.{}.description", self.name)
    }
}

/// Why a pack contribution failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackError {
    /// The declaration does not read.
    Malformed(String),
    /// A dependent declares more packs than [`MAX_DECLARED_PACKS`].
    TooMany(usize),
}

impl std::fmt::Display for PackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(detail) => write!(formatter, "{detail}"),
            Self::TooMany(ceiling) => {
                write!(formatter, "a dependent declares over {ceiling} packs")
            }
        }
    }
}

impl std::error::Error for PackError {}

/// Which kind of pack a contribution is asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PackKind {
    /// A pack of client-facing resources.
    ClientFacing,
    /// A pack of server-facing data.
    ServerFacing,
}

/// A pack that reached the game.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Contributed {
    /// The declaring dependent's namespace.
    pub namespace: String,
    /// The declared name.
    pub name: String,
    /// The description shown, or the translation key it falls back to.
    pub description: String,
    /// Whether the pack is hidden from the pack list.
    pub hidden: bool,
    /// Whether the pack is on and cannot be turned off.
    pub always_on: bool,
    /// Whether the pack's own metadata was read, rather than synthesised.
    pub own_metadata: bool,
}

/// A dependent whose contribution failed, and why.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Failure {
    /// Whose declaration failed.
    pub namespace: String,
    /// What went wrong, in the words of the failure itself.
    pub cause: String,
}

/// What one scan produced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct Contribution {
    /// The packs that reached the game.
    pub packs: Vec<Contributed>,
    /// The dependents whose declarations did not, each with its cause.
    pub failures: Vec<Failure>,
}

/// What one dependent declares and what its file actually holds.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Declaring {
    /// The dependent's namespace.
    pub namespace: String,
    /// What the descriptor carries under the declaration key.
    pub declarations: Vec<JsonValue>,
    /// The pack kinds each declared name's directory actually contains.
    pub present: BTreeMap<String, Vec<PackKind>>,
    /// The names whose own metadata reads.
    pub metadata: Vec<String>,
}

impl Declaring {
    /// Starts a dependent's declaration with no packs or discovered resources.
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            declarations: Vec::new(),
            present: BTreeMap::new(),
            metadata: Vec::new(),
        }
    }
}

/// Contributes every declared pack of one kind, on either side.
///
/// A declaration whose directory does not contain the requested pack kind is
/// skipped silently, which is the original's rule and is right: a client pack in
/// a server scan is not a failure.
///
/// A required pack is contributed hidden and always-on with synthesised
/// metadata; a non-required one reads its own metadata and falls back to
/// synthesised metadata when there is none.
///
/// The scan carries on past a dependent that fails and records the cause, which
/// is what makes one bad descriptor cost one dependent rather than every one
/// after it.
///
/// **Declared change, and the whole of what `side` is for.** The side is taken
/// and deliberately not consulted: the original scans and registers its listener
/// only on the client branch, so a dependent shipping data through it silently
/// has none on a dedicated server. Taking the parameter is what makes the answer
/// a caller gets on a server the same statement as the one it gets on a client,
/// rather than a caller on a server having no way to ask.
#[must_use]
pub fn contribute(declaring: &[Declaring], kind: PackKind, side: Side) -> Contribution {
    // Not consulted, and that is the declared change; see above.
    let _ = side;
    let mut contribution = Contribution::default();
    for dependent in declaring {
        if dependent.declarations.len() > MAX_DECLARED_PACKS {
            contribution.failures.push(Failure {
                namespace: dependent.namespace.clone(),
                cause: PackError::TooMany(MAX_DECLARED_PACKS).to_string(),
            });
            continue;
        }
        for document in &dependent.declarations {
            match Declaration::decode(document) {
                Err(error) => contribution.failures.push(Failure {
                    namespace: dependent.namespace.clone(),
                    cause: error.to_string(),
                }),
                Ok(declaration) => {
                    let holds = dependent
                        .present
                        .get(&declaration.name)
                        .is_some_and(|kinds| kinds.contains(&kind));
                    if !holds {
                        continue;
                    }
                    let own_metadata =
                        !declaration.required && dependent.metadata.contains(&declaration.name);
                    let description = if own_metadata {
                        declaration
                            .description
                            .clone()
                            .unwrap_or_else(|| declaration.name.clone())
                    } else {
                        declaration
                            .description
                            .clone()
                            .unwrap_or_else(|| declaration.description_key(&dependent.namespace))
                    };
                    contribution.packs.push(Contributed {
                        namespace: dependent.namespace.clone(),
                        name: declaration.name.clone(),
                        description,
                        hidden: declaration.required,
                        always_on: declaration.required,
                        own_metadata,
                    });
                }
            }
        }
    }
    contribution
}

#[cfg(test)]
mod tests {
    use ravel_sdk::{Side, json};

    use super::{
        Declaration, Declaring, PackError, PackKind, Spellings, SpellingsUnavailable, contribute,
    };

    #[test]
    fn spellings_are_trimmed_before_storage_and_collision_checks() {
        let spellings = Spellings::parse(" packs ", " outlines ", " segments ")
            .expect("distinct nonempty spellings");
        assert_eq!(spellings.descriptor_key(), "packs");
        assert_eq!(spellings.directory(), "outlines");
        assert_eq!(spellings.lines_key(), "segments");
        assert_eq!(
            Spellings::parse("packs", " outlines ", "outlines"),
            Err(SpellingsUnavailable::NotDistinct)
        );
        assert_eq!(
            Spellings::parse("packs", "  ", "segments"),
            Err(SpellingsUnavailable::Empty)
        );
    }

    #[test]
    fn a_bad_record_does_not_discard_other_packs_from_the_same_dependent() {
        let mut dependent = Declaring::new("example");
        dependent.declarations = vec![
            json!({"name": "first", "required": "yes"}),
            json!("second"),
            json!({"name": "third", "description": "Explicit", "required": true}),
        ];
        dependent.present = ["first", "second", "third"]
            .map(|name| (name.to_owned(), vec![PackKind::ServerFacing]))
            .into_iter()
            .collect();
        dependent.metadata.push("third".to_owned());
        let result = contribute(&[dependent], PackKind::ServerFacing, Side::DedicatedServer);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].namespace, "example");
        assert_eq!(result.failures[0].cause, "a required flag is not a boolean");
        assert_eq!(result.packs.len(), 2);
        assert_eq!(result.packs[0].name, "second");
        assert_eq!(
            result.packs[0].description,
            "pack.example.second.description"
        );
        assert_eq!(result.packs[1].name, "third");
        assert_eq!(result.packs[1].description, "Explicit");
        assert!(result.packs[1].hidden);
        assert!(result.packs[1].always_on);
        assert!(!result.packs[1].own_metadata);
    }

    #[test]
    fn named_declarations_check_names_and_supply_path_and_description_defaults() {
        let declaration = Declaration::named("pack_01").expect("a checked pack name");
        assert_eq!(declaration.path(), "resourcepacks/pack_01");
        assert_eq!(
            declaration.description_key("example"),
            "pack.example.pack_01.description"
        );
        assert_eq!(declaration.description, None);
        assert!(!declaration.required);
        let longest = "a".repeat(64);
        assert_eq!(
            Declaration::named(&longest)
                .expect("the maximum name length")
                .name,
            longest
        );
        for name in [
            "",
            "Upper",
            "with-dash",
            "../pack",
            " pack",
            "påck",
            &"a".repeat(65),
        ] {
            assert_eq!(
                Declaration::named(name),
                Err(PackError::Malformed(format!("`{name}` is not a pack name")))
            );
        }
    }
}
