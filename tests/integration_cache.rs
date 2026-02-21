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
fn test_no_cache_flag_produces_correct_output() {
    let dir = setup_project(&[
        ("main.rs", "// TODO: first task\n// FIXME: second task\n"),
        ("lib.rs", "// HACK: workaround\n"),
    ]);

    todox()
        .args(["list", "--no-cache", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("HACK"))
        .stdout(predicate::str::contains("3 items"));
}

#[test]
fn test_consecutive_runs_produce_identical_results() {
    let dir = setup_project(&[
        (
            "main.rs",
            "// TODO: implement feature\n// FIXME(alice): broken parsing\n",
        ),
        ("lib.rs", "// HACK: temporary workaround\n"),
        ("utils.rs", "// NOTE: this is intentional\n"),
    ]);

    let root = dir.path().to_str().unwrap();

    // First run (cold cache)
    let first = todox()
        .args(["list", "--format", "json", "--root", root])
        .output()
        .unwrap();
    assert!(first.status.success());
    let first_stdout = String::from_utf8(first.stdout).unwrap();

    // Second run (warm cache)
    let second = todox()
        .args(["list", "--format", "json", "--root", root])
        .output()
        .unwrap();
    assert!(second.status.success());
    let second_stdout = String::from_utf8(second.stdout).unwrap();

    // Parse JSON and compare items (order may differ due to parallel vs sequential walk)
    let first_json: serde_json::Value = serde_json::from_str(&first_stdout).unwrap();
    let second_json: serde_json::Value = serde_json::from_str(&second_stdout).unwrap();

    let mut first_items: Vec<serde_json::Value> =
        serde_json::from_value(first_json["items"].clone()).unwrap();
    let mut second_items: Vec<serde_json::Value> =
        serde_json::from_value(second_json["items"].clone()).unwrap();

    // Sort by file then line for deterministic comparison
    first_items.sort_by(|a, b| {
        let fa = a["file"].as_str().unwrap_or("");
        let fb = b["file"].as_str().unwrap_or("");
        fa.cmp(fb).then(a["line"].as_u64().cmp(&b["line"].as_u64()))
    });
    second_items.sort_by(|a, b| {
        let fa = a["file"].as_str().unwrap_or("");
        let fb = b["file"].as_str().unwrap_or("");
        fa.cmp(fb).then(a["line"].as_u64().cmp(&b["line"].as_u64()))
    });

    assert_eq!(first_items, second_items);
}

#[test]
fn test_json_output_identical_with_and_without_cache() {
    let dir = setup_project(&[
        ("main.rs", "// TODO: task one\n// FIXME(bob): task two\n"),
        ("lib.rs", "// BUG: !! critical issue\n"),
    ]);

    let root = dir.path().to_str().unwrap();

    // Run with --no-cache
    let no_cache = todox()
        .args(["list", "--format", "json", "--no-cache", "--root", root])
        .output()
        .unwrap();
    assert!(no_cache.status.success());
    let no_cache_stdout = String::from_utf8(no_cache.stdout).unwrap();

    // Prime cache
    todox()
        .args(["list", "--format", "json", "--root", root])
        .output()
        .unwrap();

    // Run with cache
    let with_cache = todox()
        .args(["list", "--format", "json", "--root", root])
        .output()
        .unwrap();
    assert!(with_cache.status.success());
    let with_cache_stdout = String::from_utf8(with_cache.stdout).unwrap();

    // Parse and compare (sort for order independence)
    let nc_json: serde_json::Value = serde_json::from_str(&no_cache_stdout).unwrap();
    let wc_json: serde_json::Value = serde_json::from_str(&with_cache_stdout).unwrap();

    let mut nc_items: Vec<serde_json::Value> =
        serde_json::from_value(nc_json["items"].clone()).unwrap();
    let mut wc_items: Vec<serde_json::Value> =
        serde_json::from_value(wc_json["items"].clone()).unwrap();

    nc_items.sort_by(|a, b| {
        let fa = a["file"].as_str().unwrap_or("");
        let fb = b["file"].as_str().unwrap_or("");
        fa.cmp(fb).then(a["line"].as_u64().cmp(&b["line"].as_u64()))
    });
    wc_items.sort_by(|a, b| {
        let fa = a["file"].as_str().unwrap_or("");
        let fb = b["file"].as_str().unwrap_or("");
        fa.cmp(fb).then(a["line"].as_u64().cmp(&b["line"].as_u64()))
    });

    assert_eq!(nc_items, wc_items);
}

#[test]
fn test_cache_detects_modified_file() {
    let dir = setup_project(&[("main.rs", "// TODO: original task\n")]);

    let root = dir.path().to_str().unwrap();

    // First run to populate cache
    let first = todox()
        .args(["list", "--format", "json", "--root", root])
        .output()
        .unwrap();
    assert!(first.status.success());

    // Modify the file
    // Small delay to ensure mtime changes
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(
        dir.path().join("main.rs"),
        "// TODO: original task\n// FIXME: new task added\n",
    )
    .unwrap();

    // Second run should detect the change
    let second = todox()
        .args(["list", "--format", "json", "--root", root])
        .output()
        .unwrap();
    assert!(second.status.success());
    let second_stdout = String::from_utf8(second.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&second_stdout).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn test_check_command_works_with_cache() {
    let dir = setup_project(&[("main.rs", "// TODO: task\n")]);

    let root = dir.path().to_str().unwrap();

    todox()
        .args(["check", "--max", "10", "--root", root])
        .assert()
        .success();

    // With --no-cache
    todox()
        .args(["check", "--max", "10", "--no-cache", "--root", root])
        .assert()
        .success();
}
