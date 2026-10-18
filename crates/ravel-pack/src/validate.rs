use std::collections::BTreeMap;

mod files;

use ravel_config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_core::{ModId, RegistryKey, RelativePath, Side};
use ravel_data::{ContentDeclaration, Override};
use ravel_package::PackageManifest;

use crate::declare::PackDeclaration;
use crate::error::{ClosureError, UnprovidedEntry};

use files::written_files;

/// The game's namespace; entries here are allowed without verifying they exist.
pub const VANILLA_NAMESPACE: &str = "minecraft";

/// The output directory reserved for Ravel-owned files.
pub const RESERVED_PREFIX: &str = "ravel/";

/// One file a validated pack writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackFile {
    path: RelativePath,
    bytes: Vec<u8>,
}

impl PackFile {
    /// Returns where the file belongs in the written pack.
    #[must_use]
    pub const fn path(&self) -> &RelativePath {
        &self.path
    }

    /// Returns the file's exact content.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A validated pack's files and recorded member overrides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPack {
    files: Vec<PackFile>,
    overrides: Vec<Override>,
}

impl ValidatedPack {
    /// Returns every output file in path order, with one file per path.
    #[must_use]
    pub fn files(&self) -> &[PackFile] {
        &self.files
    }

    /// Returns the file at `path`, when the pack writes one.
    #[must_use]
    pub fn file(&self, path: &RelativePath) -> Option<&PackFile> {
        self.files
            .binary_search_by(|file| file.path.as_str().cmp(path.as_str()))
            .ok()
            .map(|found| &self.files[found])
    }

    /// Returns every member path replaced by the pack, in path order.
    #[must_use]
    pub fn overrides(&self) -> &[Override] {
        &self.overrides
    }
}

/// Checks dependency, registry, progression, settings, and resource closure.
///
/// Dependencies resolve on every [`Side::ALL`] side; pack files may override member files.
///
/// # Errors
///
/// Returns the first [`ClosureError`], checking the pinned set before its dependent closures.
pub fn validate(
    pack: &PackDeclaration,
    manifests: &[PackageManifest],
    content: &[ContentDeclaration],
    configurations: &[ConfigurationDeclaration],
) -> Result<ValidatedPack, ClosureError> {
    let installed = pinned_set(pack, manifests)?;
    for side in Side::ALL {
        ravel_resolve::resolve(manifests, side).map_err(|source| ClosureError::Dependency {
            side,
            source: Box::new(source),
        })?;
    }

    let declarations = declared_content(pack, content)?;
    registry_closure(pack, &declarations)?;
    progression_closure(pack, &declarations)?;
    settings_closure(pack, configurations)?;
    let (files, overrides) = written_files(pack, &installed, &declarations)?;

    Ok(ValidatedPack { files, overrides })
}

/// Returns every pinned member's manifest, refusing a set that is not the pinned one.
fn pinned_set<'a>(
    pack: &PackDeclaration,
    manifests: &'a [PackageManifest],
) -> Result<BTreeMap<&'a ModId, &'a PackageManifest>, ClosureError> {
    let mut installed: BTreeMap<&ModId, &PackageManifest> = BTreeMap::new();
    for manifest in manifests {
        let Some(pinned) = pack.pinned(&manifest.id) else {
            return Err(ClosureError::UnpinnedPackage {
                package: manifest.id.clone(),
            });
        };
        if &manifest.version != pinned {
            return Err(ClosureError::PinMismatch {
                member: manifest.id.clone(),
                pinned: pinned.clone(),
                supplied: manifest.version.clone(),
            });
        }
        installed.insert(&manifest.id, manifest);
    }
    for (member, _) in pack.members() {
        if !installed.contains_key(member) {
            return Err(ClosureError::MemberMissing {
                member: member.clone(),
            });
        }
    }
    Ok(installed)
}

