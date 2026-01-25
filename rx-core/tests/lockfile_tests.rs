//! Integration tests for the lockfile module

use tempfile::TempDir;
use rx_core::Lockfile;

#[test]
fn test_new_lockfile() {
    let lockfile = Lockfile::new();
    assert!(lockfile.is_empty());
    assert_eq!(lockfile.len(), 0);
}

#[test]
fn test_save_and_load_lockfile() {
    let temp = TempDir::new().unwrap();
    let lockfile_path = temp.path().join("rx.lock");

    let lockfile = Lockfile::new();
    lockfile.save(&lockfile_path).unwrap();

    assert!(lockfile_path.exists());

    let loaded = Lockfile::load(&lockfile_path).unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn test_lockfile_contains_and_get() {
    let content = r#"
version = "2"

[packages.requests]
version = "2.31.0"
url = "https://example.com/requests-2.31.0.whl"
hash = "sha256:abc123"
dependencies = ["certifi", "urllib3"]

[packages.urllib3]
version = "2.0.0"
"#;

    let lockfile = Lockfile::parse(content).unwrap();

    assert!(lockfile.contains("requests"));
    assert!(lockfile.contains("urllib3"));
    assert!(!lockfile.contains("django"));

    let requests = lockfile.get("requests").unwrap();
    assert_eq!(requests.version, "2.31.0");
    assert_eq!(requests.hash.as_ref().unwrap(), "sha256:abc123");
    assert_eq!(requests.dependencies.len(), 2);
}

#[test]
fn test_lockfile_dependency_graph() {
    let content = r#"
version = "2"

[packages.django]
version = "4.2.0"
dependencies = ["asgiref", "sqlparse"]

[packages.asgiref]
version = "3.7.0"
dependencies = []

[packages.sqlparse]
version = "0.4.4"
dependencies = []
"#;

    let lockfile = Lockfile::parse(content).unwrap();

    let graph = lockfile.dependency_graph();
    assert_eq!(graph.get("django").unwrap().len(), 2);
    assert!(graph.get("asgiref").unwrap().is_empty());

    let reverse = lockfile.reverse_dependencies("asgiref");
    assert!(reverse.contains(&"django".to_string()));
}

#[test]
fn test_lockfile_round_trip() {
    let content = r#"
version = "2"

[packages.mypackage]
version = "1.0.0"
url = "https://example.com/mypackage.whl"
hash = "sha256:test123"
dependencies = ["dep1", "dep2"]
markers = "python_version >= '3.8'"
"#;

    let lockfile = Lockfile::parse(content).unwrap();
    let serialized = lockfile.to_string().unwrap();
    let reparsed = Lockfile::parse(&serialized).unwrap();

    assert_eq!(lockfile.len(), reparsed.len());
    let pkg = reparsed.get("mypackage").unwrap();
    assert_eq!(pkg.version, "1.0.0");
    assert_eq!(pkg.markers.as_ref().unwrap(), "python_version >= '3.8'");
}

#[test]
fn test_to_resolution() {
    let content = r#"
version = "2"

[packages.requests]
version = "2.31.0"
url = "https://example.com/requests.whl"
hash = "sha256:abc"
dependencies = ["urllib3"]

[packages.urllib3]
version = "2.0.0"
"#;

    let lockfile = Lockfile::parse(content).unwrap();
    let resolution = lockfile.to_resolution();

    assert_eq!(resolution.packages.len(), 2);
}
