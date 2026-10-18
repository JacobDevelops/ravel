//! Serialized version inputs and their shared accessors.

use serde::Deserialize;

/// Which verification stages a version is ready to exercise, not a support claim.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Eligibility {
    /// Generation, adapter tests, and every harness section are required.
    #[default]
    Live,
    /// Generation is available; adapter and harness obligations remain unmet.
    GenerationOnly,
}

/// Where the names the generator resolves against come from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MappingSource {
    /// The jar declares the names the reviewed inventory carries.
    UnobfuscatedClassFiles,
    /// The version's own published mapping file relates the two name spaces.
    OfficialProguardMappings,
    /// A Tiny v2 mapping with explicitly declared namespaces.
    FeatherTinyMappings,
}

impl MappingSource {
    /// How the adapter inputs and the generated locks write this down.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UnobfuscatedClassFiles => "unobfuscated-class-files",
            Self::OfficialProguardMappings => "official-proguard-mappings",
            Self::FeatherTinyMappings => "feather-tiny-mappings",
        }
    }

    /// Whether this source needs a published mapping file to be read.
    pub(crate) const fn needs_mappings(self) -> bool {
        matches!(
            self,
            Self::OfficialProguardMappings | Self::FeatherTinyMappings
        )
    }
}

/// Shape of a hook target, which the class file's flags have to agree with.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A class-level method.
    StaticMethod,
    /// An object method.
    InstanceMethod,
    /// A class-level field.
    StaticField,
    /// An object field.
    InstanceField,
}

impl Kind {
    /// The spelling used in reviewed inventory documents.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticMethod => "static-method",
            Self::InstanceMethod => "instance-method",
            Self::StaticField => "static-field",
            Self::InstanceField => "instance-field",
        }
    }

    /// Whether the target is a method rather than a field.
    pub const fn is_method(self) -> bool {
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

/// Whether a reviewed target is shared with the official dedicated server.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum HookSide {
    /// Must resolve against both client and dedicated-server class files.
    #[default]
    Shared,
    /// Only the client jar is expected to declare the target class.
    ClientOnly,
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
    /// Client-only targets are explicitly exempt from dedicated-server checks.
    #[serde(default)]
    pub(crate) side: HookSide,
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

/// A complete SHA-256 pin, canonicalized before it can enter generation.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Sha256Digest(String);

impl TryFrom<String> for Sha256Digest {
    type Error = &'static str;

    fn try_from(mut text: String) -> Result<Self, Self::Error> {
        if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("sha256 must contain exactly 64 ASCII hex digits");
        }
        text.make_ascii_lowercase();
        Ok(Self(text))
    }
}

impl Sha256Digest {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Sha256Digest {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One pinned input file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) sha256: Option<Sha256Digest>,
    #[serde(default)]
    pub(crate) bytes: Option<u64>,
    #[serde(default)]
    pub(crate) upstream: Option<String>,
}

/// An explicitly reviewed exception to modern version metadata.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub(crate) enum Capability {
    #[serde(rename = "legacy-1-12")]
    Legacy112,
}

/// The namespaces a published mapping translates between.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MappingNamespaces {
    pub(crate) runtime: String,
    pub(crate) reviewed: String,
}

/// Inputs the generator is allowed to read.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Inputs {
    pub(crate) client_jar: Input,
    /// Official dedicated-server jar used to verify shared hook targets.
    #[serde(default)]
    pub(crate) server_jar: Option<Input>,
    /// The version's published client mapping, where the source calls for one.
    #[serde(default)]
    pub(crate) client_mappings: Option<Input>,
    #[serde(default)]
    pub(crate) mapping_io_jar: Option<Input>,
    /// Library jar declaring serialization types absent from the client jar.
    #[serde(default)]
    pub(crate) data_fixer_jar: Option<Input>,
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

impl Inputs {
    /// The complete pinned jar set.
    pub(crate) fn jars(&self) -> impl Iterator<Item = &Input> + Clone {
        std::iter::once(&self.client_jar)
            .chain(self.data_fixer_jar.as_ref())
            .chain(self.server_jar.as_ref())
    }
}

/// The tracked adapter inputs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Adapter {
    pub(crate) schema_version: u8,
    pub(crate) minecraft: String,
    #[serde(default)]
    pub(crate) eligibility: Eligibility,
    pub(crate) mapping_source: MappingSource,
    #[serde(default)]
    pub(crate) mapping_namespaces: Option<MappingNamespaces>,
    #[serde(default)]
    pub(crate) capabilities: Vec<Capability>,
    /// The Java release every jar built for this version is compiled for.
    pub(crate) java_release: u16,
    /// Release required by the upstream source jar; defaults to javaRelease.
    #[serde(default)]
    pub(crate) source_java_release: Option<u16>,
    /// The pack formats this version's own resource and data packs declare.
    pub(crate) pack_format: PackFormat,
    pub(crate) inputs: Inputs,
}