/// Returns every pinned member's content declaration, one per member.
fn declared_content<'a>(
    pack: &PackDeclaration,
    content: &'a [ContentDeclaration],
) -> Result<BTreeMap<&'a ModId, &'a ContentDeclaration>, ClosureError> {
    let mut declarations: BTreeMap<&ModId, &ContentDeclaration> = BTreeMap::new();
    for declaration in content {
        if pack.pinned(declaration.owner()).is_none() {
            return Err(ClosureError::ContentUnpinned {
                owner: declaration.owner().clone(),
            });
        }
        if declarations
            .insert(declaration.owner(), declaration)
            .is_some()
        {
            return Err(ClosureError::ContentTwice {
                owner: declaration.owner().clone(),
            });
        }
    }
    for (member, _) in pack.members() {
        if !declarations.contains_key(member) {
            return Err(ClosureError::ContentMissing {
                member: member.clone(),
            });
        }
    }
    Ok(declarations)
}

/// Refuses a contested registry claim and a tag member nothing provides.
fn registry_closure(
    pack: &PackDeclaration,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
) -> Result<(), ClosureError> {
    let mut claimed: BTreeMap<&RegistryKey, &ModId> = BTreeMap::new();
    for (owner, declaration) in declarations {
        for key in declaration.entries() {
            if let Some(first) = claimed.get(key) {
                return Err(ClosureError::EntryContested {
                    key: key.clone(),
                    first: (*first).clone(),
                    second: (*owner).clone(),
                });
            }
            claimed.insert(key, owner);
        }
    }

    for declaration in declarations.values() {
        for tag in declaration.tags() {
            for member in declaration.tag_members(tag) {
                provided(pack, declarations, member).map_err(|kind| ClosureError::TagMember {
                    tag: tag.clone(),
                    member: member.clone(),
                    kind,
                })?;
            }
        }
    }
    Ok(())
}

/// Refuses a dangling prerequisite, an unreachable step, and a gate nothing provides.
fn progression_closure(
    pack: &PackDeclaration,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
) -> Result<(), ClosureError> {
    let progression = pack.progression();
    if let Some((state, prerequisite)) = progression.dangling().into_iter().next() {
        return Err(ClosureError::PrerequisiteMissing {
            state,
            prerequisite,
        });
    }
    if let Some(state) = progression.unreachable().into_iter().next() {
        return Err(ClosureError::StateUnreachable { state });
    }
    for state in progression.states() {
        if let Some(gate) = state.gate() {
            provided(pack, declarations, gate).map_err(|kind| ClosureError::Gate {
                state: state.id().clone(),
                entry: gate.clone(),
                kind,
            })?;
        }
    }
    Ok(())
}

/// Resolves the pack layer without a player layer.
fn settings_closure(
    pack: &PackDeclaration,
    configurations: &[ConfigurationDeclaration],
) -> Result<(), ClosureError> {
    for (owner, document) in pack.settings() {
        let Some(declaration) = configurations
            .iter()
            .find(|declaration| declaration.owner() == owner)
        else {
            return Err(ClosureError::SettingsUndeclared {
                owner: owner.clone(),
            });
        };
        LayeredConfiguration::resolve(declaration.clone(), Some(document), None).map_err(
            |source| ClosureError::Settings {
                owner: owner.clone(),
                source,
            },
        )?;
    }
    Ok(())
}

/// Applies the same entry-availability rule to tag members and progression gates.
fn provided(
    pack: &PackDeclaration,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
    key: &RegistryKey,
) -> Result<(), UnprovidedEntry> {
    let namespace = key.id().namespace();
    if namespace.as_str() == VANILLA_NAMESPACE {
        return Ok(());
    }
    if pack.pinned(namespace).is_none() {
        return Err(UnprovidedEntry::NoSuchMember);
    }
    let declares = declarations
        .get(namespace)
        .is_some_and(|declaration| declaration.entries().contains(key));
    if declares {
        Ok(())
    } else {
        Err(UnprovidedEntry::MemberDoesNotDeclareIt)
    }
}

#[cfg(test)]
mod tests;
