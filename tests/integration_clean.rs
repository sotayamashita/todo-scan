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

// --- Duplicate detection ---

#[test]
fn test_clean_pass_no_duplicates() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: implement feature A\n"),
        ("b.rs", "// TODO: implement feature B\n"),
    ]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_clean_fail_duplicates() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: implement feature\n"),
        ("b.rs", "// TODO: implement feature\n"),
    ]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success() // No --check, so always exit 0
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("duplicate"));
}

#[test]
fn test_clean_duplicates_case_insensitive() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: Implement Feature\n"),
        ("b.rs", "// TODO: implement feature\n"),
    ]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate"));
}

#[test]
fn test_clean_duplicates_whitespace_normalized() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: implement   feature\n"),
        ("b.rs", "// TODO: implement feature\n"),
    ]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate"));
}

// --- --check flag exit codes ---

#[test]
fn test_clean_check_exit_0_no_violations() {
    let dir = setup_project(&[("a.rs", "// TODO: unique message\n")]);

    todox()
        .args(["clean", "--check", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(0);
}

#[test]
fn test_clean_check_exit_1_with_violations() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: same message\n"),
        ("b.rs", "// TODO: same message\n"),
    ]);

    todox()
        .args(["clean", "--check", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(1);
}

#[test]
fn test_clean_no_check_exit_0_even_with_violations() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: same message\n"),
        ("b.rs", "// TODO: same message\n"),
    ]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .code(0);
}

// --- Output formats ---

#[test]
fn test_clean_json_format_pass() {
    let dir = setup_project(&[("a.rs", "// TODO: unique task\n")]);

    todox()
        .args([
            "clean",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": true"))
        .stdout(predicate::str::contains("\"duplicate_count\": 0"))
        .stdout(predicate::str::contains("\"stale_count\": 0"));
}

#[test]
fn test_clean_json_format_fail() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: same task\n"),
        ("b.rs", "// TODO: same task\n"),
    ]);

    todox()
        .args([
            "clean",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": false"))
        .stdout(predicate::str::contains("\"rule\": \"duplicate\""));
}

#[test]
fn test_clean_github_actions_format_pass() {
    let dir = setup_project(&[("a.rs", "// TODO: unique task\n")]);

    todox()
        .args([
            "clean",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("::notice::todox clean: PASS"));
}

#[test]
fn test_clean_github_actions_format_fail() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: same task\n"),
        ("b.rs", "// TODO: same task\n"),
    ]);

    todox()
        .args([
            "clean",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("::error"))
        .stdout(predicate::str::contains("todox clean: FAIL"));
}

#[test]
fn test_clean_sarif_format_pass() {
    let dir = setup_project(&[("a.rs", "// TODO: unique task\n")]);

    todox()
        .args([
            "clean",
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
fn test_clean_sarif_format_fail() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: same task\n"),
        ("b.rs", "// TODO: same task\n"),
    ]);

    todox()
        .args([
            "clean",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "sarif",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("todox/clean/duplicate"))
        .stdout(predicate::str::contains("\"level\": \"error\""));
}

#[test]
fn test_clean_markdown_format_pass() {
    let dir = setup_project(&[("a.rs", "// TODO: unique task\n")]);

    todox()
        .args([
            "clean",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("## PASS"))
        .stdout(predicate::str::contains("All clean checks passed"));
}

#[test]
fn test_clean_markdown_format_fail() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: same task\n"),
        ("b.rs", "// TODO: same task\n"),
    ]);

    todox()
        .args([
            "clean",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("## FAIL"))
        .stdout(predicate::str::contains("duplicate"));
}

// --- Empty project ---

#[test]
fn test_clean_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- No issue refs → no stale violations ---

#[test]
fn test_clean_no_issue_refs_no_stale() {
    let dir = setup_project(&[("a.rs", "// TODO: no issue reference here\n")]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

// --- Config file integration ---

#[test]
fn test_clean_config_disables_duplicates() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: same message\n"),
        ("b.rs", "// TODO: same message\n"),
        (
            ".todox.toml",
            r#"
[clean]
duplicates = false
"#,
        ),
    ]);

    todox()
        .args(["clean", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}
