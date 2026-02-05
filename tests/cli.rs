//! Integration tests for the asc-crash-fetcher CLI.

use std::process::Command;

fn bin() -> Command {
    // In integration tests, cargo puts the binary in target/debug/ or target/release/
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    });
    path.push("asc-crash-fetcher");
    Command::new(path)
}

#[test]
fn help_works() {
    let output = bin().arg("--help").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Manage TestFlight crash feedback"));
}

#[test]
fn version_works() {
    let output = bin().arg("--version").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("asc-crash-fetcher"));
}

#[test]
fn init_creates_data_dir() {
    let work_dir = tempfile::TempDir::new().unwrap();

    let output = bin()
        .arg("init")
        .current_dir(work_dir.path())
        .output()
        .expect("init failed");

    assert!(output.status.success());
    assert!(work_dir.path().join("asc-crashes/config.toml").exists());
    assert!(work_dir.path().join("asc-crashes/crashes.db").exists());
    assert!(work_dir.path().join("asc-crashes/logs").is_dir());
}

fn setup_test_env() -> tempfile::TempDir {
    let work_dir = tempfile::TempDir::new().unwrap();

    // Initialize
    bin()
        .arg("init")
        .current_dir(work_dir.path())
        .output()
        .expect("init failed");

    // Write a minimal valid config (key doesn't need to be real for offline commands)
    let config = r#"
[api]
issuer_id = "test-issuer"
key_id = "TESTKEY123"
private_key = """
-----BEGIN EC PRIVATE KEY-----
MHQCAQEEIBkg4LVWM9nuwNSk3yByxZpYRTBnVFNRMNRqZ7JDKMFdoAcGBSuBBAAi
oWQDYgAEY1GlPyRPrzIhfA9mRHkEbfYBbMEBZAlYoKlpOOGVxMJhThLHYWmqC5YH
ObecFMSEwIagaBecRPFROPyBK5VB1kT8Lf7KBHXho/D29iLA7+GhvS2VRGBHC4HR6
StXB7L
-----END EC PRIVATE KEY-----
"""

[[apps]]
bundle_id = "com.test.app"
"#;
    std::fs::write(work_dir.path().join("asc-crashes/config.toml"), config).unwrap();

    work_dir
}

#[test]
fn list_on_fresh_db_returns_empty_json() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["list", "--format", "json"])
        .output()
        .expect("list failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(parsed["count"], 0);
}

#[test]
fn list_on_fresh_db_returns_empty_text() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .arg("list")
        .output()
        .expect("list failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No crashes found"));
}

#[test]
fn stats_on_fresh_db() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["stats", "--format", "json"])
        .output()
        .expect("stats failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(parsed["total"], 0);
    assert_eq!(parsed["unfixed"], 0);
}

#[test]
fn show_nonexistent_crash_fails() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["show", "999"])
        .output()
        .expect("show failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));
}

#[test]
fn fix_nonexistent_crash_fails() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["fix", "999", "--notes", "test"])
        .output()
        .expect("fix failed");

    assert!(!output.status.success());
}

#[test]
fn log_nonexistent_crash_fails() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["log", "999"])
        .output()
        .expect("log failed");

    assert!(!output.status.success());
}

#[test]
fn no_config_gives_helpful_error() {
    let work_dir = tempfile::TempDir::new().unwrap();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("nonexistent").to_str().unwrap(),
        ])
        .arg("list")
        .output()
        .expect("list failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No config found") || stderr.contains("config"));
}

// ─── Feedback command tests ───────────────────────────────────────────────────

#[test]
fn init_creates_screenshots_dir() {
    let work_dir = tempfile::TempDir::new().unwrap();

    let output = bin()
        .arg("init")
        .current_dir(work_dir.path())
        .output()
        .expect("init failed");

    assert!(output.status.success());
    assert!(work_dir.path().join("asc-crashes/screenshots").is_dir());
}

#[test]
fn feedback_list_on_fresh_db_returns_empty_json() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["feedback", "list", "--format", "json"])
        .output()
        .expect("feedback list failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(parsed["count"], 0);
}

#[test]
fn feedback_list_on_fresh_db_returns_empty_text() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["feedback", "list"])
        .output()
        .expect("feedback list failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No feedback found"));
}

#[test]
fn feedback_show_nonexistent_fails() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["feedback", "show", "999"])
        .output()
        .expect("feedback show failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));
}

#[test]
fn feedback_stats_on_fresh_db() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["feedback", "stats", "--format", "json"])
        .output()
        .expect("feedback stats failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(parsed["total"], 0);
    assert_eq!(parsed["unfixed"], 0);
}

#[test]
fn feedback_fix_nonexistent_fails() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["feedback", "fix", "999", "--notes", "test"])
        .output()
        .expect("feedback fix failed");

    assert!(!output.status.success());
}

#[test]
fn feedback_screenshot_nonexistent_fails() {
    let work_dir = setup_test_env();

    let output = bin()
        .args([
            "--data-dir",
            work_dir.path().join("asc-crashes").to_str().unwrap(),
        ])
        .args(["feedback", "screenshot", "999"])
        .output()
        .expect("feedback screenshot failed");

    assert!(!output.status.success());
}
