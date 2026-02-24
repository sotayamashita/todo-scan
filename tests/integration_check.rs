use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn todo_scan() -> Command {
    assert_cmd::cargo_bin_cmd!("todo-scan")
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

    todo_scan()
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

    todo_scan()
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

    todo_scan()
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

    todo_scan()
        .args(["check", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    todo_scan()
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

    todo_scan()
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

    todo_scan()
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
        .stdout(predicate::str::contains("::notice::todo-scan check: PASS"));
}

#[test]
fn test_check_github_actions_format_fail() {
    let dir = setup_project(&[("main.rs", "// TODO: one\n// TODO: two\n")]);

    todo_scan()
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
        .stdout(predicate::str::contains("::error::todo-scan check: FAIL"));
}

#[test]
fn test_check_sarif_format() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    todo_scan()
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

    todo_scan()
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

    todo_scan()
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

// --- Check with --since (git-based max-new) ---

fn setup_git_repo(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    let cwd = dir.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(cwd)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(cwd)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(cwd)
        .output()
        .unwrap();

    for (path, content) in files {
        let full_path = cwd.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(cwd)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(cwd)
        .output()
        .unwrap();

    dir
}

#[test]
fn test_check_since_max_new_passes() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: existing\nfn main() {}\n")]);
    let cwd = dir.path();

    // Add one new TODO
    fs::write(
        cwd.join("main.rs"),
        "// TODO: existing\n// TODO: new one\nfn main() {}\n",
    )
    .unwrap();

    todo_scan()
        .args([
            "check",
            "--root",
            cwd.to_str().unwrap(),
            "--since",
            "HEAD",
            "--max-new",
            "5",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_since_max_new_fails() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
    let cwd = dir.path();

    // Add multiple new TODOs exceeding --max-new
    fs::write(
        cwd.join("main.rs"),
        "// TODO: new one\n// TODO: new two\n// TODO: new three\nfn main() {}\n",
    )
    .unwrap();

    todo_scan()
        .args([
            "check",
            "--root",
            cwd.to_str().unwrap(),
            "--since",
            "HEAD",
            "--max-new",
            "1",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("exceeds max_new"));
}

// --- Expired deadline tests ---

#[test]
fn test_check_expired_deadline_fails() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-01-01): this is overdue\n")]);

    todo_scan()
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

    todo_scan()
        .args(["check", "--root", dir.path().to_str().unwrap(), "--expired"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_no_expired_flag_ignores_deadline() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-01-01): this is overdue\n")]);

    // Without --expired flag, even old deadlines should pass
    todo_scan()
        .args(["check", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_expired_author_and_date() {
    let dir = setup_project(&[("main.rs", "// TODO(alice, 2020-06-01): overdue task\n")]);

    todo_scan()
        .args(["check", "--root", dir.path().to_str().unwrap(), "--expired"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("expired"));
}

#[test]
fn test_check_expired_quarter_format() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-Q1): overdue quarter task\n")]);

    todo_scan()
        .args(["check", "--root", dir.path().to_str().unwrap(), "--expired"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("expired"));
}

#[test]
fn test_check_max_does_not_count_ignored() {
    // 3 TODOs total, but 1 is ignored via todo-scan:ignore
    // So effective count is 2, which should pass --max 2
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: one\n// TODO: two\n// TODO: three todo-scan:ignore\n",
    )]);

    todo_scan()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--max",
            "2",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_check_max_fails_without_ignore() {
    // Same content but without todo-scan:ignore - should fail --max 2
    let dir = setup_project(&[("main.rs", "// TODO: one\n// TODO: two\n// TODO: three\n")]);

    todo_scan()
        .args([
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--max",
            "2",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"));
}
