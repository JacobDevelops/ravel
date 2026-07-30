#![forbid(unsafe_code)]

//! Developer tooling for Ravel.

use std::env;
use std::ffi::OsString;
use std::fmt;
use std::os::unix::process::ExitStatusExt as _;
use std::path::PathBuf;
use std::process::{ExitCode, ExitStatus};
use std::time::Duration;

const DEFAULT_DESCRIPTOR: &str = "tests/fixtures/descriptors/atm11-0.2.1.json";
const DEFAULT_ARCHIVE: &str = "tests/fixtures/local/atm11-0.2.1/source/atm11-0.2.1.zip";

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if let Some(exit_code) = dispatch_linux_supervisor(&args) {
        return exit_code;
    }
    let root = match env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("cannot determine the workspace root: {error}");
            return ExitCode::FAILURE;
        }
    };
    let Some(result) = dispatch(&args, &root) else {
        print_usage();
        return ExitCode::from(2);
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("operation failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch_linux_supervisor(args: &[String]) -> Option<ExitCode> {
    let [
        performance,
        supervise,
        socket,
        separator,
        program,
        arguments @ ..,
    ] = args
    else {
        return None;
    };
    if performance != "performance" || supervise != "supervise-initial-linux" || separator != "--" {
        return None;
    }

    Some(
        match supervise_initial_linux(&PathBuf::from(socket), program, arguments) {
            Ok(status) if status.success() => {
                println!("Supervised Linux process completed its lifecycle markers.");
                ExitCode::SUCCESS
            }
            Ok(status) => supervised_exit_code(status),
            Err(error) => {
                eprintln!("operation failed: {error}");
                ExitCode::FAILURE
            }
        },
    )
}

fn supervised_exit_code(status: ExitStatus) -> ExitCode {
    let code = status.code().or_else(|| {
        status
            .signal()
            .and_then(|signal| 128_i32.checked_add(signal))
    });
    code.and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::FAILURE, ExitCode::from)
}

fn dispatch(args: &[String], root: &std::path::Path) -> Option<Result<(), CliError>> {
    dispatch_fixture(args, root).or_else(|| dispatch_performance(args, root))
}

fn dispatch_fixture(args: &[String], root: &std::path::Path) -> Option<Result<(), CliError>> {
    let result = match args {
        [fixture, reconstruct] if fixture == "fixture" && reconstruct == "reconstruct" => {
            ravel_cli::reconstruct_fixture(root, &root.join(DEFAULT_DESCRIPTOR))
                .map(|archive| {
                    println!("Verified fixture archive: {}", archive.display());
                })
                .map_err(CliError::Fixture)
        }
        [fixture, reconstruct, descriptor]
            if fixture == "fixture" && reconstruct == "reconstruct" =>
        {
            ravel_cli::reconstruct_fixture(root, &PathBuf::from(descriptor))
                .map(|archive| {
                    println!("Verified fixture archive: {}", archive.display());
                })
                .map_err(CliError::Fixture)
        }
        [fixture, verify] if fixture == "fixture" && verify == "verify" => {
            verify_archive(&root.join(DEFAULT_DESCRIPTOR), &root.join(DEFAULT_ARCHIVE))
        }
        [fixture, verify, descriptor, archive] if fixture == "fixture" && verify == "verify" => {
            verify_archive(&PathBuf::from(descriptor), &PathBuf::from(archive))
        }
        _ => return None,
    };
    Some(result)
}

