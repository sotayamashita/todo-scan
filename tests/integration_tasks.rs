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
fn test_tasks_dry_run_outputs_json() {
    let dir = setup_project(&[("main.rs", "// TODO: implement feature\n// BUG: fix crash\n")]);

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"subject\""))
        .stdout(predicate::str::contains("\"activeForm\""))
        .stdout(predicate::str::contains("\"metadata\""))
        .stdout(predicate::str::contains("\"total\": 2"));
}

#[test]
fn test_tasks_subject_action_verb_prefix() {
    let dir = setup_project(&[(
        "main.rs",
        "// BUG: crash on startup\n// TODO: add logging\n// HACK: temp workaround\n",
    )]);

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Fix crash on startup"))
        .stdout(predicate::str::contains("Implement add logging"))
        .stdout(predicate::str::contains("Refactor temp workaround"));
}

#[test]
fn test_tasks_tag_filter() {
    let dir = setup_project(&[
        ("bug.rs", "// BUG: critical bug\n"),
        ("task.rs", "// TODO: normal task\n"),
        ("hack.rs", "// HACK: quick hack\n"),
    ]);

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
            "--tag",
            "BUG",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 1"))
        .stdout(predicate::str::contains("critical bug"))
        .stdout(predicate::str::contains("normal task").not());
}

#[test]
fn test_tasks_priority_ordering() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: normal task\n// TODO!!: urgent task\n// TODO!: high task\n",
    )]);

    let output = todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let tasks = json["tasks"].as_array().unwrap();

    assert_eq!(tasks[0]["metadata"]["todox_priority"], "urgent");
    assert_eq!(tasks[1]["metadata"]["todox_priority"], "high");
    assert_eq!(tasks[2]["metadata"]["todox_priority"], "normal");
}

#[test]
fn test_tasks_context_in_description() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n    let z = 3;\n}\n",
    )]);

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
            "-C",
            "2",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("let y = 2"));
}

#[test]
fn test_tasks_output_writes_files() {
    let dir = setup_project(&[("main.rs", "// TODO: first task\n// BUG: second task\n")]);

    let output_dir = dir.path().join("tasks-output");

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--output",
            output_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(output_dir.exists());
    assert!(output_dir.join("task-0001.json").exists());
    assert!(output_dir.join("task-0002.json").exists());

    // Verify file content is valid JSON with expected fields
    let content = fs::read_to_string(output_dir.join("task-0001.json")).unwrap();
    let task: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(task.get("subject").is_some());
    assert!(task.get("activeForm").is_some());
    assert!(task.get("metadata").is_some());
}

#[test]
fn test_tasks_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 0"))
        .stdout(predicate::str::contains("\"tasks\": []"));
}

#[test]
fn test_tasks_metadata_fields() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    let output = todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let meta = &json["tasks"][0]["metadata"];

    assert_eq!(meta["todox_file"], "main.rs");
    assert_eq!(meta["todox_line"], 1);
    assert_eq!(meta["todox_tag"], "TODO");
    assert_eq!(meta["todox_author"], "alice");
    assert_eq!(meta["todox_issue_ref"], "#123");
    assert!(meta.get("todox_match_key").is_some());
}

#[test]
fn test_tasks_dry_run_skips_file_write() {
    let dir = setup_project(&[("main.rs", "// TODO: some task\n")]);

    let output_dir = dir.path().join("tasks-output");

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--output",
            output_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"subject\""));

    // Directory should NOT be created when --dry-run is set
    assert!(!output_dir.exists());
}

#[test]
fn test_tasks_text_format() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: implement feature\n// BUG: critical crash\n",
    )]);

    todox()
        .args(["tasks", "--root", dir.path().to_str().unwrap(), "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tasks exported"))
        .stdout(predicate::str::contains("Implement implement feature"))
        .stdout(predicate::str::contains("Fix critical crash"));
}

#[test]
fn test_tasks_filter_by_author() {
    let dir = setup_project(&[
        ("alice.rs", "// TODO(alice): alice task\n"),
        ("bob.rs", "// TODO(bob): bob task\n"),
    ]);

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
            "--author",
            "alice",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 1"))
        .stdout(predicate::str::contains("alice task"))
        .stdout(predicate::str::contains("bob task").not());
}

#[test]
fn test_tasks_filter_by_path() {
    let dir = setup_project(&[
        ("src/lib.rs", "// TODO: in src\n"),
        ("tests/test.rs", "// TODO: in tests\n"),
    ]);

    todox()
        .args([
            "tasks",
            "--root",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
            "--path",
            "src/**",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 1"))
        .stdout(predicate::str::contains("in src"))
        .stdout(predicate::str::contains("in tests").not());
}
