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

use num_traits::ToPrimitive;
use ravel_sdk::JsonValue;

use crate::colour::Colour;

mod declarations;

pub use declarations::{
    Contributed, Contribution, Declaration, Declaring, Failure, MAX_DECLARED_PACKS, PACK_ROOT,
    PackError, PackKind, Spellings, SpellingsUnavailable, contribute,
};

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
    Loaded {
        /// The flattened segments to draw.
        segments: &'a [f32],
        /// The process-wide shipped colour and opacity.
        colour: Colour,
    },
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
    outlines
        .at(state)
        .map_or(Drawn::HostDefault, |segments| Drawn::Loaded {
            segments,
            colour: Colour::new(0x00, 0x00, 0x00, 0x66),
        })
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
                flat.push(
                    number
                        .to_f32()
                        .expect("float narrowing includes infinities and NaN"),
                );
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
        Contributed, Declaration, Declaring, Drawn, FLOATS_PER_SEGMENT, Failure, OutlineError,
        Outlines, PackError, PackKind, Segment, Spellings, SpellingsUnavailable, StateId,
        contribute, draw, flatten_segment, load,
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
        let mut declaration = Declaration::named("alternative").expect("a checked pack name");
        assert_eq!(
            Declaration::decode(&json!("alternative")),
            Ok(declaration.clone())
        );
        declaration.description = Some("a second look".to_owned());
        declaration.required = true;
        assert_eq!(
            Declaration::decode(&json!({
                "name": "alternative",
                "description": "a second look",
                "required": true
            })),
            Ok(declaration)
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
        assert_eq!(
            cause,
            &PackError::Malformed("a declaration is neither a name nor a record".to_owned())
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
        let Drawn::Loaded { segments, colour } = draw(&outlines, StateId(1), false) else {
            panic!("a loaded outline is drawn when the block declares none");
        };
        assert_eq!(
            segments,
            outlines.at(StateId(1)).expect("the loaded geometry")
        );
        assert_eq!(colour, crate::colour::Colour::new(0, 0, 0, 0x66));
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
    fn one_segment_flattens_to_start_then_end_then_normal() {
        let flat = flatten_segment(Segment {
            start: [1.0, 2.0, 3.0],
            end: [4.0, 5.0, 6.0],
            normal: [7.0, 8.0, 9.0],
        });
        assert_eq!(flat, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }
}
