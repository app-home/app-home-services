// Integration tests for startup hardening.
// These tests verify the system exits with error when the database is unreachable.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites: None (this test intentionally uses an invalid database URL)

use std::process::Command;

#[tokio::test]
#[ignore]
async fn test_startup_fails_on_unreachable_db() {
    let output = Command::new("cargo")
        .args(["run", "--quiet"])
        .env("DATABASE_URL", "postgres://invalid:5432/unreachable")
        .env("JWT_SECRET", "test-secret")
        .output()
        .expect("Failed to run cargo");

    // The process should exit with non-zero status code
    assert!(
        !output.status.success(),
        "Process should exit with error on unreachable DB"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Default user check failed") || stderr.contains("Failed to create database pool"),
        "Should log a startup failure message, got: {stderr}"
    );
}
