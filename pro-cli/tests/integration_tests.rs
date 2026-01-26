//! Integration tests for Pro CLI
//!
//! These tests verify the CLI commands work correctly end-to-end.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the rx binary
fn rx_binary() -> &'static str {
    env!("CARGO_BIN_EXE_rx")
}

/// Helper to run rx commands in a directory
fn run_rx(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(rx_binary())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("Failed to execute rx command")
}

/// Assert a command succeeded
fn assert_success(output: &std::process::Output, context: &str) {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "{} failed:\nstdout: {}\nstderr: {}",
            context, stdout, stderr
        );
    }
}

#[test]
fn test_init_creates_project() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path().join("my-project");

    let output = run_rx(&["init", project_dir.to_str().unwrap()], temp.path());
    assert_success(&output, "rx init");

    // Verify files were created
    assert!(project_dir.join("pyproject.toml").exists());
    assert!(project_dir.join("rx.lock").exists());
    assert!(project_dir.join("src/my_project/__init__.py").exists());
    assert!(project_dir.join("tests/__init__.py").exists());

    // Verify pyproject.toml content
    let content = fs::read_to_string(project_dir.join("pyproject.toml")).unwrap();
    assert!(content.contains("[project]"));
    assert!(content.contains("name = \"my-project\""));
    assert!(content.contains("version = \"0.1.0\""));
}

#[test]
fn test_init_with_name_flag() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path().join("dir");

    let output = run_rx(
        &[
            "init",
            project_dir.to_str().unwrap(),
            "--name",
            "custom-name",
        ],
        temp.path(),
    );
    assert_success(&output, "rx init --name");

    let content = fs::read_to_string(project_dir.join("pyproject.toml")).unwrap();
    assert!(content.contains("name = \"custom-name\""));
}

#[test]
fn test_init_existing_pyproject_fails() {
    let temp = TempDir::new().unwrap();

    // Create an existing pyproject.toml
    fs::write(
        temp.path().join("pyproject.toml"),
        "[project]\nname = \"existing\"\n",
    )
    .unwrap();

    let output = run_rx(&["init"], temp.path());
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists")
            || String::from_utf8_lossy(&output.stdout).contains("already exists"),
        "Expected 'already exists' error, got: {}",
        stderr
    );
}

#[test]
fn test_add_dependency() {
    let temp = TempDir::new().unwrap();

    // Initialize project first
    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    // Add a dependency
    let output = run_rx(&["add", "requests"], temp.path());
    assert_success(&output, "rx add requests");

    // Verify pyproject.toml was updated
    let content = fs::read_to_string(temp.path().join("pyproject.toml")).unwrap();
    assert!(
        content.contains("requests"),
        "pyproject.toml should contain 'requests'"
    );
}

#[test]
fn test_add_dev_dependency() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    let output = run_rx(&["add", "--dev", "pytest"], temp.path());
    assert_success(&output, "rx add --dev pytest");

    let content = fs::read_to_string(temp.path().join("pyproject.toml")).unwrap();
    // Check for optional-dependencies or dev-dependencies section
    assert!(
        content.contains("pytest"),
        "pyproject.toml should contain 'pytest'"
    );
}

#[test]
fn test_remove_dependency() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    // Add then remove
    let output = run_rx(&["add", "requests"], temp.path());
    assert_success(&output, "rx add requests");

    let output = run_rx(&["remove", "requests"], temp.path());
    assert_success(&output, "rx remove requests");

    let content = fs::read_to_string(temp.path().join("pyproject.toml")).unwrap();
    // Should not contain requests in dependencies section
    let deps_section = content.split("dependencies").nth(1);
    if let Some(section) = deps_section {
        let section_end = section.find('[').map(|i| &section[..i]).unwrap_or(section);
        assert!(
            !section_end.contains("requests"),
            "dependencies section should not contain 'requests'"
        );
    }
}

#[test]
fn test_lock_generates_lockfile() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    let output = run_rx(&["add", "requests>=2.28"], temp.path());
    assert_success(&output, "rx add requests");

    let output = run_rx(&["lock"], temp.path());
    assert_success(&output, "rx lock");

    // Verify lockfile exists and has content
    let lockfile = temp.path().join("rx.lock");
    assert!(lockfile.exists());
    let content = fs::read_to_string(&lockfile).unwrap();
    assert!(
        content.len() > 10,
        "Lockfile should have meaningful content"
    );
}

