//! Performance benchmarks for Pro core operations
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs;
use tempfile::TempDir;

use rx_core::pep::pep440::Version;
use rx_core::pep::{PyProject, Requirement, VersionSpecifiers};
use rx_core::Lockfile;

/// Benchmark PEP 440 version parsing
fn bench_version_parsing(c: &mut Criterion) {
    let versions = vec![
        "1.0.0",
        "2.31.0",
        "1.0a1",
        "1.0b2",
        "1.0rc1",
        "1.0.post1",
        "1.0.dev1",
        "1!2.0.0",
        "21.3.0+local",
        "1.0a1.post2.dev3",
    ];

    let mut group = c.benchmark_group("version_parsing");

    for version in versions {
        group.bench_with_input(BenchmarkId::from_parameter(version), version, |b, v| {
            b.iter(|| Version::parse(black_box(v)))
        });
    }

    group.finish();
}

/// Benchmark version comparison
fn bench_version_comparison(c: &mut Criterion) {
    let v1 = Version::parse("2.31.0").unwrap();
    let v2 = Version::parse("2.31.1").unwrap();
    let v3 = Version::parse("2.31.0a1").unwrap();

    c.bench_function("version_compare_equal", |b| {
        let v1 = v1.clone();
        let v2 = v1.clone();
        b.iter(|| black_box(&v1) == black_box(&v2))
    });

    c.bench_function("version_compare_greater", |b| {
        b.iter(|| black_box(&v2) > black_box(&v1))
    });

    c.bench_function("version_compare_prerelease", |b| {
        b.iter(|| black_box(&v1) > black_box(&v3))
    });
}

/// Benchmark PEP 508 requirement parsing
fn bench_requirement_parsing(c: &mut Criterion) {
    let requirements = vec![
        "requests",
        "requests>=2.28.0",
        "requests>=2.28.0,<3.0.0",
        "numpy[dev]>=1.24.0",
        "torch[cuda]; sys_platform == 'linux'",
        "requests>=2.28.0; python_version >= '3.8'",
    ];

    let mut group = c.benchmark_group("requirement_parsing");

    for req in requirements {
        group.bench_with_input(BenchmarkId::from_parameter(req), req, |b, r| {
            b.iter(|| Requirement::parse(black_box(r)))
        });
    }

    group.finish();
}

/// Benchmark specifier matching
fn bench_specifier_matching(c: &mut Criterion) {
    let specifier = VersionSpecifiers::parse(">=2.28.0,<3.0.0").unwrap();
    let versions = vec![
        Version::parse("2.28.0").unwrap(),
        Version::parse("2.31.0").unwrap(),
        Version::parse("3.0.0").unwrap(),
        Version::parse("2.27.0").unwrap(),
    ];

    c.bench_function("specifier_contains", |b| {
        b.iter(|| {
            for v in &versions {
                black_box(specifier.contains(v));
            }
        })
    });
}

/// Benchmark lockfile parsing
fn bench_lockfile_parsing(c: &mut Criterion) {
    // Create a sample lockfile with multiple packages
    let mut lockfile_content = String::from("version = \"2\"\n\n");

    for i in 0..100 {
        lockfile_content.push_str(&format!(
            r#"[packages.package-{}]
version = "{}.0.0"
url = "https://example.com/package-{}.whl"
hash = "sha256:abc123def456"
dependencies = ["dep-a", "dep-b"]

"#,
            i, i, i
        ));
    }

    c.bench_function("lockfile_parse_100_packages", |b| {
        b.iter(|| Lockfile::parse(black_box(&lockfile_content)))
    });
}

/// Benchmark lockfile serialization
fn bench_lockfile_serialization(c: &mut Criterion) {
    let lockfile_content = r#"
version = "2"

[packages.requests]
version = "2.31.0"
url = "https://example.com/requests.whl"
hash = "sha256:abc123"
dependencies = ["urllib3", "certifi"]

[packages.urllib3]
version = "2.0.0"
"#;

    let lockfile = Lockfile::parse(lockfile_content).unwrap();

    c.bench_function("lockfile_serialize", |b| {
        b.iter(|| black_box(&lockfile).to_string())
    });
}