fn dispatch_performance(args: &[String], root: &std::path::Path) -> Option<Result<(), CliError>> {
    let result = match args {
        [performance, verify, csv]
            if performance == "performance" && verify == "verify-tracy-frames" =>
        {
            verify_tracy_frames(&PathBuf::from(csv)).map_err(CliError::Performance)
        }
        [performance, export, trace, csv]
            if performance == "performance" && export == "export-tracy-frames" =>
        {
            export_tracy_frames(root, &PathBuf::from(trace), &PathBuf::from(csv))
                .map_err(CliError::Performance)
        }
        [performance, capture, seconds, trace]
            if performance == "performance" && capture == "capture-tracy" =>
        {
            capture_tracy(root, seconds, &PathBuf::from(trace)).map_err(CliError::Performance)
        }
        [performance, capture, pid, seconds, csv]
            if performance == "performance" && capture == "capture-linux-memory" =>
        {
            capture_linux_memory(root, pid, seconds, &PathBuf::from(csv))
                .map_err(CliError::Performance)
        }
        [performance, verify, csv]
            if performance == "performance" && verify == "verify-initial-markers" =>
        {
            verify_initial_markers(&PathBuf::from(csv)).map_err(CliError::Performance)
        }
        [performance, record, socket, csv]
            if performance == "performance" && record == "record-initial-markers" =>
        {
            record_initial_markers(root, &PathBuf::from(socket), &PathBuf::from(csv))
                .map_err(CliError::Performance)
        }
        [performance, record, socket, markers, memory]
            if performance == "performance" && record == "record-initial-linux" =>
        {
            record_initial_linux(
                root,
                &PathBuf::from(socket),
                &PathBuf::from(markers),
                &PathBuf::from(memory),
            )
            .map_err(CliError::Performance)
        }
        [performance, mark, socket, kind]
            if performance == "performance" && mark == "mark-initial" =>
        {
            mark_initial(&PathBuf::from(socket), kind, None).map_err(CliError::Performance)
        }
        [performance, mark, socket, kind, dimension]
            if performance == "performance" && mark == "mark-initial" =>
        {
            mark_initial(&PathBuf::from(socket), kind, Some(dimension))
                .map_err(CliError::Performance)
        }
        [performance, mark, socket, kind]
            if performance == "performance" && mark == "mark-initial-linux" =>
        {
            mark_initial_linux(&PathBuf::from(socket), kind, None).map_err(CliError::Performance)
        }
        [performance, mark, socket, kind, value]
            if performance == "performance" && mark == "mark-initial-linux" =>
        {
            mark_initial_linux(&PathBuf::from(socket), kind, Some(value))
                .map_err(CliError::Performance)
        }
        [
            performance,
            execute,
            socket,
            separator,
            program,
            arguments @ ..,
        ] if performance == "performance"
            && execute == "exec-initial-linux-child"
            && separator == "--" =>
        {
            exec_initial_linux_child(&PathBuf::from(socket), program, arguments)
                .map_err(CliError::Performance)
        }
        _ => return None,
    };
    Some(result)
}

fn print_usage() {
    eprintln!(
        "usage:\n  ravel-cli fixture reconstruct [descriptor.json]\n  \
         ravel-cli fixture verify [descriptor.json archive.zip]\n  \
         ravel-cli performance verify-tracy-frames tracy-frames.csv\n  \
         ravel-cli performance export-tracy-frames capture.tracy tracy-frames.csv\n  \
         ravel-cli performance capture-tracy seconds capture.tracy\n  \
         ravel-cli performance capture-linux-memory pid|self seconds linux-memory.csv\n  \
         ravel-cli performance verify-initial-markers markers.csv\n  \
         ravel-cli performance record-initial-markers socket markers.csv\n  \
         ravel-cli performance mark-initial socket kind [dimension]\n  \
         ravel-cli performance record-initial-linux socket markers.csv linux-memory.csv\n  \
         ravel-cli performance mark-initial-linux socket kind [pid|dimension]\n  \
         ravel-cli performance supervise-initial-linux socket -- java [args...]"
    );
}

fn verify_archive(descriptor: &std::path::Path, archive: &std::path::Path) -> Result<(), CliError> {
    let verified = ravel_cli::verify_fixture_archive(descriptor, archive)?;
    println!(
        "Verified {} references for Minecraft {} with {}.",
        verified.reference_count, verified.minecraft, verified.mod_loader
    );
    Ok(())
}

fn verify_tracy_frames(csv: &std::path::Path) -> Result<(), ravel_cli::PerformanceError> {
    let counts = ravel_cli::verify_tracy_frame_csv(csv)?;
    println!(
        "Verified {} client frames and {} integrated-server ticks.",
        counts.client_frames, counts.integrated_server_ticks
    );
    Ok(())
}

fn export_tracy_frames(
    root: &std::path::Path,
    trace: &std::path::Path,
    csv: &std::path::Path,
) -> Result<(), ravel_cli::PerformanceError> {
    ravel_cli::validate_private_evidence_output(root, csv)?;
    let counts = ravel_cli::export_tracy_frame_csv(trace, csv)?;
    println!(
        "Exported and verified {} client frames and {} integrated-server ticks: {}",
        counts.client_frames,
        counts.integrated_server_ticks,
        csv.display()
    );
    Ok(())
}

fn capture_tracy(
    root: &std::path::Path,
    seconds: &str,
    trace: &std::path::Path,
) -> Result<(), ravel_cli::PerformanceError> {
    let duration = seconds.parse::<u64>().map_err(|error| {
        ravel_cli::PerformanceError::TracyTool(format!(
            "invalid Tracy capture duration `{seconds}`: {error}"
        ))
    })?;
    ravel_cli::validate_private_evidence_output(root, trace)?;
    ravel_cli::capture_tracy(duration, trace)?;
    println!(
        "Captured Tracy trace without desktop input: {}",
        trace.display()
    );
    Ok(())
}