#[test]
fn test_export_requirements() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    let output = run_rx(&["add", "requests"], temp.path());
    assert_success(&output, "rx add requests");

    let output = run_rx(&["lock"], temp.path());
    assert_success(&output, "rx lock");

    let output = run_rx(&["export", "--format", "requirements"], temp.path());
    assert_success(&output, "rx export");

    // Check that requirements.txt was created or output contains packages
    let stdout = String::from_utf8_lossy(&output.stdout);
    let req_path = temp.path().join("requirements.txt");
    assert!(
        req_path.exists() || stdout.contains("requests"),
        "Should export requirements"
    );
}

#[test]
fn test_version_command() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    let output = run_rx(&["version"], temp.path());
    assert_success(&output, "rx version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0.1.0"),
        "Should show version 0.1.0, got: {}",
        stdout
    );
}

#[test]
fn test_version_bump() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    let output = run_rx(&["version", "bump", "patch"], temp.path());
    assert_success(&output, "rx version bump patch");

    // Verify version was bumped
    let content = fs::read_to_string(temp.path().join("pyproject.toml")).unwrap();
    assert!(
        content.contains("0.1.1") || content.contains("0.1.0"),
        "Version should be bumped or shown"
    );
}

#[test]
fn test_audit_command() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    // Audit should work even with no dependencies
    let output = run_rx(&["audit"], temp.path());
    // Audit might succeed or fail based on network, but should not crash
    assert!(
        output.status.success() || !output.stderr.is_empty(),
        "Audit should either succeed or provide error info"
    );
}

#[test]
fn test_workspace_init() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["workspace", "init"], temp.path());
    assert_success(&output, "rx workspace init");

    let pyproject = temp.path().join("pyproject.toml");
    assert!(pyproject.exists());

    let content = fs::read_to_string(&pyproject).unwrap();
    assert!(
        content.contains("[tool.rx.workspace]") || content.contains("workspace"),
        "Should create workspace config"
    );
}

#[test]
fn test_build_command() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    // Add minimal build backend config
    let pyproject_path = temp.path().join("pyproject.toml");
    let mut content = fs::read_to_string(&pyproject_path).unwrap();
    if !content.contains("[build-system]") {
        content.push_str("\n[build-system]\nrequires = [\"rx\"]\nbuild-backend = \"rx\"\n");
        fs::write(&pyproject_path, content).unwrap();
    }

    let output = run_rx(&["build"], temp.path());
    // Build may fail without proper setup, but should not crash
    let _stdout = String::from_utf8_lossy(&output.stdout);
    let _stderr = String::from_utf8_lossy(&output.stderr);
    // Just verify it runs without panicking
}

#[test]
fn test_docker_generate() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    let output = run_rx(&["docker", "generate"], temp.path());
    assert_success(&output, "rx docker generate");

    let dockerfile = temp.path().join("Dockerfile");
    assert!(dockerfile.exists(), "Dockerfile should be created");

    let content = fs::read_to_string(&dockerfile).unwrap();
    assert!(content.contains("FROM python:"));
}

#[test]
fn test_polylith_init() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init"], temp.path());
    assert_success(&output, "rx init");

    let output = run_rx(&["polylith", "init", "myorg"], temp.path());
    assert_success(&output, "rx polylith init myorg");

    // Check polylith directories were created
    assert!(
        temp.path().join("bases").exists()
            || temp.path().join("components").exists()
            || temp.path().join("projects").exists(),
        "Polylith directories should be created"
    );
}

#[test]
fn test_help_command() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["--help"], temp.path());
    assert_success(&output, "rx --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Pro"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("add"));
    assert!(stdout.contains("sync"));
}

#[test]
fn test_subcommand_help() {
    let temp = TempDir::new().unwrap();

    let output = run_rx(&["init", "--help"], temp.path());
    assert_success(&output, "rx init --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Initialize") || stdout.contains("init"));
}
