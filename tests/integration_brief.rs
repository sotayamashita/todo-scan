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
fn test_brief_basic_output() {
    let dir = setup_project(&[
        ("main.rs", "// TODO: implement feature\n// FIXME: broken\n"),
        ("lib.rs", "// HACK: workaround\n"),
    ]);

    todo_scan()
        .args(["brief", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 TODOs across 2 files"));
}

#[test]
fn test_brief_with_priorities() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO!!: urgent task\n// TODO!: high task\n// TODO: normal task\n",
    )]);

    todo_scan()
        .args(["brief", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 urgent"))
        .stdout(predicate::str::contains("1 high"));
}

#[test]
fn test_brief_no_priority_shown_when_all_normal() {
    let dir = setup_project(&[("main.rs", "// TODO: task one\n// TODO: task two\n")]);

    todo_scan()
        .args(["brief", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 TODOs across 1 files"))
        .stdout(predicate::str::contains("urgent").not())
        .stdout(predicate::str::contains("high").not());
}

#[test]
fn test_brief_top_urgent_line() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: normal\n// BUG!!: crash on large files\n",
    )]);

    todo_scan()
        .args(["brief", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Top urgent:"))
        .stdout(predicate::str::contains("BUG!!"))
        .stdout(predicate::str::contains("crash on large files"));
}

#[test]
fn test_brief_top_urgent_high_priority() {
    // Only high-priority items (no urgent), so top_urgent shows "!" marker
    let dir = setup_project(&[("main.rs", "// TODO: normal\n// BUG!: high priority crash\n")]);

    todo_scan()
        .args(["brief", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Top urgent:"))
        .stdout(predicate::str::contains("BUG!"))
        .stdout(predicate::str::contains("high priority crash"));
}

#[test]
fn test_brief_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: json test\n// FIXME!: high fix\n")]);

    todo_scan()
        .args([
            "brief",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_items\": 2"))
        .stdout(predicate::str::contains("\"total_files\": 1"))
        .stdout(predicate::str::contains("\"priority_counts\""))
        .stdout(predicate::str::contains("\"top_urgent\""));
}

#[test]
fn test_brief_budget_limits_lines() {
    let dir = setup_project(&[("main.rs", "// TODO!!: urgent task\n// TODO: normal task\n")]);

    // With budget=1, only the summary line should appear
    todo_scan()
        .args([
            "brief",
            "--root",
            dir.path().to_str().unwrap(),
            "--budget",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODOs across"))
        .stdout(predicate::str::contains("Top urgent:").not());
}

#[test]
fn test_brief_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todo_scan()
        .args(["brief", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 TODOs across 0 files"));
}

// --- Brief with --since (trend line) ---

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
fn test_brief_with_trend() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: old task\nfn main() {}\n")]);
    let cwd = dir.path();

    // Add a new TODO
    fs::write(
        cwd.join("main.rs"),
        "// TODO: old task\n// TODO: new task\nfn main() {}\n",
    )
    .unwrap();

    todo_scan()
        .args(["brief", "--root", cwd.to_str().unwrap(), "--since", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 TODOs across 1 files"))
        .stdout(predicate::str::contains("Trends vs HEAD"))
        .stdout(predicate::str::contains("+1 added"));
}
