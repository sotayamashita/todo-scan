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
fn test_list_finds_todos() {
    let dir = setup_project(&[
        (
            "main.rs",
            "// TODO: implement feature\nfn main() {}\n// FIXME: broken\n",
        ),
        ("lib.rs", "// HACK: workaround\n"),
    ]);

    todox()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("HACK"))
        .stdout(predicate::str::contains("3 items"));
}

#[test]
fn test_list_tag_filter() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: task one\n// FIXME: task two\n// HACK: task three\n",
    )]);

    todox()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--tag",
            "FIXME",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("1 items"));
}

#[test]
fn test_list_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: json test\n")]);

    todox()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tag\": \"TODO\""))
        .stdout(predicate::str::contains("\"message\": \"json test\""));
}

#[test]
fn test_list_alias_ls() {
    let dir = setup_project(&[("main.rs", "// TODO: alias test\n")]);

    todox()
        .args(["ls", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"));
}

#[test]
fn test_list_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todox()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 items"));
}

#[test]
fn test_list_with_author_and_issue() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    todox()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"author\": \"alice\""))
        .stdout(predicate::str::contains("\"issue_ref\": \"#123\""));
}

#[test]
fn test_list_text_issue_ref_no_double_hash() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    todox()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("(#123)"))
        .stdout(predicate::str::contains("(##123)").not());
}

#[test]
fn test_list_filters_false_positives() {
    let dir = setup_project(&[(
        "main.rs",
        r#"// TODO: real comment
let service = TodoService::new();
if isTodoCompleted() { return; }
let msg = "TODO: not real";
let todo_count = 42;
// FIXME(bob): another real comment
struct TodoItem { done: bool }
"#,
    )]);

    // Verify only 2 real comment items found (text output)
    todox()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items"));

    // Verify correct messages via JSON output
    todox()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"message\": \"real comment\""))
        .stdout(predicate::str::contains(
            "\"message\": \"another real comment\"",
        ));
}

#[test]
fn test_list_multi_language_comments() {
    let dir = setup_project(&[
        ("app.py", "# TODO: python todo\nx = 1\n"),
        ("style.css", "/* FIXME: css fixme */\n"),
        ("query.sql", "-- HACK: sql hack\n"),
        ("page.html", "<!-- NOTE: html note -->\n"),
    ]);

    todox()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("4 items"));
}

#[test]
fn test_list_github_actions_format() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: implement feature\n// BUG: critical issue\n",
    )]);

    todox()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "::warning file=main.rs,line=1,title=TODO::[TODO] implement feature",
        ))
        .stdout(predicate::str::contains(
            "::error file=main.rs,line=2,title=BUG::[BUG] critical issue",
        ))
        .stdout(predicate::str::contains("::notice::todox: 2 items found"));
}

#[test]
fn test_list_sarif_format() {
    let dir = setup_project(&[("main.rs", "// TODO: sarif test\n")]);

    todox()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "sarif",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"2.1.0\""))
        .stdout(predicate::str::contains("\"ruleId\": \"todox/TODO\""))
        .stdout(predicate::str::contains("\"text\": \"sarif test\""));
}

#[test]
fn test_list_markdown_format() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): implement feature #42\n")]);

    todox()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "| File | Line | Tag | Priority | Message | Author | Issue |",
        ))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("alice"))
        .stdout(predicate::str::contains("**1 items found**"));
}
