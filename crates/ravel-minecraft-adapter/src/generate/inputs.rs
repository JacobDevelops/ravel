//! The tracked inputs the generator is allowed to read.
//!
//! ADG-3 pins three files per version: the reviewed hook inventory, the adapter
//! inputs with their upstream hashes, and the generated mapping lock. Nothing
//! else is read, and an input whose bytes no longer match what is tracked is
//! refused before a single class is looked at.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{GenerateError, SELECTION, jar};

/// Every pinned version's data lives here, relative to the repository root.
const VERSIONS: &str = "minecraft/versions";

/// Where the names the generator resolves against come from.
///
/// ADG-3's amendment admits two, and nothing else: an unknown value is refused
/// here rather than carried through as descriptive text, because this string
/// decides whether a name is translated before an archive is asked for it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MappingSource {
    /// The jar declares the names the reviewed inventory carries.
    UnobfuscatedClassFiles,
    /// The version's own published mapping file relates the two name spaces.
    OfficialProguardMappings,
}

impl MappingSource {
    /// How the adapter inputs and the generated locks write this down.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::UnobfuscatedClassFiles => "unobfuscated-class-files",
            Self::OfficialProguardMappings => "official-proguard-mappings",
        }
    }

    /// Whether this source needs a published mapping file to be read.
    pub(super) const fn needs_mappings(self) -> bool {
        matches!(self, Self::OfficialProguardMappings)
    }
}

/// Shape of a hook target, which the class file's flags have to agree with.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Kind {
    StaticMethod,
    InstanceMethod,
    StaticField,
    InstanceField,
}

impl Kind {
    /// Whether the target is a method rather than a field.
    pub(super) const fn is_method(self) -> bool {
        matches!(self, Self::StaticMethod | Self::InstanceMethod)
    }

    /// Whether the class file has to declare the target static.
    pub(super) const fn is_static(self) -> bool {
        matches!(self, Self::StaticMethod | Self::StaticField)
    }

    /// The `ravel-minecraft` variant the generated binding names.
    pub(super) const fn variant(self) -> &'static str {
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
pub(super) struct Hook {
    pub(super) id: String,
    pub(super) class: String,
    pub(super) member: String,
    pub(super) descriptor: String,
    pub(super) kind: Kind,
    pub(super) capability: String,
    /// Whether this target's prologue may return from it before its body runs.
    ///
    /// ADG-1's Phase C amendment bounds the power by making the opt-in generated
    /// data rather than Java: the refusable set is exactly what this inventory
    /// names, so widening it re-pins a hash and shows up in a diff. Absent means
    /// false, which is what every observation hook is.
    #[serde(default)]
    pub(super) refuses: bool,
    /// Where Ravel's own title-menu row goes, measured down from the column top.
    ///
    /// Carried by the title-menu seam alone, and only where that seam's shape
    /// cannot supply it. A version whose column builder returns the row it took
    /// says where the next row is; one that returns nothing does not, and the
    /// caller that places the game's own lower row may compute it from a
    /// constant rather than from the column, so nothing about the column says
    /// what is free. That number is version data for the same reason a linked
    /// name is: it is read off one version's `init` and is the layout of no
    /// other.
    #[serde(default, rename = "titleMenuRow")]
    pub(super) title_menu_row: Option<i16>,
}

/// One capability this version reviewed and deliberately does not carry.
///
/// Declared rather than left to the header's prose, because a caller that names
/// a hook by id has to tell a decision from a typo and prose cannot be asked.
/// The reason travels with the id for the reader who finds it missing.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NonGoal {
    pub(super) id: String,
    pub(super) reason: String,
}

/// The tracked hook inventory.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Inventory {
    pub(super) schema_version: u8,
    pub(super) minecraft: String,
    #[serde(rename = "hook")]
    pub(super) hooks: Vec<Hook>,
    /// Absent means this version carries every capability it reviewed.
    #[serde(default, rename = "nonGoal")]
    pub(super) non_goals: Vec<NonGoal>,
}

/// One member a screen template names, and the member this version spells it.
///
/// A template writes a role, so where the game renamed a member and not the job
/// it does, the version's seed list is what says so. Written as a bare string
/// where the two are the same word, which is every member under the version the
/// templates were authored against and most members under any other.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum SeedMember {
    /// The template's word is the class's own member name.
    Same(String),
    /// The class spells this member differently from the word templates use.
    Bound(BoundMember),
}

/// Which overload of one name a role means.
///
/// A count where a count separates them, which is the ordinary case, and the
/// declared types where it does not: a version can overload one name on two
/// three-parameter shapes and link the two under different names, and then a
/// count names both and neither.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum Overload {
    /// How many parameters the descriptor declares.
    Count(usize),
    /// The reviewed types the descriptor declares, in order, as Java source
    /// writes them.
    Types(Vec<String>),
}

impl Overload {
    /// Whether one descriptor's parameters are the ones this names.
    pub(super) fn matches(&self, parameters: &[String]) -> bool {
        match self {
            Self::Count(count) => parameters.len() == *count,
            Self::Types(types) => types.as_slice() == parameters,
        }
    }
}

/// A member role bound to the name one version's class declares.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BoundMember {
    pub(super) role: String,
    pub(super) member: String,
    /// Which overload of that name this role means.
    ///
    /// Absent for every member whose overloads link under one name, which is all
    /// of them under an unobfuscated version and most under any other. It is
    /// needed where they do not: an obfuscated version can link
    /// `translatable(String)` and `translatable(String, Object[])` under
    /// different names, so a reference to one is not a reference to the other
    /// and the seed list is what says which the screen means.
    ///
    /// A count rather than a descriptor wherever a count is enough, and the
    /// distinction is the rule this file already keeps: a descriptor is a
    /// transcription of the jar, which the generated stubs exist to avoid, and
    /// it would have to be rewritten in the linked name space besides. Where a
    /// count separates nothing the parameter types are written instead. Those
    /// are still the reviewed names this file already writes for a class, and
    /// they are matched against the reviewed descriptor, so neither objection
    /// applies to them.
    ///
    /// It counts the parameters the *descriptor* declares, which is usually what
    /// a reader counts at the call site and is not always: the constructor of a
    /// non-static nested class carries its enclosing instance as a first
    /// parameter the source never writes, so a count taken from the call site
    /// would be one short and select nothing. Nothing seeded today is such a
    /// constructor. It is a filter and not an assertion, so an overload set this
    /// does not separate is still refused rather than picked between, and a
    /// count that separates nothing is refused rather than ignored.
    #[serde(default)]
    pub(super) parameters: Option<Overload>,
}

impl SeedMember {
    /// The word a template names this member by.
    pub(super) fn role(&self) -> &str {
        match self {
            Self::Same(name) => name,
            Self::Bound(bound) => &bound.role,
        }
    }

    /// The name this version's class declares.
    pub(super) fn member(&self) -> &str {
        match self {
            Self::Same(name) => name,
            Self::Bound(bound) => &bound.member,
        }
    }

    /// Which overload this role means, where the binding says.
    pub(super) const fn parameters(&self) -> Option<&Overload> {
        match self {
            Self::Same(_) => None,
            Self::Bound(bound) => bound.parameters.as_ref(),
        }
    }
}

/// One class the settings screen compiles against.
///
/// Three names for the same reason [`Reflected`] carries three: `role` is the
/// word a screen template names the class by and is the same under every
/// version, and `class` is the Mojang name this version calls it. A screen
/// source is compiled, so it cannot resolve a name the way the agent jar does,
/// and a template that spelled a game class directly would be one version's
/// spelling asserted over every other.
///
/// The members are named and nothing else: every descriptor comes from the jar,
/// which is the whole reason these are generated rather than written by hand. A
/// name that the class overloads stubs every overload, because picking one would
/// mean writing down the descriptor that distinguishes them.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Seed {
    pub(super) role: String,
    pub(super) class: String,
    pub(super) members: Vec<SeedMember>,
}

