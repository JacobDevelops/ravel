//! ADG-3 binding extraction and generation from the pinned 26.1.2 inputs.
//!
//! The generator reads the tracked hook inventory, resolves every entry against
//! the pinned client jar, and writes one Rust binding plus one mapping lock. It
//! refuses to write anything unless every entry resolves exactly once, with the
//! declared shape, from inputs whose hashes match what is tracked.
//!
//! Two extractors read the same jar: one parses the class files, one asks the
//! JDK's `javap`. They are compared against each other, because two independent
//! readings agreeing is the only cheap evidence that either is right.

#![forbid(unsafe_code)]

pub mod classfile;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// Tracked hook inventory, relative to the repository root.
pub const HOOKS: &str = "minecraft/versions/26.1.2/hooks.toml";
/// Tracked adapter inputs, relative to the repository root.
pub const ADAPTER: &str = "minecraft/versions/26.1.2/adapter.toml";
/// Tracked mapping provenance the generator writes, relative to the root.
pub const MAPPINGS_LOCK: &str = "minecraft/versions/26.1.2/mappings.lock";

/// Anything that stops a binding from being generated.
#[derive(Debug)]
pub enum GenerateError {
    /// An input is missing or unreadable.
    Input(String),
    /// An input's content does not match what is tracked.
    Drift(String),
    /// A hook entry does not resolve against the jar.
    Unresolved(String),
    /// An extractor could not read the jar.
    Extraction(String),
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(detail) => write!(formatter, "an input is unusable: {detail}"),
            Self::Drift(detail) => write!(formatter, "an input drifted: {detail}"),
            Self::Unresolved(detail) => write!(formatter, "a hook does not resolve: {detail}"),
            Self::Extraction(detail) => write!(formatter, "extraction failed: {detail}"),
        }
    }
}

impl std::error::Error for GenerateError {}

/// One reviewed hook entry.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    /// Stable identifier the generated binding uses.
    pub id: String,
    /// Fully-qualified class name.
    pub class: String,
    /// Declared member name.
    pub member: String,
    /// Exact JVM descriptor.
    pub descriptor: String,
    /// Declared shape, which the class file's flags have to agree with.
    pub kind: Kind,
    /// Why the hook exists, for the humans reviewing the inventory.
    pub capability: String,
}

/// Shape of a hook target.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    StaticMethod,
    InstanceMethod,
    StaticField,
    InstanceField,
}

impl Kind {
    fn is_method(self) -> bool {
        matches!(self, Self::StaticMethod | Self::InstanceMethod)
    }

    fn is_static(self) -> bool {
        matches!(self, Self::StaticMethod | Self::StaticField)
    }
}

/// The tracked hook inventory.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inventory {
    pub schema_version: u8,
    pub minecraft: String,
    #[serde(rename = "hook")]
    pub hooks: Vec<Hook>,
}

/// One pinned input file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub path: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub upstream: Option<String>,
}

/// Inputs the generator is allowed to read.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inputs {
    pub client_jar: Input,
    pub version_manifest: Input,
    pub hook_inventory: Input,
}

/// The tracked adapter inputs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Adapter {
    pub schema_version: u8,
    pub minecraft: String,
    pub mapping_source: String,
    pub inputs: Inputs,
}

/// Which reading of the jar to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extractor {
    /// Parse the class files out of the jar.
    ClassFile,
    /// Ask the JDK's `javap` to describe them.
    Javap,
}

impl Extractor {
    /// Identifier used in results and on the command line.
    #[must_use]
    pub fn flag(self) -> &'static str {
        match self {
            Self::ClassFile => "classfile",
            Self::Javap => "javap",
        }
    }

    /// Parses an identifier back.
    #[must_use]
    pub fn parse(flag: &str) -> Option<Self> {
        match flag {
            "classfile" => Some(Self::ClassFile),
            "javap" => Some(Self::Javap),
            _ => None,
        }
    }
}

/// Reads and validates the tracked adapter inputs.
///
/// # Errors
///
/// Returns an error when a file is missing, malformed, or has drifted from the
/// hash the adapter inputs pin.
pub fn adapter(root: &Path) -> Result<Adapter, GenerateError> {
    let adapter: Adapter = parse(root, ADAPTER)?;
    let jar = root.join(&adapter.inputs.client_jar.path);
    if !jar.is_file() {
        return Err(GenerateError::Input(format!(
            "the pinned {} is absent",
            file_name(&adapter.inputs.client_jar.path)
        )));
    }
    verify_input(root, &adapter.inputs.client_jar)?;
    verify_input(root, &adapter.inputs.version_manifest)?;
    Ok(adapter)
}