impl Adapter {
    /// Whether this adapter uses the explicitly declared legacy game surface.
    pub(crate) fn is_legacy(&self) -> bool {
        self.capabilities.contains(&Capability::Legacy112)
    }

    /// Source-jar Java release, independent from the generated adapter target.
    pub(crate) fn effective_source_java_release(&self) -> u16 {
        self.source_java_release.unwrap_or(self.java_release)
    }
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
    pub(crate) const fn key(self) -> &'static str {
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

#[cfg(test)]
mod tests {
    use serde::de::DeserializeOwned;

    use super::{
        Adapter, BoundMember, Eligibility, Hook, HookSide, Input, Inputs, Inventory, Lookup,
        MappingNamespaces, NonGoal, PackFormat, Reflected, ReflectedMember, Reflection, Seed,
        SeedMember, Seeds, Spelling,
    };

    const HOOK: &str = "\
id = \"tick\"\nclass = \"example.Game\"\nmember = \"tick\"\n\
descriptor = \"()V\"\nkind = \"instance-method\"\ncapability = \"tick\"\n";
    const PACK_FORMAT: &str = "\
resource = 7\nresourceMinor = 0\ndata = 11\ndataMinor = 0\nspelling = \"pack-format\"\n";
    const INPUTS: &str = "\
clientJar = { path = \"client.jar\" }\nversionManifest = { path = \"version.json\" }\n\
hookInventory = { path = \"hooks.toml\" }\nstubSeeds = { path = \"seeds.toml\" }\n\
reflectiveSurface = { path = \"reflection.toml\" }\nmappingsLock = { path = \"mappings.lock\" }\n\
stubsLock = { path = \"stubs.lock\" }\n";
    const ADAPTER: &str = "\
schemaVersion = 1\nminecraft = \"test\"\njavaRelease = 25\n\
mappingSource = \"unobfuscated-class-files\"\n\
packFormat = { resource = 7, resourceMinor = 0, data = 11, dataMinor = 0, spelling = \"pack-format\" }\n\
inputs = { clientJar = { path = \"a\" }, versionManifest = { path = \"b\" }, \
hookInventory = { path = \"c\" }, stubSeeds = { path = \"d\" }, \
reflectiveSurface = { path = \"e\" }, mappingsLock = { path = \"f\" }, \
stubsLock = { path = \"g\" } }\n";

    fn refused<T: DeserializeOwned>(text: &str, diagnostic: &str) {
        match toml::from_str::<T>(text) {
            Err(error) => assert!(error.to_string().contains(diagnostic), "{error}"),
            Ok(_) => panic!("schema accepted a document requiring refusal: {text}"),
        }
    }

    fn required_fields<T: DeserializeOwned>(
        text: &str,
        fields: &[&str],
    ) -> Result<(), toml::de::Error> {
        toml::from_str::<T>(text)?;
        refused::<T>(
            &format!("{text}unknown = true\n"),
            "unknown field `unknown`",
        );
        for field in fields {
            let prefix = format!("{field} =");
            let without_field = text
                .lines()
                .filter(|line| !line.starts_with(&prefix))
                .collect::<Vec<_>>()
                .join("\n");
            assert_ne!(without_field, text.trim_end(), "{field}");
            refused::<T>(&without_field, &format!("missing field `{field}`"));
        }
        Ok(())
    }

    #[test]
    fn hook_fields_are_required_but_behavior_flags_default() -> Result<(), toml::de::Error> {
        required_fields::<Hook>(
            HOOK,
            &["id", "class", "member", "descriptor", "kind", "capability"],
        )?;
        let hook: Hook = toml::from_str(HOOK)?;
        assert_eq!(hook.side, HookSide::Shared);
        assert_eq!(hook.title_menu_row, None);
        let explicit: Hook = toml::from_str(&format!(
            "{HOOK}side = \"client-only\"\ntitleMenuRow = -2\n"
        ))?;
        assert_eq!(explicit.side, HookSide::ClientOnly);
        assert_eq!(explicit.title_menu_row, Some(-2));
        refused::<Hook>(&format!("{HOOK}title_menu_row = 1\n"), "unknown field");
        refused::<Hook>(&format!("{HOOK}refuses = true\n"), "unknown field");
        Ok(())
    }

