//! Repository inventories and pinned bytes for jj workspaces and plain Git checkouts.

use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy)]
enum Backend {
    Jj,
    Git,
}

impl Backend {
    fn at(root: &Path) -> Result<Self, String> {
        if root.join(".jj").exists() {
            return Ok(Self::Jj);
        }
        let bytes = Self::Git.run(root, &["rev-parse", "--show-toplevel"])?;
        let toplevel = String::from_utf8(bytes).map_err(|error| error.to_string())?;
        let actual = Path::new(toplevel.trim_end_matches('\n'))
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if actual != root.canonicalize().map_err(|error| error.to_string())? {
            return Err("supplied root is not the Git repository root".to_owned());
        }
        Ok(Self::Git)
    }

    fn run(self, root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
        let program = match self {
            Self::Jj => "jj",
            Self::Git => "git",
        };
        let output = Command::new(program)
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot run {program}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "{program}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(output.stdout)
    }
}

pub(crate) fn tracked_files(root: &Path, revision: Option<&str>) -> Result<Vec<String>, String> {
    let backend =
        Backend::at(root).map_err(|error| format!("cannot list tracked files: {error}"))?;
    let bytes = match (backend, revision) {
        (Backend::Jj, Some(revision)) => backend.run(
            root,
            &["file", "list", "-r", revision, "-T", "path ++ \"\\0\""],
        ),
        (Backend::Jj, None) => backend.run(root, &["file", "list", "-T", "path ++ \"\\0\""]),
        (Backend::Git, Some(revision)) => backend.run(
            root,
            &["ls-tree", "-r", "--name-only", "-z", revision, "--"],
        ),
        (Backend::Git, None) => backend.run(root, &["ls-files", "-z", "--cached", "--deduplicate"]),
    }
    .map_err(|error| format!("cannot list tracked files: {error}"))?;
    Ok(String::from_utf8_lossy(&bytes)
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

pub(crate) fn resolve_revision(root: &Path, revision: &str) -> Result<String, String> {
    let backend = Backend::at(root)?;
    let bytes = match backend {
        Backend::Jj => backend.run(
            root,
            &[
                "log",
                "--color",
                "never",
                "-r",
                &format!("(::visible_heads() & ({revision})) ~ root()"),
                "--no-graph",
                "-T",
                r#"commit_id ++ "\n""#,
            ],
        )?,
        Backend::Git => {
            let resolved = backend.run(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{revision}^{{commit}}"),
                ],
            )?;
            let commit = String::from_utf8_lossy(&resolved);
            let visible = backend.run(root, &["rev-list", "--all", "HEAD"])?;
            if !String::from_utf8_lossy(&visible)
                .lines()
                .any(|line| line == commit.trim())
            {
                return Err("names no commit in visible history".to_owned());
            }
            resolved
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut commits = text.lines().filter(|line| !line.trim().is_empty());
    let commit = commits.next().ok_or("names no commit in visible history")?;
    if commits.next().is_some() {
        return Err("names more than one visible commit".to_owned());
    }
    Ok(commit.to_owned())
}

pub(crate) fn revision_bytes(root: &Path, revision: &str, path: &str) -> Result<Vec<u8>, String> {
    let backend = Backend::at(root)?;
    match backend {
        Backend::Jj => backend.run(
            root,
            &["file", "show", "-r", revision, "--", &literal_jj_path(path)],
        ),
        Backend::Git => backend.run(root, &["show", &format!("{revision}:{path}")]),
    }
    .map_err(|error| format!("cannot read it at revision {revision}: {error}"))
}

pub(crate) fn last_modifying_revision(root: &Path, path: &str) -> Result<String, String> {
    let backend = Backend::at(root)?;
    require_complete_history(root, backend)?;
    let bytes = match backend {
        Backend::Jj => backend.run(
            root,
            &[
                "log",
                "--no-graph",
                "--color",
                "never",
                "-r",
                &format!("latest(::@ & files({}))", literal_jj_path(path)),
                "-T",
                "commit_id",
            ],
        ),
        Backend::Git => backend.run(
            root,
            &[
                "--literal-pathspecs",
                "log",
                "-1",
                "--format=%H",
                "HEAD",
                "--",
                path,
            ],
        ),
    }?;
    let revision = String::from_utf8_lossy(&bytes).trim().to_owned();
    if revision.is_empty() {
        return Err(format!("cannot resolve the last revision modifying {path}"));
    }
    Ok(revision)
}

fn literal_jj_path(path: &str) -> String {
    use std::fmt::Write as _;

    let mut literal = String::from("root-file:\"");
    for character in path.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            control if control.is_ascii_control() => {
                write!(&mut literal, "\\x{:02x}", u32::from(control))
                    .expect("writing to String cannot fail");
            }
            other => literal.push(other),
        }
    }
    literal.push('"');
    literal
}

fn require_complete_history(root: &Path, backend: Backend) -> Result<(), String> {
    let bytes = match backend {
        Backend::Git => backend.run(root, &["rev-parse", "--is-shallow-repository"])?,
        Backend::Jj => {
            let directory = backend.run(root, &["git", "root"])?;
            let directory = String::from_utf8(directory).map_err(|error| error.to_string())?;
            Backend::Git.run(
                root,
                &[
                    "--git-dir",
                    directory.trim_end_matches('\n'),
                    "rev-parse",
                    "--is-shallow-repository",
                ],
            )?
        }
    };
    if bytes.starts_with(b"true") {
        return Err("cannot resolve modifying revision from shallow history".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
