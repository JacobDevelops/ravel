#![forbid(unsafe_code)]

use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let root = env::current_dir().expect("current directory must be available");
    let (subject, violations) = match args.as_slice() {
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
        _ => {
            eprintln!(
                "usage:\n  cargo xtask verify scaffold\n  cargo xtask verify spikes\n  \
                 cargo xtask verify milestone M<N>\n  \
                 cargo xtask verify budget <run.json>..."
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
