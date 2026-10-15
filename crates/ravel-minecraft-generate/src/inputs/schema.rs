//! Serialized version inputs and their shared accessors.

use serde::Deserialize;

/// Where the names the generator resolves against come from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MappingSource {
    /// The jar declares the names the reviewed inventory carries.
    UnobfuscatedClassFiles,
    /// The version's own published mapping file relates the two name spaces.
    OfficialProguardMappings,
}

impl MappingSource {
    /// How the adapter inputs and the generated locks write this down.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UnobfuscatedClassFiles => "unobfuscated-class-files",
            Self::OfficialProguardMappings => "official-proguard-mappings",
        }
    }

    /// Whether this source needs a published mapping file to be read.
    pub(crate) const fn needs_mappings(self) -> bool {
        matches!(self, Self::OfficialProguardMappings)
    }
}

/// Shape of a hook target, which the class file's flags have to agree with.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    StaticMethod,
    InstanceMethod,
    StaticField,
    InstanceField,
}

impl Kind {
    /// Whether the target is a method rather than a field.
    pub(crate) const fn is_method(self) -> bool {
        matches!(self, Self::StaticMethod | Self::InstanceMethod)
    }

    /// Whether the class file has to declare the target static.
    pub(crate) const fn is_static(self) -> bool {
        matches!(self, Self::StaticMethod | Self::StaticField)
    }

    /// The `ravel-minecraft` variant the generated binding names.
    pub(crate) const fn variant(self) -> &'static str {
        match self {
            Self::StaticMethod => "StaticMethod",
            Self::InstanceMethod => "InstanceMethod",
            Self::StaticField => "StaticField",
            Self::InstanceField => "InstanceField",
        }
    }
}

/// One reviewed hook entry.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Hook {
    pub(crate) id: String,
    pub(crate) class: String,
    pub(crate) member: String,
    pub(crate) descriptor: String,
    pub(crate) kind: Kind,
    pub(crate) capability: String,
    /// Whether this target's prologue may return from it before its body runs.
    #[serde(default)]
    pub(crate) refuses: bool,
    /// Where Ravel's own title-menu row goes, measured down from the column top.
    #[serde(default, rename = "titleMenuRow")]
    pub(crate) title_menu_row: Option<i16>,
}

/// One capability this version reviewed and deliberately does not carry.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NonGoal {
    pub(crate) id: String,
    pub(crate) reason: String,
}

/// The tracked hook inventory.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Inventory {
    pub(crate) schema_version: u8,
    pub(crate) minecraft: String,
    #[serde(rename = "hook")]
    pub(crate) hooks: Vec<Hook>,
    /// Absent means this version carries every capability it reviewed.
    #[serde(default, rename = "nonGoal")]
    pub(crate) non_goals: Vec<NonGoal>,
}

/// One member a screen template names, and the member this version spells it.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SeedMember {
    /// The template's word is the class's own member name.
    Same(String),
    /// The class spells this member differently from the word templates use.
    Bound(BoundMember),
}

/// Which overload of one name a role means.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Overload {
    /// Descriptor parameter count, including implicit enclosing instances.
    Count(usize),
    /// Ordered descriptor parameter types in Java source spelling.
    Types(Vec<String>),
}

impl Overload {
    /// Whether one descriptor's parameters are the ones this names.
    pub(crate) fn matches(&self, parameters: &[String]) -> bool {
        match self {
            Self::Count(count) => parameters.len() == *count,
            Self::Types(types) => types.as_slice() == parameters,
        }
    }
}

/// A member role bound to the name one version's class declares.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoundMember {
    pub(crate) role: String,
    pub(crate) member: String,
    /// Which overload of that name this role means.
    #[serde(default)]
    pub(crate) parameters: Option<Overload>,
}

impl SeedMember {
    /// The word a template names this member by.
    pub(crate) fn role(&self) -> &str {
        match self {
            Self::Same(name) => name,
            Self::Bound(bound) => &bound.role,
        }
    }

    /// The name this version's class declares.
    pub(crate) fn member(&self) -> &str {
        match self {
            Self::Same(name) => name,
            Self::Bound(bound) => &bound.member,
        }
    }

    /// Which overload this role means, where the binding says.
    pub(crate) const fn parameters(&self) -> Option<&Overload> {
        match self {
            Self::Same(_) => None,
            Self::Bound(bound) => bound.parameters.as_ref(),
        }
    }
}

