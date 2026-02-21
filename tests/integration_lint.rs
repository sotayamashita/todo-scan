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

// --- Pass cases ---

#[test]
fn test_lint_pass_valid_todos() {
    let dir = setup_project(&[("main.rs", "// TODO: implement this feature\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_lint_pass_with_author_and_issue() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): fix issue #123\n// FIXME(bob): handle edge case JIRA-456\n",
    )]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--require-author",
            "TODO,FIXME",
            "--require-issue-ref",
            "TODO,FIXME",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- Bare tags ---

#[test]
fn test_lint_fail_bare_tag() {
    let dir = setup_project(&[("main.rs", "// TODO:\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("no_bare_tags"));
}

#[test]
fn test_lint_fail_bare_tag_no_colon() {
    let dir = setup_project(&[("main.rs", "// TODO\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"));
}

// --- Uppercase tag ---

#[test]
fn test_lint_fail_lowercase_tag() {
    let dir = setup_project(&[("main.rs", "// todo: lowercase tag\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("uppercase_tag"));
}

#[test]
fn test_lint_fail_mixed_case_tag() {
    let dir = setup_project(&[("main.rs", "// Todo: mixed case\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("uppercase_tag"));
}

// --- Require colon ---

#[test]
fn test_lint_fail_missing_colon() {
    let dir = setup_project(&[("main.rs", "// TODO fix without colon\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("require_colon"));
}

// --- Require author ---

#[test]
fn test_lint_fail_missing_author() {
    let dir = setup_project(&[("main.rs", "// TODO: no author here\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--require-author",
            "TODO",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("require_author"));
}

#[test]
fn test_lint_pass_author_present() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): has author\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--require-author",
            "TODO",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_lint_require_author_ignores_other_tags() {
    let dir = setup_project(&[("main.rs", "// NOTE: no author needed\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--require-author",
            "TODO",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- Require issue ref ---

#[test]
fn test_lint_fail_missing_issue_ref() {
    let dir = setup_project(&[("main.rs", "// BUG: no issue ref\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--require-issue-ref",
            "BUG",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("require_issue_ref"));
}

#[test]
fn test_lint_pass_issue_ref_present() {
    let dir = setup_project(&[("main.rs", "// BUG: fix crash #42\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--require-issue-ref",
            "BUG",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- Max message length ---

#[test]
fn test_lint_fail_message_too_long() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: this is a very long message that exceeds the limit\n",
    )]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--max-message-length",
            "10",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("max_message_length"));
}

#[test]
fn test_lint_pass_message_within_limit() {
    let dir = setup_project(&[("main.rs", "// TODO: short\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--max-message-length",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- Output formats ---

#[test]
fn test_lint_json_format_pass() {
    let dir = setup_project(&[("main.rs", "// TODO: valid task\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": true"))
        .stdout(predicate::str::contains("\"violation_count\": 0"));
}

#[test]
fn test_lint_json_format_fail() {
    let dir = setup_project(&[("main.rs", "// todo: lowercase\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("\"passed\": false"))
        .stdout(predicate::str::contains("\"rule\": \"uppercase_tag\""));
}

#[test]
fn test_lint_github_actions_format_pass() {
    let dir = setup_project(&[("main.rs", "// TODO: valid task\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("::notice::todox lint: PASS"));
}

#[test]
fn test_lint_github_actions_format_fail() {
    let dir = setup_project(&[("main.rs", "// todo: lowercase\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "::error file=main.rs,line=1,title=uppercase_tag",
        ))
        .stdout(predicate::str::contains("::error::todox lint: FAIL"));
}

#[test]
fn test_lint_sarif_format_pass() {
    let dir = setup_project(&[("main.rs", "// TODO: valid task\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "sarif",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"2.1.0\""))
        .stdout(predicate::str::contains("\"level\": \"note\""));
}

#[test]
fn test_lint_sarif_format_fail() {
    let dir = setup_project(&[("main.rs", "// todo: lowercase\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "sarif",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("todox/lint/uppercase_tag"))
        .stdout(predicate::str::contains("\"level\": \"error\""));
}

#[test]
fn test_lint_markdown_format_pass() {
    let dir = setup_project(&[("main.rs", "// TODO: valid task\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("## PASS"))
        .stdout(predicate::str::contains("All lint checks passed"));
}

#[test]
fn test_lint_markdown_format_fail() {
    let dir = setup_project(&[("main.rs", "// todo: lowercase\n")]);

    todox()
        .args([
            "lint",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("## FAIL"))
        .stdout(predicate::str::contains("uppercase_tag"));
}

// --- Config file integration ---

#[test]
fn test_lint_config_file_require_author() {
    let dir = setup_project(&[
        ("main.rs", "// TODO: no author\n"),
        (
            ".todox.toml",
            r#"
[lint]
require_author = ["TODO"]
"#,
        ),
    ]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("require_author"));
}

#[test]
fn test_lint_config_file_disable_defaults() {
    let dir = setup_project(&[
        ("main.rs", "// todo fix something\n"),
        (
            ".todox.toml",
            r#"
[lint]
no_bare_tags = false
uppercase_tag = false
require_colon = false
"#,
        ),
    ]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- Multiple violations ---

#[test]
fn test_lint_multiple_violations_same_file() {
    let dir = setup_project(&[(
        "main.rs",
        "// todo fix something\n// FIXME no colon either\n",
    )]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL"));
}

// --- Exit code verification ---

#[test]
fn test_lint_exit_code_0_on_pass() {
    let dir = setup_project(&[("main.rs", "// TODO: valid task\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(0);
}

#[test]
fn test_lint_exit_code_1_on_fail() {
    let dir = setup_project(&[("main.rs", "// todo: lowercase\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1);
}

// --- Empty project ---

#[test]
fn test_lint_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- Multiple files ---

#[test]
fn test_lint_multiple_files() {
    let dir = setup_project(&[
        ("src/a.rs", "// TODO: valid in file a\n"),
        ("src/b.rs", "// TODO: valid in file b\n"),
    ]);

    todox()
        .args(["lint", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}
