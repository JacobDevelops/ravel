#![forbid(unsafe_code)]

use std::env;
use std::path::Path;
use std::process::ExitCode;

mod commands;

/// Reports changed digests while treating a successful rewrite as success.
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
    let root = match env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("operation failed: cannot read current directory: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(result) = commands::dispatch(&args, &root) {
        return result;
    }
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
        // Emit sanitized evidence on both success and failure.
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
                 cargo xtask host build <instance> <cache> <minecraft>\n  \
                 cargo xtask fixture reconstruct [descriptor]\n  \
                 cargo xtask fixture verify [descriptor archive]\n  \
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