/// The tracked list of classes to stub.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Seeds {
    pub(super) schema_version: u8,
    pub(super) minecraft: String,
    #[serde(rename = "stub")]
    pub(super) stubs: Vec<Seed>,
}

/// One game class an adapter under `jvm/bootstrap` resolves by name.
///
/// Three names rather than one, because three audiences read them. `name` is the
/// role the adapter reaches the class for and is the same word under every
/// version; `class` is the Mojang name a reviewer reads; `runtime` is what the
/// JVM links, which is the Mojang name under an unobfuscated source and an
/// obfuscated one otherwise.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reflected {
    pub(super) name: String,
    pub(super) class: String,
    pub(super) runtime: String,
    /// Every member of this class the agent jar reaches by name.
    ///
    /// Empty where the jar names the class and nothing on it, which a class
    /// reached only as a parameter type is.
    #[serde(default, rename = "member")]
    pub(super) members: Vec<ReflectedMember>,
}

/// One member of a reflected class the agent jar resolves by name.
///
/// Declared the way the jar asks for it, which is one of two ways and never
/// both. `descriptor` is what a lookup naming its parameter types asks for, and
/// it is written in the reviewed name space like every other descriptor in this
/// crate's inputs. `parameters` is what a lookup by name and argument count
/// asks for, which is how the persistence adapter reaches a member whose
/// parameter types it cannot name without carrying every one of them as a game
/// name too.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReflectedMember {
    pub(super) name: String,
    #[serde(default)]
    pub(super) descriptor: Option<String>,
    #[serde(default)]
    pub(super) parameters: Option<usize>,
}

/// How the agent jar asks a class for one of its members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lookup<'a> {
    /// By the parameter types it names, which fix exactly one descriptor.
    Descriptor(&'a str),
    /// By name and argument count, which is what the persistence adapter does.
    Parameters(usize),
}

impl ReflectedMember {
    /// Which of the two ways this member is declared.
    ///
    /// A declaration carrying both would hold two answers that can disagree,
    /// and one carrying neither says only that some member of that name exists,
    /// which every overload of it satisfies. Both are refused here rather than
    /// in each caller, so the rule is written once.
    ///
    /// # Errors
    ///
    /// Returns the detail of a declaration that names both ways or neither.
    pub(super) fn lookup(&self) -> Result<Lookup<'_>, String> {
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
///
/// It is a generator input rather than only a declaration the scaffold sweeps,
/// because the runtime names in it are rendered into the agent jar's own
/// `GameNames` class. A shared source tree cannot spell one version's names and
/// be right about another, so the sources spell the role and this file decides
/// what the role resolves to.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Reflection {
    pub(super) schema_version: u8,
    pub(super) minecraft: String,
    #[serde(rename = "reflected")]
    pub(super) classes: Vec<Reflected>,
}

/// One pinned input file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    pub(super) path: String,
    #[serde(default)]
    pub(super) sha256: Option<String>,
    #[serde(default)]
    pub(super) bytes: Option<u64>,
    #[serde(default)]
    pub(super) upstream: Option<String>,
}

/// Inputs the generator is allowed to read.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Inputs {
    pub(super) client_jar: Input,
    /// The version's published client mapping, where the source calls for one.
    ///
    /// It is declared exactly as the jar is and never tracked, for the reasons
    /// ADG-3's amendment records: it is upstream material this repository does
    /// not own, it is derived from the build that produced the jar, and it is
    /// larger than every tracked file of a version put together. What is tracked
    /// is its hash.
    #[serde(default)]
    pub(super) client_mappings: Option<Input>,
    /// The library jar carrying the serialization types the game's signatures
    /// name. Those classes are not in the client jar, so a stub for one has to be
    /// read from the library that declares it rather than guessed at.
    pub(super) data_fixer_jar: Input,
    pub(super) version_manifest: Input,
    pub(super) hook_inventory: Input,
    pub(super) stub_seeds: Input,
    /// The reviewed set of classes the agent jar's adapters resolve by name.
    pub(super) reflective_surface: Input,
    /// The generated mapping lock, pinned to the jars that produced it.
    pub(super) mappings_lock: Input,
    /// The generated stub lock, pinned to the jars that produced it.
    pub(super) stubs_lock: Input,
}

/// The tracked adapter inputs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Adapter {
    pub(super) schema_version: u8,
    pub(super) minecraft: String,
    pub(super) mapping_source: MappingSource,
    /// The Java release every jar built for this version is compiled for.
    ///
    /// A game refuses a class file newer than the runtime it declares, and the
    /// first real launch against an older version had to be started with the
    /// runtime compatibility check switched off, because Ravel's jars were built
    /// by whatever JDK the shell happened to hold. So it is version data like
    /// every other build fact here: declared for the machines that hold no
    /// version manifest, and re-derived from the manifest's
    /// `javaVersion.majorVersion` wherever that file is present.
    pub(super) java_release: u16,
    /// The pack formats this version's own resource and data packs declare.
    ///
    /// Version data like `java_release`, and for the same reason: a pack whose
    /// `pack_format` the game does not recognise is a pack the game refuses to
    /// load, and the number changes with the version. The jar states it in its own
    /// `version.json`, so it is declared for the machines that hold no jar and
    /// re-derived from the jar wherever one is present.
    pub(super) pack_format: PackFormat,
    /// What the JVM harness cannot run for this version, and what closes it.
    ///
    /// Absent is the ordinary case and means the harness runs every section. A
    /// version that declares something here is saying a section is not run and
    /// naming the work package that would change that, which is the only way the
    /// gate accepts an absent section: an undeclared one is indistinguishable
    /// from a harness that quietly stopped covering a version.
    #[serde(default)]
    pub(super) harness: Harness,
    pub(super) inputs: Inputs,
}

/// The harness sections one version does not run, each with its reason.
///
/// This is version data rather than a list in the harness because the reason is
/// about the version: the harness stands in for the game under Mojang names, and
/// a version whose linked names are its Mojang names can run every section while
/// an obfuscated one cannot. Writing that in the harness would assert one
/// version's answer over all of them, which is the defect the reflective surface
/// was moved here to fix.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Harness {
    /// Each section this version does not run, against why and what closes it.
    ///
    /// A section absent from the record and absent from here is a harness that
    /// stopped covering a version, which is the failure this table exists to
    /// tell apart from a section nobody can run yet. So the value is not a flag:
    /// it is the sentence a reader needs, and the gate requires it to name the
    /// work package rather than only the obstacle.
    #[serde(default, rename = "notRun")]
    pub(super) not_run: std::collections::BTreeMap<String, String>,
}

/// The pack formats one Minecraft version declares, and how it spells them.
///
/// Two formats rather than one because a version states them separately and the
/// tracked versions disagree by more than sixty. Each carries a minor because the
/// newer of the two versions split every format in half, and the spelling is here
/// because the same split changed the shape of the document a pack writes: the
/// older version's own packs declare `pack_format`, and the newer one's declare a
/// `max_format` beside a `min_format` pair. A pack written in the wrong spelling
/// is a pack the game declines, so the shape is version data like the numbers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PackFormat {
    /// What a resource pack this version accepts declares.
    pub(super) resource: u16,
    /// The resource format's minor, which a version that states none has as zero.
    pub(super) resource_minor: u16,
    /// What a data pack this version accepts declares.
    pub(super) data: u16,
    /// The data format's minor, which a version that states none has as zero.
    pub(super) data_minor: u16,
    /// The document shape this version's own packs are written in.
    pub(super) spelling: Spelling,
}

