use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

const DEFAULT_DESCRIPTOR: &str = "tests/fixtures/descriptors/atm10-tts-2.0.2.json";
const DEFAULT_ARCHIVE: &str = "tests/fixtures/local/atm10-tts-2.0.2/source/atm10-tts-2.0.2.zip";

pub fn dispatch(args: &[String], root: &Path) -> Option<ExitCode> {
    let result = match args {
        [host, build, instance, cache, minecraft] if host == "host" && build == "build" => {
            build_host(Path::new(instance), root, Path::new(cache), minecraft)
        }
        [fixture, reconstruct] if fixture == "fixture" && reconstruct == "reconstruct" => {
            reconstruct_fixture(root, &root.join(DEFAULT_DESCRIPTOR))
        }
        [fixture, reconstruct, descriptor]
            if fixture == "fixture" && reconstruct == "reconstruct" =>
        {
            reconstruct_fixture(root, Path::new(descriptor))
        }
        [fixture, verify] if fixture == "fixture" && verify == "verify" => {
            verify_archive(&root.join(DEFAULT_DESCRIPTOR), &root.join(DEFAULT_ARCHIVE))
        }
        [fixture, verify, descriptor, archive] if fixture == "fixture" && verify == "verify" => {
            verify_archive(Path::new(descriptor), Path::new(archive))
        }
        _ => return None,
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("operation failed: {error}");
            ExitCode::FAILURE
        }
    })
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
        "Verified {} references for Minecraft {} with {}.",
        verified.reference_count, verified.minecraft, verified.mod_loader
    );
    Ok(())
}
