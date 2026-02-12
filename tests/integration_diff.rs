use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process;
use tempfile::TempDir;

fn todox() -> Command {
    Command::cargo_bin("todox").unwrap()
}

fn setup_git_repo(initial_files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    let cwd = dir.path();

    // Initialize git repo
    process::Command::new("git")
        .args(["init"])
        .current_dir(cwd)
        .output()
        .unwrap();

    process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(cwd)
        .output()
        .unwrap();

    process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(cwd)
        .output()
        .unwrap();

    // Create initial files and commit
    for (path, content) in initial_files {
        let full_path = cwd.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    process::Command::new("git")
        .args(["add", "."])
        .current_dir(cwd)
        .output()
        .unwrap();

    process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(cwd)
        .output()
        .unwrap();

    dir
}

#[test]
fn test_diff_shows_added_todos() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
    let cwd = dir.path();

    // Add a TODO after the initial commit
    fs::write(cwd.join("main.rs"), "// TODO: new feature\nfn main() {}\n").unwrap();

    todox()
        .args(["diff", "HEAD", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("+"))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("new feature"));
}

#[test]
fn test_diff_shows_removed_todos() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: old task\nfn main() {}\n")]);
    let cwd = dir.path();

    // Remove the TODO
    fs::write(cwd.join("main.rs"), "fn main() {}\n").unwrap();

    todox()
        .args(["diff", "HEAD", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("-"))
        .stdout(predicate::str::contains("old task"));
}

#[test]
fn test_diff_json_format() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
    let cwd = dir.path();

    fs::write(cwd.join("main.rs"), "// FIXME: urgent fix\nfn main() {}\n").unwrap();

    todox()
        .args([
            "diff",
            "HEAD",
            "--root",
            cwd.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"added\""))
        .stdout(predicate::str::contains("\"tag\": \"FIXME\""));
}

#[test]
fn test_diff_no_changes() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: existing\nfn main() {}\n")]);
    let cwd = dir.path();

    // Don't modify files - diff should show nothing
    todox()
        .args(["diff", "HEAD", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("+0 -0"));
}
