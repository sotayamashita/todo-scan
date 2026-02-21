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

#[test]
fn test_diff_with_multiple_files_only_one_changed() {
    let dir = setup_git_repo(&[
        ("a.rs", "// TODO: task in a\nfn a() {}\n"),
        ("b.rs", "// TODO: task in b\nfn b() {}\n"),
    ]);
    let cwd = dir.path();

    // Only modify a.rs, leave b.rs unchanged
    fs::write(
        cwd.join("a.rs"),
        "// TODO: task in a\n// TODO: new task in a\nfn a() {}\n",
    )
    .unwrap();

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
        .stdout(predicate::str::contains("new task in a"))
        .stdout(predicate::str::contains("\"status\": \"added\""))
        // b.rs should not appear in diff at all
        .stdout(predicate::str::contains("task in b").not());
}

#[test]
fn test_diff_deleted_file() {
    let dir = setup_git_repo(&[
        ("main.rs", "fn main() {}\n"),
        (
            "removeme.rs",
            "// TODO: this will be removed\nfn gone() {}\n",
        ),
    ]);
    let cwd = dir.path();

    // Delete the file
    fs::remove_file(cwd.join("removeme.rs")).unwrap();

    todox()
        .args(["diff", "HEAD", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("-"))
        .stdout(predicate::str::contains("this will be removed"));
}

#[test]
fn test_diff_new_untracked_file() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
    let cwd = dir.path();

    // Add a new untracked file with TODOs
    fs::write(
        cwd.join("newfile.rs"),
        "// TODO: brand new task\nfn new_fn() {}\n",
    )
    .unwrap();

    todox()
        .args(["diff", "HEAD", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("+"))
        .stdout(predicate::str::contains("brand new task"));
}

#[test]
fn test_diff_unchanged_files_no_false_positives() {
    let dir = setup_git_repo(&[
        ("a.rs", "// TODO: task a\nfn a() {}\n"),
        ("b.rs", "// FIXME: task b\nfn b() {}\n"),
        ("c.rs", "// HACK: task c\nfn c() {}\n"),
    ]);
    let cwd = dir.path();

    // Don't modify any files
    todox()
        .args(["diff", "HEAD", "--root", cwd.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("+0 -0"));
}

#[test]
fn test_diff_renamed_file() {
    let dir = setup_git_repo(&[(
        "old_name.rs",
        "// TODO: task in renamed file\nfn old() {}\n",
    )]);
    let cwd = dir.path();

    // Rename the file (without git mv, just filesystem rename)
    fs::rename(cwd.join("old_name.rs"), cwd.join("new_name.rs")).unwrap();

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
        // Old file's TODOs should be removed
        .stdout(predicate::str::contains("\"status\": \"removed\""))
        // New file's TODOs should be added
        .stdout(predicate::str::contains("\"status\": \"added\""));
}

#[test]
fn test_diff_mixed_changes() {
    let dir = setup_git_repo(&[
        ("keep.rs", "// TODO: keep this\nfn keep() {}\n"),
        ("modify.rs", "// TODO: existing in modify\nfn modify() {}\n"),
        ("delete.rs", "// TODO: will be deleted\nfn delete() {}\n"),
    ]);
    let cwd = dir.path();

    // keep.rs: unchanged
    // modify.rs: add a new TODO
    fs::write(
        cwd.join("modify.rs"),
        "// TODO: existing in modify\n// FIXME: new fixme in modify\nfn modify() {}\n",
    )
    .unwrap();
    // delete.rs: remove file
    fs::remove_file(cwd.join("delete.rs")).unwrap();
    // add.rs: new file
    fs::write(cwd.join("add.rs"), "// BUG: found a bug\nfn add() {}\n").unwrap();

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
        // modify.rs: new FIXME should be added
        .stdout(predicate::str::contains("new fixme in modify"))
        // delete.rs: TODO should be removed
        .stdout(predicate::str::contains("will be deleted"))
        // add.rs: BUG should be added
        .stdout(predicate::str::contains("found a bug"))
        // keep.rs: should NOT appear in diff
        .stdout(predicate::str::contains("keep this").not());
}

#[test]
fn test_diff_github_actions_format() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
    let cwd = dir.path();

    fs::write(cwd.join("main.rs"), "// TODO: new feature\nfn main() {}\n").unwrap();

    todox()
        .args([
            "diff",
            "HEAD",
            "--root",
            cwd.to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "::warning file=main.rs,line=1,title=TODO::[TODO] new feature",
        ))
        .stdout(predicate::str::contains("::notice::todox diff: +1 -0"));
}

#[test]
fn test_diff_sarif_format() {
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
            "sarif",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"2.1.0\""))
        .stdout(predicate::str::contains("\"diffStatus\": \"added\""))
        .stdout(predicate::str::contains("\"ruleId\": \"todox/FIXME\""));
}

#[test]
fn test_diff_markdown_format() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {}\n")]);
    let cwd = dir.path();

    fs::write(cwd.join("main.rs"), "// TODO: new task\nfn main() {}\n").unwrap();

    todox()
        .args([
            "diff",
            "HEAD",
            "--root",
            cwd.to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "| Status | File | Line | Tag | Message |",
        ))
        .stdout(predicate::str::contains("| + |"))
        .stdout(predicate::str::contains("new task"));
}

#[test]
fn test_diff_with_context() {
    let dir = setup_git_repo(&[("main.rs", "fn main() {\n    let x = 1;\n}\n")]);
    let cwd = dir.path();

    fs::write(
        cwd.join("main.rs"),
        "fn main() {\n    let x = 1;\n    // TODO: new feature\n    let y = 2;\n}\n",
    )
    .unwrap();

    todox()
        .args(["diff", "HEAD", "--root", cwd.to_str().unwrap(), "-C", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("new feature"))
        .stdout(predicate::str::contains("let x = 1"));
}
