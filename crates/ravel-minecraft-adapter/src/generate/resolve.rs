//! Resolving every reviewed hook to exactly one member of the pinned jar.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::inputs::{Adapter, Inventory};
use super::{GenerateError, classfile, jar};

/// A member as the generator read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Resolved {
    /// Hook identifier this resolves.
    pub(super) id: String,
    /// Internal class name, as the JVM writes it.
    pub(super) internal_class: String,
    /// Declared member name.
    pub(super) member: String,
    /// Exact JVM descriptor.
    pub(super) descriptor: String,
    /// Whether the class file declares the member static.
    pub(super) is_static: bool,
}

/// One pinned member, as the tracked mapping lock records it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pinned {
    class: String,
    member: String,
    descriptor: String,
    #[serde(rename = "static")]
    is_static: bool,
}

/// The tracked mapping lock.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lock {
    schema_version: u8,
    minecraft: String,
    mapping_source: String,
    extractor: String,
    client_jar_sha256: String,
    #[serde(rename = "target")]
    targets: BTreeMap<String, Pinned>,
}

/// Resolves every hook entry against the pinned jar's class files.
pub(super) fn from_jar(
    archive: &jar::Archive,
    inventory: &Inventory,
) -> Result<Vec<Resolved>, GenerateError> {
    let mut classes: BTreeMap<&str, Vec<classfile::Member>> = BTreeMap::new();
    let mut resolved = Vec::new();

    for hook in &inventory.hooks {
        // Each class is read once, however many hooks it holds.
        if !classes.contains_key(hook.class.as_str()) {
            let entry = format!("{}.class", hook.class.replace('.', "/"));
            let bytes = archive.entry(&entry)?;
            let members = classfile::members(&bytes)
                .map_err(|error| GenerateError::Extraction(format!("{}: {error}", hook.class)))?;
            classes.insert(&hook.class, members);
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

/// Resolves every hook entry against the tracked mapping lock.
///
/// This is what a machine without the restricted client jar generates from, and
/// it is why `cargo check --workspace` succeeds on one. The lock already carries
/// every name the jar resolved to, and it is tracked, so a silent rename shows
/// up in a diff rather than in a build that quietly disagreed with it.
pub(super) fn from_lock(
    tracked: &str,
    adapter: &Adapter,
    inventory: &Inventory,
) -> Result<Vec<Resolved>, GenerateError> {
    let lock: Lock = toml::from_str(tracked)
        .map_err(|error| GenerateError::Input(format!("cannot parse mappings.lock: {error}")))?;

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

    let mut resolved = Vec::with_capacity(inventory.hooks.len());
    for hook in &inventory.hooks {
        let pinned = lock.targets.get(&hook.id).ok_or_else(|| {
            GenerateError::Drift(format!("mappings.lock pins no target for {}", hook.id))
        })?;
        let internal_class = hook.class.replace('.', "/");
        if pinned.class != internal_class
            || pinned.member != hook.member
            || pinned.descriptor != hook.descriptor
            || pinned.is_static != hook.kind.is_static()
        {
            return Err(GenerateError::Drift(format!(
                "mappings.lock pins {} to a member the inventory does not review",
                hook.id
            )));
        }
        resolved.push(Resolved {
            id: hook.id.clone(),
            internal_class,
            member: pinned.member.clone(),
            descriptor: pinned.descriptor.clone(),
            is_static: pinned.is_static,
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::inputs::{self, PinnedVersion};
    use super::super::jar::fixture;
    use super::{from_jar, from_lock};

    fn pinned() -> PinnedVersion {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>");
        inputs::pinned(root).expect("the tracked version data should parse")
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
            assert_eq!(target.internal_class, hook.class.replace('.', "/"));
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

        let resolved = from_jar(&archive, &inventory).expect("the member resolves");

        assert_eq!(resolved[0].id, inventory.hooks[0].id);
        assert_eq!(resolved[0].member, inventory.hooks[0].member);
        assert_eq!(resolved[0].descriptor, inventory.hooks[0].descriptor);
        assert_eq!(resolved[0].internal_class, "net/minecraft/util/StringUtil");
        assert!(resolved[0].is_static);
    }

    #[test]
    fn a_member_the_jar_declares_with_the_other_static_flag_is_refused() {
        let (inventory, bytes) = one_hook(0);
        let archive = fixture::parse(bytes).expect("a synthetic archive");

        let error = from_jar(&archive, &inventory).expect_err("static must agree");

        assert!(
            error.to_string().contains("disagrees about static"),
            "{error}"
        );
    }

    #[test]
    fn a_jar_that_does_not_carry_the_reviewed_class_is_refused_by_entry_name() {
        let (inventory, _) = one_hook(0x0008);
        let archive = fixture::parse(fixture::stored("other/Class.class", b"\0")).expect("archive");

        let error = from_jar(&archive, &inventory).expect_err("a missing class must be refused");

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
}
