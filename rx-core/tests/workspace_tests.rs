//! Integration tests for workspace functionality

use std::fs;
use tempfile::TempDir;
use rx_core::Workspace;

#[test]
fn test_workspace_init() {
    let temp = TempDir::new().unwrap();

    // Create a workspace pyproject.toml
    let pyproject = r#"
[project]
name = "my-workspace"
version = "0.1.0"

[tool.rx.workspace]
members = ["packages/*"]
"#;

    fs::write(temp.path().join("pyproject.toml"), pyproject).unwrap();

    let ws = Workspace::load(temp.path());
    assert!(ws.is_ok());
}

#[test]
fn test_workspace_with_members() {
    let temp = TempDir::new().unwrap();

    // Create workspace root
    let root_pyproject = r#"
[project]
name = "workspace-root"
version = "0.1.0"

[tool.rx.workspace]
members = ["packages/pkg-a", "packages/pkg-b"]
"#;

    fs::write(temp.path().join("pyproject.toml"), root_pyproject).unwrap();

    // Create member packages
    fs::create_dir_all(temp.path().join("packages/pkg-a")).unwrap();
    fs::create_dir_all(temp.path().join("packages/pkg-b")).unwrap();

    let pkg_a_pyproject = r#"
[project]
name = "pkg-a"
version = "0.1.0"
dependencies = []
"#;

    let pkg_b_pyproject = r#"
[project]
name = "pkg-b"
version = "0.1.0"
dependencies = ["pkg-a"]
"#;

    fs::write(
        temp.path().join("packages/pkg-a/pyproject.toml"),
        pkg_a_pyproject,
    )
    .unwrap();
    fs::write(
        temp.path().join("packages/pkg-b/pyproject.toml"),
        pkg_b_pyproject,
    )
    .unwrap();

    let ws = Workspace::load(temp.path()).unwrap();
    assert_eq!(ws.members().len(), 2);
}

#[test]
fn test_workspace_glob_members() {
    let temp = TempDir::new().unwrap();

    // Create workspace with glob pattern
    let root_pyproject = r#"
[project]
name = "glob-workspace"
version = "0.1.0"

[tool.rx.workspace]
members = ["packages/*"]
"#;

    fs::write(temp.path().join("pyproject.toml"), root_pyproject).unwrap();

    // Create packages that match glob
    fs::create_dir_all(temp.path().join("packages/alpha")).unwrap();
    fs::create_dir_all(temp.path().join("packages/beta")).unwrap();
    fs::create_dir_all(temp.path().join("packages/gamma")).unwrap();

    for name in &["alpha", "beta", "gamma"] {
        let pyproject = format!(
            r#"
[project]
name = "{}"
version = "0.1.0"
"#,
            name
        );
        fs::write(
            temp.path().join(format!("packages/{}/pyproject.toml", name)),
            pyproject,
        )
        .unwrap();
    }

    let ws = Workspace::load(temp.path()).unwrap();
    assert_eq!(ws.members().len(), 3);
}

#[test]
fn test_workspace_create() {
    let temp = TempDir::new().unwrap();

    let ws = Workspace::create(temp.path(), true).unwrap();
    assert!(ws.shared_venv);
    assert_eq!(ws.members().len(), 0);

    // Verify pyproject.toml was created
    let content = fs::read_to_string(temp.path().join("pyproject.toml")).unwrap();
    assert!(content.contains("[tool.rx.workspace]"));
    assert!(content.contains("shared-venv = true"));
}

#[test]
fn test_workspace_add_member() {
    let temp = TempDir::new().unwrap();

    // Create workspace
    let mut ws = Workspace::create(temp.path(), false).unwrap();

    // Create member directory
    fs::create_dir_all(temp.path().join("packages/mylib")).unwrap();
    let lib_pyproject = r#"
[project]
name = "mylib"
version = "0.1.0"
"#;
    fs::write(
        temp.path().join("packages/mylib/pyproject.toml"),
        lib_pyproject,
    )
    .unwrap();

    // Add member
    ws.add_member("packages/mylib").unwrap();
    assert_eq!(ws.members().len(), 1);
}

#[test]
fn test_workspace_member_info() {
    let temp = TempDir::new().unwrap();

    // Create workspace with members
    let root_pyproject = r#"
[project]
name = "info-workspace"
version = "0.1.0"

[tool.rx.workspace]
members = ["packages/lib"]
"#;

    fs::write(temp.path().join("pyproject.toml"), root_pyproject).unwrap();

    fs::create_dir_all(temp.path().join("packages/lib")).unwrap();
    let lib_pyproject = r#"
[project]
name = "mylib"
version = "1.0.0"
dependencies = ["requests", "numpy"]
"#;
    fs::write(
        temp.path().join("packages/lib/pyproject.toml"),
        lib_pyproject,
    )
    .unwrap();

    let ws = Workspace::load(temp.path()).unwrap();
    let info = ws.member_info().unwrap();

    assert_eq!(info.len(), 1);
    assert_eq!(info[0].name.as_ref().unwrap(), "mylib");
    assert_eq!(info[0].version.as_ref().unwrap(), "1.0.0");
    assert_eq!(info[0].dependency_count, 2);
}

#[test]
fn test_is_workspace_root() {
    let temp = TempDir::new().unwrap();

    // Not a workspace initially
    assert!(!Workspace::is_workspace_root(temp.path()));

    // Create workspace
    Workspace::create(temp.path(), false).unwrap();

    // Now it should be detected
    assert!(Workspace::is_workspace_root(temp.path()));
}

#[test]
fn test_workspace_lockfile_path() {
    let temp = TempDir::new().unwrap();

    let ws = Workspace::create(temp.path(), false).unwrap();
    let lockfile_path = ws.lockfile_path();

    assert_eq!(lockfile_path, temp.path().join("rx.lock"));
}
