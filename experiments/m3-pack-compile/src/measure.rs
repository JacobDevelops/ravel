//! Runs the builds and watches what they cost while they run.
//!
//! Three things are observed per build: wall-clock time on the monotonic clock,
//! the resident set of the whole cargo process tree sampled while it compiles,
//! and what the build left on disk. The memory figure is a sampled sum rather
//! than a kernel high-water mark, because `getrusage` reports the largest
//! single descendant and the question here is what a build needs at once.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use ravel_spike_harness as harness;

/// How often the sampler walks the process tree.
///
/// Fast enough that a sub-second rebuild still yields samples, slow enough that
/// reading `/proc` is not itself part of what is being measured.
const SAMPLE_INTERVAL: Duration = Duration::from_millis(25);

/// What one `cargo build` cost.
#[derive(Clone, Copy, Debug)]
pub struct BuildRun {
    /// Wall-clock milliseconds.
    pub milliseconds: u64,
    /// Largest summed resident set across the cargo process tree.
    pub peak_tree_rss_bytes: u64,
    /// Largest resident set any single process in that tree reached.
    pub peak_process_rss_bytes: u64,
    /// Samples the two peaks were taken from.
    pub samples: u64,
}

/// Discards the build directory and builds, so nothing is cached.
///
/// # Errors
///
/// Returns a message when the cache cannot be cleared or cargo refuses the
/// generated workspace.
pub fn cold(workspace: &Path, jobs: Option<usize>) -> Result<BuildRun, String> {
    let target = workspace.join("target");
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|error| format!("cannot clear a generated build directory: {error}"))?;
    }
    build(workspace, jobs)
}

/// Changes one mod's source and rebuilds, which is what adding or updating a
/// single mod costs against a warm cache.
///
/// # Errors
///
/// Returns a message when the source cannot be edited or the rebuild fails.
pub fn warm(workspace: &Path, source: &Path, jobs: Option<usize>) -> Result<BuildRun, String> {
    let mut text = fs::read_to_string(source)
        .map_err(|error| format!("cannot read a generated mod: {error}"))?;
    text.push_str("\n// edited\n");
    fs::write(source, text).map_err(|error| format!("cannot edit a generated mod: {error}"))?;
    build(workspace, jobs)
}

/// Runs one release build with the sampler attached.
fn build(workspace: &Path, jobs: Option<usize>) -> Result<BuildRun, String> {
    let mut arguments = vec![
        "build".to_owned(),
        "--release".to_owned(),
        "--quiet".to_owned(),
    ];
    if let Some(jobs) = jobs {
        arguments.push("--jobs".to_owned());
        arguments.push(jobs.to_string());
    }
    let started = Instant::now();
    let mut child = Command::new("cargo")
        .args(&arguments)
        .current_dir(workspace)
        .spawn()
        .map_err(|error| format!("cannot run cargo: {error}"))?;

    let running = Arc::new(AtomicBool::new(true));
    let sampler = spawn_sampler(child.id(), &running);

    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for cargo: {error}"))?;
    let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    running.store(false, Ordering::Relaxed);
    let observed = sampler
        .join()
        .map_err(|_| "the memory sampler panicked".to_owned())?;

    if !status.success() {
        return Err(format!("the generated pack did not build: cargo {status}"));
    }
    Ok(BuildRun {
        milliseconds,
        peak_tree_rss_bytes: observed.peak_tree,
        peak_process_rss_bytes: observed.peak_process,
        samples: observed.samples,
    })
}

/// What the sampler saw over one build.
struct Observed {
    peak_tree: u64,
    peak_process: u64,
    samples: u64,
}

/// Watches the resident set of `root_pid` and everything descended from it.
fn spawn_sampler(root_pid: u32, running: &Arc<AtomicBool>) -> thread::JoinHandle<Observed> {
    let running = Arc::clone(running);
    thread::spawn(move || {
        let proc_root = Path::new("/proc");
        let mut observed = Observed {
            peak_tree: 0,
            peak_process: 0,
            samples: 0,
        };
        while running.load(Ordering::Relaxed) {
            let (tree, largest) = sample(proc_root, root_pid);
            // A sample that found nothing means the tree was between processes,
            // not that it used no memory, so it is not counted.
            if tree > 0 {
                observed.peak_tree = observed.peak_tree.max(tree);
                observed.peak_process = observed.peak_process.max(largest);
                observed.samples = observed.samples.saturating_add(1);
            }
            thread::sleep(SAMPLE_INTERVAL);
        }
        observed
    })
}