/// How a version's own packs spell the format they declare.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Spelling {
    /// One `pack_format` integer.
    PackFormat,
    /// A `max_format` integer beside a `min_format` major and minor.
    MinMaxFormat,
}

impl Spelling {
    /// The key a pack.mcmeta written in this spelling always carries.
    const fn key(self) -> &'static str {
        match self {
            Self::PackFormat => "\"pack_format\"",
            Self::MinMaxFormat => "\"min_format\"",
        }
    }

    /// The name this spelling is declared under.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::PackFormat => "pack-format",
            Self::MinMaxFormat => "min-max-format",
        }
    }
}

/// The one version this build generates against, and everything tracked for it.
#[derive(Clone, Debug)]
pub(super) struct PinnedVersion {
    pub(super) directory: PathBuf,
    pub(super) adapter: Adapter,
    pub(super) inventory: Inventory,
    pub(super) seeds: Seeds,
    pub(super) surface: Reflection,
}

/// Reads the tracked data of the version this build generates against.
///
/// One build emits one version's artefacts, which is what
/// `docs/decisions/version-selection-is-a-build-time-choice.md` decided: one
/// hook class, one exported symbol set, one stub tree, chosen before the
/// compiler runs rather than by the running game. Several directories under
/// `minecraft/versions` may be tracked and reviewed side by side, and `selected`
/// is which of them this build reads.
///
/// A directory this build did not select is not read at all. Whether every
/// tracked version is well formed is the scaffold contract's guarantee, which
/// holds over all of them at once regardless of what any build chose, and one
/// rule written in two places is two rules that can disagree.
pub(super) fn pinned(root: &Path, selected: Option<&str>) -> Result<PinnedVersion, GenerateError> {
    let versions = root.join(VERSIONS);
    let mut names = Vec::new();
    let entries = fs::read_dir(&versions)
        .map_err(|error| GenerateError::Input(format!("cannot read the version data: {error}")))?;
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();

    let name = choose(&names, selected)?;
    let directory = versions.join(name);
    let adapter: Adapter = parse(&directory.join("adapter.toml"))?;
    if adapter.minecraft != name {
        return Err(GenerateError::Drift(format!(
            "the version directory is named {name} but its adapter inputs pin {}",
            adapter.minecraft
        )));
    }
    // A harness section this version says it does not run has to say why, here,
    // where the claim is. Whether the reason is a good one and whether the
    // section really was skipped are the gate's to judge — it reads the run's own
    // record and this file cannot — but an empty reason is not a judgement call,
    // and refusing it where the file is parsed keeps a blank line from reaching
    // the reader who needs it most.
    for (section, reason) in &adapter.harness.not_run {
        if reason.trim().is_empty() {
            return Err(GenerateError::Drift(format!(
                "{name} declares the {section} harness section is not run and gives no reason, \
                 so nothing says what would change it"
            )));
        }
    }
    // The inventory is verified here rather than beside the jar, because it is
    // tracked and therefore present on every machine: a build that falls back to
    // the mapping lock still has to know the inventory is the reviewed one. It is
    // verified before it is parsed, so nothing reads meaning out of bytes that
    // have not been checked.
    verify_input(root, &adapter.inputs.hook_inventory)?;
    let inventory: Inventory = parse(&root.join(&adapter.inputs.hook_inventory.path))?;
    if inventory.minecraft != adapter.minecraft {
        return Err(GenerateError::Drift(format!(
            "the inventory pins {} but the adapter inputs pin {}",
            inventory.minecraft, adapter.minecraft
        )));
    }
    if inventory.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the inventory is schema {} but the adapter inputs are schema {}",
            inventory.schema_version, adapter.schema_version
        )));
    }
    check_non_goals(&inventory)?;
    check_title_menu_row(&inventory)?;

    // The seed list is tracked and therefore present everywhere too, for the
    // same reason: a build that generates stubs from the stub lock still has to
    // know which classes the lock was asked to cover.
    verify_input(root, &adapter.inputs.stub_seeds)?;
    let seeds: Seeds = parse(&root.join(&adapter.inputs.stub_seeds.path))?;
    if seeds.minecraft != adapter.minecraft || seeds.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the stub seeds pin {} at schema {} but the adapter inputs pin {} at schema {}",
            seeds.minecraft, seeds.schema_version, adapter.minecraft, adapter.schema_version
        )));
    }
    check_roles(&seeds)?;

    let surface = reflection(root, &adapter)?;

    // The source decides whether a mapping file exists, so a pair that
    // disagrees is refused rather than resolved one way: a mapping file nothing
    // reads is a pin nobody maintains, and a source with none to read cannot
    // translate the name it is about to ask an archive for.
    match (
        adapter.mapping_source.needs_mappings(),
        adapter.inputs.client_mappings.is_some(),
    ) {
        (true, false) => {
            return Err(GenerateError::Input(format!(
                "{} reads a published mapping but the adapter inputs declare none",
                adapter.mapping_source.as_str()
            )));
        }
        (false, true) => {
            return Err(GenerateError::Input(format!(
                "{} reads names from the class files but the adapter inputs declare a mapping",
                adapter.mapping_source.as_str()
            )));
        }
        _ => {}
    }

    for input in [
        &adapter.inputs.client_jar,
        &adapter.inputs.data_fixer_jar,
        &adapter.inputs.version_manifest,
        &adapter.inputs.hook_inventory,
        &adapter.inputs.stub_seeds,
        &adapter.inputs.reflective_surface,
    ]
    .into_iter()
    .chain(adapter.inputs.client_mappings.as_ref())
    {
        // An input that records where it came from has to record a fetchable
        // origin, because that string is the only trail back to the bytes the
        // hash pins.
        if let Some(upstream) = &input.upstream
            && !upstream.starts_with("https://")
        {
            return Err(GenerateError::Input(format!(
                "{} names an upstream that is not an https address",
                file_name(&input.path)
            )));
        }
    }

    verify_java_release(root, &adapter)?;

    Ok(PinnedVersion {
        directory,
        adapter,
        inventory,
        seeds,
        surface,
    })
}

/// Refuses a declared Java release the version's own manifest disagrees with.
///
/// The manifest is upstream material this repository does not track, so most
/// machines cannot read it and the declaration is what they build from. Where it
/// is present the declaration is re-derived rather than trusted, which is the
/// same split `runtime` already makes in the reflective surface: a record for the
/// machines that cannot check, and a check wherever the material is there.
///
/// # Errors
///
/// Returns an error when the manifest is present and unreadable, states no
/// `javaVersion.majorVersion`, or states one the adapter inputs do not declare.
fn verify_java_release(root: &Path, adapter: &Adapter) -> Result<(), GenerateError> {
    let path = root.join(&adapter.inputs.version_manifest.path);
    if !path.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| {
        GenerateError::Input(format!("{} is unreadable: {error}", path.display()))
    })?;
    let declared = major_version(&text).ok_or_else(|| {
        GenerateError::Input(format!(
            "{} states no javaVersion.majorVersion, so nothing says what release this version runs",
            file_name(&adapter.inputs.version_manifest.path)
        ))
    })?;
    if declared != adapter.java_release {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare javaRelease {} but {} states {declared}",
            adapter.java_release,
            file_name(&adapter.inputs.version_manifest.path)
        )));
    }
    Ok(())
}

