use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

pub(super) fn ravel_cli_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ravel-cli"));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        let profile = PathBuf::from(profile);
        let file_name = profile
            .file_name()
            .expect("coverage profile should have a filename")
            .to_string_lossy();
        command.env(
            "LLVM_PROFILE_FILE",
            profile.with_file_name(format!("child-%p-{file_name}")),
        );
    }
    command
}

pub(super) fn temporary_root(label: &str) -> PathBuf {
    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    loop {
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ravel-cli-{}-{sequence}-{label}",
            std::process::id()
        ));
        match fs::DirBuilder::new().mode(0o700).create(&root) {
            Ok(()) => return root,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("temporary root should be creatable: {error}"),
        }
    }
}

pub(super) fn write_proc_snapshot(proc_root: &Path, pid: u32) {
    fs::create_dir_all(proc_root.join(pid.to_string()))
        .expect("synthetic process directory should be creatable");
    fs::create_dir_all(proc_root.join("pressure"))
        .expect("synthetic pressure directory should be creatable");
    fs::write(
        proc_root.join(pid.to_string()).join("smaps_rollup"),
        "00400000-00452000 r--p 00000000 00:00 0 [rollup]\n\
         Rss:                3072 kB\n\
         Pss:                1536 kB\n",
    )
    .expect("synthetic smaps should be writable");
    fs::write(
        proc_root.join(pid.to_string()).join("stat"),
        format!("{pid} (java worker) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 100\n"),
    )
    .expect("synthetic process identity should be writable");
    fs::write(
        proc_root.join("meminfo"),
        "MemTotal:       16777216 kB\n\
         MemFree:         1048576 kB\n\
         MemAvailable:    4194304 kB\n",
    )
    .expect("synthetic meminfo should be writable");
    fs::write(
        proc_root.join("pressure/memory"),
        "some total=100\nfull total=10\n",
    )
    .expect("synthetic PSI should be writable");
    fs::write(proc_root.join("vmstat"), "pswpin 2\npswpout 3\n")
        .expect("synthetic vmstat should be writable");
}

pub(super) fn wait_for_socket(socket: &Path) {
    for _ in 0..1_000 {
        if socket.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("recorder socket should become ready");
}