    #[test]
    fn inventory_requires_hooks_but_defaults_only_non_goals() -> Result<(), toml::de::Error> {
        let text = "schemaVersion = 1\nminecraft = \"test\"\nhook = []\n";
        required_fields::<Inventory>(text, &["schemaVersion", "minecraft", "hook"])?;
        let inventory: Inventory = toml::from_str(text)?;
        assert!(inventory.hooks.is_empty());
        assert!(inventory.non_goals.is_empty());
        let explicit: Inventory = toml::from_str(&format!(
            "{text}nonGoal = [{{ id = \"absent\", reason = \"not reviewed\" }}]\n"
        ))?;
        assert_eq!(explicit.non_goals.len(), 1);
        required_fields::<NonGoal>(
            "id = \"absent\"\nreason = \"not reviewed\"\n",
            &["id", "reason"],
        )?;
        Ok(())
    }

    #[test]
    fn seeds_require_members_and_bound_members_reject_extra_fields() -> Result<(), toml::de::Error>
    {
        required_fields::<Seeds>(
            "schemaVersion = 1\nminecraft = \"test\"\nstub = []\n",
            &["schemaVersion", "minecraft", "stub"],
        )?;
        required_fields::<Seed>(
            "role = \"screen\"\nclass = \"example.Screen\"\nmembers = []\n",
            &["role", "class", "members"],
        )?;
        required_fields::<BoundMember>("role = \"open\"\nmember = \"a\"\n", &["role", "member"])?;
        let bound: BoundMember = toml::from_str("role = \"open\"\nmember = \"a\"\n")?;
        assert!(bound.parameters.is_none());
        let seed: Seed = toml::from_str(
            "role = \"screen\"\nclass = \"example.Screen\"\n\
             members = [\"tick\", { role = \"open\", member = \"a\", parameters = [\"int\"] }]\n",
        )?;
        assert!(matches!(&seed.members[0], SeedMember::Same(name) if name == "tick"));
        assert_eq!(seed.members[1].role(), "open");
        assert_eq!(seed.members[1].member(), "a");
        assert!(seed.members[1].parameters().is_some());
        refused::<Seed>(
            "role = \"screen\"\nclass = \"example.Screen\"\n\
             members = [{ role = \"open\", member = \"a\", typo = true }]\n",
            "did not match any variant",
        );
        Ok(())
    }

    #[test]
    fn reflection_requires_classes_but_class_members_default() -> Result<(), toml::de::Error> {
        required_fields::<Reflection>(
            "schemaVersion = 1\nminecraft = \"test\"\nreflected = []\n",
            &["schemaVersion", "minecraft", "reflected"],
        )?;
        let text = "name = \"GAME\"\nclass = \"example.Game\"\nruntime = \"a\"\n";
        required_fields::<Reflected>(text, &["name", "class", "runtime"])?;
        let reflected: Reflected = toml::from_str(text)?;
        assert!(reflected.members.is_empty());
        Ok(())
    }

    #[test]
    fn reflected_members_require_exactly_one_lookup() -> Result<(), toml::de::Error> {
        for fields in ["", "descriptor = \"()V\"\nparameters = 0\n"] {
            refused::<ReflectedMember>(
                &format!("name = \"tick\"\n{fields}"),
                "tick must declare exactly one of descriptor or parameters",
            );
        }
        required_fields::<ReflectedMember>("name = \"tick\"\ndescriptor = \"()V\"\n", &["name"])?;
        let descriptor: ReflectedMember =
            toml::from_str("name = \"tick\"\ndescriptor = \"()V\"\n")?;
        assert_eq!(descriptor.lookup, Lookup::Descriptor("()V".to_owned()));
        let parameters: ReflectedMember = toml::from_str("name = \"tick\"\nparameters = 0\n")?;
        assert_eq!(parameters.lookup, Lookup::Parameters(0));
        refused::<ReflectedMember>("name = \"tick\"\nparameters = -1\n", "invalid value");
        Ok(())
    }

    #[test]
    fn input_path_is_required_but_pin_metadata_is_optional() -> Result<(), toml::de::Error> {
        required_fields::<Input>("path = \"client.jar\"\n", &["path"])?;
        let input: Input = toml::from_str("path = \"client.jar\"\n")?;
        assert!(input.sha256.is_none());
        assert!(input.bytes.is_none());
        assert!(input.upstream.is_none());
        let pinned: Input = toml::from_str(&format!(
            "path = \"client.jar\"\nsha256 = \"{}\"\nbytes = 0\n\
                 upstream = \"https://example.invalid/client.jar\"\n",
            "AB".repeat(32)
        ))?;
        assert_eq!(pinned.sha256.as_deref(), Some("ab".repeat(32).as_str()));
        assert_eq!(pinned.bytes, Some(0));
        assert_eq!(
            pinned.upstream.as_deref(),
            Some("https://example.invalid/client.jar")
        );
        refused::<Input>("path = \"client.jar\"\nbytes = -1\n", "invalid value");
        Ok(())
    }

