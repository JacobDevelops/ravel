use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroU32;

use ravel_core::{
    DependencyRelationship, FailureKind, ModId, Side, SideSet, Version, VersionRange,
};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

use crate::PackageFailure;
use crate::values::{NonEmptyText, RelativePath, SchemaVersion, Sha256Digest};

/// Everything `schemas/package.schema.json` declares about one package.
///
/// The field order, the camel-case names, the optional fields, and the refusal
/// of anything else are the schema's, not this crate's: the schema is the
/// contract a package is admitted against, and a manifest that parses here must
/// be one the schema validates. `additionalProperties: false` appears on every
/// object in the document, so every struct here carries `deny_unknown_fields`,
/// and a typo in a field name is a rejection rather than a silently dropped
/// declaration.
///
/// Parsing establishes identity, compatibility, and the integrity claims a
/// package makes. It does not select a target, load an artifact, or interpret
/// an artifact `role`: ADG-2 is deferred to M3 and
/// `docs/architecture/dependency-rules.md` forbids production code selecting a
/// mod execution model before it is accepted.
///
/// # Examples
///
/// ```
/// use ravel_package::PackageManifest;
///
/// let document = r#"{
///   "schemaVersion": 1,
///   "id": "example_mod",
///   "version": "1.4.0",
///   "compatibility": { "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0"] },
///   "sides": ["client"],
///   "targets": [
///     {
///       "platform": "linux",
///       "architecture": "x86_64",
///       "artifacts": [
///         {
///           "path": "artifacts/mod.bin",
///           "role": "module",
///           "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
///         }
///       ]
///     }
///   ],
///   "authors": [{ "name": "Example" }],
///   "license": "MIT OR Apache-2.0",
///   "source": "https://example.invalid/example_mod",
///   "provenance": {
///     "record": "provenance.json",
///     "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
///   }
/// }"#;
///
/// let manifest = PackageManifest::parse(document)?;
/// assert_eq!(manifest.id.as_str(), "example_mod");
/// assert!(manifest.compatibility.check_minecraft("26.0.0").is_ok());
/// # Ok::<(), ravel_package::ManifestError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PackageManifest {
    /// The manifest contract this document was written against.
    pub schema_version: SchemaVersion,
    /// The canonical mod identity.
    pub id: ModId,
    /// The exact released version of this package.
    pub version: Version,
    /// A human-readable name, defaulting to nothing rather than to the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<NonEmptyText>,
    /// Free-form description text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What this package will run against.
    pub compatibility: Compatibility,
    /// The sides this package loads on.
    #[serde(deserialize_with = "deserialize_sides")]
    pub sides: SideSet,
    /// Declared relationships to other packages, empty when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
    /// The platform and architecture combinations this package ships for.
    #[serde(deserialize_with = "deserialize_non_empty")]
    pub targets: Vec<Target>,
    /// Package-relative resource paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<RelativePath>,
    /// Package-relative configuration schema paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configuration_schemas: Vec<RelativePath>,
    /// Saved-data versions this package owns.
    #[serde(
        default,
        deserialize_with = "deserialize_unique",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub saved_data_versions: Vec<NonZeroU32>,
    /// Who wrote it.
    #[serde(deserialize_with = "deserialize_non_empty")]
    pub authors: Vec<Author>,
    /// An SPDX licence expression.
    pub license: NonEmptyText,
    /// Where the package came from.
    ///
    /// The schema annotates this with `format: uri` and asserts nothing else,
    /// which in draft 2020-12 is an annotation rather than a constraint, so
    /// this crate keeps the declared text as it was written.
    pub source: String,
    /// The provenance record and the digest that proves it was not altered.
    pub provenance: Provenance,
}

impl PackageManifest {
    /// Parses a package manifest.
    ///
    /// The document is accepted only when it satisfies
    /// `schemas/package.schema.json` in full, which is what admits a package to
    /// [`ravel_core::LifecycleState::Validated`] alongside its integrity check.
    /// No mod code has run at this point and none may: `SPEC.md` §14 requires
    /// schema and metadata acceptance to precede everything else.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Syntax`] when the bytes are not JSON, and
    /// [`ManifestError::Schema`] when they are JSON that the package schema
    /// refuses.
    pub fn parse(document: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(document).map_err(|error| ManifestError::from_json(&error))
    }

