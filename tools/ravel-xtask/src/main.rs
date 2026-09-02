#![forbid(unsafe_code)]

use std::env;
use std::path::Path;
use std::process::ExitCode;

/// Refreshes the digests a package manifest records, or checks them.
///
/// This one does not fit the `(subject, violations)` shape the rest share.
/// Rewriting a manifest is not a verification, and its ordinary successful
/// outcome — digests were wrong and are now right — has to print what changed
/// and still exit 0, because an author ran it precisely to make that change.
fn package_digests(directory: &Path, check: bool) -> ExitCode {
    match ravel_xtask::refresh_digests(directory, check) {
        Err(refusals) => {
            eprintln!("Ravel package digests failed:");
            for refusal in refusals {
                eprintln!("- {refusal}");
            }
            ExitCode::FAILURE
        }
        Ok(ravel_xtask::DigestOutcome::Current) => {
            println!("Every digest the manifest records is the digest of the file on disk.");
            ExitCode::SUCCESS
        }
        Ok(ravel_xtask::DigestOutcome::Stale(stale)) => {
            if check {
                eprintln!("Ravel package digests are stale:");
                for entry in stale {
                    eprintln!("- {entry}");
                }
                eprintln!("Run `cargo xtask package digests <package directory>` to refresh them.");
                return ExitCode::FAILURE;
            }
            println!("Refreshed the digest of:");
            for entry in stale {
                println!("- {}", entry.path);
            }
            ExitCode::SUCCESS
        }
    }
}

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let root = env::current_dir().expect("current directory must be available");
    let (subject, violations) = match args.as_slice() {
        [package, digests, directory] if package == "package" && digests == "digests" => {
            return package_digests(&root.join(directory), false);
        }
        [package, digests, flag, directory] | [package, digests, directory, flag]
            if package == "package" && digests == "digests" && flag == "--check" =>
        {
            return package_digests(&root.join(directory), true);
        }
        [verify, scaffold] if verify == "verify" && scaffold == "scaffold" => {
            ("scaffold", ravel_xtask::verify_scaffold(&root))
        }
        [verify, milestone, milestone_id] if verify == "verify" && milestone == "milestone" => (
            "milestone",
            ravel_xtask::verify_milestone(&root, milestone_id),
        ),
        [verify, spikes] if verify == "verify" && spikes == "spikes" => {
            ("spikes", ravel_xtask::verify_spikes(&root))
        }
        [verify, versions] if verify == "verify" && versions == "gate-versions" => {
            ("gate versions", ravel_xtask::verify_gate_versions(&root))
        }
        [verify, budget, runs @ ..]
            if verify == "verify" && budget == "budget" && !runs.is_empty() =>
        {
            let runs = runs.iter().map(Path::new).collect::<Vec<_>>();
            let report = ravel_xtask::verify_budget(&root, &runs);
            // Notes say what was not compared, and they print whether or not the
            // comparison passed. A limit that is quietly not checked reads
            // exactly like one that passed, which is the failure this command
            // exists to end.
            for note in &report.notes {
                println!("- {note}");
            }
            ("budget", report.violations)
        }
        // The report prints whether or not the instance is clean, because the
        // sanitized facts are what an evidence record embeds and a scan that
        // only spoke up on failure could not supply them.
        [scan, instance, directory] if scan == "scan" && instance == "instance" => {
            let report = ravel_xtask::scan_instance(Path::new(directory));
            println!("{}", report.to_json());
            let mut violations = report
                .findings
                .iter()
                .map(|finding| {
                    format!(
                        "{} is present: {} ({})",
                        finding.loader, finding.marker, finding.kind
                    )
                })
                .collect::<Vec<_>>();
            violations.extend(
                report
                    .obstacles
                    .iter()
                    .map(|obstacle| (*obstacle).to_owned()),
            );
            if report.mod_archives > 0 {
                violations.push(format!(
                    "the game directory holds mod archives ({}), which only another loader reads",
                    report.mod_archives
                ));
            }
            ("instance scan", violations)
        }
        _ => {
            eprintln!(
                "usage:\n  cargo xtask verify scaffold\n  cargo xtask verify spikes\n  \
                 cargo xtask verify gate-versions\n  \
                 cargo xtask verify milestone M<N>\n  \
                 cargo xtask verify budget <run.json>...\n  \
                 cargo xtask scan instance <instance-directory>\n  \
                 cargo xtask package digests [--check] <package-directory>"
            );
            return ExitCode::from(2);
        }
    };

    if violations.is_empty() {
        println!("Ravel {subject} verification passed.");
        ExitCode::SUCCESS
    } else {
        eprintln!("Ravel {subject} verification failed:");
        for violation in violations {
            eprintln!("- {violation}");
        }
        ExitCode::FAILURE
    }
}
