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
fn test_context_text_output() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n    let z = 3;\n}\n",
    )]);

    todo_scan()
        .args([
            "context",
            "main.rs:3",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("main.rs:3"))
        .stdout(predicate::str::contains("TODO: fix this"))
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("let y = 2"));
}

#[test]
fn test_context_json_output() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n}\n",
    )]);

    todo_scan()
        .args([
            "context",
            "main.rs:3",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"file\": \"main.rs\""))
        .stdout(predicate::str::contains("\"line\": 3"))
        .stdout(predicate::str::contains("\"todo_line\""))
        .stdout(predicate::str::contains("\"before\""))
        .stdout(predicate::str::contains("\"after\""));
}

#[test]
fn test_context_related_todos() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: first todo\nfn main() {\n    // TODO: second todo\n    let x = 1;\n}\n",
    )]);

    todo_scan()
        .args([
            "context",
            "main.rs:3",
            "-C",
            "5",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"related_todos\""))
        .stdout(predicate::str::contains("first todo"));
}

#[test]
fn test_context_related_todos_text_format() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: first todo\nfn main() {\n    // TODO: second todo\n    let x = 1;\n}\n",
    )]);

    todo_scan()
        .args([
            "context",
            "main.rs:3",
            "-C",
            "5",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Related TODOs:"))
        .stdout(predicate::str::contains("first todo"));
}

#[test]
fn test_context_invalid_format() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todo_scan()
        .args(["context", "main.rs", "--root", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid location format"));
}

#[test]
fn test_context_file_not_found() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todo_scan()
        .args([
            "context",
            "nonexistent.rs:1",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot read file"));
}

#[test]
fn test_context_custom_window() {
    let dir = setup_project(&[(
        "main.rs",
        "line1\nline2\nline3\n// TODO: target\nline5\nline6\nline7\nline8\n",
    )]);

    todo_scan()
        .args([
            "context",
            "main.rs:4",
            "-C",
            "1",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"line_number\": 3"))
        .stdout(predicate::str::contains("\"line_number\": 5"))
        // Lines beyond context=1 should not appear
        .stdout(predicate::str::contains("\"line_number\": 1").not())
        .stdout(predicate::str::contains("\"line_number\": 6").not());
}

#[test]
fn test_context_resolves_stable_id() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n}\n",
    )]);

    // Use the stable ID format instead of file:line
    todo_scan()
        .args([
            "context",
            "main.rs:TODO:fix this",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("main.rs:3"))
        .stdout(predicate::str::contains("TODO: fix this"));
}
