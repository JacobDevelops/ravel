use std::env;
use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

use ravel_minecraft_generate as generate;

const DEFAULT_DESCRIPTOR: &str = "tests/fixtures/descriptors/sevtech-ages-3.2.3.json";
const DEFAULT_ARCHIVE: &str =
    "tests/fixtures/local/sevtech-ages-3.2.3/source/SevTech_Ages_3.2.3.zip";
const USAGE: &str = "usage:
  cargo xtask verify scaffold
  cargo xtask verify spikes
  cargo xtask verify gate-versions
  cargo xtask version-matrix <minecraft>
  cargo xtask generate java
  cargo xtask generate locks
  cargo xtask verify milestone M<N>
  cargo xtask host build <instance> <cache> <minecraft>
  cargo xtask fixture reconstruct [descriptor]
  cargo xtask fixture verify [descriptor archive]
  cargo xtask scan instance <instance-directory>
  cargo xtask package digests [--check] <package-directory>";

pub(crate) fn dispatch(args: &[String], root: &Path) -> ExitCode {
    let (subject, violations) = match args {
        [matrix, version] if matrix == "version-matrix" => {
            return matrix_eligibility(root, version);
        }
        [host, build, instance, cache, minecraft] if host == "host" && build == "build" => {
            return operation(build_host(
                Path::new(instance),
                root,
                Path::new(cache),
                minecraft,
            ));
        }
        [fixture, reconstruct] if fixture == "fixture" && reconstruct == "reconstruct" => {
            return operation(reconstruct_fixture(root, &root.join(DEFAULT_DESCRIPTOR)));
        }
        [fixture, reconstruct, descriptor]
            if fixture == "fixture" && reconstruct == "reconstruct" =>
        {
            return operation(reconstruct_fixture(root, Path::new(descriptor)));
        }
        [fixture, verify] if fixture == "fixture" && verify == "verify" => {
            return operation(verify_archive(
                &root.join(DEFAULT_DESCRIPTOR),
                &root.join(DEFAULT_ARCHIVE),
            ));
        }
        [fixture, verify, descriptor, archive] if fixture == "fixture" && verify == "verify" => {
            return operation(verify_archive(Path::new(descriptor), Path::new(archive)));
        }
        [generate, java] if generate == "generate" && java == "java" => {
            return generate_sources(root, false);
        }
        [generate, locks] if generate == "generate" && locks == "locks" => {
            return generate_sources(root, true);
        }
        [package, digests, directory] if package == "package" && digests == "digests" => {
            return package_digests(&root.join(directory), false);
        }
        [package, digests, flag, directory] | [package, digests, directory, flag]
            if package == "package" && digests == "digests" && flag == "--check" =>
        {
            return package_digests(&root.join(directory), true);
        }
        [verify, scaffold] if verify == "verify" && scaffold == "scaffold" => {
            ("scaffold", ravel_xtask::verify_scaffold(root))
        }
        [verify, milestone, id] if verify == "verify" && milestone == "milestone" => {
            ("milestone", ravel_xtask::verify_milestone(root, id))
        }
        [verify, spikes] if verify == "verify" && spikes == "spikes" => {
            ("spikes", ravel_xtask::verify_spikes(root))
        }
        [verify, versions] if verify == "verify" && versions == "gate-versions" => {
            let report = ravel_xtask::verify_gate_versions(root);
            for notice in report.notices {
                eprintln!("{notice}");
            }
            ("gate versions", report.violations)
        }
        [scan, instance, directory] if scan == "scan" && instance == "instance" => {
            ("instance scan", scan_instance(Path::new(directory)))
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    verification(subject, violations)
}

fn matrix_eligibility(root: &Path, version: &str) -> ExitCode {
    match ravel_xtask::version_matrix(root, version) {
        Ok(eligibility) => {
            println!("{eligibility}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn verification(subject: &str, violations: Vec<String>) -> ExitCode {
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

fn operation(result: Result<(), Box<dyn Error>>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("operation failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn build_host(
    instance: &Path,
    root: &Path,
    cache: &Path,
    minecraft: &str,
) -> Result<(), Box<dyn Error>> {
    let built = ravel_xtask::host::build_host(instance, root, cache, minecraft)?;
    println!(
        "Built a host for {} package(s) at fingerprint {}: {}",
        built.members.len(),
        built.fingerprint,
        built.library.display()
    );
    Ok(())
}

fn reconstruct_fixture(root: &Path, descriptor: &Path) -> Result<(), Box<dyn Error>> {
    let archive = ravel_xtask::fixture::reconstruct_fixture(root, descriptor)?;
    println!("Verified fixture archive: {}", archive.display());
    Ok(())
}

fn verify_archive(descriptor: &Path, archive: &Path) -> Result<(), Box<dyn Error>> {
    let verified = ravel_xtask::fixture::verify_fixture_archive(descriptor, archive)?;
    println!(
        "Verified {} publisher references and {} bundled override jars for Minecraft {} with {}; \
         loaded container count is unknown.",
        verified.reference_count,
        verified.override_jar_count,
        verified.minecraft,
        verified.mod_loader
    );
    Ok(())
}

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

fn generate_sources(root: &Path, locks: bool) -> ExitCode {
    let result = generate::selection(env::var_os(generate::SELECTION)).and_then(|selected| {
        if locks {
            generate::regenerate_locks(root, selected.as_deref())
        } else {
            generate::generate_java(root, selected.as_deref()).map(|_| ())
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Generation failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn scan_instance(directory: &Path) -> Vec<String> {
    let report = ravel_xtask::scan_instance(directory);
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
    violations
}

#[cfg(test)]
mod tests {
    use super::dispatch;
    use std::path::Path;
    use std::process::ExitCode;

    #[test]
    fn unknown_and_incomplete_commands_return_usage_status() {
        for args in [
            vec![],
            vec!["verify"],
            vec!["generate", "unknown"],
            vec!["host", "build"],
            vec!["fixture", "verify", "descriptor"],
        ] {
            let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(dispatch(&args, Path::new(".")), ExitCode::from(2));
        }
    }
}