/// Reads the tracked hook inventory.
///
/// # Errors
///
/// Returns an error when the inventory is missing or malformed.
pub fn inventory(root: &Path, adapter: &Adapter) -> Result<Inventory, GenerateError> {
    let inventory: Inventory = parse(root, &adapter.inputs.hook_inventory.path)?;
    if inventory.minecraft != adapter.minecraft {
        return Err(GenerateError::Drift(format!(
            "the inventory pins {} but the adapter inputs pin {}",
            inventory.minecraft, adapter.minecraft
        )));
    }
    Ok(inventory)
}

fn parse<T: for<'de> Deserialize<'de>>(root: &Path, relative: &str) -> Result<T, GenerateError> {
    let path = root.join(relative);
    let name = file_name(relative);
    let text = fs::read_to_string(&path)
        .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
    toml::from_str(&text)
        .map_err(|error| GenerateError::Input(format!("cannot parse {name}: {error}")))
}

/// Refuses an input whose bytes no longer match the tracked hash.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when its bytes or length
/// differ from what the adapter inputs pin.
pub fn verify_input(root: &Path, input: &Input) -> Result<(), GenerateError> {
    // Messages name inputs by file name, never by path: these strings end up in
    // recorded evidence, and a path would say whose machine produced it.
    let name = file_name(&input.path);
    let bytes = fs::read(root.join(&input.path))
        .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
    if let Some(expected) = &input.sha256 {
        let actual = ravel_spike_harness::sha256_bytes(&bytes);
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

/// The last path segment, which is all a recorded message may say about a file.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A member as one extractor read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// Hook identifier this resolves.
    pub id: String,
    /// Internal class name, as the JVM writes it.
    pub internal_class: String,
    /// Declared member name.
    pub member: String,
    /// Exact JVM descriptor.
    pub descriptor: String,
    /// Whether the class file declares the member static.
    pub is_static: bool,
}

/// Resolves every hook entry against the pinned jar.
///
/// # Errors
///
/// Returns an error when the jar cannot be read, or when an entry does not
/// resolve to exactly one member of the declared shape.
pub fn resolve(
    jar: &Path,
    inventory: &Inventory,
    extractor: Extractor,
) -> Result<Vec<Resolved>, GenerateError> {
    let mut classes: BTreeMap<&str, Vec<classfile::Member>> = BTreeMap::new();
    let mut resolved = Vec::new();

    for hook in &inventory.hooks {
        // Each class is read once, however many hooks it holds.
        if !classes.contains_key(hook.class.as_str()) {
            let read = match extractor {
                Extractor::ClassFile => from_class_file(jar, &hook.class)?,
                Extractor::Javap => from_javap(jar, &hook.class)?,
            };
            classes.insert(&hook.class, read);
        }
        let members = &classes[hook.class.as_str()];

        let matches = members
            .iter()
            .filter(|member| member.name == hook.member && member.descriptor == hook.descriptor)
            .collect::<Vec<_>>();
        let [member] = matches.as_slice() else {
            return Err(GenerateError::Unresolved(format!(
                "{} resolves {} times in {}",
                hook.id,
                matches.len(),
                hook.class
            )));
        };
        if member.method != hook.kind.is_method() {
            return Err(GenerateError::Unresolved(format!(
                "{} is declared {:?} but the jar declares the other kind",
                hook.id, hook.kind
            )));
        }
        if member.is_static() != hook.kind.is_static() {
            return Err(GenerateError::Unresolved(format!(
                "{} is declared {:?} but the jar disagrees about static",
                hook.id, hook.kind
            )));
        }

        resolved.push(Resolved {
            id: hook.id.clone(),
            internal_class: hook.class.replace('.', "/"),
            member: member.name.clone(),
            descriptor: member.descriptor.clone(),
            is_static: member.is_static(),
        });
    }
    Ok(resolved)
}

/// Reads a class's members by parsing the class file out of the jar.
fn from_class_file(jar: &Path, class: &str) -> Result<Vec<classfile::Member>, GenerateError> {
    let entry = format!("{}.class", class.replace('.', "/"));
    // ponytail: `unzip -p` is the standard library of the shell. A Rust zip
    // reader is a real dependency decision, recorded in the ADR for production.
    let output = Command::new("unzip")
        .arg("-p")
        .arg(jar)
        .arg(&entry)
        .output()
        .map_err(|error| GenerateError::Extraction(format!("cannot run unzip: {error}")))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(GenerateError::Extraction(format!(
            "the jar has no entry {entry}"
        )));
    }
    classfile::members(&output.stdout)
        .map_err(|error| GenerateError::Extraction(format!("{class}: {error}")))
}

