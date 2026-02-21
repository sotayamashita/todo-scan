use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn todox() -> Command {
    Command::cargo_bin("todox").unwrap()
}

fn setup_project(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (path, content) in files {
        let full_path = dir.path().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }
    dir
}

#[test]
fn test_stats_basic_output() {
    let dir = setup_project(&[
        (
            "main.rs",
            "// TODO: implement feature\n// FIXME: broken\n// TODO: another task\n",
        ),
        ("lib.rs", "// HACK: workaround\n"),
    ]);

    todox()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tags"))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("HACK"))
        .stdout(predicate::str::contains("4 items across 2 files"));
}

#[test]
fn test_stats_priority_counts() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO!!: urgent task\n// TODO!: high task\n// TODO: normal task\n",
    )]);

    todox()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("normal: 1"))
        .stdout(predicate::str::contains("high: 1"))
        .stdout(predicate::str::contains("urgent: 1"));
}

#[test]
fn test_stats_with_authors() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): alice task\n// TODO(bob): bob task\n// TODO: no author\n",
    )]);

    todox()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Authors"))
        .stdout(predicate::str::contains("alice"))
        .stdout(predicate::str::contains("bob"))
        .stdout(predicate::str::contains("unassigned"));
}

#[test]
fn test_stats_hotspot_files() {
    let dir = setup_project(&[
        ("main.rs", "// TODO: one\n// TODO: two\n// TODO: three\n"),
        ("lib.rs", "// TODO: single\n"),
    ]);

    todox()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hotspots"))
        .stdout(predicate::str::contains("main.rs (3)"))
        .stdout(predicate::str::contains("lib.rs (1)"));
}

#[test]
fn test_stats_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: json test\n// FIXME: another\n")]);

    todox()
        .args([
            "stats",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_items\": 2"))
        .stdout(predicate::str::contains("\"total_files\": 1"))
        .stdout(predicate::str::contains("\"tag_counts\""))
        .stdout(predicate::str::contains("\"priority_counts\""));
}

#[test]
fn test_stats_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todox()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 items across 0 files"));
}

#[test]
fn test_stats_bar_chart_present() {
    let dir = setup_project(&[("main.rs", "// TODO: one\n// TODO: two\n// FIXME: three\n")]);

    todox()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{2588}"));
}
