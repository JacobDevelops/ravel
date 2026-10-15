use std::fmt;

use ravel_config::LayerError;
use ravel_core::{ContentId, ModId, RegistryKey, RelativePath, Side, Version};
use ravel_resolve::ResolutionError;

use crate::validate::{RESERVED_PREFIX, VANILLA_NAMESPACE};

/// Why a registry entry is one this pack cannot account for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnprovidedEntry {
    /// The entry's namespace is not the game's and is not a pinned member.
    NoSuchMember,
    /// The namespace is a pinned member and that member declares no such entry.
    MemberDoesNotDeclareIt,
}

impl fmt::Display for UnprovidedEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchMember => write!(
                formatter,
                "its namespace is neither `{VANILLA_NAMESPACE}` nor a mod this pack pins"
            ),
            Self::MemberDoesNotDeclareIt => {
                formatter.write_str("the mod that owns its namespace declares no such entry")
            }
        }
    }
}

/// A failed dependency, registry, progression, settings, or resource closure.
#[derive(Clone, Debug, PartialEq)]
pub enum ClosureError {
    /// A pinned member's manifest was not supplied.
    MemberMissing {
        /// The member with no manifest.
        member: ModId,
    },
    /// A manifest was supplied for a package the pack does not pin.
    UnpinnedPackage {
        /// The package nothing pinned.
        package: ModId,
    },
    /// A member's manifest is at a version the pack did not pin.
    PinMismatch {
        /// The member.
        member: ModId,
        /// The version the pack pinned.
        pinned: Version,
        /// The version the manifest carries.
        supplied: Version,
    },
    /// The pinned set does not resolve on one of the sides the pack ships to.
    Dependency {
        /// The side it does not resolve on.
        side: Side,
        /// What the resolver refused.
        source: Box<ResolutionError>,
    },
    /// A pinned member declared no content at all, not even an empty declaration.
    ContentMissing {
        /// The member whose declaration is absent.
        member: ModId,
    },
    /// A content declaration belongs to a mod the pack does not pin.
    ContentUnpinned {
        /// The mod that owns it.
        owner: ModId,
    },
    /// Two content declarations claim the same owner.
    ContentTwice {
        /// The owner named twice.
        owner: ModId,
    },
    /// Two members claim the same registry entry.
    EntryContested {
        /// The contested entry.
        key: RegistryKey,
        /// The member that claimed it first.
        first: ModId,
        /// The member that claimed it second.
        second: ModId,
    },
    /// A tag names a member nothing in this pack provides.
    TagMember {
        /// The tag, rendered — a `TagId` is `ravel-data`'s and this is a report.
        tag: String,
        /// The member nothing provides.
        member: RegistryKey,
        /// Why nothing provides it.
        kind: UnprovidedEntry,
    },
    /// A progression step is gated on an entry nothing in this pack provides.
    Gate {
        /// The step.
        state: ContentId,
        /// The entry it is gated on.
        entry: RegistryKey,
        /// Why nothing provides it.
        kind: UnprovidedEntry,
    },
    /// Two members would write the same file.
    PathContested {
        /// The contested path.
        path: RelativePath,
        /// The member that writes it first.
        first: ModId,
        /// The member that writes it second.
        second: ModId,
    },
    /// The pack supplies one path twice.
    SuppliedTwice {
        /// The path supplied twice.
        path: RelativePath,
    },
    /// Something other than Ravel would write into Ravel's own directory.
    ReservedPath {
        /// The path.
        path: RelativePath,
    },
    /// The pack refers to a path nothing supplies.
    ResourceMissing {
        /// The path with no file behind it.
        path: RelativePath,
        /// The pack's own thing that refers to it.
        referrer: ContentId,
    },
    /// A progression step requires a step the pack does not declare.
    PrerequisiteMissing {
        /// The step that requires it.
        state: ContentId,
        /// The step nothing declares.
        prerequisite: ContentId,
    },
    /// A progression step could never be reached.
    StateUnreachable {
        /// The stranded step.
        state: ContentId,
    },
    /// The pack states settings for a member that declares none.
    SettingsUndeclared {
        /// The member with no configuration declaration.
        owner: ModId,
    },
    /// A member's own declaration refuses the settings the pack states for it.
    Settings {
        /// The member whose settings they are.
        owner: ModId,
        /// What `ravel-config` refused, which is what a launch would have refused.
        source: LayerError,
    },
}