/// Sums the resident set of one process tree, and notes its largest member.
fn sample(proc_root: &Path, root_pid: u32) -> (u64, u64) {
    let children = children_by_parent(proc_root);
    let mut pending = vec![root_pid];
    let mut tree = 0u64;
    let mut largest = 0u64;
    while let Some(pid) = pending.pop() {
        if let Ok(snapshot) = harness::linux_rss(proc_root, pid) {
            tree = tree.saturating_add(snapshot.current_bytes);
            largest = largest.max(snapshot.current_bytes);
        }
        if let Some(descendants) = children.get(&pid) {
            pending.extend(descendants);
        }
    }
    (tree, largest)
}

/// Reads every live process's parent, so a tree can be walked downward.
fn children_by_parent(proc_root: &Path) -> BTreeMap<u32, Vec<u32>> {
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(proc_root) else {
        return children;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some(parent) = parent_of(&stat) {
            children.entry(parent).or_default().push(pid);
        }
    }
    children
}

/// Extracts the parent pid from a `/proc/<pid>/stat` line.
///
/// The command name is the second field, is parenthesised, and may itself
/// contain spaces and parentheses, so the fields after it are found from the
/// last closing parenthesis rather than by splitting the whole line.
fn parent_of(stat: &str) -> Option<u32> {
    let tail = stat.rsplit_once(')')?.1;
    tail.split_whitespace().nth(1)?.parse().ok()
}

/// The wall-clock milliseconds of a set of runs.
#[must_use]
pub fn samples(runs: &[BuildRun]) -> Vec<u64> {
    runs.iter().map(|run| run.milliseconds).collect()
}

/// The median wall-clock time of a set of runs.
#[must_use]
pub fn median_ms(runs: &[BuildRun]) -> Option<u64> {
    if runs.is_empty() {
        return None;
    }
    let mut times = samples(runs);
    times.sort_unstable();
    Some(times[times.len() / 2])
}

/// The fastest of a set of runs, which is the closest estimate of the cost.
///
/// Build noise only ever adds time, so with few samples the minimum says more
/// than the mean. Used where a figure has two runs behind it rather than three.
#[must_use]
pub fn fastest_ms(runs: &[BuildRun]) -> Option<u64> {
    runs.iter().map(|run| run.milliseconds).min()
}

/// The largest summed process-tree resident set any of the runs reached.
#[must_use]
pub fn peak_tree_rss(runs: &[BuildRun]) -> Option<u64> {
    runs.iter().map(|run| run.peak_tree_rss_bytes).max()
}

/// The largest single-process resident set any of the runs reached.
#[must_use]
pub fn peak_process_rss(runs: &[BuildRun]) -> Option<u64> {
    runs.iter().map(|run| run.peak_process_rss_bytes).max()
}

/// Sums the apparent size of every file under `path`.
///
/// Apparent size, not block usage: the toolchain lives in a store that
/// hard-links identical files, and a sum counts those once per path. The record
/// says so.
#[must_use]
pub fn directory_bytes(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.path().symlink_metadata() else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    total
}

/// Size of one file, or zero when it is missing.
#[must_use]
pub fn file_bytes(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

/// The Rust toolchain a compile-on-install mechanism would have to ship.
///
/// # Errors
///
/// Returns a message when `rustc` cannot report its sysroot.
pub fn toolchain_root() -> Result<PathBuf, String> {
    let output = Command::new("rustc")
        .arg("--print=sysroot")
        .output()
        .map_err(|error| format!("cannot run rustc: {error}"))?;
    if !output.status.success() {
        return Err("rustc would not report its sysroot".to_owned());
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{directory_bytes, file_bytes, parent_of};

    #[test]
    fn a_parent_pid_survives_a_command_name_full_of_punctuation() {
        let stat = "41 (rust c) (odd) R 7 41 41 0 -1 4194304 100 0";

        assert_eq!(parent_of(stat), Some(7));
    }

    #[test]
    fn a_stat_line_without_a_command_name_yields_no_parent() {
        assert_eq!(parent_of("41 rustc R 7"), None);
    }

    #[test]
    fn a_directory_is_the_sum_of_the_files_under_it() {
        let root = std::env::temp_dir().join(format!("ravel-m3-sizes-{}", std::process::id()));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("temporary tree should be creatable");
        fs::write(root.join("a"), b"1234").expect("file should be writable");
        fs::write(nested.join("b"), b"12345").expect("file should be writable");

        assert_eq!(directory_bytes(&root), 9);
        assert_eq!(file_bytes(&root.join("a")), 4);
        assert_eq!(file_bytes(&root.join("absent")), 0);
        assert_eq!(directory_bytes(&root.join("absent")), 0);

        fs::remove_dir_all(root).expect("temporary tree should be removable");
    }
}
