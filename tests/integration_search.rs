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
fn test_search_basic_substring_case_insensitive() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: Implement Feature\n// FIXME: broken thing\n",
    )]);

    todo_scan()
        .args([
            "search",
            "implement",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Implement Feature"))
        .stdout(predicate::str::contains("1 matches across 1 files"));
}

#[test]
fn test_search_exact_flag() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: Implement Feature\n// TODO: implement other\n",
    )]);

    // With --exact, case matters
    todo_scan()
        .args([
            "search",
            "Implement",
            "--exact",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Implement Feature"))
        .stdout(predicate::str::contains("implement other").not())
        .stdout(predicate::str::contains("1 matches across 1 files"));
}

#[test]
fn test_search_issue_ref() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): fix issue #123\n// TODO: other\n",
    )]);

    todo_scan()
        .args(["search", "#123", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix issue"))
        .stdout(predicate::str::contains("1 matches across 1 files"));
}

#[test]
fn test_search_author_filter() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): alice task fix\n// TODO(bob): bob task fix\n",
    )]);

    todo_scan()
        .args([
            "search",
            "fix",
            "--author",
            "alice",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice task fix"))
        .stdout(predicate::str::contains("bob task fix").not())
        .stdout(predicate::str::contains("1 matches"));
}

#[test]
fn test_search_tag_filter() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: todo task\n// FIXME: fixme task\n// BUG: bug task\n",
    )]);

    todo_scan()
        .args([
            "search",
            "task",
            "--tag",
            "FIXME",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fixme task"))
        .stdout(predicate::str::contains("todo task").not())
        .stdout(predicate::str::contains("bug task").not())
        .stdout(predicate::str::contains("1 matches"));
}

#[test]
fn test_search_path_filter() {
    let dir = setup_project(&[
        ("src/lib.rs", "// TODO: fix src lib\n"),
        ("tests/test.rs", "// TODO: fix test\n"),
    ]);

    todo_scan()
        .args([
            "search",
            "fix",
            "--path",
            "src/**",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix src lib"))
        .stdout(predicate::str::contains("fix test").not())
        .stdout(predicate::str::contains("1 matches"));
}

#[test]
fn test_search_no_matches() {
    let dir = setup_project(&[("main.rs", "// TODO: something\n")]);

    todo_scan()
        .args([
            "search",
            "nonexistent",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 matches across 0 files"));
}

#[test]
fn test_search_summary_line_format() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: fix alpha\n"),
        ("b.rs", "// TODO: fix beta\n"),
    ]);

    todo_scan()
        .args(["search", "fix", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2 matches across 2 files (query: \"fix\")",
        ));
}

#[test]
fn test_search_context_lines() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n    let z = 3;\n}\n",
    )]);

    todo_scan()
        .args([
            "search",
            "fix",
            "-C",
            "2",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("fix this"))
        .stdout(predicate::str::contains("let y = 2"));
}

#[test]
fn test_search_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: fix the bug\n// TODO: other thing\n")]);

    todo_scan()
        .args([
            "search",
            "fix",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"query\": \"fix\""))
        .stdout(predicate::str::contains("\"exact\": false"))
        .stdout(predicate::str::contains("\"match_count\": 1"))
        .stdout(predicate::str::contains("\"file_count\": 1"))
        .stdout(predicate::str::contains("\"message\": \"fix the bug\""));
}

#[test]
fn test_search_alias_s() {
    let dir = setup_project(&[("main.rs", "// TODO: alias test\n")]);

    todo_scan()
        .args(["s", "alias", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("alias test"))
        .stdout(predicate::str::contains("1 matches"));
}

#[test]
fn test_search_detail_minimal_hides_author() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): fix the bug #123\n// FIXME(bob): another issue\n",
    )]);

    todo_scan()
        .args([
            "search",
            "fix",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "minimal",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix the bug"))
        .stdout(predicate::str::contains("(@alice)").not())
        .stdout(predicate::str::contains("(#123)").not());
}

#[test]
fn test_search_detail_minimal_json() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix the bug #123\n")]);

    let output = todo_scan()
        .args([
            "search",
            "fix",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "minimal",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let item = &json["items"][0];

    assert!(
        item.get("author").is_none(),
        "author should not be in minimal search JSON"
    );
    assert!(
        item.get("issue_ref").is_none(),
        "issue_ref should not be in minimal search JSON"
    );
}

#[test]
fn test_search_json_contains_id_field() {
    let dir = setup_project(&[("main.rs", "// TODO: search id test\n// FIXME: other\n")]);

    let output = todo_scan()
        .args([
            "search",
            "search id",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let item = &json["items"][0];
    assert_eq!(item["id"].as_str().unwrap(), "main.rs:TODO:search id test");
}

// --- Text format search with context ---

#[test]
fn test_search_text_context_lines() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix memory leak\n    let y = 2;\n}\n",
    )]);

    todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "-C",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("let y = 2"))
        .stdout(predicate::str::contains("→"));
}

