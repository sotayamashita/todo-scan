use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process;
use tempfile::TempDir;

fn todo_scan() -> Command {
    assert_cmd::cargo_bin_cmd!("todo-scan")
}

fn setup_git_repo(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    let cwd = dir.path();

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
        .args(["config", "user.name", "Test Author"])
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
fn test_blame_basic_output() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: implement feature\nfn main() {}\n")]);
    let cwd = dir.path();

    todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Author"))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("implement feature"))
        .stdout(predicate::str::contains("days ago"));
}

#[test]
fn test_blame_sort_by_age() {
    let dir = setup_git_repo(&[
        ("a.rs", "// TODO: old task\n"),
        ("b.rs", "// FIXME: another task\n"),
    ]);
    let cwd = dir.path();

    todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap(), "--sort", "age"])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("FIXME"));
}

#[test]
fn test_blame_filter_by_author() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: task one\n// FIXME: task two\n")]);
    let cwd = dir.path();

    // Filter by "Test Author" (our git config user)
    todo_scan()
        .args([
            "blame",
            "--root",
            cwd.to_str().unwrap(),
            "--author",
            "Test Author",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("task one"))
        .stdout(predicate::str::contains("task two"));

    // Filter by non-existent author
    todo_scan()
        .args([
            "blame",
            "--root",
            cwd.to_str().unwrap(),
            "--author",
            "Nobody",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("task one").not())
        .stdout(predicate::str::contains("0 items"));
}

#[test]
fn test_blame_filter_by_min_age() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: recent task\n")]);
    let cwd = dir.path();

    // Min-age 0d should include everything
    todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap(), "--min-age", "0d"])
        .assert()
        .success()
        .stdout(predicate::str::contains("recent task"));

    // Min-age 99999d should exclude everything
    todo_scan()
        .args([
            "blame",
            "--root",
            cwd.to_str().unwrap(),
            "--min-age",
            "99999d",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("recent task").not());
}

#[test]
fn test_blame_json_format() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: test json\nfn main() {}\n")]);
    let cwd = dir.path();

    todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap(), "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"author\""))
        .stdout(predicate::str::contains("\"age_days\""))
        .stdout(predicate::str::contains("\"stale\""))
        .stdout(predicate::str::contains("\"avg_age_days\""));
}

#[test]
fn test_blame_markdown_format() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: markdown test\n")]);
    let cwd = dir.path();

    todo_scan()
        .args([
            "blame",
            "--root",
            cwd.to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "| File | Line | Tag | Message | Author | Date | Age (days) | Stale |",
        ))
        .stdout(predicate::str::contains("Test Author"));
}

#[test]
fn test_blame_github_actions_format() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: ga test\n")]);
    let cwd = dir.path();

    todo_scan()
        .args([
            "blame",
            "--root",
            cwd.to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("::notice"))
        .stdout(predicate::str::contains("todo-scan blame:"));
}

#[test]
fn test_blame_sarif_format() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: sarif test\n")]);
    let cwd = dir.path();

    todo_scan()
        .args([
            "blame",
            "--root",
            cwd.to_str().unwrap(),
            "--format",
            "sarif",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"2.1.0\""))
        .stdout(predicate::str::contains("\"blame\""))
        .stdout(predicate::str::contains("\"ageDays\""));
}

#[test]
fn test_blame_stale_marker() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: stale test\n")]);
    let cwd = dir.path();

    // With a threshold of 0 days, everything is stale
    todo_scan()
        .args([
            "blame",
            "--root",
            cwd.to_str().unwrap(),
            "--stale-threshold",
            "0d",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("STALE"));
}

#[test]
fn test_blame_summary_line() {
    let dir = setup_git_repo(&[("a.rs", "// TODO: first\n"), ("b.rs", "// FIXME: second\n")]);
    let cwd = dir.path();

    todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items"))
        .stdout(predicate::str::contains("avg age"))
        .stdout(predicate::str::contains("stale"));
}

#[test]
fn test_blame_tag_filter() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: keep\n// FIXME: filter out\n")]);
    let cwd = dir.path();

    todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap(), "--tag", "TODO"])
        .assert()
        .success()
        .stdout(predicate::str::contains("keep"))
        .stdout(predicate::str::contains("filter out").not());
}

#[test]
fn test_blame_empty_repo() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
    let cwd = dir.path();

    todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 items"));
}

#[test]
fn test_blame_json_contains_id_field() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: blame id test\nfn main() {}\n")]);
    let cwd = dir.path();

    let output = todo_scan()
        .args(["blame", "--root", cwd.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let entry = &json["entries"][0];
    assert_eq!(entry["id"].as_str().unwrap(), "main.rs:TODO:blame id test");
}

// --- Blame sort by author ---

#[test]
fn test_blame_sort_by_author() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: task a\n// FIXME: task b\n")]);

    todo_scan()
        .args([
            "blame",
            "--root",
            dir.path().to_str().unwrap(),
            "--sort",
            "author",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items"));
}

// --- Blame sort by tag ---

#[test]
fn test_blame_sort_by_tag() {
    let dir = setup_git_repo(&[(
        "main.rs",
        "// NOTE: a note\n// BUG: a bug\n// TODO: a task\n",
    )]);

    todo_scan()
        .args([
            "blame",
            "--root",
            dir.path().to_str().unwrap(),
            "--sort",
            "tag",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 items"));
}

// --- Blame path filter ---

#[test]
fn test_blame_path_filter() {
    let dir = setup_git_repo(&[
        ("src/lib.rs", "// TODO: in src\n"),
        ("tests/test.rs", "// TODO: in tests\n"),
    ]);

    todo_scan()
        .args([
            "blame",
            "--root",
            dir.path().to_str().unwrap(),
            "--path",
            "src/**",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("in src"))
        .stdout(predicate::str::contains("in tests").not())
        .stdout(predicate::str::contains("1 items"));
}

// --- Blame config stale threshold ---

#[test]
fn test_blame_config_stale_threshold() {
    let dir = setup_git_repo(&[("main.rs", "// TODO: stale check\n")]);

    // Write config with stale_threshold
    fs::write(
        dir.path().join(".todo-scan.toml"),
        "[blame]\nstale_threshold = \"1d\"\n",
    )
    .unwrap();

    todo_scan()
        .args(["blame", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("threshold: 1 days"));
}
