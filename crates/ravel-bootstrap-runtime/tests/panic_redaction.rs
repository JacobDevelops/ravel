//! The panic hook is process-global, so its output is exercised in a child process.

#[test]
fn panic_hook_redacts_stderr_without_chaining_the_default_hook() {
    const CHILD: &str = "RAVEL_PANIC_REDACTION_CHILD";
    if std::env::var_os(CHILD).is_some() {
        ravel_bootstrap_runtime::install_panic_hook();
        panic!(
            "{}",
            concat!("token=", "ghp_", "secretvalue /home/player/.minecraft")
        );
    }
    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "panic_hook_redacts_stderr_without_chaining_the_default_hook",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .expect("the child test runs");
    assert!(
        !output.status.success(),
        "the child must exercise an actual panic"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Ravel host panic:"), "{stderr}");
    assert!(stderr.contains("[redacted]"), "{stderr}");
    assert!(!stderr.contains(concat!("ghp_", "secretvalue")), "{stderr}");
    assert!(!stderr.contains("/home/player"), "{stderr}");
    assert!(
        !stderr.contains("thread '"),
        "the default hook leaked: {stderr}"
    );
}
