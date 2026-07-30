#![forbid(unsafe_code)]

use std::env;
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
        _ => {
            eprintln!(
                "usage:\n  cargo xtask verify scaffold\n  cargo xtask verify spikes\n  \
                 cargo xtask verify milestone M<N>"
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