/// One class the settings screen compiles against.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Seed {
    pub(crate) role: String,
    pub(crate) class: String,
    pub(crate) members: Vec<SeedMember>,
}

/// The tracked list of classes to stub.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Seeds {
    pub(crate) schema_version: u8,
    pub(crate) minecraft: String,
    #[serde(rename = "stub")]
    pub(crate) stubs: Vec<Seed>,
}

/// One game class an adapter under `jvm/bootstrap` resolves by name.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reflected {
    pub(crate) name: String,
    pub(crate) class: String,
    pub(crate) runtime: String,
    /// Every member of this class the agent jar reaches by name.
    #[serde(default, rename = "member")]
    pub(crate) members: Vec<ReflectedMember>,
}

/// One member, validated once while reading the reflection declaration.
#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "RawReflectedMember")]
pub(crate) struct ReflectedMember {
    pub(crate) name: String,
    pub(crate) lookup: Lookup,
}

/// How the agent jar asks a class for one of its members.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Lookup {
    Descriptor(String),
    Parameters(usize),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReflectedMember {
    name: String,
    descriptor: Option<String>,
    parameters: Option<usize>,
}

impl TryFrom<RawReflectedMember> for ReflectedMember {
    type Error = String;

    fn try_from(raw: RawReflectedMember) -> Result<Self, Self::Error> {
        let lookup = match (raw.descriptor, raw.parameters) {
            (Some(descriptor), None) => Lookup::Descriptor(descriptor),
            (None, Some(parameters)) => Lookup::Parameters(parameters),
            _ => {
                return Err(format!(
                    "{} must declare exactly one of descriptor or parameters",
                    raw.name
                ));
            }
        };
        Ok(Self {
            name: raw.name,
            lookup,
        })
    }
}

/// The tracked set of classes the always-present agent jar resolves by name.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Reflection {
    pub(crate) schema_version: u8,
    pub(crate) minecraft: String,
    #[serde(rename = "reflected")]
    pub(crate) classes: Vec<Reflected>,
}

/// One pinned input file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) sha256: Option<String>,
    #[serde(default)]
    pub(crate) bytes: Option<u64>,
    #[serde(default)]
    pub(crate) upstream: Option<String>,
}

/// Inputs the generator is allowed to read.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Inputs {
    pub(crate) client_jar: Input,
    /// The version's published client mapping, where the source calls for one.
    #[serde(default)]
    pub(crate) client_mappings: Option<Input>,
    /// Library jar declaring serialization types absent from the client jar.
    pub(crate) data_fixer_jar: Input,
    pub(crate) version_manifest: Input,
    pub(crate) hook_inventory: Input,
    pub(crate) stub_seeds: Input,
    /// The reviewed set of classes the agent jar's adapters resolve by name.
    pub(crate) reflective_surface: Input,
    /// The generated mapping lock, pinned to the jars that produced it.
    pub(crate) mappings_lock: Input,
    /// The generated stub lock, pinned to the jars that produced it.
    pub(crate) stubs_lock: Input,
}

/// The tracked adapter inputs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Adapter {
    pub(crate) schema_version: u8,
    pub(crate) minecraft: String,
    pub(crate) mapping_source: MappingSource,
    /// The Java release every jar built for this version is compiled for.
    pub(crate) java_release: u16,
    /// The pack formats this version's own resource and data packs declare.
    pub(crate) pack_format: PackFormat,
    pub(crate) inputs: Inputs,
}

/// The pack formats one Minecraft version declares, and how it spells them.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PackFormat {
    /// What a resource pack this version accepts declares.
    pub(crate) resource: u16,
    /// The resource format's minor, which a version that states none has as zero.
    pub(crate) resource_minor: u16,
    /// What a data pack this version accepts declares.
    pub(crate) data: u16,
    /// The data format's minor, which a version that states none has as zero.
    pub(crate) data_minor: u16,
    /// The document shape this version's own packs are written in.
    pub(crate) spelling: Spelling,
}

/// How a version's own packs spell the format they declare.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Spelling {
    /// One `pack_format` integer.
    PackFormat,
    /// A `max_format` integer beside a `min_format` major and minor.
    MinMaxFormat,
}

impl Spelling {
    /// The key a pack.mcmeta written in this spelling always carries.
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::PackFormat => "\"pack_format\"",
            Self::MinMaxFormat => "\"min_format\"",
        }
    }

    /// The name this spelling is declared under.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::PackFormat => "pack-format",
            Self::MinMaxFormat => "min-max-format",
        }
    }
}
