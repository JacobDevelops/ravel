//! Resolving every reviewed hook to exactly one member of the pinned jar.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::inputs::{Adapter, Inventory, MappingSource};
use super::mapping::Mapping;
use super::{GenerateError, classfile, jar};

/// A resolved hook's reviewed names for documentation and linked names for target tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Resolved {
    /// Hook identifier this resolves.
    pub(super) id: String,
    /// Internal class name the reviewed inventory carries.
    pub(super) mojang_class: String,
    /// Internal class name the JVM links.
    pub(super) runtime_class: String,
    /// Member name the reviewed inventory carries.
    pub(super) member: String,
    /// Member name the JVM links.
    pub(super) runtime_member: String,
    /// JVM descriptor the reviewed inventory carries.
    pub(super) descriptor: String,
    /// JVM descriptor the JVM links.
    pub(super) runtime_descriptor: String,
    /// Whether the class file declares the member static.
    pub(super) is_static: bool,
}

/// A tracked member; runtime names are omitted when they match reviewed names.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pinned {
    class: String,
    member: String,
    descriptor: String,
    #[serde(rename = "static")]
    is_static: bool,
    #[serde(default)]
    runtime_class: Option<String>,
    #[serde(default)]
    runtime_member: Option<String>,
    #[serde(default)]
    runtime_descriptor: Option<String>,
}

/// The tracked mapping lock.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lock {
    schema_version: u8,
    minecraft: String,
    mapping_source: MappingSource,
    extractor: String,
    client_jar_sha256: String,
    /// The published mapping the linked names below were read from.
    ///
    /// Absent under a source that reads none, which is the same shape the
    /// adapter inputs declare it in.
    #[serde(default)]
    client_mappings_sha256: Option<String>,
    #[serde(rename = "target")]
    targets: BTreeMap<String, Pinned>,
}

/// Resolves every hook entry against the pinned jar's class files.
pub(super) fn from_jar(
    archive: &jar::Archive,
    inventory: &Inventory,
    mapping: &Mapping,
) -> Result<Vec<Resolved>, GenerateError> {
    let mut classes: BTreeMap<&str, Vec<classfile::Member>> = BTreeMap::new();
    let mut resolved = Vec::new();

    for hook in &inventory.hooks {
        let mojang_class = hook.class.replace('.', "/");
        // Each class is read once, however many hooks it holds.
        if !classes.contains_key(hook.class.as_str()) {
            let bytes = archive.entry(&mapping.entry(&mojang_class))?;
            let members = classfile::members(&bytes)
                .map_err(|error| GenerateError::Extraction(format!("{}: {error}", hook.class)))?;
            classes.insert(&hook.class, members);
        }
        let members = &classes[hook.class.as_str()];

        // The inventory reviews a Mojang name and the class file declares a
        // linked one, so the two are compared in the class file's own name space
        // and never by hoping they coincide.
        let (runtime_member, runtime_descriptor) =
            mapping.runtime_member(&mojang_class, &hook.member, &hook.descriptor)?;
        let matches = members
            .iter()
            .filter(|member| {
                member.name == runtime_member && member.descriptor == runtime_descriptor
            })
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
            runtime_class: mapping.runtime_class(&mojang_class).to_owned(),
            mojang_class,
            member: hook.member.clone(),
            runtime_member: member.name.clone(),
            descriptor: hook.descriptor.clone(),
            runtime_descriptor: member.descriptor.clone(),
            is_static: member.is_static(),
        });
    }
    Ok(resolved)
}

/// Refuses a lock whose header disagrees with the inputs it claims to describe.
fn header(lock: &Lock, adapter: &Adapter, inventory: &Inventory) -> Result<(), GenerateError> {
    for (what, tracked, pinned) in [
        (
            "the version",
            lock.minecraft.as_str(),
            adapter.minecraft.as_str(),
        ),
        (
            "the mapping source",
            lock.mapping_source.as_str(),
            adapter.mapping_source.as_str(),
        ),
        (
            "the client jar hash",
            lock.client_jar_sha256.as_str(),
            adapter.inputs.client_jar.sha256.as_deref().unwrap_or(""),
        ),
        // Nothing on this path opens the mapping file, so its hash is the only
        // thing that can say the lock's linked names came from the mapping the
        // inputs now pin.
        (
            "the client mapping hash",
            lock.client_mappings_sha256.as_deref().unwrap_or(""),
            adapter
                .inputs
                .client_mappings
                .as_ref()
                .and_then(|input| input.sha256.as_deref())
                .unwrap_or(""),
        ),
    ] {
        if tracked != pinned {
            return Err(GenerateError::Drift(format!(
                "mappings.lock records {what} as {tracked} but the adapter inputs pin {pinned}"
            )));
        }
    }
    if lock.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "mappings.lock is schema {} but the adapter inputs are schema {}",
            lock.schema_version, adapter.schema_version
        )));
    }
    if lock.extractor != super::EXTRACTOR {
        return Err(GenerateError::Drift(format!(
            "mappings.lock was written by the {} extractor",
            lock.extractor
        )));
    }
    if lock.targets.len() != inventory.hooks.len() {
        return Err(GenerateError::Drift(format!(
            "mappings.lock pins {} targets but the inventory reviews {}",
            lock.targets.len(),
            inventory.hooks.len()
        )));
    }
    Ok(())
}

