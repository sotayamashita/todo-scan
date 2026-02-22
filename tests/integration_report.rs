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

fn setup_git_project(files: &[(&str, &str)]) -> TempDir {
    let dir = setup_project(files);
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    dir
}

#[test]
fn test_report_creates_html_file() {
    let dir = setup_project(&[("main.rs", "// TODO: test report\n")]);
    let output_path = dir.path().join("report.html");

    todo_scan()
        .args([
            "report",
            "--root",
            dir.path().to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--history",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Report written to"));

    let content = fs::read_to_string(&output_path).expect("report file should exist");
    assert!(content.starts_with("<!DOCTYPE html>"));
}

#[test]
fn test_report_default_output_path() {
    let dir = setup_project(&[("main.rs", "// TODO: test default\n")]);

    todo_scan()
        .current_dir(dir.path())
        .args(["report", "--history", "0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("todo-scan-report.html"));

    let default_path = dir.path().join("todo-scan-report.html");
    assert!(default_path.exists(), "default report file should exist");
}

#[test]
fn test_report_custom_output_path() {
    let dir = setup_project(&[("main.rs", "// TODO: custom path\n")]);
    let custom_path = dir.path().join("my-report.html");

    todo_scan()
        .args([
            "report",
            "--root",
            dir.path().to_str().unwrap(),
            "--output",
            custom_path.to_str().unwrap(),
            "--history",
            "0",
        ])
        .assert()
        .success();

    assert!(custom_path.exists(), "custom output path should exist");
}

#[test]
fn test_report_contains_all_sections() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice)!: urgent task\n// FIXME: fix this\n// HACK: workaround\n",
    )]);
    let output_path = dir.path().join("report.html");

    todo_scan()
        .args([
            "report",
            "--root",
            dir.path().to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--history",
            "0",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&output_path).unwrap();
    // Summary section
    assert!(content.contains("Total Items"));
    assert!(content.contains("Urgent"));
    assert!(content.contains("High Priority"));
    // Charts
    assert!(content.contains("chart-tags"));
    assert!(content.contains("chart-priority"));
    assert!(content.contains("chart-age"));
    // Table
    assert!(content.contains("items-table"));
    // Data
    assert!(content.contains("REPORT_DATA"));
}

#[test]
fn test_report_with_history() {
    let dir = setup_git_project(&[("main.rs", "// TODO: first task\n")]);

    // Add a second commit
    fs::write(
        dir.path().join("main.rs"),
        "// TODO: first task\n// TODO: second task\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "add second todo"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let output_path = dir.path().join("report.html");

    todo_scan()
        .args([
            "report",
            "--root",
            dir.path().to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--history",
            "5",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&output_path).unwrap();
    // Should contain history data (not the empty message)
    assert!(content.contains("\"history\""));
}

#[test]
fn test_report_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);
    let output_path = dir.path().join("report.html");

    todo_scan()
        .args([
            "report",
            "--root",
            dir.path().to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--history",
            "0",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.starts_with("<!DOCTYPE html>"));
    // Should have zero counts
    assert!(content.contains("\"total_items\":0"));
}

#[test]
fn test_report_embedded_json_valid() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: task one\n// FIXME(bob): task two\n// BUG!!: critical\n",
    )]);
    let output_path = dir.path().join("report.html");

    todo_scan()
        .args([
            "report",
            "--root",
            dir.path().to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--history",
            "0",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&output_path).unwrap();
    // Extract and parse REPORT_DATA JSON
    let start_marker = "const REPORT_DATA = ";
    let start = content
        .find(start_marker)
        .expect("REPORT_DATA should exist")
        + start_marker.len();
    let end = content[start..].find(";\n").unwrap() + start;
    let json_str = &content[start..end];
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).expect("embedded JSON should be valid");

    assert!(parsed["summary"]["total_items"].as_u64().unwrap() >= 3);
    assert!(parsed["items"].as_array().unwrap().len() >= 3);
}