/// Reads a class's members by asking the JDK to describe them.
fn from_javap(jar: &Path, class: &str) -> Result<Vec<classfile::Member>, GenerateError> {
    let output = Command::new("javap")
        .args(["-p", "-s", "-cp"])
        .arg(jar)
        .arg(class)
        .output()
        .map_err(|error| GenerateError::Extraction(format!("cannot run javap: {error}")))?;
    if !output.status.success() {
        return Err(GenerateError::Extraction(format!(
            "javap refused to describe {class}"
        )));
    }
    Ok(parse_javap(&String::from_utf8_lossy(&output.stdout), class))
}

/// Reads `javap -s` output back into members.
///
/// `javap` prints a source-level signature followed by the real descriptor, so
/// the descriptor is exact but the name has to be recovered from prose.
#[must_use]
pub fn parse_javap(text: &str, class: &str) -> Vec<classfile::Member> {
    let mut members = Vec::new();
    let mut signature: Option<&str> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(descriptor) = trimmed.strip_prefix("descriptor: ") {
            let Some(declaration) = signature.take() else {
                continue;
            };
            let method = declaration.contains('(');
            let Some(name) = javap_name(declaration, class, method) else {
                continue;
            };
            members.push(classfile::Member {
                name,
                descriptor: descriptor.to_owned(),
                flags: if declaration.contains("static ") {
                    classfile::ACC_STATIC
                } else {
                    0
                },
                method,
            });
        } else if trimmed.ends_with(';') && !trimmed.starts_with("Compiled") {
            signature = Some(trimmed);
        }
    }
    members
}

/// Recovers a member name from one `javap` declaration.
fn javap_name(declaration: &str, class: &str, method: bool) -> Option<String> {
    let head = if method {
        declaration.split('(').next()?
    } else {
        declaration.trim_end_matches(';')
    };
    let last = head.split_whitespace().last()?;
    if method && last == class {
        // A constructor. `javap` prints the class name, so the name the JVM uses
        // is not in the output at all.
        return Some("<init>".to_owned());
    }
    Some(last.to_owned())
}

/// What one generation produced.
pub struct Generated {
    /// Files written, in the order they were written.
    pub files: Vec<PathBuf>,
    /// Digest over the generated bytes, independent of where they were written.
    pub sha256: String,
    /// Total generated bytes.
    pub bytes: u64,
}

/// Writes the Rust binding and the mapping lock for one resolution.
///
/// Both files are written to a temporary name and renamed into place, so a
/// generator that dies half way leaves nothing behind for a build to pick up.
///
/// # Errors
///
/// Returns an error when a file cannot be written.
pub fn generate(
    directory: &Path,
    adapter: &Adapter,
    inventory: &Inventory,
    resolved: &[Resolved],
    extractor: Extractor,
) -> Result<Generated, GenerateError> {
    let binding = render_binding(adapter, inventory, resolved);
    let lock = render_lock(adapter, resolved, extractor);

    fs::create_dir_all(directory).map_err(|error| {
        GenerateError::Input(format!("cannot create {}: {error}", directory.display()))
    })?;
    let mut files = Vec::new();
    let mut bytes = 0;
    let mut digest = String::new();
    for (name, content) in [("bindings.rs", &binding), ("mappings.lock", &lock)] {
        let path = directory.join(name);
        write_atomically(&path, content)?;
        bytes += content.len() as u64;
        let _ = write!(
            digest,
            "{name}\0{}\0",
            ravel_spike_harness::sha256_bytes(content.as_bytes())
        );
        files.push(path);
    }
    Ok(Generated {
        files,
        sha256: ravel_spike_harness::sha256_bytes(digest.as_bytes()),
        bytes,
    })
}

fn write_atomically(path: &Path, content: &str) -> Result<(), GenerateError> {
    let pending = path.with_extension("pending");
    fs::write(&pending, content).map_err(|error| {
        GenerateError::Input(format!("cannot write {}: {error}", path.display()))
    })?;
    fs::rename(&pending, path)
        .map_err(|error| GenerateError::Input(format!("cannot place {}: {error}", path.display())))
}