    /// Renders the manifest back into the JSON document form it parses from.
    ///
    /// Absent optional fields stay absent and an empty array is written as the
    /// omission the schema defaults it to, so a manifest that parses from a
    /// document renders back to an equivalent one.
    ///
    /// # Panics
    ///
    /// Panics if the manifest cannot be written as JSON. Every field is a
    /// string, a number, a sequence, or a struct with string keys, so the only
    /// way this fires is a serializer that has stopped being one.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a parsed manifest always serializes")
    }
}

/// What a package will run against (`SPEC.md` §3.4).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    /// The Ravel versions this package supports, as a range.
    pub ravel: VersionRange,
    /// The exact Minecraft versions this package supports.
    ///
    /// This is a list of opaque exact strings and deliberately not a range. A
    /// Minecraft release is not required to be semver-shaped, so ordering two
    /// of them is not a question this crate is allowed to answer.
    #[serde(deserialize_with = "deserialize_unique_non_empty")]
    pub minecraft: Vec<NonEmptyText>,
}

/// One declared relationship to another package.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    /// The other package's canonical identity.
    pub id: ModId,
    /// How the relationship constrains the load graph.
    pub relationship: DependencyRelationship,
    /// The versions of the other package this relationship applies to.
    pub version: VersionRange,
    /// The sides the relationship narrows to, absent when it applies to all of
    /// them.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_sides",
        skip_serializing_if = "Option::is_none"
    )]
    pub sides: Option<SideSet>,
    /// The ordering this relationship requires, absent when order does not
    /// matter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<DependencyOrder>,
}

/// Where a dependant loads relative to its dependency.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyOrder {
    /// The declaring package loads first.
    Before,
    /// The declaring package loads second.
    After,
}

/// One platform and architecture combination a package ships for.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// The platform name, matching the value a failure report carries.
    pub platform: NonEmptyText,
    /// The architecture name, matching the value a failure report carries.
    pub architecture: NonEmptyText,
    /// The files this target ships.
    #[serde(deserialize_with = "deserialize_non_empty")]
    pub artifacts: Vec<Artifact>,
}

/// One file a target ships, with the digest that proves it was not altered.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Where the file sits inside the package.
    pub path: RelativePath,
    /// What the package says the file is for.
    ///
    /// The loader records the declared role and acts on none of it. What a role
    /// means to execution is ADG-2's question and M3 answers it.
    pub role: ArtifactRole,
    /// The digest [`crate::verify_artifact`] checks the file against.
    pub sha256: Sha256Digest,
}

/// What a package says one of its files is for.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactRole {
    /// A standalone executable.
    Executable,
    /// A loadable module.
    Module,
    /// A supporting library.
    Library,
    /// Game or mod content.
    Resource,
    /// Descriptive data about the package.
    Metadata,
    /// Anything the other roles do not name.
    Other,
}

/// One author of a package.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Author {
    /// The author's name.
    pub name: NonEmptyText,
    /// A URL for the author, absent when none was declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// The provenance record required by `SPEC.md` §3.4 and its digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Where the record sits inside the package.
    pub record: RelativePath,
    /// The digest [`crate::verify_provenance`] checks the record against.
    pub sha256: Sha256Digest,
}

/// Enforces a schema `minItems: 1` array.
fn deserialize_non_empty<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Vec::<T>::deserialize(deserializer)?;
    if items.is_empty() {
        return Err(de::Error::invalid_length(0, &"at least one entry"));
    }
    Ok(items)
}

/// Enforces a schema `uniqueItems: true` array.
fn deserialize_unique<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Ord + fmt::Display,
{
    let items = Vec::<T>::deserialize(deserializer)?;
    let mut seen = BTreeSet::new();
    for item in &items {
        if !seen.insert(item) {
            return Err(de::Error::custom(format_args!(
                "`{item}` is declared more than once"
            )));
        }
    }
    Ok(items)
}