/// Refuses a declared pack format the version's own jar disagrees with.
///
/// The jar states both numbers in a `version.json` at its root, so wherever a
/// build holds the jar the declaration is re-derived rather than trusted — the
/// same split [`verify_java_release`] makes over the launcher's manifest. A jar
/// that states none is refused rather than skipped: the jar is here, so the
/// answer is here, and a pack written under a number nothing checked is a pack
/// the game silently declines to load.
///
/// # Errors
///
/// Returns an error when the jar holds no `version.json`, when that document
/// states no pack versions, or when it states numbers the adapter inputs do not
/// declare.
pub(super) fn verify_pack_format(
    adapter: &Adapter,
    archive: &jar::Archive,
) -> Result<(), GenerateError> {
    let text = jar_text(archive, JAR_VERSION)?;
    let declared = adapter.pack_format;
    let stated = pack_versions(&text).ok_or_else(|| {
        GenerateError::Input(format!(
            "{JAR_VERSION} in the pinned jar states no pack_version, so nothing says which pack \
             formats this version reads"
        ))
    })?;
    let expected = (
        declared.resource,
        declared.resource_minor,
        declared.data,
        declared.data_minor,
    );
    if stated != expected {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat {expected:?} but {JAR_VERSION} in the pinned \
             jar states {stated:?}, as resource major, resource minor, data major, data minor"
        )));
    }
    verify_spelling(declared.spelling, archive)
}

/// Refuses a declared spelling this version's own packs do not use.
///
/// The correlation between the split numbers and the newer document shape is
/// real and it is still an inference, so it is checked rather than assumed: the
/// jar ships packs of its own, every one of them written by the game's authors in
/// the shape the game's own reader accepts, and that is the evidence.
///
/// # Errors
///
/// Returns an error when the jar ships no pack of its own, and when the packs it
/// ships are written in the other spelling.
fn verify_spelling(spelling: Spelling, archive: &jar::Archive) -> Result<(), GenerateError> {
    let Some(name) = archive.names().find(|name| name.ends_with(PACK_METADATA)) else {
        return Err(GenerateError::Input(format!(
            "the pinned jar ships no {PACK_METADATA} of its own, so nothing says which shape a \
             pack this version reads is written in"
        )));
    };
    let name = name.to_owned();
    let text = jar_text(archive, &name)?;
    if !text.contains(spelling.key()) {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat spelling {} but {name} in the pinned jar \
             carries no {}",
            spelling.as_str(),
            spelling.key()
        )));
    }
    Ok(())
}

/// One named entry of the pinned jar, as text.
fn jar_text(archive: &jar::Archive, name: &str) -> Result<String, GenerateError> {
    let bytes = archive.entry(name)?;
    String::from_utf8(bytes).map_err(|error| {
        GenerateError::Input(format!("{name} in the pinned jar is not UTF-8: {error}"))
    })
}

/// The document every pack, Ravel's included, declares its format in.
const PACK_METADATA: &str = "pack.mcmeta";

/// The document inside a client jar that states what the build is.
const JAR_VERSION: &str = "version.json";

/// The resource and data pack formats one jar's `version.json` states.
///
/// Two spellings, because the tracked versions use two: an older one states
/// `resource` and `data`, and a newer one splits each into a major and a minor.
/// The major is the format a pack declares in both, so the split spelling is read
/// for its major and its minor is not a number this generator has a use for.
///
/// The scan is bounded to the `pack_version` object rather than run over the
/// whole document, because `data` is not a name only that object uses and a
/// manifest carrying one elsewhere would hand this the wrong number under the
/// right name.
fn pack_versions(text: &str) -> Option<(u16, u16, u16, u16)> {
    let named = text.find("\"pack_version\"")?;
    let open = text[named..].find('{')? + named;
    let close = text[open..].find('}')? + open;
    let scope = &text[open..close];
    if let Some(resource) = number_after(scope, "resource") {
        return Some((resource, 0, number_after(scope, "data")?, 0));
    }
    Some((
        number_after(scope, "resource_major")?,
        number_after(scope, "resource_minor")?,
        number_after(scope, "data_major")?,
        number_after(scope, "data_minor")?,
    ))
}

/// The number one JSON key is set to, where the key is spelled exactly.
///
/// The quotes are part of the search, so `resource` does not match
/// `resource_major` and the two spellings stay distinguishable.
fn number_after(text: &str, key: &str) -> Option<u16> {
    let quoted = format!("\"{key}\"");
    let at = text.find(&quoted)? + quoted.len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The `javaVersion.majorVersion` one version manifest states.
///
/// Read by scanning rather than by deserializing the manifest, because the
/// manifest is a large upstream document whose every other field is none of this
/// generator's business, and a struct for it would be a second thing to maintain
/// against Mojang.
///
/// The scan starts at `javaVersion` rather than at the number, because
/// `majorVersion` is not a name only that object uses: a manifest carrying one
/// anywhere earlier — a library rule, a future sibling of `javaVersion` — would
/// hand this the wrong number under the right name, and the drift check that
/// reads it would then compare the adapter's release against something else
/// entirely.
fn major_version(text: &str) -> Option<u16> {
    let object = text.find("\"javaVersion\"")?;
    let at = text[object..].find("\"majorVersion\"")? + object + "\"majorVersion\"".len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The seam whose shape decides whether a title-menu row has to be declared.
pub(super) const TITLE_MENU_SEAM: &str = "screen.title-menu-options";

/// Checks that the title-menu row is declared exactly where it is needed.
///
/// The row is derivable from a column builder that returns the row it took, and
/// is derivable from nothing at all when it returns void. So the descriptor
/// decides whether the number has to be there, and a number carried by any other
/// hook is a number nothing reads.
///
/// # Errors
///
/// Returns an error when a hook other than the title-menu seam carries a row,
/// when a void-returning title-menu seam carries none, or when one that hands
/// the row back carries one anyway.
fn check_title_menu_row(inventory: &Inventory) -> Result<(), GenerateError> {
    for hook in &inventory.hooks {
        if hook.title_menu_row.is_some() && hook.id != TITLE_MENU_SEAM {
            return Err(GenerateError::Drift(format!(
                "{} declares a title-menu row and nothing reads one for it; only {TITLE_MENU_SEAM} \
                 places a row",
                hook.id
            )));
        }
    }
    let Some(seam) = inventory
        .hooks
        .iter()
        .find(|hook| hook.id == TITLE_MENU_SEAM)
    else {
        return Ok(());
    };
    let returns_void = seam.descriptor.ends_with(")V");
    match (returns_void, seam.title_menu_row) {
        (true, None) => Err(GenerateError::Drift(format!(
            "{TITLE_MENU_SEAM} returns void here, so it hands back no row and nothing else says \
             where one is free; declare titleMenuRow, read off this version's title screen"
        ))),
        (false, Some(row)) => Err(GenerateError::Drift(format!(
            "{TITLE_MENU_SEAM} hands back the row it took, so the declared titleMenuRow of {row} \
             is a second answer to a question this version already answers"
        ))),
        _ => Ok(()),
    }
}

/// Checks that a version's declared non-goals say something the hooks do not.
///
/// One id is either carried or not carried, so an id on both lists is two
/// answers to one question. A repeated non-goal is the same defect a repeated
/// hook is, and an empty reason is a declaration that explains nothing to the
/// reader who went looking for the capability.
///
/// # Errors
///
/// Returns an error when a non-goal is also a hook, when one is declared twice,
/// or when one carries no reason.
fn check_non_goals(inventory: &Inventory) -> Result<(), GenerateError> {
    let mut seen = BTreeSet::new();
    for non_goal in &inventory.non_goals {
        if inventory.hooks.iter().any(|hook| hook.id == non_goal.id) {
            return Err(GenerateError::Drift(format!(
                "the inventory declares {} as a hook and as a non-goal, and one id is either \
                 carried or not",
                non_goal.id
            )));
        }
        if non_goal.reason.trim().is_empty() {
            return Err(GenerateError::Drift(format!(
                "the inventory declares {} a non-goal with no reason, which tells a reader who \
                 went looking for it nothing",
                non_goal.id
            )));
        }
        if !seen.insert(non_goal.id.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the inventory declares the non-goal {} twice",
                non_goal.id
            )));
        }
    }
    Ok(())
}

