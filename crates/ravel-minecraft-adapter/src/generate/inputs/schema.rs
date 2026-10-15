//! Serialized version inputs and their shared accessors.

use serde::Deserialize;

/// Where the names the generator resolves against come from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(in crate::generate) enum MappingSource {
    /// The jar declares the names the reviewed inventory carries.
    UnobfuscatedClassFiles,
    /// The version's own published mapping file relates the two name spaces.
    OfficialProguardMappings,
}

impl MappingSource {
    /// How the adapter inputs and the generated locks write this down.
    pub(in crate::generate) const fn as_str(self) -> &'static str {
        match self {
            Self::UnobfuscatedClassFiles => "unobfuscated-class-files",
            Self::OfficialProguardMappings => "official-proguard-mappings",
        }
    }

    /// Whether this source needs a published mapping file to be read.
    pub(in crate::generate) const fn needs_mappings(self) -> bool {
        matches!(self, Self::OfficialProguardMappings)
    }
}

/// Shape of a hook target, which the class file's flags have to agree with.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(in crate::generate) enum Kind {
    StaticMethod,
    InstanceMethod,
    StaticField,
    InstanceField,
}

impl Kind {
    /// Whether the target is a method rather than a field.
    pub(in crate::generate) const fn is_method(self) -> bool {
        matches!(self, Self::StaticMethod | Self::InstanceMethod)
    }

    /// Whether the class file has to declare the target static.
    pub(in crate::generate) const fn is_static(self) -> bool {
        matches!(self, Self::StaticMethod | Self::StaticField)
    }

    /// The `ravel-minecraft` variant the generated binding names.
    pub(in crate::generate) const fn variant(self) -> &'static str {
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
pub(in crate::generate) struct Hook {
    pub(in crate::generate) id: String,
    pub(in crate::generate) class: String,
    pub(in crate::generate) member: String,
    pub(in crate::generate) descriptor: String,
    pub(in crate::generate) kind: Kind,
    pub(in crate::generate) capability: String,
    /// Whether this target's prologue may return from it before its body runs.
    #[serde(default)]
    pub(in crate::generate) refuses: bool,
    /// Where Ravel's own title-menu row goes, measured down from the column top.
    #[serde(default, rename = "titleMenuRow")]
    pub(in crate::generate) title_menu_row: Option<i16>,
}

/// One capability this version reviewed and deliberately does not carry.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::generate) struct NonGoal {
    pub(in crate::generate) id: String,
    pub(in crate::generate) reason: String,
}

/// The tracked hook inventory.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::generate) struct Inventory {
    pub(in crate::generate) schema_version: u8,
    pub(in crate::generate) minecraft: String,
    #[serde(rename = "hook")]
    pub(in crate::generate) hooks: Vec<Hook>,
    /// Absent means this version carries every capability it reviewed.
    #[serde(default, rename = "nonGoal")]
    pub(in crate::generate) non_goals: Vec<NonGoal>,
}

/// One member a screen template names, and the member this version spells it.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(in crate::generate) enum SeedMember {
    /// The template's word is the class's own member name.
    Same(String),
    /// The class spells this member differently from the word templates use.
    Bound(BoundMember),
}

/// Which overload of one name a role means.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(in crate::generate) enum Overload {
    /// Descriptor parameter count, including implicit enclosing instances.
    Count(usize),
    /// Ordered descriptor parameter types in Java source spelling.
    Types(Vec<String>),
}

impl Overload {
    /// Whether one descriptor's parameters are the ones this names.
    pub(in crate::generate) fn matches(&self, parameters: &[String]) -> bool {
        match self {
            Self::Count(count) => parameters.len() == *count,
            Self::Types(types) => types.as_slice() == parameters,
        }
    }
}

/// A member role bound to the name one version's class declares.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::generate) struct BoundMember {
    pub(in crate::generate) role: String,
    pub(in crate::generate) member: String,
    /// Which overload of that name this role means.
    #[serde(default)]
    pub(in crate::generate) parameters: Option<Overload>,
}

impl SeedMember {
    /// The word a template names this member by.
    pub(in crate::generate) fn role(&self) -> &str {
        match self {
            Self::Same(name) => name,
            Self::Bound(bound) => &bound.role,
        }
    }

    /// The name this version's class declares.
    pub(in crate::generate) fn member(&self) -> &str {
        match self {
            Self::Same(name) => name,
            Self::Bound(bound) => &bound.member,
        }
    }

    /// Which overload this role means, where the binding says.
    pub(in crate::generate) const fn parameters(&self) -> Option<&Overload> {
        match self {
            Self::Same(_) => None,
            Self::Bound(bound) => bound.parameters.as_ref(),
        }
    }
}

/// One class the settings screen compiles against.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::generate) struct Seed {
    pub(in crate::generate) role: String,
    pub(in crate::generate) class: String,
    pub(in crate::generate) members: Vec<SeedMember>,
}

