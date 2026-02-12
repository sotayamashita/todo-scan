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
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): fix issue #123\n",
    )]);

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
