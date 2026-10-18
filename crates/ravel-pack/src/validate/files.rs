use std::collections::BTreeMap;

use ravel_core::paths::{INSTANCE_DIRECTORY, PACK_CONFIG_DIRECTORY};
use ravel_core::{ModId, RelativePath};
use ravel_data::{ContentDeclaration, Override, TagSet};
use ravel_package::PackageManifest;
use serde_json::{Map, Value};

use super::PackFile;
use crate::declare::PackDeclaration;
use crate::error::ClosureError;

/// Builds the pack's files, refusing a contested or reserved path.
pub(super) fn written_files(
    pack: &PackDeclaration,
    installed: &BTreeMap<&ModId, &PackageManifest>,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
) -> Result<(Vec<PackFile>, Vec<Override>), ClosureError> {
    let mut writer: BTreeMap<RelativePath, Writer> = BTreeMap::new();
    let mut files: BTreeMap<RelativePath, Vec<u8>> = BTreeMap::new();
    let mut overrides: Vec<Override> = Vec::new();

    // Merge tag contributions before member files can overwrite each other's tags.
    let tags = TagSet::merged(declarations.values().copied());
    for file in tags.generate().files() {
        reserved(file.path())?;
        writer.insert(file.path().clone(), Writer::Members);
        files.insert(file.path().clone(), file.bytes().to_vec());
    }

    member_files(declarations, &mut writer, &mut files)?;

    pack_files(pack, &tags, &mut writer, &mut files, &mut overrides)?;

    // Manifest resources answer references without becoming pack output.
    let mut supplied_elsewhere: BTreeMap<&str, &ModId> = BTreeMap::new();
    for (owner, manifest) in installed {
        for path in &manifest.resources {
            supplied_elsewhere.insert(path.as_str(), owner);
        }
    }

    for reference in pack.references() {
        let answered = writer.contains_key(&reference.path)
            || supplied_elsewhere.contains_key(reference.path.as_str());
        if !answered {
            return Err(ClosureError::ResourceMissing {
                path: reference.path.clone(),
                referrer: reference.referrer.clone(),
            });
        }
    }

    for (owner, document) in pack.settings() {
        files.insert(
            pack_settings_path(owner),
            document_bytes(&document.to_json()),
        );
    }
    files.insert(ravel_path("pack.json"), pack_manifest(pack));

    Ok((
        files
            .into_iter()
            .map(|(path, bytes)| PackFile { path, bytes })
            .collect(),
        overrides,
    ))
}

fn member_files(
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
    writer: &mut BTreeMap<RelativePath, Writer>,
    files: &mut BTreeMap<RelativePath, Vec<u8>>,
) -> Result<(), ClosureError> {
    for (owner, declaration) in declarations {
        for file in declaration.generate().files() {
            let path = file.path().clone();
            if matches!(writer.get(&path), Some(Writer::Members)) {
                // The merged tag file above already holds this mod's contribution.
                continue;
            }
            reserved(&path)?;
            if let Some(Writer::Member(first)) = writer.get(&path) {
                return Err(ClosureError::PathContested {
                    path,
                    first: first.clone(),
                    second: (*owner).clone(),
                });
            }
            writer.insert(path.clone(), Writer::Member((*owner).clone()));
            files.insert(path, file.bytes().to_vec());
        }
    }
    Ok(())
}

fn pack_files(
    pack: &PackDeclaration,
    tags: &TagSet,
    writer: &mut BTreeMap<RelativePath, Writer>,
    files: &mut BTreeMap<RelativePath, Vec<u8>>,
    overrides: &mut Vec<Override>,
) -> Result<(), ClosureError> {
    for supplied in pack.supplied() {
        reserved(&supplied.path)?;
        match writer.get(&supplied.path) {
            Some(Writer::Member(first)) => overrides.push(Override {
                path: supplied.path.clone(),
                winner: pack.id().clone(),
                shadowed: vec![first.clone()],
            }),
            Some(Writer::Members) => {
                let contributors = tags
                    .tags()
                    .find(|tag| tag.generated_path() == supplied.path)
                    .map(|tag| tags.contributors(tag).to_vec())
                    .unwrap_or_default();
                overrides.push(Override {
                    path: supplied.path.clone(),
                    winner: pack.id().clone(),
                    shadowed: contributors,
                });
            }
            None | Some(Writer::Pack) => {}
        }
        writer.insert(supplied.path.clone(), Writer::Pack);
        files.insert(supplied.path.clone(), supplied.bytes.clone());
    }
    Ok(())
}

/// Tracks merged tags separately so overrides name every contributor.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Writer {
    /// Every member that contributed to one tag.
    Members,
    /// One member's own generated or supplied file.
    Member(ModId),
    /// The pack itself.
    Pack,
}

/// Refuses a path in the directory Ravel's own files live in.
fn reserved(path: &RelativePath) -> Result<(), ClosureError> {
    if path
        .as_str()
        .split('/')
        .find(|part| !part.is_empty() && *part != ".")
        == Some(INSTANCE_DIRECTORY)
    {
        return Err(ClosureError::ReservedPath { path: path.clone() });
    }
    Ok(())
}

/// Returns a path under the instance-private root.
fn ravel_path(rest: &str) -> RelativePath {
    RelativePath::try_from(format!("{INSTANCE_DIRECTORY}/{rest}"))
        .expect("a mod id cannot spell an illegal path")
}

/// Uses the same pack-settings directory as the configuration reader.
fn pack_settings_path(owner: &ModId) -> RelativePath {
    ravel_path(&format!("{PACK_CONFIG_DIRECTORY}/{owner}.json"))
}

/// Appends a trailing newline if needed.
fn document_bytes(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

/// Renders the pack identity, version, and exact member pins.
fn pack_manifest(pack: &PackDeclaration) -> Vec<u8> {
    let mut members = Map::new();
    for (member, version) in pack.members() {
        members.insert(
            member.as_str().to_owned(),
            Value::String(version.to_string()),
        );
    }

    let mut document = Map::new();
    document.insert(
        "id".to_owned(),
        Value::String(pack.id().as_str().to_owned()),
    );
    document.insert("members".to_owned(), Value::Object(members));
    document.insert(
        "version".to_owned(),
        Value::String(pack.version().to_string()),
    );

    let text = serde_json::to_string_pretty(&Value::Object(document))
        .expect("a map of strings always serializes");
    document_bytes(&text)
}

#[cfg(test)]
mod tests {
    use super::{ClosureError, INSTANCE_DIRECTORY, RelativePath, reserved};

    #[test]
    fn the_reserved_directory_is_a_whole_path_segment() {
        for text in [
            INSTANCE_DIRECTORY.to_owned(),
            format!("{INSTANCE_DIRECTORY}/pack.json"),
            format!("./{INSTANCE_DIRECTORY}/config/member.json"),
            format!("././{INSTANCE_DIRECTORY}"),
        ] {
            let path = RelativePath::try_from(text).expect("a relative path");
            assert_eq!(reserved(&path), Err(ClosureError::ReservedPath { path }));
        }
        let sibling = RelativePath::try_from(format!("{INSTANCE_DIRECTORY}-mod/file.json"))
            .expect("a relative path");
        assert_eq!(reserved(&sibling), Ok(()));
    }
}
