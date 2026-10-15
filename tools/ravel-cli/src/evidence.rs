//! Private raw-evidence destination policy and partial-output cleanup.

use std::fs;
use std::io;
use std::path::{Component, Path};

use crate::error::PerformanceError;

/// Verifies that raw evidence written inside the workspace uses a repository
/// ignore boundary.
/// External paths are accepted; relative paths start at the workspace root.
///
/// # Errors
///
/// Returns an error when a workspace-local destination is tracked or escapes
/// through a relative traversal.
pub fn validate_private_evidence_output(
    workspace_root: &Path,
    destination: &Path,
) -> Result<(), PerformanceError> {
    let relative = if destination.is_absolute() {
        let Ok(relative) = destination.strip_prefix(workspace_root) else {
            return Ok(());
        };
        relative
    } else {
        destination
    };
    let components = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => Err(PerformanceError::TracyTool(format!(
                "raw evidence destination must not traverse the workspace: {}",
                destination.display()
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let ignored_fixture = components.starts_with(&[
        std::ffi::OsStr::new("tests"),
        std::ffi::OsStr::new("fixtures"),
        std::ffi::OsStr::new("local"),
    ]) && components.len() > 3;
    let private_evidence = components.starts_with(&[
        std::ffi::OsStr::new("evidence"),
        std::ffi::OsStr::new("private"),
    ]) && components.len() > 2;
    let raw_evidence = components.first() == Some(&std::ffi::OsStr::new("evidence"))
        && components
            .iter()
            .take(components.len().saturating_sub(1))
            .any(|component| {
                *component == std::ffi::OsStr::new("raw")
                    || *component == std::ffi::OsStr::new("captures")
            });
    if ignored_fixture || private_evidence || raw_evidence {
        return Ok(());
    }

    Err(PerformanceError::TracyTool(format!(
        "raw evidence inside the workspace must use an ignored private boundary: {}",
        destination.display()
    )))
}

pub(crate) fn remove_partial_performance_output(path: &Path, label: &str) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        eprintln!(
            "warning: cannot remove partial {label} {}: {error}",
            path.display()
        );
    }
}
