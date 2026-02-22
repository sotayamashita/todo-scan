use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn todox() -> Command {
    assert_cmd::cargo_bin_cmd!("todox")
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

// --- Expired deadline tests ---

#[test]
fn test_check_expired_deadline_fails() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-01-01): this is overdue\n")]);

    todox()
        .args(["check", "--root", dir.path().to_str().unwrap(), "--expired"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("expired"))
        .stdout(predicate::str::contains("2020-01-01"));
}

#[test]
fn test_check_future_deadline_passes() {
    let dir = setup_project(&[("main.rs", "// TODO(2099-12-31): far future task\n")]);

    todox()
        .args(["check", "--root", dir.path().to_str().unwrap(), "--expired"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_no_expired_flag_ignores_deadline() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-01-01): this is overdue\n")]);

    // Without --expired flag, even old deadlines should pass
    todox()
        .args(["check", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_expired_author_and_date() {
    let dir = setup_project(&[("main.rs", "// TODO(alice, 2020-06-01): overdue task\n")]);

    todox()
        .args(["check", "--root", dir.path().to_str().unwrap(), "--expired"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("expired"));
}

#[test]
fn test_check_expired_quarter_format() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-Q1): overdue quarter task\n")]);

    todox()
        .args(["check", "--root", dir.path().to_str().unwrap(), "--expired"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("expired"));
}