/// Checks that every role a screen template could name is declared exactly once.
///
/// A role is what a template writes, so two seeds claiming one role is two
/// answers to one placeholder and neither is the obvious one. The same holds
/// inside a class for a member role. Roles are checked here rather than where a
/// template is read because the seed list is what declares them, and a version
/// whose data is ambiguous is ambiguous whether or not a template happens to
/// name the ambiguous one.
///
/// # Errors
///
/// Returns an error when a role is not spelled like a name a template can write,
/// when two seeds declare one class role, or when one seed declares one member
/// role twice.
fn check_roles(seeds: &Seeds) -> Result<(), GenerateError> {
    let mut seen = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for seed in &seeds.stubs {
        if !role_reference(&seed.role) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare {} as a class role, which is not a name a template can \
                 write",
                seed.role
            )));
        }
        if !class_name(&seed.class) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare {} as a class, which is not one",
                seed.class
            )));
        }
        if !seen.insert(seed.role.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare the class role {} twice, and one role names one class",
                seed.role
            )));
        }
        // Two seeds naming one class is refused on the jar path by `from_jar`,
        // and was refused nowhere else: `from_lock` looks a seed up by class and
        // is content to find the same stub twice. So the refusal existed only on
        // a machine holding the restricted jar, which is the opposite of where a
        // seed-list check belongs. It lives here now, where neither the jar nor
        // the lock decides whether it runs.
        if !classes.insert(seed.class.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds bind two roles to {}, and one class is stubbed once",
                seed.class
            )));
        }
        let mut members = BTreeSet::new();
        let mut bound = BTreeSet::new();
        for member in &seed.members {
            if !member_reference(member.role()) || !member_reference(member.member()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds bind {}'s member role {} to {}, and one of the two is not a \
                     name a template can write",
                    seed.role,
                    member.role(),
                    member.member()
                )));
            }
            if !members.insert(member.role()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds declare {}'s member role {} twice, and one role names one \
                     member",
                    seed.role,
                    member.role()
                )));
            }
            // Two roles bound to one member stub that member's overloads twice,
            // and `from_jar` pushes every match once per binding with no dedup,
            // so the generated stub declares the same method twice and does not
            // compile. Refused here rather than discovered by javac.
            if !bound.insert(member.member()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds bind two of {}'s member roles to {}, which stubs it twice",
                    seed.role,
                    member.member()
                )));
            }
        }
    }
    Ok(())
}

/// Whether a value is spelled like a role a screen template can write.
///
/// Narrower than the JVM's rule and wider than a Java identifier, because a
/// nested type's role carries the `$` its binary name does. What has to fail is
/// anything holding the punctuation a placeholder body or a Java declaration
/// would end on, which is what keeps a role from becoming a second declaration.
fn role_reference(value: &str) -> bool {
    !value.is_empty()
        && value.starts_with(|first: char| first.is_ascii_alphabetic() || first == '_')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
}

/// Whether a value is spelled like a member a seed list can bind.
///
/// Wider than [`role_reference`] by exactly the two names the JVM spells with
/// angle brackets and Java cannot: a constructor is a member the screen calls
/// through and the seed list has to be able to name it.
fn member_reference(value: &str) -> bool {
    matches!(value, "<init>" | "<clinit>") || role_reference(value)
}

/// Reads the reviewed set of classes the agent jar's adapters resolve by name.
///
/// Tracked and read on every build for the same reason the inventory and the
/// seeds are: it decides the names compiled into the agent jar, so a build that
/// never opens a jar still has to know it is the reviewed set.
///
/// # Errors
///
/// Returns an error when the file is missing, has drifted from its pinned hash,
/// pins another version or schema, or declares one role twice.
fn reflection(root: &Path, adapter: &Adapter) -> Result<Reflection, GenerateError> {
    verify_input(root, &adapter.inputs.reflective_surface)?;
    let surface: Reflection = parse(&root.join(&adapter.inputs.reflective_surface.path))?;
    if surface.minecraft != adapter.minecraft || surface.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the reflective surface pins {} at schema {} but the adapter inputs pin {} at schema \
             {}",
            surface.minecraft, surface.schema_version, adapter.minecraft, adapter.schema_version
        )));
    }
    verify_surface(&surface)?;
    Ok(surface)
}

/// Refuses a surface that cannot be rendered or cannot be checked.
///
/// Separate from reading the file so the rules can be asked a question without
/// one: every refusal here is about what the file says and none of it about
/// where the file came from.
///
/// # Errors
///
/// Returns an error when a name is not one, when a role is declared twice, or
/// when a member says which member it is in no way or in two ways at once.
fn verify_surface(surface: &Reflection) -> Result<(), GenerateError> {
    // Every one of the three is interpolated into Java: the role becomes a field
    // name, the runtime name becomes that field's string literal, and the
    // reviewed name becomes the comment beside it. A file is reviewed and pinned,
    // so this is not the door anything walks in through; it is the reason a
    // typo cannot become a second declaration in a class ADG-1 gives one.
    for reflected in &surface.classes {
        for (what, value, legal) in [
            ("role", &reflected.name, role_name as fn(&str) -> bool),
            ("runtime name", &reflected.runtime, class_name),
            ("reviewed name", &reflected.class, class_name),
        ] {
            if !legal(value) {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {value} as a {what}, which is not one"
                )));
            }
        }
    }
    // One role names one class, so a file that declares a role twice has said
    // two things about one generated constant and only one can be rendered.
    let mut seen = BTreeSet::new();
    for reflected in &surface.classes {
        if !seen.insert(reflected.name.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the reflective surface declares the role {} twice, and one role names one class",
                reflected.name
            )));
        }
    }
    for reflected in &surface.classes {
        let mut declared = BTreeSet::new();
        for member in &reflected.members {
            if !member_name(&member.name) {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {} of {} as a member, which is not a member \
                     name",
                    member.name, reflected.class
                )));
            }
            let shape = match member.lookup().map_err(|detail| {
                GenerateError::Drift(format!(
                    "the reflective surface declares a member of {}: {detail}",
                    reflected.class
                ))
            })? {
                Lookup::Descriptor(descriptor) => descriptor.to_owned(),
                Lookup::Parameters(count) => format!("/{count}"),
            };
            if !declared.insert(format!("{}{shape}", member.name)) {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {}{shape} of {} twice",
                    member.name, reflected.class
                )));
            }
        }
    }
    Ok(())
}

/// Whether a value is a member name and nothing else.
///
/// Narrow for the reason [`class_name`] is, with the two names the JVM spells
/// in angle brackets admitted because a constructor is a member the agent jar
/// reaches like any other.
fn member_name(value: &str) -> bool {
    matches!(value, "<init>" | "<clinit>")
        || (!value.is_empty()
            && !value.starts_with(|first: char| first.is_ascii_digit())
            && value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_'))
}

/// Whether a role is spelled the way the generated constants are.
fn role_name(value: &str) -> bool {
    value.starts_with(|first: char| first.is_ascii_uppercase())
        && value.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Whether a value is a binary class name and nothing else.
///
/// Deliberately narrower than the JVM's own rule, which admits almost any
/// character in an identifier. What has to pass is the names Mojang and a
/// proguard mapping write, and what has to fail is anything carrying the
/// punctuation that would end a Java string literal or a declaration.
fn class_name(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with(|first: char| first.is_ascii_digit())
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
                })
        })
}

