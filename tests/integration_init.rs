use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn todox() -> Command {
    assert_cmd::cargo_bin_cmd!("todox")
}

#[test]
fn test_init_creates_config() {
    let dir = TempDir::new().unwrap();

    todox()
        .args(["init", "--yes", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stderr(predicate::str::contains("Created .todox.toml"));

    let config_path = dir.path().join(".todox.toml");
    assert!(config_path.exists());

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("TODO"));
    assert!(content.contains("FIXME"));
    assert!(content.contains("HACK"));

    // Verify parseable as valid config
    let _: toml::Value = toml::from_str(&content).unwrap();
}

#[test]
fn test_init_refuses_overwrite_in_non_interactive() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".todox.toml"), "tags = [\"TODO\"]").unwrap();

    todox()
        .args(["init", "--yes", "--root", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_init_detects_rust_project() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\n",
    )
    .unwrap();

    todox()
        .args(["init", "--yes", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stderr(predicate::str::contains("Rust"));

    let content = fs::read_to_string(dir.path().join(".todox.toml")).unwrap();
    assert!(content.contains("target"));
}

#[test]
fn test_init_detects_node_project() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();

    todox()
        .args(["init", "--yes", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stderr(predicate::str::contains("JavaScript"));

    let content = fs::read_to_string(dir.path().join(".todox.toml")).unwrap();
    assert!(content.contains("node_modules"));
}

#[test]
fn test_init_empty_project_no_exclude_dirs() {
    let dir = TempDir::new().unwrap();

    todox()
        .args(["init", "--yes", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join(".todox.toml")).unwrap();
    // exclude_dirs should be an empty array
    assert!(content.contains("exclude_dirs = []"));
}

#[test]
fn test_init_config_has_all_default_tags() {
    let dir = TempDir::new().unwrap();

    todox()
        .args(["init", "--yes", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join(".todox.toml")).unwrap();
    for tag in &["TODO", "FIXME", "HACK", "XXX", "BUG", "NOTE"] {
        assert!(content.contains(tag), "missing tag: {}", tag);
    }
}