/// Benchmark lockfile round-trip (parse + serialize)
fn bench_lockfile_roundtrip(c: &mut Criterion) {
    let mut lockfile_content = String::from("version = \"2\"\n\n");

    for i in 0..50 {
        lockfile_content.push_str(&format!(
            r#"[packages.package-{}]
version = "{}.0.0"
dependencies = []

"#,
            i, i
        ));
    }

    c.bench_function("lockfile_roundtrip_50_packages", |b| {
        b.iter(|| {
            let lockfile = Lockfile::parse(black_box(&lockfile_content)).unwrap();
            black_box(lockfile.to_string())
        })
    });
}

/// Benchmark lockfile file I/O
fn bench_lockfile_io(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();
    let lockfile_path = temp.path().join("rx.lock");

    // Create a lockfile with some packages
    let mut lockfile = Lockfile::new();
    for i in 0..50 {
        lockfile.packages.insert(
            format!("package-{}", i),
            rx_core::lockfile::LockedPackage {
                version: format!("{}.0.0", i),
                url: Some(format!("https://example.com/package-{}.whl", i)),
                hash: Some("sha256:abc123".to_string()),
                dependencies: vec!["dep-a".to_string()],
                markers: None,
                files: vec![],
            },
        );
    }

    // Save once for the read benchmark
    lockfile.save(&lockfile_path).unwrap();

    c.bench_function("lockfile_save_50_packages", |b| {
        let save_path = temp.path().join("rx.lock.bench");
        b.iter(|| lockfile.save(black_box(&save_path)))
    });

    c.bench_function("lockfile_load_50_packages", |b| {
        b.iter(|| Lockfile::load(black_box(&lockfile_path)))
    });
}

/// Benchmark pyproject.toml parsing
fn bench_pyproject_parsing(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();

    let pyproject_content = r#"
[project]
name = "my-project"
version = "1.0.0"
description = "A test project"
requires-python = ">=3.8"
dependencies = [
    "requests>=2.28.0",
    "numpy>=1.24.0",
    "pandas>=2.0.0",
    "scipy>=1.10.0",
    "matplotlib>=3.7.0",
    "scikit-learn>=1.2.0",
    "tensorflow>=2.12.0",
    "torch>=2.0.0",
    "transformers>=4.28.0",
    "fastapi>=0.95.0",
]

[project.optional-dependencies]
dev = ["pytest>=7.0.0", "black>=23.0.0", "mypy>=1.0.0"]
docs = ["sphinx>=6.0.0", "furo>=2023.0.0"]

[tool.rx]
cache-dir = ".rx-cache"
"#;

    fs::write(temp.path().join("pyproject.toml"), pyproject_content).unwrap();

    c.bench_function("pyproject_load", |b| {
        b.iter(|| PyProject::load(black_box(temp.path())))
    });
}

/// Benchmark dependency graph operations
fn bench_dependency_graph(c: &mut Criterion) {
    let lockfile_content = r#"
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

[packages.requests]
version = "2.31.0"
dependencies = ["urllib3", "certifi", "charset-normalizer", "idna"]

[packages.urllib3]
version = "2.0.0"
dependencies = []

[packages.certifi]
version = "2023.7.22"
dependencies = []

[packages.charset-normalizer]
version = "3.2.0"
dependencies = []

[packages.idna]
version = "3.4"
dependencies = []
"#;

    let lockfile = Lockfile::parse(lockfile_content).unwrap();

    c.bench_function("dependency_graph_build", |b| {
        b.iter(|| black_box(&lockfile).dependency_graph())
    });

    c.bench_function("reverse_dependencies_lookup", |b| {
        b.iter(|| {
            black_box(&lockfile).reverse_dependencies("urllib3");
            black_box(&lockfile).reverse_dependencies("asgiref");
        })
    });
}

criterion_group!(
    benches,
    bench_version_parsing,
    bench_version_comparison,
    bench_requirement_parsing,
    bench_specifier_matching,
    bench_lockfile_parsing,
    bench_lockfile_serialization,
    bench_lockfile_roundtrip,
    bench_lockfile_io,
    bench_pyproject_parsing,
    bench_dependency_graph,
);

criterion_main!(benches);