/// Enforces a schema array that is both `minItems: 1` and `uniqueItems: true`.
fn deserialize_unique_non_empty<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Ord + fmt::Display,
{
    let items = deserialize_unique(deserializer)?;
    if items.is_empty() {
        return Err(de::Error::invalid_length(0, &"at least one entry"));
    }
    Ok(items)
}

/// Reads a side declaration, keeping both schema assertions the collapsing
/// [`SideSet`] representation would otherwise lose.
fn deserialize_sides<'de, D>(deserializer: D) -> Result<SideSet, D::Error>
where
    D: Deserializer<'de>,
{
    let declared = Vec::<Side>::deserialize(deserializer)?;
    let sides = SideSet::new(declared.iter().copied()).map_err(de::Error::custom)?;
    if sides.sides().count() != declared.len() {
        return Err(de::Error::custom("a side is declared more than once"));
    }
    Ok(sides)
}

/// Reads the optional side narrowing a dependency may declare.
fn deserialize_optional_sides<'de, D>(deserializer: D) -> Result<Option<SideSet>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_sides(deserializer).map(Some)
}

/// Why a document is not a package manifest.
///
/// The two variants separate a document that is not JSON from a document that
/// is JSON and breaks the package contract, because they are different author
/// mistakes and `SPEC.md` §9.2 requires a report a mod author can act on.
/// Neither carries a filesystem path: the detail comes from the deserializer,
/// which reports a line and a column rather than a location on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    /// The bytes are not a JSON document.
    Syntax(String),
    /// The document is JSON that `schemas/package.schema.json` refuses.
    Schema(String),
}

