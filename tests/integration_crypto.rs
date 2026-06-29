// Integration tests for the acme-client binary.
// These tests call the compiled binary directly via std::process::Command.

use std::process::Command;

fn acme_client() -> Command {
    Command::new(env!("CARGO_BIN_EXE_acme-client"))
}

#[test]
fn test_binary_help_exits_successfully() {
    let output = acme_client()
        .arg("--help")
        .output()
        .expect("failed to run acme-client --help");
    assert!(
        output.status.success(),
        "acme-client --help must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("acme-client"),
        "help output must mention acme-client"
    );
}

#[test]
fn test_issue_subcommand_help() {
    let output = acme_client()
        .args(["issue", "--help"])
        .output()
        .expect("failed to run acme-client issue --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--domain"),
        "issue help must mention --domain"
    );
    assert!(
        stdout.contains("--email"),
        "issue help must mention --email"
    );
}

#[test]
fn test_show_nonexistent_domain_fails_gracefully() {
    let output = acme_client()
        .args([
            "show",
            "--domain",
            "nonexistent-integration-test.example.com",
        ])
        .output()
        .expect("failed to run acme-client show");
    // Must fail (no cert on disk), but must NOT panic
    assert!(
        !output.status.success(),
        "show for unknown domain must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("thread 'main' panicked"),
        "must not panic; stderr: {stderr}"
    );
}
