//! Builds an instance's host from its installed packages.

use std::fmt;
use std::path::{Path, PathBuf};

use ravel_build::{BuildRefused, plan};
use ravel_core::{ModId, Sha256Digest};
use ravel_loader::{DiscoveryRefused, discover};

/// What one host build produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltHost {
    /// The library the JVM loads, inside the instance.
    pub library: PathBuf,
    /// The mod id of every package the host was built from, in mod-id order.
    pub members: Vec<ModId>,
    /// The digest of the host inputs, including the pinned compiler identity.
    pub fingerprint: Sha256Digest,
}

/// Discovers an instance's packages and compiles a host that names them.
///
/// # Errors
///
/// Returns [`HostBuildRefused::Discovery`] when the instance's packages cannot be
/// established, and [`HostBuildRefused::Build`] when the host cannot be planned,
/// written or compiled.
pub fn build_host(
    game_directory: &Path,
    ravel_root: &Path,
    cache: &Path,
    minecraft: &str,
) -> Result<BuiltHost, HostBuildRefused> {
    let discovered = discover(game_directory).map_err(HostBuildRefused::Discovery)?;
    let planned = plan(&discovered, ravel_root).map_err(HostBuildRefused::Build)?;
    let library = ravel_build::build(&planned, game_directory, cache, minecraft)
        .map_err(HostBuildRefused::Build)?;

    Ok(BuiltHost {
        members: planned
            .members()
            .iter()
            .map(|member| member.mod_id.clone())
            .collect(),
        fingerprint: *planned.fingerprint(),
        library,
    })
}

/// A host build failure.
#[derive(Debug)]
pub enum HostBuildRefused {
    /// The instance's installed packages could not be established.
    Discovery(DiscoveryRefused),
    /// The host could not be planned, written or compiled.
    Build(BuildRefused),
}

impl fmt::Display for HostBuildRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovery(_) => formatter.write_str("host package discovery failed"),
            Self::Build(_) => formatter.write_str("host build failed"),
        }
    }
}

impl std::error::Error for HostBuildRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Discovery(refused) => Some(refused),
            Self::Build(refused) => Some(refused),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{HostBuildRefused, build_host};
    use std::error::Error;

    #[test]
    fn discovery_refusal_preserves_the_underlying_source_chain() {
        let error = HostBuildRefused::Discovery(ravel_loader::DiscoveryRefused::Unreadable {
            directory: "packages".to_owned(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        });
        let discovery = error.source().expect("discovery refusal is retained");
        assert_eq!(error.to_string(), "host package discovery failed");
        assert!(discovery.is::<ravel_loader::DiscoveryRefused>());
        let io = discovery.source().expect("I/O refusal is retained");
        assert_eq!(
            io.downcast_ref::<std::io::Error>()
                .expect("original I/O error")
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn a_refused_compile_carries_no_instance_path() {
        let root = std::env::temp_dir().join(format!("ravel-xtask-host-{}", std::process::id()));
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("remove stale temporary instance: {error}"),
        }
        fs::create_dir_all(&root).expect("the temporary instance is creatable");
        let installation = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("xtask must remain under tools/ravel-xtask");
        for file in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
            fs::copy(installation.join(file), root.join(file))
                .expect("installation input should be copied");
        }

        // Missing Ravel crates force refusal before version selection is read.
        let error = build_host(&root, &root, &root, "unread-by-a-refused-compile")
            .expect_err("no Ravel crates live under the temporary root, so the compile refuses");

        assert!(
            matches!(error, HostBuildRefused::Build(_)),
            "a compile that fails refuses as a build error, got {error}"
        );
        assert_eq!(error.to_string(), "host build failed");
        let cause = error.source().expect("build refusal is retained");
        assert!(cause.is::<ravel_build::BuildRefused>());
        let reported = format!("{error}: {cause}");
        assert!(
            !reported.contains(&root.display().to_string()),
            "the refusal names the instance directory: {reported}"
        );
        assert!(
            reported.contains("<instance>"),
            "the refusal dropped the path instead of redacting it: {reported}"
        );

        fs::remove_dir_all(&root).expect("remove temporary instance");
    }
}