impl fmt::Display for ClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MemberMissing { member } => fmt_member_missing(formatter, member),
            Self::UnpinnedPackage { package } => fmt_unpinned_package(formatter, package),
            Self::PinMismatch {
                member,
                pinned,
                supplied,
            } => fmt_pin_mismatch(formatter, member, pinned, supplied),
            Self::Dependency { side, source } => write!(
                formatter,
                "this pack does not resolve on `{}`: {source}",
                side.name()
            ),
            Self::ContentMissing { member } => fmt_content_missing(formatter, member),
            Self::ContentUnpinned { owner } => write!(
                formatter,
                "`{owner}` declared content and this pack pins no version of it"
            ),
            Self::ContentTwice { owner } => {
                write!(formatter, "`{owner}` declared its content twice")
            }
            Self::EntryContested { key, first, second } => write!(
                formatter,
                "`{first}` and `{second}` both claim `{key}`, and one entry has one owner"
            ),
            Self::TagMember { tag, member, kind } => write!(
                formatter,
                "`{tag}` holds `{member}`, which nothing in this pack provides: {kind}"
            ),
            Self::Gate { state, entry, kind } => write!(
                formatter,
                "`{state}` is gated on `{entry}`, which nothing in this pack provides: {kind}"
            ),
            Self::PathContested {
                path,
                first,
                second,
            } => fmt_path_contested(formatter, path, first, second),
            Self::SuppliedTwice { path } => write!(
                formatter,
                "this pack supplies `{path}` twice, and one path holds one file"
            ),
            Self::ReservedPath { path } => write!(
                formatter,
                "`{path}` is under `{RESERVED_PREFIX}`, which holds Ravel's own files only"
            ),
            Self::ResourceMissing { path, referrer } => write!(
                formatter,
                "`{referrer}` refers to `{path}` and nothing in this pack supplies it"
            ),
            Self::PrerequisiteMissing {
                state,
                prerequisite,
            } => write!(
                formatter,
                "`{state}` requires `{prerequisite}`, which this pack does not declare"
            ),
            Self::StateUnreachable { state } => write!(
                formatter,
                "`{state}` can never be reached: every route to it needs a step that needs it"
            ),
            Self::SettingsUndeclared { owner } => fmt_settings_undeclared(formatter, owner),
            Self::Settings { owner, source } => write!(
                formatter,
                "this pack's settings for `{owner}` are ones `{owner}` refuses: {source}"
            ),
        }
    }
}

impl std::error::Error for ClosureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dependency { source, .. } => Some(source.as_ref()),
            Self::Settings { source, .. } => Some(source),
            Self::MemberMissing { .. }
            | Self::UnpinnedPackage { .. }
            | Self::PinMismatch { .. }
            | Self::ContentMissing { .. }
            | Self::ContentUnpinned { .. }
            | Self::ContentTwice { .. }
            | Self::EntryContested { .. }
            | Self::TagMember { .. }
            | Self::Gate { .. }
            | Self::PathContested { .. }
            | Self::SuppliedTwice { .. }
            | Self::ReservedPath { .. }
            | Self::ResourceMissing { .. }
            | Self::PrerequisiteMissing { .. }
            | Self::StateUnreachable { .. }
            | Self::SettingsUndeclared { .. } => None,
        }
    }
}

fn fmt_member_missing(formatter: &mut fmt::Formatter<'_>, member: &ModId) -> fmt::Result {
    write!(
        formatter,
        "this pack pins `{member}` and no manifest for it was supplied, so nothing could \
         check what it needs"
    )
}

fn fmt_unpinned_package(formatter: &mut fmt::Formatter<'_>, package: &ModId) -> fmt::Result {
    write!(
        formatter,
        "`{package}` is installed and this pack pins no version of it; a pack decides its \
         own contents"
    )
}

fn fmt_content_missing(formatter: &mut fmt::Formatter<'_>, member: &ModId) -> fmt::Result {
    write!(
        formatter,
        "`{member}` is pinned and declared no content, so nothing could check its registry \
         entries; a mod that contributes nothing declares an empty declaration"
    )
}

fn fmt_path_contested(
    formatter: &mut fmt::Formatter<'_>,
    path: &RelativePath,
    first: &ModId,
    second: &ModId,
) -> fmt::Result {
    write!(
        formatter,
        "`{first}` and `{second}` would both write `{path}`; a pack settles that by \
         supplying the file itself"
    )
}

fn fmt_settings_undeclared(formatter: &mut fmt::Formatter<'_>, owner: &ModId) -> fmt::Result {
    write!(
        formatter,
        "this pack states settings for `{owner}` and `{owner}` declares none, so nothing \
         could say what they mean"
    )
}

fn fmt_pin_mismatch(
    formatter: &mut fmt::Formatter<'_>,
    member: &ModId,
    pinned: &Version,
    supplied: &Version,
) -> fmt::Result {
    write!(
        formatter,
        "this pack pins `{member}` at `{pinned}` and `{supplied}` was supplied"
    )
}