/// Picks the one version directory this build reads.
///
/// One tracked directory needs no selection, because there is no other answer.
/// More than one requires the build to name the one it means: alphabetical
/// order, the most recent version, and whichever one was there first are all
/// silent answers to a question an operator should have answered. A name no
/// directory carries is refused with the names that were actually found, so the
/// refusal says what the build can act on rather than only what it wanted.
fn choose<'a>(names: &'a [String], selected: Option<&str>) -> Result<&'a str, GenerateError> {
    if let Some(wanted) = selected {
        return names
            .iter()
            .find(|name| name.as_str() == wanted)
            .map(String::as_str)
            .ok_or_else(|| {
                let what = if wanted.is_empty() {
                    "names nothing".to_owned()
                } else {
                    format!("names {wanted}")
                };
                GenerateError::Input(format!(
                    "{SELECTION} {what}, but the version data holds {}",
                    listed(names)
                ))
            });
    }
    match names {
        [] => Err(GenerateError::Input(
            "the version data holds no version directory to generate against".to_owned(),
        )),
        [only] => Ok(only),
        _ => Err(GenerateError::Input(format!(
            "the adapter emits one version per build and the version data holds {}, so \
             {SELECTION} has to name the one this build means",
            listed(names)
        ))),
    }
}

/// Names the version directories a refusal found, for a reader of that refusal.
fn listed(names: &[String]) -> String {
    if names.is_empty() {
        "no version directory".to_owned()
    } else {
        names.join(", ")
    }
}

/// Reads one tracked TOML document.
pub(super) fn parse<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, GenerateError> {
    let displayed = path.to_string_lossy();
    let name = file_name(&displayed);
    let text = fs::read_to_string(path)
        .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
    toml::from_str(&text)
        .map_err(|error| GenerateError::Input(format!("cannot parse {name}: {error}")))
}

/// Refuses an input whose bytes no longer match the tracked hash.
///
/// An input that pins neither a hash nor a length is refused too. ADG-3 says
/// generation runs only from inputs whose hashes match what is tracked, and an
/// input with nothing to check against does not meet that by having nothing to
/// fail: it just makes the check vacuous, silently, for the one file nobody
/// remembered to pin.
pub(super) fn verify_input(root: &Path, input: &Input) -> Result<(), GenerateError> {
    // Messages name inputs by file name, never by path: these strings end up in
    // recorded evidence, and a path would say whose machine produced it.
    let name = file_name(&input.path);
    if input.sha256.is_none() && input.bytes.is_none() {
        return Err(GenerateError::Input(format!(
            "{name} pins neither a hash nor a length, so nothing about it is verified"
        )));
    }
    let bytes = fs::read(root.join(&input.path))
        .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
    if let Some(expected) = &input.sha256 {
        let actual = sha256(&bytes);
        if &actual != expected {
            return Err(GenerateError::Drift(format!(
                "{name} hashes {actual} but the adapter inputs pin {expected}"
            )));
        }
    }
    if let Some(expected) = input.bytes
        && bytes.len() as u64 != expected
    {
        return Err(GenerateError::Drift(format!(
            "{name} is {} bytes but the adapter inputs pin {expected}",
            bytes.len()
        )));
    }
    Ok(())
}

/// Rewrites what the adapter inputs pin for the two generated locks.
///
/// Only the generated locks are ever re-pinned, and only by the deliberate
/// regeneration path that has just written them from the jar. An upstream pin is
/// a claim about bytes nobody here produced, so nothing rewrites one.
pub(super) fn repin_locks(
    root: &Path,
    directory: &Path,
    inputs: &Inputs,
) -> Result<(), GenerateError> {
    let path = directory.join("adapter.toml");
    let mut document = fs::read_to_string(&path)
        .map_err(|error| GenerateError::Input(format!("cannot read adapter.toml: {error}")))?;
    for (table, input) in [
        ("[inputs.mappingsLock]", &inputs.mappings_lock),
        ("[inputs.stubsLock]", &inputs.stubs_lock),
    ] {
        let name = file_name(&input.path);
        let bytes = fs::read(root.join(&input.path))
            .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
        document = replace_pin(&document, table, &sha256(&bytes), bytes.len() as u64)?;
    }
    fs::write(&path, document)
        .map_err(|error| GenerateError::Input(format!("cannot write adapter.toml: {error}")))
}

/// Replaces the hash and length one table of the adapter inputs records.
///
/// The document is edited line by line rather than deserialized and written
/// back, because every sentence of prose in `adapter.toml` explains a pin and a
/// round trip through the parser would drop all of it.
pub(super) fn replace_pin(
    document: &str,
    table: &str,
    sha256: &str,
    bytes: u64,
) -> Result<String, GenerateError> {
    let mut rewritten = String::with_capacity(document.len());
    let mut inside = false;
    let mut pinned_hash = false;
    let mut pinned_length = false;
    for line in document.lines() {
        if line.starts_with('[') {
            inside = line.trim() == table;
        }
        if inside && line.starts_with("sha256 = ") {
            rewritten.push_str("sha256 = \"");
            rewritten.push_str(sha256);
            rewritten.push('"');
            pinned_hash = true;
        } else if inside && line.starts_with("bytes = ") {
            rewritten.push_str("bytes = ");
            rewritten.push_str(&bytes.to_string());
            pinned_length = true;
        } else {
            rewritten.push_str(line);
        }
        rewritten.push('\n');
    }
    if !pinned_hash || !pinned_length {
        return Err(GenerateError::Input(format!(
            "the adapter inputs have no {table} hash and length to rewrite"
        )));
    }
    Ok(rewritten)
}

/// The last path segment, which is all a recorded message may say about a file.
pub(super) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Hex-encoded SHA-256 of some bytes.
pub(super) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        text.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    text
}

#[cfg(test)]
/// Every version directory this repository tracks, in name order.
///
/// Read rather than spelled. A version literal inside a crate is one
/// version's data asserted over every other and `ravel-verify-scaffold`
/// refuses it, and a test that named a version would be a test of whichever
/// version its author happened to have. Sweeping is also the stronger
/// assertion: what these tests check is true of a tracked version because it
/// is tracked, not because it is the one the build selects.
pub(super) fn tracked_versions(root: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(root.join(VERSIONS))
        .expect("the version data should be readable")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert!(!names.is_empty(), "the repository tracks a version");
    names
}