/// The tracked list of classes to stub.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::generate) struct Seeds {
    pub(in crate::generate) schema_version: u8,
    pub(in crate::generate) minecraft: String,
    #[serde(rename = "stub")]
    pub(in crate::generate) stubs: Vec<Seed>,
}

/// One game class an adapter under `jvm/bootstrap` resolves by name.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::generate) struct Reflected {
    pub(in crate::generate) name: String,
    pub(in crate::generate) class: String,
    pub(in crate::generate) runtime: String,
    /// Every member of this class the agent jar reaches by name.
    #[serde(default, rename = "member")]
    pub(in crate::generate) members: Vec<ReflectedMember>,
}

/// One member of a reflected class the agent jar resolves by name.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::generate) struct ReflectedMember {
    pub(in crate::generate) name: String,
    #[serde(default)]
    pub(in crate::generate) descriptor: Option<String>,
    #[serde(default)]
    pub(in crate::generate) parameters: Option<usize>,
}

/// How the agent jar asks a class for one of its members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::generate) enum Lookup<'a> {
    /// By the parameter types it names, which fix exactly one descriptor.
    Descriptor(&'a str),
    /// By name and argument count, which is what the persistence adapter does.
    Parameters(usize),
}

impl ReflectedMember {
    /// Which of the two ways this member is declared.
    ///
    /// # Errors
    ///
    /// Returns the detail of a declaration that names both ways or neither.
    pub(in crate::generate) fn lookup(&self) -> Result<Lookup<'_>, String> {
        match (&self.descriptor, self.parameters) {
            (Some(descriptor), None) => Ok(Lookup::Descriptor(descriptor)),
            (None, Some(count)) => Ok(Lookup::Parameters(count)),
            (Some(_), Some(_)) => Err(format!(
                "{} is declared by both a descriptor and a parameter count, and a member is \
                 looked up one way",
                self.name
            )),
            (None, None) => Err(format!(
                "{} is declared by neither a descriptor nor a parameter count, so nothing says \
                 which member it is",
                self.name
            )),
        }
    }
}

/// The tracked set of classes the always-present agent jar resolves by name.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::generate) struct Reflection {
    pub(in crate::generate) schema_version: u8,
    pub(in crate::generate) minecraft: String,
    #[serde(rename = "reflected")]
    pub(in crate::generate) classes: Vec<Reflected>,
}

/// One pinned input file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::generate) struct Input {
    pub(in crate::generate) path: String,
    #[serde(default)]
    pub(in crate::generate) sha256: Option<String>,
    #[serde(default)]
    pub(in crate::generate) bytes: Option<u64>,
    #[serde(default)]
    pub(in crate::generate) upstream: Option<String>,
}

/// Inputs the generator is allowed to read.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::generate) struct Inputs {
    pub(in crate::generate) client_jar: Input,
    /// The version's published client mapping, where the source calls for one.
    #[serde(default)]
    pub(in crate::generate) client_mappings: Option<Input>,
    /// Library jar declaring serialization types absent from the client jar.
    pub(in crate::generate) data_fixer_jar: Input,
    pub(in crate::generate) version_manifest: Input,
    pub(in crate::generate) hook_inventory: Input,
    pub(in crate::generate) stub_seeds: Input,
    /// The reviewed set of classes the agent jar's adapters resolve by name.
    pub(in crate::generate) reflective_surface: Input,
    /// The generated mapping lock, pinned to the jars that produced it.
    pub(in crate::generate) mappings_lock: Input,
    /// The generated stub lock, pinned to the jars that produced it.
    pub(in crate::generate) stubs_lock: Input,
}

/// The tracked adapter inputs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::generate) struct Adapter {
    pub(in crate::generate) schema_version: u8,
    pub(in crate::generate) minecraft: String,
    pub(in crate::generate) mapping_source: MappingSource,
    /// The Java release every jar built for this version is compiled for.
    pub(in crate::generate) java_release: u16,
    /// The pack formats this version's own resource and data packs declare.
    pub(in crate::generate) pack_format: PackFormat,
    pub(in crate::generate) inputs: Inputs,
}

/// The pack formats one Minecraft version declares, and how it spells them.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::generate) struct PackFormat {
    /// What a resource pack this version accepts declares.
    pub(in crate::generate) resource: u16,
    /// The resource format's minor, which a version that states none has as zero.
    pub(in crate::generate) resource_minor: u16,
    /// What a data pack this version accepts declares.
    pub(in crate::generate) data: u16,
    /// The data format's minor, which a version that states none has as zero.
    pub(in crate::generate) data_minor: u16,
    /// The document shape this version's own packs are written in.
    pub(in crate::generate) spelling: Spelling,
}

/// How a version's own packs spell the format they declare.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(in crate::generate) enum Spelling {
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
    pub(in crate::generate) const fn as_str(self) -> &'static str {
        match self {
            Self::PackFormat => "pack-format",
            Self::MinMaxFormat => "min-max-format",
        }
    }
}