impl ManifestError {
    fn from_json(error: &serde_json::Error) -> Self {
        match error.classify() {
            serde_json::error::Category::Data => Self::Schema(error.to_string()),
            _ => Self::Syntax(error.to_string()),
        }
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(detail) => write!(formatter, "the manifest is not JSON: {detail}"),
            Self::Schema(detail) => {
                write!(
                    formatter,
                    "the manifest breaks the package schema: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ManifestError {}

impl PackageFailure for ManifestError {
    /// A malformed manifest has no `SPEC.md` §9.2 kind.
    ///
    /// That list names the dependency-graph outcomes a player is shown before
    /// world access. A manifest that never parsed has no identity to attribute
    /// a graph failure to, so this reports no kind rather than borrowing one
    /// that would misdescribe it.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{DependencyRelationship, FailureKind, LifecycleState, Redaction, Side};
    use serde_json::{Value, json};

    use super::{ArtifactRole, DependencyOrder, ManifestError, PackageManifest};
    use crate::PackageFailure;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const OTHER_DIGEST: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    const REQUIRED_FIELDS: [&str; 10] = [
        "schemaVersion",
        "id",
        "version",
        "compatibility",
        "sides",
        "targets",
        "authors",
        "license",
        "source",
        "provenance",
    ];

    /// A manifest that declares every required field and nothing else.
    fn minimal() -> Value {
        json!({
            "schemaVersion": 1,
            "id": "example_mod",
            "version": "1.4.0",
            "compatibility": { "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0"] },
            "sides": ["client"],
            "targets": [{
                "platform": "linux",
                "architecture": "x86_64",
                "artifacts": [{ "path": "artifacts/mod.bin", "role": "module", "sha256": DIGEST }]
            }],
            "authors": [{ "name": "Example" }],
            "license": "MIT OR Apache-2.0",
            "source": "https://example.invalid/example_mod",
            "provenance": { "record": "provenance.json", "sha256": OTHER_DIGEST }
        })
    }

    /// A manifest that declares every field the schema defines.
    fn complete() -> Value {
        json!({
            "schemaVersion": 1,
            "id": "example_mod",
            "version": "1.4.0-rc.1+build.7",
            "displayName": "Example Mod",
            "description": "An example.",
            "compatibility": {
                "ravel": ">=0.1, <0.2",
                "minecraft": ["26.0.0", "26.0.1"]
            },
            "sides": ["client", "integrated-server"],
            "dependencies": [
                {
                    "id": "example_library",
                    "relationship": "required",
                    "version": "^1.0.0",
                    "sides": ["client"],
                    "order": "after"
                },
                { "id": "example_rival", "relationship": "incompatible", "version": "*" }
            ],
            "targets": [{
                "platform": "linux",
                "architecture": "x86_64",
                "artifacts": [
                    { "path": "artifacts/mod.bin", "role": "module", "sha256": DIGEST },
                    { "path": "artifacts/data.pack", "role": "resource", "sha256": OTHER_DIGEST }
                ]
            }],
            "resources": ["resources/lang/en.json"],
            "configurationSchemas": ["schemas/config.json"],
            "savedDataVersions": [1, 2],
            "authors": [
                { "name": "Example", "url": "https://example.invalid/example" },
                { "name": "Second Example" }
            ],
            "license": "MIT OR Apache-2.0",
            "source": "https://example.invalid/example_mod",
            "provenance": { "record": "provenance.json", "sha256": OTHER_DIGEST }
        })
    }

    fn parse(document: &Value) -> Result<PackageManifest, ManifestError> {
        PackageManifest::parse(&document.to_string())
    }

    fn rejection(document: &Value) -> String {
        let error = parse(document).expect_err("the manifest should be refused");
        assert_eq!(error.failure_kind(), None);
        assert!(matches!(error, ManifestError::Schema(_)), "{error}");
        error.to_string()
    }

    #[test]
    fn a_complete_manifest_round_trips_through_its_document_form() {
        let document = complete();
        let manifest = parse(&document).expect("a complete manifest");

        let rendered = manifest.to_json();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered).expect("rendered manifest is JSON"),
            document,
            "{rendered}"
        );
        assert_eq!(
            PackageManifest::parse(&rendered).expect("a rendered manifest re-parses"),
            manifest
        );
    }

    #[test]
    fn a_minimal_manifest_round_trips_without_gaining_defaulted_fields() {
        let document = minimal();
        let manifest = parse(&document).expect("a minimal manifest");

        assert_eq!(manifest.display_name, None);
        assert_eq!(manifest.description, None);
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.resources.is_empty());
        assert!(manifest.configuration_schemas.is_empty());
        assert!(manifest.saved_data_versions.is_empty());

        let rendered = manifest.to_json();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered).expect("rendered manifest is JSON"),
            document,
            "{rendered}"
        );
    }

    #[test]
    fn an_explicitly_empty_optional_array_renders_back_as_the_omission_it_defaults_to() {
        let mut document = minimal();
        document["dependencies"] = json!([]);
        document["resources"] = json!([]);
        document["configurationSchemas"] = json!([]);
        document["savedDataVersions"] = json!([]);

        let manifest = parse(&document).expect("empty optional arrays are legal");
        assert_eq!(
            serde_json::from_str::<Value>(&manifest.to_json()).expect("rendered manifest is JSON"),
            minimal()
        );
    }

    #[test]
    fn a_manifest_carries_every_declaration_the_document_made() {
        let manifest = parse(&complete()).expect("a complete manifest");

        assert_eq!(manifest.schema_version.get(), 1);
        assert_eq!(manifest.id.as_str(), "example_mod");
        assert_eq!(manifest.version.to_string(), "1.4.0-rc.1+build.7");
        assert_eq!(
            manifest
                .display_name
                .as_ref()
                .map(super::NonEmptyText::as_str),
            Some("Example Mod")
        );
        assert_eq!(manifest.description.as_deref(), Some("An example."));
        assert!(manifest.sides.loads_on(Side::IntegratedServer));
        assert!(!manifest.sides.loads_on(Side::DedicatedServer));
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(
            manifest.dependencies[0].relationship,
            DependencyRelationship::Required
        );
        assert_eq!(manifest.dependencies[0].order, Some(DependencyOrder::After));
        assert_eq!(
            manifest.dependencies[0]
                .sides
                .as_ref()
                .map(|sides| sides.loads_on(Side::Client)),
            Some(true)
        );
        assert_eq!(
            manifest.dependencies[1].relationship,
            DependencyRelationship::Incompatible
        );
        assert_eq!(manifest.dependencies[1].sides, None);
        assert_eq!(manifest.dependencies[1].order, None);
        assert_eq!(manifest.targets[0].platform.as_str(), "linux");
        assert_eq!(manifest.targets[0].architecture.as_str(), "x86_64");
        assert_eq!(manifest.targets[0].artifacts[0].role, ArtifactRole::Module);
        assert_eq!(
            manifest.targets[0].artifacts[1].role,
            ArtifactRole::Resource
        );
        assert_eq!(manifest.targets[0].artifacts[0].sha256.to_string(), DIGEST);
        assert_eq!(manifest.resources[0].as_str(), "resources/lang/en.json");
        assert_eq!(manifest.configuration_schemas.len(), 1);
        assert_eq!(manifest.saved_data_versions[1].get(), 2);
        assert_eq!(manifest.authors[0].name.as_str(), "Example");
        assert_eq!(manifest.authors[1].url, None);
        assert_eq!(manifest.license.as_str(), "MIT OR Apache-2.0");
        assert_eq!(manifest.source, "https://example.invalid/example_mod");
        assert_eq!(manifest.provenance.record.file_name(), "provenance.json");
    }

    #[test]
    fn every_artifact_role_the_schema_names_parses() {
        for (name, expected) in [
            ("executable", ArtifactRole::Executable),
            ("module", ArtifactRole::Module),
            ("library", ArtifactRole::Library),
            ("resource", ArtifactRole::Resource),
            ("metadata", ArtifactRole::Metadata),
            ("other", ArtifactRole::Other),
        ] {
            let mut document = minimal();
            document["targets"][0]["artifacts"][0]["role"] = json!(name);
            let manifest = parse(&document).expect(name);
            assert_eq!(manifest.targets[0].artifacts[0].role, expected);
        }
        let mut document = minimal();
        document["targets"][0]["artifacts"][0]["role"] = json!("plugin");
        assert!(rejection(&document).contains("unknown variant"));
    }

    #[test]
    fn every_dependency_ordering_the_schema_names_parses() {
        for (name, expected) in [
            ("before", DependencyOrder::Before),
            ("after", DependencyOrder::After),
        ] {
            let mut document = minimal();
            document["dependencies"] = json!([{
                "id": "example_library", "relationship": "optional", "version": "*", "order": name
            }]);
            let manifest = parse(&document).expect(name);
            assert_eq!(manifest.dependencies[0].order, Some(expected));
        }
    }

    #[test]
    fn a_document_that_is_not_json_is_a_syntax_failure_distinct_from_a_schema_failure() {
        let syntax = PackageManifest::parse("not json at all").expect_err("not a document");
        assert!(matches!(syntax, ManifestError::Syntax(_)), "{syntax}");
        assert!(syntax.to_string().starts_with("the manifest is not JSON"));
        assert_eq!(syntax.failure_kind(), None);
        assert_eq!(syntax.lifecycle_state(), LifecycleState::Discovered);

        let schema = parse(&json!({})).expect_err("an empty object");
        assert!(matches!(schema, ManifestError::Schema(_)), "{schema}");
        assert!(
            schema
                .to_string()
                .starts_with("the manifest breaks the package schema")
        );
        assert_ne!(syntax, schema);
    }

    #[test]
    fn a_manifest_failure_publishes_a_redacted_diagnostic() {
        let error = parse(&json!({})).expect_err("an empty object");
        let diagnostic = error.diagnostic();
        assert_eq!(diagnostic.redaction, Redaction::Applied);
        assert_eq!(diagnostic.text, error.to_string());
    }

    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        for (owner, field) in [
            ("", "signature"),
            ("compatibility", "fabric"),
            ("provenance", "signature"),
        ] {
            let mut document = minimal();
            if owner.is_empty() {
                document[field] = json!("anything");
            } else {
                document[owner][field] = json!("anything");
            }
            assert!(rejection(&document).contains("unknown field"), "{field}");
        }

        let mut document = minimal();
        document["targets"][0]["artifacts"][0]["size"] = json!(12);
        assert!(rejection(&document).contains("unknown field"));

        let mut document = minimal();
        document["authors"][0]["email"] = json!("someone@example.invalid");
        assert!(rejection(&document).contains("unknown field"));

        let mut document = minimal();
        document["dependencies"] = json!([{
            "id": "example_library", "relationship": "optional", "version": "*", "reason": "why"
        }]);
        assert!(rejection(&document).contains("unknown field"));
    }

    #[test]
    fn every_required_field_is_required() {
        for field in REQUIRED_FIELDS {
            let mut document = minimal();
            document
                .as_object_mut()
                .expect("the manifest is an object")
                .remove(field);
            assert!(
                rejection(&document).contains(&format!("missing field `{field}`")),
                "{field}"
            );
        }

        for (owner, field) in [
            ("compatibility", "ravel"),
            ("compatibility", "minecraft"),
            ("provenance", "record"),
            ("provenance", "sha256"),
        ] {
            let mut document = minimal();
            document[owner]
                .as_object_mut()
                .expect("a nested object")
                .remove(field);
            assert!(
                rejection(&document).contains(&format!("missing field `{field}`")),
                "{owner}.{field}"
            );
        }
    }

    #[test]
    fn a_schema_version_other_than_the_implemented_one_is_refused() {
        let mut document = minimal();
        document["schemaVersion"] = json!(2);
        assert!(rejection(&document).contains("schemaVersion 1, not 2"));
    }

    #[test]
    fn an_identity_that_breaks_the_mod_id_grammar_is_refused() {
        for candidate in ["Example", "9lives", "example_", ""] {
            let mut document = minimal();
            document["id"] = json!(candidate);
            assert!(rejection(&document).contains("mod id"), "{candidate}");
        }
    }

    #[test]
    fn a_version_that_is_not_an_exact_release_is_refused() {
        for candidate in ["1.4", "1", "latest", ""] {
            let mut document = minimal();
            document["version"] = json!(candidate);
            assert!(!rejection(&document).is_empty(), "{candidate}");
        }
    }

    #[test]
    fn a_malformed_range_is_refused_wherever_a_range_appears() {
        for candidate in [">=", "!1.0.0", ""] {
            let mut document = minimal();
            document["compatibility"]["ravel"] = json!(candidate);
            rejection(&document);

            let mut document = minimal();
            document["dependencies"] = json!([{
                "id": "example_library", "relationship": "required", "version": candidate
            }]);
            rejection(&document);
        }
    }

    #[test]
    fn a_side_declaration_must_name_at_least_one_side_and_name_none_twice() {
        let mut document = minimal();
        document["sides"] = json!([]);
        assert!(rejection(&document).contains("at least one side"));

        let mut document = minimal();
        document["sides"] = json!(["client", "client"]);
        assert!(rejection(&document).contains("declared more than once"));

        let mut document = minimal();
        document["sides"] = json!(["server"]);
        assert!(rejection(&document).contains("unknown variant"));

        let mut document = minimal();
        document["dependencies"] = json!([{
            "id": "example_library", "relationship": "required", "version": "*", "sides": []
        }]);
        assert!(rejection(&document).contains("at least one side"));

        let mut document = minimal();
        document["dependencies"] = json!([{
            "id": "example_library",
            "relationship": "required",
            "version": "*",
            "sides": ["client", "client"]
        }]);
        assert!(rejection(&document).contains("declared more than once"));
    }

    #[test]
    fn an_absolute_or_traversing_path_is_refused_wherever_a_path_appears() {
        for candidate in ["/etc/passwd", "../escape", "nested/../../escape", ""] {
            for pointer in ["resources", "configurationSchemas"] {
                let mut document = minimal();
                document[pointer] = json!([candidate]);
                let rendered = rejection(&document);
                assert!(rendered.contains("a package path"), "{rendered}");
                assert!(!rendered.contains("passwd"), "{rendered}");
            }

            let mut document = minimal();
            document["targets"][0]["artifacts"][0]["path"] = json!(candidate);
            assert!(rejection(&document).contains("a package path"));

            let mut document = minimal();
            document["provenance"]["record"] = json!(candidate);
            assert!(rejection(&document).contains("a package path"));
        }
    }

    #[test]
    fn a_digest_that_is_not_the_recorded_shape_is_refused() {
        for candidate in ["", "abc", &DIGEST.to_uppercase(), &format!("{DIGEST}0")] {
            let mut document = minimal();
            document["targets"][0]["artifacts"][0]["sha256"] = json!(candidate);
            assert!(
                rejection(&document).contains("sha256 digest"),
                "{candidate}"
            );

            let mut document = minimal();
            document["provenance"]["sha256"] = json!(candidate);
            assert!(
                rejection(&document).contains("sha256 digest"),
                "{candidate}"
            );
        }
    }

    #[test]
    fn every_list_the_schema_requires_an_entry_in_must_have_one() {
        for pointer in ["targets", "authors"] {
            let mut document = minimal();
            document[pointer] = json!([]);
            assert!(
                rejection(&document).contains("at least one entry"),
                "{pointer}"
            );
        }

        let mut document = minimal();
        document["targets"][0]["artifacts"] = json!([]);
        assert!(rejection(&document).contains("at least one entry"));

        let mut document = minimal();
        document["compatibility"]["minecraft"] = json!([]);
        assert!(rejection(&document).contains("at least one entry"));
    }

    #[test]
    fn every_list_the_schema_requires_unique_entries_in_refuses_a_repeat() {
        let mut document = minimal();
        document["compatibility"]["minecraft"] = json!(["26.0.0", "26.0.0"]);
        assert!(rejection(&document).contains("`26.0.0` is declared more than once"));

        let mut document = minimal();
        document["savedDataVersions"] = json!([1, 1]);
        assert!(rejection(&document).contains("`1` is declared more than once"));
    }

    #[test]
    fn a_saved_data_version_below_the_schema_minimum_is_refused() {
        for candidate in [json!(0), json!(-1)] {
            let mut document = minimal();
            document["savedDataVersions"] = json!([candidate]);
            rejection(&document);
        }
    }

    #[test]
    fn a_constrained_text_value_cannot_be_declared_empty() {
        for pointer in ["displayName", "license"] {
            let mut document = minimal();
            document[pointer] = json!("");
            assert!(
                rejection(&document).contains("must not be empty"),
                "{pointer}"
            );
        }

        for pointer in ["platform", "architecture"] {
            let mut document = minimal();
            document["targets"][0][pointer] = json!("");
            assert!(
                rejection(&document).contains("must not be empty"),
                "{pointer}"
            );
        }

        let mut document = minimal();
        document["authors"][0]["name"] = json!("");
        assert!(rejection(&document).contains("must not be empty"));

        let mut document = minimal();
        document["compatibility"]["minecraft"] = json!([""]);
        assert!(rejection(&document).contains("must not be empty"));
    }

    #[test]
    fn an_unknown_dependency_relationship_is_refused() {
        let mut document = minimal();
        document["dependencies"] = json!([{
            "id": "example_library", "relationship": "suggested", "version": "*"
        }]);
        assert!(rejection(&document).contains("unknown variant"));
    }

    #[test]
    fn no_manifest_failure_claims_a_dependency_failure_kind() {
        let kinds: Vec<Option<FailureKind>> = ["not json", "{}", "[]"]
            .into_iter()
            .map(|document| {
                PackageManifest::parse(document)
                    .expect_err("refused")
                    .failure_kind()
            })
            .collect();
        assert_eq!(kinds, vec![None, None, None]);
    }
}
