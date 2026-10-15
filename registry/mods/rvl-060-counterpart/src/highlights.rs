//! Declared resource packs and block highlights, from §10.1 and §10.2.
//!
//! Both are data-driven and both carry spellings §14 makes
//! compatibility-required: the descriptor key a dependent writes its pack
//! declarations under, the resource directory highlight documents load from, and
//! the key that tells the two document shapes apart. None is invented here.
//! [`Spellings::declared`] reads them from build-time inputs and
//! [`Spellings::placeholder`] carries this package's own, which agree with no
//! original.
//!
//! # The changes declared here
//!
//! - **Declared packs are contributed on both sides.** The original's
//!   pack-finder asks the event which kind is wanted, so the mechanism is
//!   capable of contributing a server-facing pack; but both the scan that reads
//!   the declarations and the registration of that listener happen only on the
//!   client branch. On a dedicated server no declared pack is contributed at
//!   all, so a dependent that ships data through this mechanism silently has no
//!   data there. [`contribute`] takes the side and answers for both.
//! - **The inner failure names its cause.** The original's outer scan logs the
//!   failure with its cause and carries on, which is right; the inner
//!   registration catches its failure, discards the cause, and logs a message
//!   that does not say what went wrong. Every [`Failure`] here carries both who
//!   and why.
//!
//! **Host-blocked.** Ravel publishes no block-outline interception point and no
//! client reload listener, so §10.2 cannot be made live. The loading, the
//! flattening, the sharing and the lookup rule are all portable and are here;
//! the drawing is not.

use std::collections::BTreeMap;

use ravel_sdk::{JsonValue, Side};

use crate::colour::Colour;

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

    fn named(name: &str) -> Result<Self, PackError> {
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
pub struct Failure {
    /// Whose declaration failed.
    pub namespace: String,
    /// What went wrong, in the words of the failure itself.
    pub cause: String,
}

/// What one scan produced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Contribution {
    /// The packs that reached the game.
    pub packs: Vec<Contributed>,
    /// The dependents whose declarations did not, each with its cause.
    pub failures: Vec<Failure>,
}

/// What one dependent declares and what its file actually holds.
#[derive(Clone, Debug, Eq, PartialEq)]
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

/// One line of an outline: a start, an end and a normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// Where the line starts.
    pub start: [f32; 3],
    /// Where it ends.
    pub end: [f32; 3],
    /// Which way it faces.
    pub normal: [f32; 3],
}

/// How many floats one segment flattens to.
pub const FLOATS_PER_SEGMENT: usize = 9;

/// The most segments one outline may hold.
pub const MAX_SEGMENTS: usize = 4096;

/// The most outlines one load may flatten.
pub const MAX_OUTLINES: usize = 8192;

/// The fixed colour every highlight is drawn in.
///
/// Process-wide, and the opacity's shipped default is the only one of the four
/// channels that is not zero.
pub const HIGHLIGHT_COLOUR: Colour = Colour::new(0x00, 0x00, 0x00, 0x66);

/// Why a highlight document was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutlineError {
    /// The document is not an object.
    NotAnObject,
    /// A segment does not carry three points of three numbers.
    BadSegment,
    /// An outline holds more than [`MAX_SEGMENTS`] segments.
    TooManySegments(usize),
    /// A load holds more than [`MAX_OUTLINES`] outlines.
    TooManyOutlines(usize),
    /// A mapping names an outline nothing defined in this load.
    UnknownName(String),
    /// A mapping is not a map from state to outline.
    BadMapping,
}

impl std::fmt::Display for OutlineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnObject => write!(formatter, "a highlight document is not an object"),
            Self::BadSegment => write!(formatter, "a segment is not three points of three numbers"),
            Self::TooManySegments(count) => write!(formatter, "an outline holds {count} segments"),
            Self::TooManyOutlines(count) => write!(formatter, "a load holds {count} outlines"),
            Self::UnknownName(name) => {
                write!(formatter, "`{name}` names no outline this load defined")
            }
            Self::BadMapping => write!(formatter, "a mapping is not a map from state to outline"),
        }
    }
}

impl std::error::Error for OutlineError {}