/// The first tracked version whose adapter inputs have the shape a test needs.
///
/// Tests name a property rather than a version for the same reason
/// [`tracked_versions`] exists: a test that asserts a runtime name equals its
/// reviewed one is asserting something true of an unobfuscated version and false
/// of an obfuscated one, and which of those it gets should not depend on how the
/// directories happen to sort.
///
/// # Panics
///
/// Panics when no tracked version has that shape, because a test that needs one
/// and finds none is asserting nothing.
#[cfg(test)]
pub(super) fn tracked_version_where(
    root: &Path,
    wanted: impl Fn(&Adapter) -> bool,
) -> PinnedVersion {
    for version in tracked_versions(root) {
        let pinned = pinned(root, Some(&version)).expect("the tracked version data should parse");
        if wanted(&pinned.adapter) {
            return pinned;
        }
    }
    panic!("no tracked version has the shape this test needs");
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        Adapter, Input, Kind, MappingSource, Reflection, file_name, pinned, sha256, verify_input,
        verify_surface,
    };

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>")
    }

    /// `majorVersion` is not a name only `javaVersion` uses, and the scan used
    /// to take the first one in the file. A manifest carrying one earlier hands
    /// the release check a number from somewhere else under the right name.
    #[test]
    fn the_java_release_is_read_from_the_java_version_object() {
        let manifest = r#"{
  "downloads": {"client": {"majorVersion": 3, "size": 1}},
  "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21}
}"#;

        assert_eq!(super::major_version(manifest), Some(21));
    }

    /// Both spellings the tracked jars use, and the trap between them.
    ///
    /// One version states `resource` and `data`; the other splits each into a
    /// major and a minor. A scan for `resource` that ignored the closing quote
    /// would match `resource_major` and read a major as though it were the whole
    /// number, and a scan not bounded to the object would find `data` in a key
    /// that has nothing to do with packs.
    #[test]
    fn both_spellings_of_a_jars_pack_version_are_read_and_neither_reads_the_other() {
        let joined = r#"{"data": 9, "pack_version": {"resource": 34, "data": 48}}"#;
        assert_eq!(super::pack_versions(joined), Some((34, 0, 48, 0)));

        let split = r#"{"pack_version": {"resource_major": 84, "resource_minor": 0,
          "data_major": 101, "data_minor": 1}, "world_version": 4790}"#;
        assert_eq!(super::pack_versions(split), Some((84, 0, 101, 1)));

        assert_eq!(super::pack_versions(r#"{"world_version": 4790}"#), None);
        assert_eq!(
            super::pack_versions(r#"{"pack_version": {"data": 48}}"#),
            None
        );
    }

    #[test]
    fn the_tracked_version_data_reads_back_as_a_reviewed_inventory() {
        let root = repository_root();
        for version in super::tracked_versions(root) {
            let pinned =
                pinned(root, Some(&version)).expect("the tracked version data should parse");

            assert_eq!(pinned.adapter.schema_version, 1);
            // The source and the inputs have to agree about whether a published
            // mapping exists, which is the invariant that holds under both of them
            // rather than the one that happens to hold under the version the author
            // had.
            assert_eq!(
                pinned.adapter.mapping_source.needs_mappings(),
                pinned.adapter.inputs.client_mappings.is_some()
            );
            assert_eq!(pinned.inventory.minecraft, pinned.adapter.minecraft);
            assert!(
                pinned.directory.ends_with(&pinned.adapter.minecraft),
                "the directory names the version it pins"
            );
            assert!(
                pinned.inventory.hooks.len() >= 6,
                "the reviewed inventory should carry every entry ADG-3 resolved"
            );
            assert!(
                pinned
                    .inventory
                    .hooks
                    .iter()
                    .all(|hook| !hook.capability.is_empty()),
                "every entry carries the sentence its human review needs"
            );
            assert_eq!(pinned.seeds.minecraft, pinned.adapter.minecraft);

            // The member check proves what the surface declares, so a surface
            // that declared nothing would pass it while saying nothing. Every
            // adapter under `jvm/bootstrap` calls members on the classes it
            // resolves, so a version whose surface names classes and no members
            // has lost the declarations rather than needed none. The floor is a
            // member per class rather than a fixed count: the set shrinks when a
            // seam moves out of the agent jar and into the sibling, which content
            // registration did, and a fixed count would then have to be edited
            // down every time — which is the same as not having one.
            //
            // Held per class rather than over the sum. A sum satisfies the floor
            // with the members of one class and nothing on the rest, which is the
            // shape a lost declaration actually takes: the classes stay, one of
            // them loses its members, and a total that another class is carrying
            // never moves.
            for reflected in &pinned.surface.classes {
                assert!(
                    !reflected.members.is_empty(),
                    "{} is reflected and declares no member the agent jar reaches",
                    reflected.class
                );
            }

            // A seed with no members earns its place by being named by something
            // else in the set. Nothing calls a member on it, so the only work it can
            // do is being a type another stub already writes down: a supertype that
            // makes the classes under it assignable, or a descriptor that a member
            // of another stub declares. A seed nothing names stubs a class that
            // cannot be reached at all, which is a seed to delete.
            let lock = std::fs::read_to_string(pinned.directory.join("stubs.lock"))
                .expect("the tracked stub lock should be readable");
            for seed in &pinned.seeds.stubs {
                if !seed.members.is_empty() {
                    continue;
                }
                assert!(
                    named_by_another_stub(&lock, &seed.class),
                    "{} is seeded with no members and nothing else in the set names it",
                    seed.class
                );
            }
        }
    }

    /// Whether anything else in `lock` writes `class` down.
    ///
    /// A supertype edge or a member descriptor, and nothing else: both are lines
    /// the generator rendered out of the jar, so neither can name a class the
    /// jar does not agree carries that relationship.
    fn named_by_another_stub(lock: &str, class: &str) -> bool {
        let internal = class.replace('.', "/");
        lock.lines().any(|line| {
            (line.starts_with("extends")
                || line.starts_with("implements")
                || line.starts_with("descriptor"))
                && line.contains(&internal)
        })
    }

    #[test]
    fn a_memberless_seed_nothing_else_names_is_refused_however_it_is_named() {
        let lock = "\
[[stub]]
class = \"net/minecraft/Named\"
extends = \"net/minecraft/Above\"

[[stub.member]]
name = \"take\"
descriptor = \"(Lnet/minecraft/Parameter;)V\"

[[stub]]
class = \"net/minecraft/Unreferenced\"
";

        // The two ways a seed earns its place, and both still earn it.
        assert!(named_by_another_stub(lock, "net.minecraft.Above"));
        assert!(named_by_another_stub(lock, "net.minecraft.Parameter"));

        // Its own declaration is not something else naming it, and neither is a
        // class the lock never mentions.
        assert!(!named_by_another_stub(lock, "net.minecraft.Unreferenced"));
        assert!(!named_by_another_stub(lock, "net.minecraft.Absent"));
    }

    #[test]
    fn a_version_tree_with_no_pinned_version_is_refused_rather_than_guessed() {
        let error =
            pinned(Path::new("/nonexistent-ravel-root"), None).expect_err("this must not parse");

        assert!(error.to_string().contains("cannot read the version data"));
    }

    #[test]
    fn every_kind_agrees_with_the_flags_a_class_file_declares() {
        for (kind, method, is_static, variant) in [
            (Kind::StaticMethod, true, true, "StaticMethod"),
            (Kind::InstanceMethod, true, false, "InstanceMethod"),
            (Kind::StaticField, false, true, "StaticField"),
            (Kind::InstanceField, false, false, "InstanceField"),
        ] {
            assert_eq!(kind.is_method(), method);
            assert_eq!(kind.is_static(), is_static);
            assert_eq!(kind.variant(), variant);
        }
    }

    #[test]
    fn an_input_that_no_longer_hashes_to_its_pin_is_refused() {
        let root = repository_root();
        let adapter: Adapter = super::parse(
            &root
                .join("minecraft/versions")
                .join(&super::tracked_versions(root)[0])
                .join("adapter.toml"),
        )
        .expect("the tracked adapter inputs should parse");

        let drifted = Input {
            path: adapter.inputs.hook_inventory.path.clone(),
            sha256: Some("0".repeat(64)),
            bytes: None,
            upstream: None,
        };

        let error = verify_input(root, &drifted).expect_err("a drifted hash must be refused");
        let message = error.to_string();

        assert!(message.contains("hooks.toml"), "{message}");
        assert!(!message.contains('/'), "a message must not name a path");
    }

    #[test]
    fn an_input_of_the_wrong_length_is_refused_even_when_no_hash_is_pinned() {
        let root = repository_root();
        let wrong_length = Input {
            path: "minecraft/README.md".to_owned(),
            sha256: None,
            bytes: Some(1),
            upstream: None,
        };
        let missing = Input {
            path: "minecraft/absent.toml".to_owned(),
            sha256: None,
            bytes: Some(1),
            upstream: None,
        };

        assert!(
            verify_input(root, &wrong_length)
                .expect_err("a wrong length must be refused")
                .to_string()
                .contains("bytes but the adapter inputs pin")
        );
        assert!(
            verify_input(root, &missing)
                .expect_err("a missing input must be refused")
                .to_string()
                .contains("cannot read absent.toml")
        );
    }

    #[test]
    fn an_input_with_nothing_to_verify_against_is_refused_rather_than_passed() {
        let unpinned = Input {
            path: "minecraft/README.md".to_owned(),
            sha256: None,
            bytes: None,
            upstream: None,
        };

        let error = verify_input(repository_root(), &unpinned)
            .expect_err("an unverifiable input must be refused");

        assert!(
            error
                .to_string()
                .contains("pins neither a hash nor a length"),
            "{error}"
        );
    }

    #[test]
    fn the_tracked_adapter_inputs_pin_every_input_they_declare() {
        let root = repository_root();
        for version in super::tracked_versions(root) {
            let pinned =
                pinned(root, Some(&version)).expect("the tracked version data should parse");
            let inputs = &pinned.adapter.inputs;

            for input in [
                &inputs.client_jar,
                &inputs.data_fixer_jar,
                &inputs.version_manifest,
                &inputs.hook_inventory,
                &inputs.stub_seeds,
                &inputs.mappings_lock,
                &inputs.stubs_lock,
            ]
            .into_iter()
            .chain(inputs.client_mappings.as_ref())
            {
                assert!(
                    input.sha256.is_some() || input.bytes.is_some(),
                    "{} is declared with nothing to verify it against",
                    file_name(&input.path)
                );
            }
            assert!(
                inputs.hook_inventory.sha256.is_some(),
                "the reviewed inventory decides what gets rewritten and must pin a hash"
            );
            assert!(
                inputs.stub_seeds.sha256.is_some(),
                "the seed list decides which game types the screen compiles against"
            );
            for lock in [&inputs.mappings_lock, &inputs.stubs_lock] {
                assert!(
                    lock.sha256.is_some(),
                    "{} is the only thing a jar-less machine can check the lock against",
                    file_name(&lock.path)
                );
            }
        }
    }

    #[test]
    fn repinning_rewrites_one_tables_pin_and_leaves_the_prose_alone() {
        let document = "\
# why the seeds are pinned
[inputs.stubSeeds]
path = \"stubs.toml\"
sha256 = \"aa\"
bytes = 1

# why the lock is pinned
[inputs.mappingsLock]
path = \"mappings.lock\"
sha256 = \"bb\"
bytes = 2
";

        let rewritten = super::replace_pin(document, "[inputs.mappingsLock]", "cc", 3)
            .expect("the table is there to rewrite");

        assert!(
            rewritten.contains("sha256 = \"cc\"\nbytes = 3\n"),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("sha256 = \"aa\"\nbytes = 1\n"),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("# why the lock is pinned"),
            "{rewritten}"
        );
    }

    #[test]
    fn repinning_a_table_that_pins_nothing_is_refused_rather_than_appended() {
        let error = super::replace_pin(
            "[inputs.stubsLock]\npath = \"a\"\n",
            "[inputs.stubsLock]",
            "cc",
            3,
        )
        .expect_err("there is no pin to rewrite");

        assert!(
            error.to_string().contains("hash and length to rewrite"),
            "{error}"
        );
    }

    #[test]
    fn a_message_names_a_file_and_never_the_directory_it_sits_in() {
        assert_eq!(file_name("a/b/client.jar"), "client.jar");
        assert_eq!(file_name("client.jar"), "client.jar");
    }

    #[test]
    fn hashing_matches_the_digest_the_adapter_inputs_pin() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"ravel"),
            sha256(b"ravel"),
            "hashing is a function of the bytes alone"
        );
        assert_ne!(sha256(b"ravel"), sha256(b"Ravel"));
    }

    /// Adapter inputs written out, with one mapping source and one mapping pin.
    fn document(source: &str, mappings: &str) -> String {
        format!(
            "schemaVersion = 1\n\
             minecraft = \"0.0.0-test\"\n\
             javaRelease = 25\n\
             mappingSource = \"{source}\"\n\
             [packFormat]\nresource = 7\nresourceMinor = 0\ndata = 11\ndataMinor = 0\n\
             spelling = \"pack-format\"\n\
             {mappings}\
             [inputs.clientJar]\npath = \"a\"\nsha256 = \"0\"\n\
             [inputs.dataFixerJar]\npath = \"b\"\nsha256 = \"0\"\n\
             [inputs.versionManifest]\npath = \"c\"\nsha256 = \"0\"\n\
             [inputs.hookInventory]\npath = \"d\"\nsha256 = \"0\"\n\
             [inputs.stubSeeds]\npath = \"e\"\nsha256 = \"0\"\n\
             [inputs.reflectiveSurface]\npath = \"h\"\nsha256 = \"0\"\n\
             [inputs.mappingsLock]\npath = \"f\"\nsha256 = \"0\"\n\
             [inputs.stubsLock]\npath = \"g\"\nsha256 = \"0\"\n"
        )
    }

    #[test]
    fn a_mapping_source_nothing_implements_is_refused_where_it_is_written() {
        let error = toml::from_str::<Adapter>(&document("community-mappings", ""))
            .expect_err("a source the generator cannot act on must be refused");

        assert!(error.to_string().contains("unknown variant"), "{error}");
        assert!(
            error.to_string().contains("unobfuscated-class-files"),
            "the refusal names the sources there are: {error}"
        );
    }

    #[test]
    fn both_admitted_sources_parse_and_write_back_the_string_they_came_from() {
        for source in [
            MappingSource::UnobfuscatedClassFiles,
            MappingSource::OfficialProguardMappings,
        ] {
            let mappings = if source.needs_mappings() {
                "[inputs.clientMappings]\npath = \"m\"\nsha256 = \"0\"\n"
            } else {
                ""
            };
            let adapter: Adapter = toml::from_str(&document(source.as_str(), mappings))
                .expect("an admitted source parses");

            assert_eq!(adapter.mapping_source, source);
            assert_eq!(
                adapter.inputs.client_mappings.is_some(),
                source.needs_mappings()
            );
        }
    }

    /// One reflected class written out, with whatever members follow it.
    fn reflected(members: &str) -> Reflection {
        toml::from_str(&format!(
            "schemaVersion = 1\nminecraft = \"0.0.0-test\"\n\
             [[reflected]]\nname = \"ALPHA\"\n\
             class = \"com.example.Alpha\"\nruntime = \"a\"\n{members}"
        ))
        .expect("a surface in the shape the version files take")
    }

    #[test]
    fn a_member_name_that_is_not_one_is_refused_before_a_jar_is_opened() {
        verify_surface(&reflected(
            "[[reflected.member]]\nname = \"<init>\"\ndescriptor = \"()V\"\n",
        ))
        .expect("a constructor is a member the agent jar reaches like any other");

        let error = verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick()\"\ndescriptor = \"()V\"\n",
        ))
        .expect_err("a name carrying punctuation is not a member name");

        assert!(error.to_string().contains("tick()"), "{error}");
        assert!(error.to_string().contains("not a member name"), "{error}");
    }

    /// Two overloads are two members and one declared twice is one, so the shape
    /// is what tells them apart rather than the name.
    #[test]
    fn one_member_declared_twice_is_refused_and_two_overloads_are_not() {
        verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick\"\ndescriptor = \"()V\"\n\
             [[reflected.member]]\nname = \"tick\"\ndescriptor = \"(I)V\"\n",
        ))
        .expect("two descriptors are two members");

        let error = verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick\"\nparameters = 1\n\
             [[reflected.member]]\nname = \"tick\"\nparameters = 1\n",
        ))
        .expect_err("one member declared twice says one thing twice");

        assert!(error.to_string().contains("tick/1"), "{error}");
        assert!(error.to_string().contains("twice"), "{error}");
    }
}
