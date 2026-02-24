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
fn test_stats_basic_output() {
    let dir = setup_project(&[
        (
            "main.rs",
            "// TODO: implement feature\n// FIXME: broken\n// TODO: another task\n",
        ),
        ("lib.rs", "// HACK: workaround\n"),
    ]);

    todo_scan()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tags"))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("HACK"))
        .stdout(predicate::str::contains("4 items across 2 files"));
}

#[test]
fn test_stats_priority_counts() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO!!: urgent task\n// TODO!: high task\n// TODO: normal task\n",
    )]);

    todo_scan()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("normal: 1"))
        .stdout(predicate::str::contains("high: 1"))
        .stdout(predicate::str::contains("urgent: 1"));
}

#[test]
fn test_stats_with_authors() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): alice task\n// TODO(bob): bob task\n// TODO: no author\n",
    )]);

    todo_scan()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Authors"))
        .stdout(predicate::str::contains("alice"))
        .stdout(predicate::str::contains("bob"))
        .stdout(predicate::str::contains("unassigned"));
}

#[test]
fn test_stats_hotspot_files() {
    let dir = setup_project(&[
        ("main.rs", "// TODO: one\n// TODO: two\n// TODO: three\n"),
        ("lib.rs", "// TODO: single\n"),
    ]);

    todo_scan()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hotspots"))
        .stdout(predicate::str::contains("main.rs (3)"))
        .stdout(predicate::str::contains("lib.rs (1)"));
}

#[test]
fn test_stats_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: json test\n// FIXME: another\n")]);

    todo_scan()
        .args([
            "stats",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_items\": 2"))
        .stdout(predicate::str::contains("\"total_files\": 1"))
        .stdout(predicate::str::contains("\"tag_counts\""))
        .stdout(predicate::str::contains("\"priority_counts\""));
}

#[test]
fn test_stats_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todo_scan()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 items across 0 files"));
}

#[test]
fn test_stats_bar_chart_present() {
    let dir = setup_project(&[("main.rs", "// TODO: one\n// TODO: two\n// FIXME: three\n")]);

    todo_scan()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{2588}"));
}

// --- Stats with --since (trend line) ---

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
fn test_stats_with_trend() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: existing task\nfn main() {}\n")]);
    let cwd = dir.path();

    // Add new TODOs
    fs::write(
        cwd.join("main.rs"),
        "// TODO: existing task\n// FIXME: new fix\nfn main() {}\n",
    )
    .unwrap();

    todo_scan()
        .args(["stats", "--root", cwd.to_str().unwrap(), "--since", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Trend since HEAD"))
        .stdout(predicate::str::contains("added"));
}

#[test]
fn test_stats_with_trend_json() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: existing task\nfn main() {}\n")]);
    let cwd = dir.path();

    fs::write(
        cwd.join("main.rs"),
        "// TODO: existing task\n// FIXME: new fix\nfn main() {}\n",
    )
    .unwrap();

    let output = todo_scan()
        .args([
            "stats",
            "--root",
            cwd.to_str().unwrap(),
            "--since",
            "HEAD",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("trend").is_some());
    assert_eq!(json["trend"]["base_ref"].as_str().unwrap(), "HEAD");
}
