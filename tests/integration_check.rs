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
fn test_check_pass_under_max() {
    let dir = setup_project(&[("main.rs", "// TODO: one task\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--max",
            "10",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_fail_over_max() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: one\n// TODO: two\n// TODO: three\n// TODO: four\n// TODO: five\n// TODO: six\n",
    )]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--max",
            "3",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("exceeds max"));
}

#[test]
fn test_check_block_tags() {
    let dir = setup_project(&[("main.rs", "// BUG: critical issue\n// TODO: normal task\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--block-tags",
            "BUG",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("Blocked tag"));
}

#[test]
fn test_check_pass_no_constraints() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    todox()
        .args(["check", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--max",
            "10",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": true"));
}

#[test]
fn test_check_fail_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: one\n// TODO: two\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--max",
            "1",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("\"passed\": false"))
        .stdout(predicate::str::contains("\"rule\": \"max\""));
}

#[test]
fn test_check_github_actions_format_pass() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
            "--max",
            "10",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("::notice::todox check: PASS"));
}

#[test]
fn test_check_github_actions_format_fail() {
    let dir = setup_project(&[("main.rs", "// TODO: one\n// TODO: two\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
            "--max",
            "1",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("::error title=max::"))
        .stdout(predicate::str::contains("::error::todox check: FAIL"));
}

#[test]
fn test_check_sarif_format() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "sarif",
            "--max",
            "10",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"2.1.0\""))
        .stdout(predicate::str::contains("\"level\": \"note\""));
}

#[test]
fn test_check_markdown_format_pass() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
            "--max",
            "10",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("## PASS"))
        .stdout(predicate::str::contains("All checks passed"));
}

#[test]
fn test_check_markdown_format_fail() {
    let dir = setup_project(&[("main.rs", "// TODO: one\n// TODO: two\n")]);

    todox()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
            "--max",
            "1",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("## FAIL"))
        .stdout(predicate::str::contains("- **max**:"));
}