/// Resolves every hook entry against the tracked mapping lock.
pub(super) fn from_lock(
    tracked: &str,
    adapter: &Adapter,
    inventory: &Inventory,
) -> Result<Vec<Resolved>, GenerateError> {
    let lock: Lock = toml::from_str(tracked)
        .map_err(|error| GenerateError::Input(format!("cannot parse mappings.lock: {error}")))?;
    header(&lock, adapter, inventory)?;

    let mut resolved = Vec::with_capacity(inventory.hooks.len());
    for hook in &inventory.hooks {
        let pinned = lock.targets.get(&hook.id).ok_or_else(|| {
            GenerateError::Drift(format!("mappings.lock pins no target for {}", hook.id))
        })?;
        let mojang_class = hook.class.replace('.', "/");
        if pinned.class != mojang_class
            || pinned.member != hook.member
            || pinned.descriptor != hook.descriptor
            || pinned.is_static != hook.kind.is_static()
        {
            return Err(GenerateError::Drift(format!(
                "mappings.lock pins {} to a member the inventory does not review",
                hook.id
            )));
        }
        // A source that translates nothing has no second name to pin, so a lock
        // that carries one under it was written by something else.
        if !adapter.mapping_source.needs_mappings()
            && (pinned.runtime_class.is_some()
                || pinned.runtime_member.is_some()
                || pinned.runtime_descriptor.is_some())
        {
            return Err(GenerateError::Drift(format!(
                "mappings.lock pins a linked name for {} under a source that links none",
                hook.id
            )));
        }
        resolved.push(Resolved {
            id: hook.id.clone(),
            runtime_class: pinned
                .runtime_class
                .clone()
                .unwrap_or_else(|| mojang_class.clone()),
            mojang_class,
            member: pinned.member.clone(),
            runtime_member: pinned
                .runtime_member
                .clone()
                .unwrap_or_else(|| pinned.member.clone()),
            descriptor: pinned.descriptor.clone(),
            runtime_descriptor: pinned
                .runtime_descriptor
                .clone()
                .unwrap_or_else(|| pinned.descriptor.clone()),
            is_static: pinned.is_static,
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::inputs::{self, MappingSource, PinnedVersion};
    use super::super::jar::fixture;
    use super::super::mapping::Mapping;
    use super::{from_jar, from_lock};

    /// The mapping every reviewed name already agrees with.
    fn identity() -> Mapping {
        Mapping::read(MappingSource::UnobfuscatedClassFiles, "")
            .expect("the identity needs no text")
    }

    fn pinned() -> PinnedVersion {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>");
        // The version whose class files are the mapping: these tests assert on a
        // lock whose linked names are its reviewed ones, which is what an
        // unobfuscated source gives and an obfuscated one does not.
        inputs::tracked_version_where(root, |adapter| !adapter.mapping_source.needs_mappings())
    }

    fn tracked_lock(pinned: &PinnedVersion) -> String {
        std::fs::read_to_string(pinned.directory.join("mappings.lock"))
            .expect("the tracked mapping lock should be readable")
    }

    #[test]
    fn the_tracked_lock_resolves_every_reviewed_hook_in_inventory_order() {
        let pinned = pinned();

        let resolved = from_lock(&tracked_lock(&pinned), &pinned.adapter, &pinned.inventory)
            .expect("resolves");

        assert_eq!(resolved.len(), pinned.inventory.hooks.len());
        for (hook, target) in pinned.inventory.hooks.iter().zip(&resolved) {
            assert_eq!(target.id, hook.id);
            assert_eq!(target.mojang_class, hook.class.replace('.', "/"));
            assert_eq!(target.runtime_class, target.mojang_class);
            assert_eq!(target.descriptor, hook.descriptor);
            assert_eq!(target.is_static, hook.kind.is_static());
        }
    }

    #[test]
    fn a_lock_that_pins_a_different_jar_is_refused() {
        let pinned = pinned();
        let tracked = tracked_lock(&pinned).replace(
            pinned
                .adapter
                .inputs
                .client_jar
                .sha256
                .as_deref()
                .expect("a pinned hash"),
            &"0".repeat(64),
        );

        let error = from_lock(&tracked, &pinned.adapter, &pinned.inventory)
            .expect_err("a drifted lock must be refused");

        assert!(error.to_string().contains("the client jar hash"), "{error}");
    }

    /// Nothing on this path opens the mapping file, so swapping it for another
    /// version's would otherwise leave every linked name in the lock stale.
    #[test]
    fn a_lock_that_pins_a_different_client_mapping_is_refused() {
        let mut pinned = pinned();
        pinned.adapter.mapping_source = MappingSource::OfficialProguardMappings;
        pinned.adapter.inputs.client_mappings = Some(inputs::Input {
            path: "nowhere/client.txt".to_owned(),
            sha256: Some("a".repeat(64)),
            bytes: None,
            upstream: None,
        });
        let tracked = tracked_lock(&pinned)
            .replace(
                MappingSource::UnobfuscatedClassFiles.as_str(),
                MappingSource::OfficialProguardMappings.as_str(),
            )
            .replace(
                "extractor = ",
                &format!(
                    "clientMappingsSha256 = \"{}\"\nextractor = ",
                    "b".repeat(64)
                ),
            );

        let error = from_lock(&tracked, &pinned.adapter, &pinned.inventory)
            .expect_err("a lock written from another mapping must be refused");

        assert!(
            error.to_string().contains("the client mapping hash"),
            "{error}"
        );
    }

    #[test]
    fn a_lock_missing_a_reviewed_hook_is_refused() {
        let pinned = pinned();
        let first = &pinned.inventory.hooks[0].id;
        let tracked = tracked_lock(&pinned)
            .replace(&format!("[target.\"{first}\"]"), "[target.\"absent.hook\"]");

        let error = from_lock(&tracked, &pinned.adapter, &pinned.inventory)
            .expect_err("a lock missing a hook must be refused");

        assert!(error.to_string().contains(first), "{error}");
    }

    #[test]
    fn a_lock_that_pins_the_wrong_member_is_refused() {
        let pinned = pinned();
        let tracked = tracked_lock(&pinned).replace("static = true", "static = false");

        let error = from_lock(&tracked, &pinned.adapter, &pinned.inventory)
            .expect_err("a lock that disagrees about static must be refused");

        assert!(
            error
                .to_string()
                .contains("a member the inventory does not review"),
            "{error}"
        );
    }

    #[test]
    fn a_lock_written_by_another_extractor_is_refused() {
        let pinned = pinned();
        let tracked =
            tracked_lock(&pinned).replace("extractor = \"classfile\"", "extractor = \"javap\"");

        let error = from_lock(&tracked, &pinned.adapter, &pinned.inventory)
            .expect_err("another extractor must be refused");

        assert!(error.to_string().contains("javap"), "{error}");
    }

    /// The first reviewed hook, with an archive holding the class it names.
    fn one_hook(flags: u16) -> (super::Inventory, Vec<u8>) {
        let mut inventory = pinned().inventory;
        inventory.hooks.truncate(1);
        let hook = inventory.hooks[0].clone();
        let entry = format!("{}.class", hook.class.replace('.', "/"));

        let mut class = Vec::new();
        class.extend(0xCAFE_BABE_u32.to_be_bytes());
        class.extend(0_u16.to_be_bytes()); // minor
        class.extend(69_u16.to_be_bytes()); // major
        class.extend(3_u16.to_be_bytes()); // constant pool count
        for text in [hook.member.as_str(), hook.descriptor.as_str()] {
            class.push(1); // modified UTF-8
            class.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            class.extend(text.as_bytes());
        }
        // access flags, this, super, interfaces, fields, one method, then the
        // method's flags, name index, descriptor index, and both attribute counts.
        for value in [0_u16, 0, 0, 0, 0, 1, flags, 1, 2, 0, 0] {
            class.extend(value.to_be_bytes());
        }
        (inventory, fixture::stored(&entry, &class))
    }

    #[test]
    fn a_class_file_that_declares_the_reviewed_member_resolves_once() {
        let (inventory, bytes) = one_hook(0x0008);
        let archive = fixture::parse(bytes).expect("a synthetic archive");

        let resolved = from_jar(&archive, &inventory, &identity()).expect("the member resolves");

        assert_eq!(resolved[0].id, inventory.hooks[0].id);
        assert_eq!(resolved[0].member, inventory.hooks[0].member);
        assert_eq!(resolved[0].descriptor, inventory.hooks[0].descriptor);
        assert_eq!(resolved[0].mojang_class, "net/minecraft/util/StringUtil");
        assert_eq!(
            resolved[0].runtime_class, resolved[0].mojang_class,
            "a source that translates nothing gives a target one name twice"
        );
        assert_eq!(resolved[0].runtime_member, resolved[0].member);
        assert_eq!(resolved[0].runtime_descriptor, resolved[0].descriptor);
        assert!(resolved[0].is_static);
    }

    #[test]
    fn a_member_the_jar_declares_with_the_other_static_flag_is_refused() {
        let (inventory, bytes) = one_hook(0);
        let archive = fixture::parse(bytes).expect("a synthetic archive");

        let error = from_jar(&archive, &inventory, &identity()).expect_err("static must agree");

        assert!(
            error.to_string().contains("disagrees about static"),
            "{error}"
        );
    }

    #[test]
    fn a_jar_that_does_not_carry_the_reviewed_class_is_refused_by_entry_name() {
        let (inventory, _) = one_hook(0x0008);
        let archive = fixture::parse(fixture::stored("other/Class.class", b"\0")).expect("archive");

        let error = from_jar(&archive, &inventory, &identity())
            .expect_err("a missing class must be refused");

        assert!(error.to_string().contains("StringUtil.class"), "{error}");
    }

    #[test]
    fn an_absent_jar_is_refused_by_file_name_and_never_by_path() {
        let error = super::jar::Archive::open(Path::new("/nowhere/client.jar"))
            .err()
            .expect("an absent jar must be refused");
        let message = error.to_string();

        assert!(message.contains("client.jar"), "{message}");
        assert!(!message.contains("/nowhere"), "{message}");
    }

    /// An invented obfuscated version: an inventory, its mapping, and its jar.
    ///
    /// Nothing here is any real version's. The point is that the reviewed
    /// inventory and the class file share no name at all, so a resolution that
    /// succeeds can only have gone through the mapping.
    fn obfuscated(member: &str) -> (super::Inventory, Mapping, Vec<u8>) {
        let inventory = toml::from_str(
            "schemaVersion = 1\n\
             minecraft = \"0.0.0-test\"\n\
             [[hook]]\n\
             id = \"invented.hook\"\n\
             class = \"com.example.Alpha\"\n\
             member = \"allowed\"\n\
             descriptor = \"(C)Z\"\n\
             kind = \"static-method\"\n\
             capability = \"an invented capability\"\n",
        )
        .expect("the invented inventory should parse");
        let mapping = Mapping::read(
            MappingSource::OfficialProguardMappings,
            "com.example.Alpha -> a:\n    17:26:boolean allowed(char) -> a\n",
        )
        .expect("the invented mapping should parse");

        let mut class = Vec::new();
        class.extend(0xCAFE_BABE_u32.to_be_bytes());
        class.extend(0_u16.to_be_bytes());
        class.extend(69_u16.to_be_bytes());
        class.extend(3_u16.to_be_bytes());
        for text in [member, "(C)Z"] {
            class.push(1);
            class.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            class.extend(text.as_bytes());
        }
        for value in [0_u16, 0, 0, 0, 0, 1, 0x0008, 1, 2, 0, 0] {
            class.extend(value.to_be_bytes());
        }
        (inventory, mapping, fixture::stored("a.class", &class))
    }

    #[test]
    fn an_obfuscated_jar_is_asked_for_the_entry_the_mapping_names() {
        let (inventory, mapping, bytes) = obfuscated("a");
        let archive = fixture::parse(bytes).expect("a synthetic archive");

        let resolved = from_jar(&archive, &inventory, &mapping).expect("the mapping resolves it");

        assert_eq!(resolved[0].mojang_class, "com/example/Alpha");
        assert_eq!(resolved[0].runtime_class, "a");
        assert_eq!(resolved[0].member, "allowed");
        assert_eq!(resolved[0].runtime_member, "a");
    }

    #[test]
    fn a_mapped_member_the_class_file_does_not_declare_is_refused() {
        let (inventory, mapping, bytes) = obfuscated("b");
        let archive = fixture::parse(bytes).expect("a synthetic archive");

        let error = from_jar(&archive, &inventory, &mapping)
            .expect_err("the class declares b and the mapping says a");

        assert!(error.to_string().contains("resolves 0 times"), "{error}");
    }

    #[test]
    fn a_lock_that_pins_a_linked_name_under_a_source_that_links_none_is_refused() {
        let pinned = pinned();
        let first = &pinned.inventory.hooks[0].id;
        let tracked = tracked_lock(&pinned).replace(
            &format!("[target.\"{first}\"]\nclass = "),
            &format!("[target.\"{first}\"]\nruntimeClass = \"a\"\nclass = "),
        );

        let error = from_lock(&tracked, &pinned.adapter, &pinned.inventory)
            .expect_err("a linked name must be refused under the identity source");

        assert!(
            error.to_string().contains("a source that links none"),
            "{error}"
        );
    }
}
