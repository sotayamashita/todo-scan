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
fn test_search_basic_substring_case_insensitive() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: Implement Feature\n// FIXME: broken thing\n",
    )]);

    todox()
        .args([
            "search",
            "implement",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Implement Feature"))
        .stdout(predicate::str::contains("1 matches across 1 files"));
}

#[test]
fn test_search_exact_flag() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: Implement Feature\n// TODO: implement other\n",
    )]);

    // With --exact, case matters
    todox()
        .args([
            "search",
            "Implement",
            "--exact",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Implement Feature"))
        .stdout(predicate::str::contains("implement other").not())
        .stdout(predicate::str::contains("1 matches across 1 files"));
}

#[test]
fn test_search_issue_ref() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): fix issue #123\n// TODO: other\n",
    )]);

    todox()
        .args(["search", "#123", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix issue"))
        .stdout(predicate::str::contains("1 matches across 1 files"));
}

#[test]
fn test_search_author_filter() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): alice task fix\n// TODO(bob): bob task fix\n",
    )]);

    todox()
        .args([
            "search",
            "fix",
            "--author",
            "alice",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice task fix"))
        .stdout(predicate::str::contains("bob task fix").not())
        .stdout(predicate::str::contains("1 matches"));
}

#[test]
fn test_search_tag_filter() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: todo task\n// FIXME: fixme task\n// BUG: bug task\n",
    )]);

    todox()
        .args([
            "search",
            "task",
            "--tag",
            "FIXME",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fixme task"))
        .stdout(predicate::str::contains("todo task").not())
        .stdout(predicate::str::contains("bug task").not())
        .stdout(predicate::str::contains("1 matches"));
}

#[test]
fn test_search_path_filter() {
    let dir = setup_project(&[
        ("src/lib.rs", "// TODO: fix src lib\n"),
        ("tests/test.rs", "// TODO: fix test\n"),
    ]);

    todox()
        .args([
            "search",
            "fix",
            "--path",
            "src/**",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix src lib"))
        .stdout(predicate::str::contains("fix test").not())
        .stdout(predicate::str::contains("1 matches"));
}

#[test]
fn test_search_no_matches() {
    let dir = setup_project(&[("main.rs", "// TODO: something\n")]);

    todox()
        .args([
            "search",
            "nonexistent",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 matches across 0 files"));
}

#[test]
fn test_search_summary_line_format() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: fix alpha\n"),
        ("b.rs", "// TODO: fix beta\n"),
    ]);

    todox()
        .args(["search", "fix", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2 matches across 2 files (query: \"fix\")",
        ));
}

#[test]
fn test_search_context_lines() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n    let z = 3;\n}\n",
    )]);

    todox()
        .args([
            "search",
            "fix",
            "-C",
            "2",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("fix this"))
        .stdout(predicate::str::contains("let y = 2"));
}

#[test]
fn test_search_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: fix the bug\n// TODO: other thing\n")]);

    todox()
        .args([
            "search",
            "fix",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"query\": \"fix\""))
        .stdout(predicate::str::contains("\"exact\": false"))
        .stdout(predicate::str::contains("\"match_count\": 1"))
        .stdout(predicate::str::contains("\"file_count\": 1"))
        .stdout(predicate::str::contains("\"message\": \"fix the bug\""));
}

#[test]
fn test_search_alias_s() {
    let dir = setup_project(&[("main.rs", "// TODO: alias test\n")]);

    todox()
        .args(["s", "alias", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("alias test"))
        .stdout(predicate::str::contains("1 matches"));
}
