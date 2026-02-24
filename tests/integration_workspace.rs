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

// --- workspace list ---

#[test]
fn workspace_list_cargo() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core", "crates/cli"]
"#,
        ),
        (
            "crates/core/main.rs",
            "// TODO: implement core feature\n// FIXME: core bug\n",
        ),
        ("crates/cli/main.rs", "// TODO: implement cli\n"),
    ]);

    todo_scan()
        .args(["workspace", "list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("cargo"))
        .stdout(predicate::str::contains("core"))
        .stdout(predicate::str::contains("cli"))
        .stdout(predicate::str::contains("2 packages"));
}

#[test]
fn workspace_list_npm() {
    let dir = setup_project(&[
        ("package.json", r#"{"workspaces": ["packages/*"]}"#),
        ("packages/alpha/index.js", "// TODO: alpha task\n"),
        (
            "packages/beta/index.js",
            "// TODO: beta task\n// FIXME: beta bug\n",
        ),
    ]);

    todo_scan()
        .args(["workspace", "list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("npm"))
        .stdout(predicate::str::contains("alpha"))
        .stdout(predicate::str::contains("beta"))
        .stdout(predicate::str::contains("2 packages"));
}

#[test]
fn workspace_list_pnpm() {
    let dir = setup_project(&[
        ("pnpm-workspace.yaml", "packages:\n  - 'apps/*'\n"),
        ("apps/web/index.ts", "// TODO: web feature\n"),
    ]);

    todo_scan()
        .args(["workspace", "list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("pnpm"))
        .stdout(predicate::str::contains("web"));
}

#[test]
fn workspace_list_go() {
    let dir = setup_project(&[
        (
            "go.work",
            "go 1.21\n\nuse (\n\t./cmd/server\n\t./pkg/lib\n)\n",
        ),
        ("cmd/server/main.go", "// TODO: server setup\n"),
        ("pkg/lib/lib.go", "// HACK: lib workaround\n"),
    ]);

    todo_scan()
        .args(["workspace", "list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("go"))
        .stdout(predicate::str::contains("server"))
        .stdout(predicate::str::contains("lib"))
        .stdout(predicate::str::contains("2 packages"));
}

#[test]
fn workspace_list_json() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core"]
"#,
        ),
        ("crates/core/main.rs", "// TODO: implement\n"),
    ]);

    todo_scan()
        .args([
            "workspace",
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"core\""))
        .stdout(predicate::str::contains("\"todo_count\""))
        .stdout(predicate::str::contains("\"total_packages\": 1"));
}

#[test]
fn workspace_list_alias() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core"]
"#,
        ),
        ("crates/core/main.rs", "// TODO: implement\n"),
    ]);

    todo_scan()
        .args(["ws", "ls", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("core"));
}

// --- workspace list with package max config (text format: Ok / Over / Uncapped status) ---

#[test]
fn workspace_list_text_shows_package_status() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/ok-pkg", "crates/over-pkg", "crates/uncapped-pkg"]
"#,
        ),
        (
            ".todo-scan.toml",
            r#"
[workspace.packages.ok-pkg]
max = 10

[workspace.packages.over-pkg]
max = 1
"#,
        ),
        ("crates/ok-pkg/main.rs", "// TODO: one task\n"),
        (
            "crates/over-pkg/main.rs",
            "// TODO: first\n// TODO: second\n// TODO: third\n",
        ),
        ("crates/uncapped-pkg/main.rs", "// TODO: uncapped task\n"),
    ]);

    todo_scan()
        .args(["workspace", "list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok-pkg"))
        .stdout(predicate::str::contains("over-pkg"))
        .stdout(predicate::str::contains("uncapped-pkg"))
        .stdout(predicate::str::contains("ok"))
        .stdout(predicate::str::contains("OVER"))
        .stdout(predicate::str::contains("3 packages"));
}

// --- --package flag ---

#[test]
fn list_with_package_flag() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core", "crates/cli"]
"#,
        ),
        (
            "crates/core/main.rs",
            "// TODO: core task\n// FIXME: core bug\n",
        ),
        ("crates/cli/main.rs", "// TODO: cli task\n"),
    ]);

    // Only core package
    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--package",
            "core",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items"));

    // Only cli package
    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--package",
            "cli",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 items"));
}

#[test]
fn package_flag_invalid_name() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core"]
"#,
        ),
        ("crates/core/main.rs", "// TODO: task\n"),
    ]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--package",
            "nonexistent",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in workspace"));
}

// --- check --workspace ---

#[test]
fn check_workspace_passes() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core", "crates/cli"]
"#,
        ),
        (
            ".todo-scan.toml",
            r#"
[workspace.packages.core]
max = 10

[workspace.packages.cli]
max = 5
"#,
        ),
        ("crates/core/main.rs", "// TODO: core task\n"),
        ("crates/cli/main.rs", "// TODO: cli task\n"),
    ]);

    todo_scan()
        .args([
            "check",
            "--workspace",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn check_workspace_fails_over_max() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core"]
"#,
        ),
        (
            ".todo-scan.toml",
            r#"
[workspace.packages.core]
max = 1
"#,
        ),
        (
            "crates/core/main.rs",
            "// TODO: first\n// TODO: second\n// TODO: third\n",
        ),
    ]);

    todo_scan()
        .args([
            "check",
            "--workspace",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("workspace/max"));
}

#[test]
fn check_workspace_block_tags() {
    let dir = setup_project(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core"]
"#,
        ),
        (
            ".todo-scan.toml",
            r#"
[workspace.packages.core]
block_tags = ["BUG"]
"#,
        ),
        ("crates/core/main.rs", "// BUG: critical issue\n"),
    ]);

    todo_scan()
        .args([
            "check",
            "--workspace",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("workspace/block-tag"));
}

// --- error cases ---

#[test]
fn workspace_list_no_workspace() {
    let dir = setup_project(&[("main.rs", "// TODO: standalone\n")]);

    todo_scan()
        .args(["workspace", "list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no workspace detected"));
}

#[test]
fn check_workspace_no_workspace() {
    let dir = setup_project(&[("main.rs", "// TODO: standalone\n")]);

    todo_scan()
        .args([
            "check",
            "--workspace",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no workspace detected"));
}