/// Renders the generated Rust binding.
fn render_binding(adapter: &Adapter, inventory: &Inventory, resolved: &[Resolved]) -> String {
    let mut text = String::with_capacity(resolved.len() * 256 + 512);
    let _ = writeln!(
        text,
        "//! Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         //!\n\
         //! Source of names: {}. Regenerate with `devenv tasks run ravel:spike-adg-3`.\n\
         \n\
         /// One hook target, resolved against the pinned jar.\n\
         pub struct Target {{\n\
         \x20   /// Internal class name, as the JVM writes it.\n\
         \x20   pub class: &'static str,\n\
         \x20   /// Declared member name.\n\
         \x20   pub member: &'static str,\n\
         \x20   /// Exact JVM descriptor.\n\
         \x20   pub descriptor: &'static str,\n\
         \x20   /// Whether the member is declared static.\n\
         \x20   pub is_static: bool,\n\
         }}\n",
        adapter.minecraft, adapter.mapping_source
    );

    for (hook, target) in inventory.hooks.iter().zip(resolved) {
        let _ = writeln!(
            text,
            "/// {}\npub const {}: Target = Target {{\n\
             \x20   class: \"{}\",\n\
             \x20   member: \"{}\",\n\
             \x20   descriptor: \"{}\",\n\
             \x20   is_static: {},\n\
             }};\n",
            hook.capability,
            constant_name(&target.id),
            target.internal_class,
            target.member,
            target.descriptor,
            target.is_static
        );
    }
    text
}

/// Renders the mapping provenance lock.
fn render_lock(adapter: &Adapter, resolved: &[Resolved], extractor: Extractor) -> String {
    let mut text = String::with_capacity(resolved.len() * 192 + 512);
    let _ = writeln!(
        text,
        "# Generated mapping provenance for Minecraft {}. Do not edit.\n\
         #\n\
         # Every line pins one resolved member. A change to the jar, the\n\
         # inventory, or the extractor changes this file, which is how drift is\n\
         # noticed before a build trusts a binding.\n\
         \n\
         schemaVersion = 1\n\
         minecraft = \"{}\"\n\
         mappingSource = \"{}\"\n\
         extractor = \"{}\"\n\
         clientJarSha256 = \"{}\"",
        adapter.minecraft,
        adapter.minecraft,
        adapter.mapping_source,
        extractor.flag(),
        adapter.inputs.client_jar.sha256.as_deref().unwrap_or("")
    );
    for target in resolved {
        let _ = writeln!(
            text,
            "\n[target.\"{}\"]\nclass = \"{}\"\nmember = \"{}\"\ndescriptor = \"{}\"\nstatic = {}",
            target.id, target.internal_class, target.member, target.descriptor, target.is_static
        );
    }
    text
}

/// Turns a hook identifier into a Rust constant name.
fn constant_name(id: &str) -> String {
    id.replace(['.', '-'], "_").to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{Extractor, constant_name, parse_javap};

    #[test]
    fn javap_output_is_read_back_into_members() {
        let text = "Compiled from \"StringUtil.java\"\n\
                    public class net.minecraft.util.StringUtil {\n\
                    \x20 public net.minecraft.util.StringUtil();\n\
                    \x20   descriptor: ()V\n\
                    \x20 public static boolean isAllowedChatCharacter(int);\n\
                    \x20   descriptor: (I)Z\n\
                    \x20 private double lastAmount;\n\
                    \x20   descriptor: D\n\
                    }\n";

        let members = parse_javap(text, "net.minecraft.util.StringUtil");

        assert_eq!(members.len(), 3);
        assert_eq!(members[0].name, "<init>", "javap names no constructor");
        assert_eq!(members[1].name, "isAllowedChatCharacter");
        assert!(members[1].is_static());
        assert!(members[1].method);
        assert_eq!(members[2].descriptor, "D");
        assert!(!members[2].method);
    }

    #[test]
    fn hook_identifiers_become_constant_names() {
        assert_eq!(constant_name("chat.filter-text"), "CHAT_FILTER_TEXT");
    }

    #[test]
    fn extractor_flags_round_trip() {
        for extractor in [Extractor::ClassFile, Extractor::Javap] {
            assert_eq!(Extractor::parse(extractor.flag()), Some(extractor));
        }
        assert_eq!(Extractor::parse("magic"), None);
    }
}