fn capture_linux_memory(
    root: &std::path::Path,
    pid: &str,
    seconds: &str,
    csv: &std::path::Path,
) -> Result<(), ravel_cli::PerformanceError> {
    let pid = if pid == "self" {
        std::process::id()
    } else {
        pid.parse::<u32>().map_err(|error| {
            ravel_cli::PerformanceError::InvalidLinuxProc(format!(
                "invalid Linux process ID `{pid}`: {error}"
            ))
        })?
    };
    let sample_count = seconds.parse::<usize>().map_err(|error| {
        ravel_cli::PerformanceError::InvalidLinuxProc(format!(
            "invalid Linux capture duration `{seconds}`: {error}"
        ))
    })?;
    ravel_cli::validate_private_evidence_output(root, csv)?;
    let page_size = ravel_cli::query_linux_page_size()?;
    let captured = ravel_cli::capture_linux_memory_csv(
        std::path::Path::new("/proc"),
        pid,
        page_size,
        Duration::from_secs(1),
        sample_count,
        csv,
    )?;
    println!(
        "Captured {captured} Linux memory samples without desktop input: {}",
        csv.display()
    );
    Ok(())
}

fn verify_initial_markers(csv: &std::path::Path) -> Result<(), ravel_cli::PerformanceError> {
    let count = ravel_cli::verify_initial_performance_marker_csv(csv)?;
    println!("Verified {count} initial-v2 lifecycle markers.");
    Ok(())
}

fn record_initial_markers(
    root: &std::path::Path,
    socket: &std::path::Path,
    csv: &std::path::Path,
) -> Result<(), ravel_cli::PerformanceError> {
    ravel_cli::validate_private_evidence_output(root, socket)?;
    ravel_cli::validate_private_evidence_output(root, csv)?;
    let count = ravel_cli::record_initial_performance_markers(socket, csv)?;
    println!(
        "Recorded {count} initial-v2 lifecycle markers without desktop input: {}",
        csv.display()
    );
    Ok(())
}

fn mark_initial(
    socket: &std::path::Path,
    kind: &str,
    dimension: Option<&str>,
) -> Result<(), ravel_cli::PerformanceError> {
    let command = ravel_cli::parse_initial_performance_marker_command(kind, dimension)?;
    let complete = ravel_cli::send_initial_performance_marker(socket, &command)?;
    if complete {
        println!("Recorded final initial-v2 lifecycle marker.");
    } else {
        println!("Recorded initial-v2 lifecycle marker.");
    }
    Ok(())
}

fn record_initial_linux(
    root: &std::path::Path,
    socket: &std::path::Path,
    markers: &std::path::Path,
    memory: &std::path::Path,
) -> Result<(), ravel_cli::PerformanceError> {
    ravel_cli::validate_private_evidence_output(root, socket)?;
    ravel_cli::validate_private_evidence_output(root, markers)?;
    ravel_cli::validate_private_evidence_output(root, memory)?;
    let page_size = ravel_cli::query_linux_page_size()?;
    let counts = ravel_cli::record_initial_linux_performance(
        socket,
        markers,
        memory,
        std::path::Path::new("/proc"),
        page_size,
        Duration::from_secs(1),
    )?;
    println!(
        "Recorded {} lifecycle markers and {} Linux memory samples on one clock without \
         desktop input.",
        counts.lifecycle_markers, counts.linux_memory_samples
    );
    Ok(())
}

fn mark_initial_linux(
    socket: &std::path::Path,
    kind: &str,
    value: Option<&str>,
) -> Result<(), ravel_cli::PerformanceError> {
    let command = ravel_cli::parse_initial_linux_performance_marker_command(kind, value)?;
    let complete = ravel_cli::send_initial_linux_performance_marker(socket, &command)?;
    if complete {
        println!("Recorded final combined Linux lifecycle marker.");
    } else {
        println!("Recorded combined Linux lifecycle marker.");
    }
    Ok(())
}

fn supervise_initial_linux(
    socket: &std::path::Path,
    program: &str,
    arguments: &[String],
) -> Result<ExitStatus, ravel_cli::PerformanceError> {
    let arguments = arguments.iter().map(OsString::from).collect::<Vec<_>>();
    ravel_cli::supervise_initial_linux_process(socket, program.as_ref(), &arguments)
}

fn exec_initial_linux_child(
    socket: &std::path::Path,
    program: &str,
    arguments: &[String],
) -> Result<(), ravel_cli::PerformanceError> {
    let arguments = arguments.iter().map(OsString::from).collect::<Vec<_>>();
    ravel_cli::exec_initial_linux_child(socket, program.as_ref(), &arguments)
}

#[derive(Debug)]
enum CliError {
    Fixture(ravel_cli::FixtureError),
    Performance(ravel_cli::PerformanceError),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixture(error) => error.fmt(formatter),
            Self::Performance(error) => error.fmt(formatter),
        }
    }
}

impl From<ravel_cli::FixtureError> for CliError {
    fn from(error: ravel_cli::FixtureError) -> Self {
        Self::Fixture(error)
    }
}