/// A state's identity, as the lookup keys by it.
///
/// The original looks up by exact state identity rather than by block, using an
/// identity-keyed map, so two structurally equal states from different
/// definitions do not share an entry. That is fine for the host's interned
/// states and would be a bug against a state this package constructed itself,
/// so this keys by the state's own identity and says so: an identity is the
/// number the load assigned, not the state's contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct StateId(pub u32);

/// The flattened outlines one load produced.
///
/// Identical outlines share one array, which is what the sharing is for: a pack
/// that maps two hundred states to one outline holds one array rather than two
/// hundred.
///
/// The named-outline table is discarded once the load finishes, so a name is
/// resolvable only during the load that defined it. That is the original's rule
/// and this type's shape is what keeps it: nothing here can name an outline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outlines {
    arrays: Vec<Vec<f32>>,
    by_state: BTreeMap<StateId, usize>,
}

impl Outlines {
    /// Builds outlines from arrays a caller flattened itself.
    ///
    /// A dependent that builds an outline in code rather than in a document
    /// reaches for this. A mapping naming an array that is not there is dropped,
    /// because a dangling index would draw somebody else's outline.
    #[must_use]
    pub fn from_arrays(arrays: Vec<Vec<f32>>, by_state: BTreeMap<StateId, usize>) -> Self {
        let kept = by_state
            .into_iter()
            .filter(|(_, at)| *at < arrays.len())
            .collect();
        Self {
            arrays,
            by_state: kept,
        }
    }

    /// Returns the flat array for a state, when one was loaded.
    ///
    /// An array whose length is not a multiple of [`FLOATS_PER_SEGMENT`] is
    /// skipped rather than drawn partially.
    #[must_use]
    pub fn at(&self, state: StateId) -> Option<&[f32]> {
        let array = self
            .by_state
            .get(&state)
            .and_then(|at| self.arrays.get(*at))?;
        if array.len() % FLOATS_PER_SEGMENT == 0 {
            Some(array)
        } else {
            None
        }
    }

    /// Returns how many distinct arrays are held.
    #[must_use]
    pub fn arrays(&self) -> usize {
        self.arrays.len()
    }

    /// Returns how many states have an outline.
    #[must_use]
    pub fn states(&self) -> usize {
        self.by_state.len()
    }

    /// Returns whether nothing was loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_state.is_empty()
    }
}

/// What is drawn around a targeted block.
#[derive(Clone, Debug, PartialEq)]
pub enum Drawn<'a> {
    /// The block's own class declares an outline, which wins outright.
    OwnOutline,
    /// A loaded outline for this exact state.
    Loaded(&'a [f32]),
    /// Nothing here, so the host draws its usual outline.
    HostDefault,
}

/// Decides what is drawn around a targeted block.
///
/// A block whose own class declares an outline wins outright and nothing else is
/// consulted; otherwise a flattened entry for the exact state is used if one
/// exists; otherwise the host draws its usual outline.
#[must_use]
pub fn draw(outlines: &Outlines, state: StateId, declares_own: bool) -> Drawn<'_> {
    if declares_own {
        return Drawn::OwnOutline;
    }
    outlines.at(state).map_or(Drawn::HostDefault, Drawn::Loaded)
}