    #[test]
    fn every_input_pin_refuses_malformed_digest_text() {
        for text in ["", "0", &"g".repeat(64), &"é".repeat(32), &" ".repeat(64)] {
            refused::<Input>(
                &format!("path = \"input.toml\"\nsha256 = \"{text}\"\n"),
                "64 ASCII hex digits",
            );
        }
    }

    #[test]
    fn inputs_require_declared_surfaces_without_requiring_optional_jars()
    -> Result<(), toml::de::Error> {
        required_fields::<Inputs>(
            INPUTS,
            &[
                "clientJar",
                "versionManifest",
                "hookInventory",
                "stubSeeds",
                "reflectiveSurface",
                "mappingsLock",
                "stubsLock",
            ],
        )?;
        let inputs: Inputs = toml::from_str(INPUTS)?;
        assert!(inputs.server_jar.is_none());
        assert!(inputs.client_mappings.is_none());
        assert!(inputs.mapping_io_jar.is_none());
        assert!(inputs.data_fixer_jar.is_none());
        refused::<Inputs>(
            &INPUTS.replace(
                "clientJar = { path = \"client.jar\" }",
                "clientJar = { path = \"client.jar\", typo = true }",
            ),
            "unknown field `typo`",
        );
        Ok(())
    }

    #[test]
    fn adapter_requires_configuration_but_defaults_explicitly_optional_policy()
    -> Result<(), toml::de::Error> {
        required_fields::<Adapter>(
            ADAPTER,
            &[
                "schemaVersion",
                "minecraft",
                "javaRelease",
                "mappingSource",
                "packFormat",
                "inputs",
            ],
        )?;
        let adapter: Adapter = toml::from_str(ADAPTER)?;
        assert_eq!(adapter.eligibility, Eligibility::Live);
        assert_eq!(adapter.capabilities, []);
        assert!(adapter.mapping_namespaces.is_none());
        assert!(adapter.source_java_release.is_none());
        assert_eq!(adapter.effective_source_java_release(), 25);
        let explicit: Adapter = toml::from_str(&format!(
            "{ADAPTER}eligibility = \"generation-only\"\ncapabilities = [\"legacy-1-12\"]\n\
             sourceJavaRelease = 8\nmappingNamespaces = {{ runtime = \"a\", reviewed = \"b\" }}\n"
        ))?;
        assert_eq!(explicit.eligibility, Eligibility::GenerationOnly);
        assert!(explicit.is_legacy());
        assert_eq!(explicit.effective_source_java_release(), 8);
        assert!(explicit.mapping_namespaces.is_some());
        Ok(())
    }

    #[test]
    fn pack_minors_and_mapping_namespaces_are_required_not_inferred() -> Result<(), toml::de::Error>
    {
        required_fields::<PackFormat>(
            PACK_FORMAT,
            &["resource", "resourceMinor", "data", "dataMinor", "spelling"],
        )?;
        let pack: PackFormat = toml::from_str(PACK_FORMAT)?;
        assert_eq!(pack.resource_minor, 0);
        assert_eq!(pack.data_minor, 0);
        assert_eq!(pack.spelling, Spelling::PackFormat);
        let alternative: PackFormat =
            toml::from_str(&PACK_FORMAT.replace("pack-format", "min-max-format"))?;
        assert_eq!(alternative.spelling, Spelling::MinMaxFormat);
        required_fields::<MappingNamespaces>(
            "runtime = \"a\"\nreviewed = \"b\"\n",
            &["runtime", "reviewed"],
        )?;
        Ok(())
    }

    #[test]
    fn enum_typos_are_refused_instead_of_becoming_defaults() {
        for text in [
            HOOK.replace("instance-method", "method"),
            format!("{HOOK}side = \"client\"\n"),
        ] {
            refused::<Hook>(&text, "unknown variant");
        }
        for text in [
            ADAPTER.replace("unobfuscated-class-files", "community-mappings"),
            format!("{ADAPTER}eligibility = \"live-typo\"\n"),
            format!("{ADAPTER}capabilities = [\"legacy\"]\n"),
        ] {
            refused::<Adapter>(&text, "unknown variant");
        }
        refused::<PackFormat>(
            &PACK_FORMAT.replace("pack-format", "pack_format"),
            "unknown variant",
        );
    }
}