// --- Text format search with group-by tag ---

#[test]
fn test_search_text_group_by_tag() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: fix memory issue\n// FIXME: fix memory leak\n",
    )]);

    todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--group-by",
            "tag",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (1 items)"))
        .stdout(predicate::str::contains("FIXME (1 items)"))
        .stdout(predicate::str::contains("2 matches across 2 groups"));
}

// --- Text format search with deadline ---

#[test]
fn test_search_text_shows_deadline() {
    let dir = setup_project(&[("main.rs", "// TODO(2099-06-15): fix memory leak\n")]);

    todo_scan()
        .args(["search", "memory", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("[deadline: 2099-06-15]"));
}

// --- Text format search with minimal detail ---

#[test]
fn test_search_text_minimal_suppresses_metadata() {
    let dir = setup_project(&[("main.rs", "// TODO(alice, 2099-06-15): fix memory leak\n")]);

    todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "minimal",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("@alice").not())
        .stdout(predicate::str::contains("deadline").not());
}

// --- Search sort by tag ---

#[test]
fn test_search_sort_by_tag() {
    let dir = setup_project(&[(
        "main.rs",
        "// NOTE: fix memory note\n// BUG: fix memory bug\n// TODO: fix memory task\n",
    )]);

    let output = todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--sort",
            "tag",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["tag"].as_str().unwrap(), "BUG");
    assert_eq!(items[1]["tag"].as_str().unwrap(), "TODO");
    assert_eq!(items[2]["tag"].as_str().unwrap(), "NOTE");
}

// --- Search sort by priority ---

#[test]
fn test_search_sort_by_priority() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: fix memory normal\n// TODO!!: fix memory urgent\n// TODO!: fix memory high\n",
    )]);

    let output = todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--sort",
            "priority",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["priority"].as_str().unwrap(), "urgent");
    assert_eq!(items[1]["priority"].as_str().unwrap(), "high");
    assert_eq!(items[2]["priority"].as_str().unwrap(), "normal");
}

// --- Search full detail with auto-context ---

#[test]
fn test_search_full_detail_auto_context() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix memory leak\n    let y = 2;\n}\n",
    )]);

    let output = todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "full",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let items = json["items"].as_array().unwrap();
    assert!(
        items[0].get("match_key").is_some(),
        "full detail should include match_key"
    );
    assert!(
        items[0].get("context").is_some(),
        "full detail should auto-include context"
    );
}

// --- Search with github-actions format ---

#[test]
fn test_search_github_actions_format() {
    let dir = setup_project(&[("main.rs", "// TODO: fix memory leak\n// FIXME: other\n")]);

    todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("::warning"))
        .stdout(predicate::str::contains("fix memory leak"));
}

// --- Search with sarif format ---

#[test]
fn test_search_sarif_format() {
    let dir = setup_project(&[("main.rs", "// TODO: fix memory leak\n// FIXME: other\n")]);

    todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "sarif",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"2.1.0\""))
        .stdout(predicate::str::contains("fix memory leak"));
}

// --- Search with markdown format ---

#[test]
fn test_search_markdown_format() {
    let dir = setup_project(&[("main.rs", "// TODO: fix memory leak\n// FIXME: other\n")]);

    todo_scan()
        .args([
            "search",
            "memory",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("| File |"))
        .stdout(predicate::str::contains("fix memory leak"));
}

// --- Search text with expired deadline ---

#[test]
fn test_search_text_shows_expired_deadline() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-01-01): fix memory leak\n")]);

    todo_scan()
        .args(["search", "memory", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("[expired: 2020-01-01]"));
}