/// Loads the two document shapes into flattened outlines.
///
/// The documents are told apart by whether the document names the lines key: one
/// shape defines a named reusable outline as a list of line segments, the other
/// maps a block's states to outlines either inline or by naming one of the
/// first. Both are read, every mapping is flattened, and identical outlines
/// share one array.
///
/// # Errors
///
/// Returns [`OutlineError`] for a document that is not an object, a segment that
/// is not three points of three numbers, a mapping that names an outline nothing
/// defined, or a load past [`MAX_SEGMENTS`] or [`MAX_OUTLINES`].
pub fn load(
    documents: &[(StateId, JsonValue)],
    named: &[JsonValue],
    spellings: &Spellings,
) -> Result<Outlines, OutlineError> {
    let mut table: BTreeMap<String, Vec<f32>> = BTreeMap::new();
    for document in named {
        let object = document.as_object().ok_or(OutlineError::NotAnObject)?;
        let Some(name) = object.get("name").and_then(JsonValue::as_str) else {
            return Err(OutlineError::BadMapping);
        };
        let lines = object
            .get(spellings.lines_key())
            .ok_or(OutlineError::BadMapping)?;
        if table.len() >= MAX_OUTLINES {
            return Err(OutlineError::TooManyOutlines(table.len() + 1));
        }
        table.insert(name.to_owned(), flatten(lines)?);
    }

    let mut outlines = Outlines::default();
    for (state, document) in documents {
        let object = document.as_object().ok_or(OutlineError::NotAnObject)?;
        let flat = if let Some(lines) = object.get(spellings.lines_key()) {
            flatten(lines)?
        } else {
            let name = object
                .get("outline")
                .and_then(JsonValue::as_str)
                .ok_or(OutlineError::BadMapping)?;
            table
                .get(name)
                .cloned()
                .ok_or_else(|| OutlineError::UnknownName(name.to_owned()))?
        };
        let at = if let Some(at) = outlines
            .arrays
            .iter()
            .position(|held| held.as_slice() == flat.as_slice())
        {
            at
        } else {
            if outlines.arrays.len() >= MAX_OUTLINES {
                return Err(OutlineError::TooManyOutlines(outlines.arrays.len() + 1));
            }
            outlines.arrays.push(flat);
            outlines.arrays.len() - 1
        };
        outlines.by_state.insert(*state, at);
    }
    // The named table is dropped here, so a name is resolvable only during the
    // load that defined it.
    Ok(outlines)
}

fn flatten(lines: &JsonValue) -> Result<Vec<f32>, OutlineError> {
    let items = lines.as_array().ok_or(OutlineError::BadSegment)?;
    if items.len() > MAX_SEGMENTS {
        return Err(OutlineError::TooManySegments(items.len()));
    }
    let mut flat = Vec::with_capacity(items.len() * FLOATS_PER_SEGMENT);
    for item in items {
        let object = item.as_object().ok_or(OutlineError::BadSegment)?;
        for field in ["start", "end", "normal"] {
            let point = object
                .get(field)
                .and_then(JsonValue::as_array)
                .ok_or(OutlineError::BadSegment)?;
            if point.len() != 3 {
                return Err(OutlineError::BadSegment);
            }
            for value in point {
                let number = value.as_f64().ok_or(OutlineError::BadSegment)?;
                flat.push(num_traits::cast::AsPrimitive::<f32>::as_(number));
            }
        }
    }
    Ok(flat)
}

/// Flattens one segment, for a caller building an outline in code.
#[must_use]
pub fn flatten_segment(segment: Segment) -> [f32; FLOATS_PER_SEGMENT] {
    [
        segment.start[0],
        segment.start[1],
        segment.start[2],
        segment.end[0],
        segment.end[1],
        segment.end[2],
        segment.normal[0],
        segment.normal[1],
        segment.normal[2],
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ravel_sdk::{Side, json};

    use super::{
        Contributed, Declaration, Declaring, Drawn, FLOATS_PER_SEGMENT, Failure, HIGHLIGHT_COLOUR,
        OutlineError, Outlines, PackError, PackKind, Segment, Spellings, SpellingsUnavailable,
        StateId, contribute, draw, flatten_segment, load,
    };

    fn spellings() -> Spellings {
        Spellings::placeholder()
    }

    fn declaring(namespace: &str, declarations: Vec<ravel_sdk::JsonValue>) -> Declaring {
        let mut present = BTreeMap::new();
        present.insert(
            "alternative".to_owned(),
            vec![PackKind::ClientFacing, PackKind::ServerFacing],
        );
        present.insert("client_only".to_owned(), vec![PackKind::ClientFacing]);
        Declaring {
            namespace: namespace.to_owned(),
            declarations,
            present,
            metadata: vec!["alternative".to_owned()],
        }
    }

    #[test]
    fn the_three_spellings_are_absent_rather_than_defaulted() {
        assert_eq!(
            Spellings::declared(),
            Err(SpellingsUnavailable::NotSupplied),
            "no build input is supplied here, and a placeholder is not an answer"
        );
    }

    #[test]
    fn a_supplied_spelling_is_refused_two_ways() {
        assert_eq!(
            Spellings::parse("", "outlines", "segments"),
            Err(SpellingsUnavailable::Empty)
        );
        assert_eq!(
            Spellings::parse("packs", "same", "same"),
            Err(SpellingsUnavailable::NotDistinct)
        );
        let supplied = Spellings::parse("packs", "dir", "lines").expect("three distinct spellings");
        assert_eq!(supplied.descriptor_key(), "packs");
        assert_eq!(supplied.directory(), "dir");
        assert_eq!(supplied.lines_key(), "lines");
    }

    #[test]
    fn a_declaration_is_either_a_bare_name_or_a_record() {
        assert_eq!(
            Declaration::decode(&json!("alternative")),
            Ok(Declaration {
                name: "alternative".to_owned(),
                description: None,
                required: false,
            })
        );
        assert_eq!(
            Declaration::decode(&json!({
                "name": "alternative",
                "description": "a second look",
                "required": true
            })),
            Ok(Declaration {
                name: "alternative".to_owned(),
                description: Some("a second look".to_owned()),
                required: true,
            })
        );
    }

    #[test]
    fn a_declaration_is_refused_five_ways() {
        for document in [
            json!(4),
            json!({}),
            json!({ "name": "a", "description": 4 }),
            json!({ "name": "a", "required": "yes" }),
            json!("Not A Name"),
        ] {
            assert!(
                matches!(Declaration::decode(&document), Err(PackError::Malformed(_))),
                "{document} is not a declaration"
            );
        }
    }

    #[test]
    fn a_pack_is_looked_for_at_a_fixed_path_inside_the_declaring_file() {
        let declaration = Declaration::decode(&json!("alternative")).expect("a bare name");
        assert_eq!(declaration.path(), "resourcepacks/alternative");
        assert_eq!(
            declaration.description_key("example"),
            "pack.example.alternative.description"
        );
    }

    #[test]
    fn a_declared_pack_is_contributed_on_a_dedicated_server_as_well_as_a_client() {
        let declaring = [declaring("example", vec![json!("alternative")])];
        for side in [Side::Client, Side::IntegratedServer, Side::DedicatedServer] {
            let contribution = contribute(&declaring, PackKind::ServerFacing, side);
            assert_eq!(
                contribution.packs.len(),
                1,
                "this is the declared change: the original contributes nothing on a server"
            );
            assert_eq!(contribution.failures.len(), 0);
        }
    }

    #[test]
    fn a_declaration_whose_directory_lacks_the_requested_kind_is_skipped_silently() {
        let declaring = [declaring("example", vec![json!("client_only")])];
        let contribution = contribute(&declaring, PackKind::ServerFacing, Side::DedicatedServer);
        assert_eq!(contribution.packs.len(), 0);
        assert_eq!(
            contribution.failures.len(),
            0,
            "a client pack in a server scan is not a failure"
        );
    }

    #[test]
    fn a_required_pack_is_hidden_always_on_and_carries_synthesised_metadata() {
        let declaring = [declaring(
            "example",
            vec![json!({ "name": "alternative", "required": true })],
        )];
        let contribution = contribute(&declaring, PackKind::ClientFacing, Side::Client);
        assert_eq!(
            contribution.packs,
            [Contributed {
                namespace: "example".to_owned(),
                name: "alternative".to_owned(),
                description: "pack.example.alternative.description".to_owned(),
                hidden: true,
                always_on: true,
                own_metadata: false,
            }]
        );
    }

    #[test]
    fn a_non_required_pack_reads_its_own_metadata_and_falls_back_when_there_is_none() {
        let with_metadata = [declaring("example", vec![json!("alternative")])];
        let contribution = contribute(&with_metadata, PackKind::ClientFacing, Side::Client);
        assert!(contribution.packs[0].own_metadata);
        assert!(!contribution.packs[0].hidden);
        assert!(!contribution.packs[0].always_on);

        let mut without = declaring("example", vec![json!("client_only")]);
        without.metadata.clear();
        let contribution = contribute(&[without], PackKind::ClientFacing, Side::Client);
        assert!(!contribution.packs[0].own_metadata);
        assert_eq!(
            contribution.packs[0].description, "pack.example.client_only.description",
            "the description falls back to a translation key derived from the name"
        );
    }

    #[test]
    fn a_declared_description_is_used_where_one_is_given() {
        let declaring = [declaring(
            "example",
            vec![json!({ "name": "alternative", "description": "a second look" })],
        )];
        let contribution = contribute(&declaring, PackKind::ClientFacing, Side::Client);
        assert_eq!(contribution.packs[0].description, "a second look");
    }

    #[test]
    fn a_failure_names_its_cause_and_the_scan_carries_on_past_it() {
        let declaring = [
            declaring("broken", vec![json!(4)]),
            declaring("sound", vec![json!("alternative")]),
        ];
        let contribution = contribute(&declaring, PackKind::ClientFacing, Side::Client);
        assert_eq!(contribution.packs.len(), 1);
        assert_eq!(contribution.packs[0].namespace, "sound");
        assert_eq!(contribution.failures.len(), 1);
        let Failure { namespace, cause } = &contribution.failures[0];
        assert_eq!(namespace, "broken");
        assert_ne!(
            cause.len(),
            0,
            "the inner failure names what went wrong, which the original discards"
        );
    }

    #[test]
    fn a_dependent_declaring_more_packs_than_the_ceiling_is_one_failure_rather_than_many() {
        let many: Vec<_> = (0..=super::MAX_DECLARED_PACKS)
            .map(|index| json!(format!("p{index}")))
            .collect();
        let contribution = contribute(
            &[declaring("example", many)],
            PackKind::ClientFacing,
            Side::Client,
        );
        assert_eq!(contribution.failures.len(), 1);
        assert_eq!(contribution.packs.len(), 0);
    }

    fn segment(offset: f32) -> ravel_sdk::JsonValue {
        json!({
            "start": [offset, 0.0, 0.0],
            "end": [offset, 1.0, 0.0],
            "normal": [0.0, 0.0, 1.0],
        })
    }

    #[test]
    fn a_mapping_flattens_to_nine_floats_per_segment() {
        let outlines = load(
            &[(
                StateId(1),
                json!({ "segments": [segment(0.0), segment(1.0)] }),
            )],
            &[],
            &spellings(),
        )
        .expect("two well-formed segments");
        let flat = outlines.at(StateId(1)).expect("the state has an outline");
        assert_eq!(flat.len(), 2 * FLOATS_PER_SEGMENT);
        assert_eq!(&flat[..3], &[0.0, 0.0, 0.0]);
        assert_eq!(&flat[9..12], &[1.0, 0.0, 0.0]);
    }

    #[test]
    fn the_two_document_shapes_are_told_apart_by_whether_the_lines_key_is_named() {
        let outlines = load(
            &[
                (StateId(1), json!({ "segments": [segment(0.0)] })),
                (StateId(2), json!({ "outline": "shared" })),
            ],
            &[json!({ "name": "shared", "segments": [segment(0.0)] })],
            &spellings(),
        )
        .expect("one inline shape and one named");
        assert_eq!(outlines.states(), 2);
        assert_eq!(outlines.arrays(), 1, "identical outlines share one array");
        assert_eq!(outlines.at(StateId(1)), outlines.at(StateId(2)));
    }

    #[test]
    fn two_different_outlines_do_not_share_an_array() {
        let outlines = load(
            &[
                (StateId(1), json!({ "segments": [segment(0.0)] })),
                (StateId(2), json!({ "segments": [segment(5.0)] })),
            ],
            &[],
            &spellings(),
        )
        .expect("two well-formed documents");
        assert_eq!(outlines.arrays(), 2);
        assert_ne!(outlines.at(StateId(1)), outlines.at(StateId(2)));
    }

    #[test]
    fn a_name_is_resolvable_only_during_the_load_that_defined_it() {
        let error = load(
            &[(StateId(1), json!({ "outline": "from_last_time" }))],
            &[],
            &spellings(),
        )
        .expect_err("nothing defined the name in this load");
        assert_eq!(
            error,
            OutlineError::UnknownName("from_last_time".to_owned())
        );
    }

    #[test]
    fn the_lookup_is_by_exact_state_identity_rather_than_by_block() {
        let outlines = load(
            &[(StateId(1), json!({ "segments": [segment(0.0)] }))],
            &[],
            &spellings(),
        )
        .expect("one well-formed document");
        assert!(outlines.at(StateId(1)).is_some());
        assert!(
            outlines.at(StateId(2)).is_none(),
            "a second state of the same block has its own entry or none"
        );
    }

    #[test]
    fn a_block_declaring_its_own_outline_wins_outright() {
        let outlines = load(
            &[(StateId(1), json!({ "segments": [segment(0.0)] }))],
            &[],
            &spellings(),
        )
        .expect("one well-formed document");
        assert_eq!(draw(&outlines, StateId(1), true), Drawn::OwnOutline);
        assert!(matches!(
            draw(&outlines, StateId(1), false),
            Drawn::Loaded(_)
        ));
        assert_eq!(draw(&outlines, StateId(9), false), Drawn::HostDefault);
    }

    #[test]
    fn an_array_whose_length_is_not_a_multiple_of_nine_is_skipped_rather_than_drawn_partially() {
        let partial = vec![0.0_f32; FLOATS_PER_SEGMENT + 1];
        let whole = vec![0.0_f32; FLOATS_PER_SEGMENT];
        let mut mapping = BTreeMap::new();
        mapping.insert(StateId(1), 0);
        mapping.insert(StateId(2), 1);
        let outlines = Outlines::from_arrays(vec![partial, whole], mapping);
        assert!(outlines.at(StateId(1)).is_none());
        assert_eq!(draw(&outlines, StateId(1), false), Drawn::HostDefault);
        assert!(outlines.at(StateId(2)).is_some());
    }

    #[test]
    fn a_mapping_naming_an_array_that_is_not_there_is_dropped() {
        let mut mapping = BTreeMap::new();
        mapping.insert(StateId(1), 9);
        let outlines = Outlines::from_arrays(Vec::new(), mapping);
        assert_eq!(outlines.states(), 0);
        assert_eq!(outlines.states(), 0);
    }

    #[test]
    fn a_highlight_document_is_refused_five_ways() {
        assert_eq!(
            load(&[(StateId(1), json!(4))], &[], &spellings()),
            Err(OutlineError::NotAnObject)
        );
        assert_eq!(
            load(&[(StateId(1), json!({}))], &[], &spellings()),
            Err(OutlineError::BadMapping)
        );
        assert_eq!(
            load(
                &[(StateId(1), json!({ "segments": [{ "start": [0.0, 0.0] }] }))],
                &[],
                &spellings()
            ),
            Err(OutlineError::BadSegment)
        );
        assert_eq!(
            load(&[(StateId(1), json!({ "segments": 4 }))], &[], &spellings()),
            Err(OutlineError::BadSegment)
        );
        let many: Vec<_> = (0..=super::MAX_SEGMENTS).map(|_| segment(0.0)).collect();
        assert_eq!(
            load(
                &[(StateId(1), json!({ "segments": many }))],
                &[],
                &spellings()
            ),
            Err(OutlineError::TooManySegments(super::MAX_SEGMENTS + 1))
        );
    }

    #[test]
    fn an_empty_load_holds_nothing_and_says_so() {
        let outlines = load(&[], &[], &spellings()).expect("nothing to load");
        assert_eq!(outlines.states(), 0);
        assert_eq!(outlines.arrays(), 0);
    }

    #[test]
    fn the_shipped_colour_is_transparent_in_every_channel_but_the_opacity() {
        assert_eq!(HIGHLIGHT_COLOUR.red, 0);
        assert_eq!(HIGHLIGHT_COLOUR.green, 0);
        assert_eq!(HIGHLIGHT_COLOUR.blue, 0);
        assert_ne!(HIGHLIGHT_COLOUR.alpha, 0);
    }

    #[test]
    fn one_segment_flattens_to_start_then_end_then_normal() {
        let flat = flatten_segment(Segment {
            start: [1.0, 2.0, 3.0],
            end: [4.0, 5.0, 6.0],
            normal: [7.0, 8.0, 9.0],
        });
        assert_eq!(flat, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }
}
