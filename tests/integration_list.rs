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
fn test_list_finds_todos() {
    let dir = setup_project(&[
        (
            "main.rs",
            "// TODO: implement feature\nfn main() {}\n// FIXME: broken\n",
        ),
        ("lib.rs", "// HACK: workaround\n"),
    ]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("HACK"))
        .stdout(predicate::str::contains("3 items"));
}

#[test]
fn test_list_tag_filter() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: task one\n// FIXME: task two\n// HACK: task three\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--tag",
            "FIXME",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("1 items"));
}

#[test]
fn test_list_json_format() {
    let dir = setup_project(&[("main.rs", "// TODO: json test\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tag\": \"TODO\""))
        .stdout(predicate::str::contains("\"message\": \"json test\""));
}

#[test]
fn test_list_alias_ls() {
    let dir = setup_project(&[("main.rs", "// TODO: alias test\n")]);

    todo_scan()
        .args(["ls", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"));
}

#[test]
fn test_list_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 items"));
}

#[test]
fn test_list_with_author_and_issue() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"author\": \"alice\""))
        .stdout(predicate::str::contains("\"issue_ref\": \"#123\""));
}

#[test]
fn test_list_text_issue_ref_no_double_hash() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("(#123)"))
        .stdout(predicate::str::contains("(##123)").not());
}

#[test]
fn test_list_filters_false_positives() {
    let dir = setup_project(&[(
        "main.rs",
        r#"// TODO: real comment
let service = TodoService::new();
if isTodoCompleted() { return; }
let msg = "TODO: not real";
let todo_count = 42;
// FIXME(bob): another real comment
struct TodoItem { done: bool }
"#,
    )]);

    // Verify only 2 real comment items found (text output)
    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items"));

    // Verify correct messages via JSON output
    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"message\": \"real comment\""))
        .stdout(predicate::str::contains(
            "\"message\": \"another real comment\"",
        ));
}

#[test]
fn test_list_multi_language_comments() {
    let dir = setup_project(&[
        ("app.py", "# TODO: python todo\nx = 1\n"),
        ("style.css", "/* FIXME: css fixme */\n"),
        ("query.sql", "-- HACK: sql hack\n"),
        ("page.html", "<!-- NOTE: html note -->\n"),
    ]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("4 items"));
}

#[test]
fn test_list_github_actions_format() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: implement feature\n// BUG: critical issue\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "github-actions",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "::warning file=main.rs,line=1,title=TODO::[TODO] implement feature",
        ))
        .stdout(predicate::str::contains(
            "::error file=main.rs,line=2,title=BUG::[BUG] critical issue",
        ))
        .stdout(predicate::str::contains(
            "::notice::todo-scan: 2 items found",
        ));
}

#[test]
fn test_list_sarif_format() {
    let dir = setup_project(&[("main.rs", "// TODO: sarif test\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "sarif",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"2.1.0\""))
        .stdout(predicate::str::contains("\"ruleId\": \"todo-scan/TODO\""))
        .stdout(predicate::str::contains("\"text\": \"sarif test\""));
}

#[test]
fn test_list_markdown_format() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): implement feature #42\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "| File | Line | Tag | Priority | Message | Author | Issue |",
        ))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("alice"))
        .stdout(predicate::str::contains("**1 items found**"));
}

// --- Deadline in output ---

#[test]
fn test_list_deadline_in_json() {
    let dir = setup_project(&[("main.rs", "// TODO(2025-06-01): deadline task\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"deadline\": \"2025-06-01\""));
}

#[test]
fn test_list_author_and_deadline_in_json() {
    let dir = setup_project(&[("main.rs", "// TODO(alice, 2025-06-01): task with both\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"author\": \"alice\""))
        .stdout(predicate::str::contains("\"deadline\": \"2025-06-01\""));
}

#[test]
fn test_list_no_deadline_null_in_json() {
    let dir = setup_project(&[("main.rs", "// TODO: no deadline\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"deadline\": null"));
}

#[test]
fn test_list_quarter_deadline_in_json() {
    let dir = setup_project(&[("main.rs", "// TODO(2025-Q2): quarter deadline\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"deadline\": \"2025-06-30\""));
}

// --- Filtering ---

#[test]
fn test_list_filter_priority() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO!!: urgent task\n// TODO!: high task\n// TODO: normal task\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--priority",
            "urgent",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("urgent task"))
        .stdout(predicate::str::contains("high task").not())
        .stdout(predicate::str::contains("normal task").not())
        .stdout(predicate::str::contains("1 items"));
}

#[test]
fn test_list_filter_author() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): alice task\n// TODO(bob): bob task\n// TODO: no author\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--author",
            "alice",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice task"))
        .stdout(predicate::str::contains("bob task").not())
        .stdout(predicate::str::contains("no author").not())
        .stdout(predicate::str::contains("1 items"));
}

#[test]
fn test_list_filter_path() {
    let dir = setup_project(&[
        ("src/lib.rs", "// TODO: in src\n"),
        ("tests/test.rs", "// TODO: in tests\n"),
    ]);

    todo_scan()
        .args([
            "list",
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

#[test]
fn test_list_limit() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: first\n// TODO: second\n// TODO: third\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--limit",
            "2",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items"));
}

// --- Group by ---

#[test]
fn test_list_group_by_tag() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: task one\n// FIXME: task two\n// TODO: task three\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--group-by",
            "tag",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (2 items)"))
        .stdout(predicate::str::contains("FIXME (1 items)"))
        .stdout(predicate::str::contains("3 items in 2 groups"));
}

#[test]
fn test_list_group_by_priority() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO!!: urgent task\n// TODO!: high task\n// TODO: normal task\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--group-by",
            "priority",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("!! Urgent (1 items)"))
        .stdout(predicate::str::contains("! High (1 items)"))
        .stdout(predicate::str::contains("Normal (1 items)"))
        .stdout(predicate::str::contains("3 items in 3 groups"));
}

#[test]
fn test_list_group_by_author() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): alice task\n// TODO(bob): bob task\n// TODO: no author\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--group-by",
            "author",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice (1 items)"))
        .stdout(predicate::str::contains("bob (1 items)"))
        .stdout(predicate::str::contains("unassigned (1 items)"))
        .stdout(predicate::str::contains("3 items in 3 groups"));
}

#[test]
fn test_list_group_by_dir() {
    let dir = setup_project(&[
        ("src/lib.rs", "// TODO: in src\n"),
        ("src/main.rs", "// TODO: also in src\n"),
        ("tests/test.rs", "// TODO: in tests\n"),
    ]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--group-by",
            "dir",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("src (2 items)"))
        .stdout(predicate::str::contains("tests (1 items)"))
        .stdout(predicate::str::contains("3 items in 2 groups"));
}

#[test]
fn test_list_group_by_with_json() {
    let dir = setup_project(&[("main.rs", "// TODO!!: urgent task\n// TODO: normal task\n")]);

    // JSON output should be flat regardless of --group-by
    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--group-by",
            "priority",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"items\""))
        .stdout(predicate::str::contains("\"tag\": \"TODO\""));
}

#[test]
fn test_list_filter_composition() {
    let dir = setup_project(&[
        ("src/lib.rs", "// TODO(alice)!!: urgent alice in src\n"),
        ("src/main.rs", "// TODO(alice): normal alice in src\n"),
        ("src/other.rs", "// TODO(bob)!!: urgent bob in src\n"),
        ("tests/test.rs", "// TODO(alice)!!: urgent alice in tests\n"),
    ]);

    // Combine: --priority urgent --author alice --path "src/**"
    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--priority",
            "urgent",
            "--author",
            "alice",
            "--path",
            "src/**",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("urgent alice in src"))
        .stdout(predicate::str::contains("normal alice in src").not())
        .stdout(predicate::str::contains("urgent bob in src").not())
        .stdout(predicate::str::contains("urgent alice in tests").not())
        .stdout(predicate::str::contains("1 items"));
}

// --- Context display ---

#[test]
fn test_list_context_shows_surrounding_lines() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n    let z = 3;\n}\n",
    )]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap(), "-C", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("let y = 2"));
}

#[test]
fn test_list_context_json_includes_context_field() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n}\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "-C",
            "2",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"context\""))
        .stdout(predicate::str::contains("\"before\""))
        .stdout(predicate::str::contains("\"after\""));
}

#[test]
fn test_list_without_context_no_context_lines() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n}\n",
    )]);

    // Without -C flag, no surrounding code should appear
    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"context\"").not());
}

// --- todo-scan:ignore suppression tests ---

#[test]
fn test_list_excludes_ignored_items_by_default() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: visible item\n// TODO: hidden item todo-scan:ignore\n",
    )]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("visible item"))
        .stdout(predicate::str::contains("hidden item").not())
        .stdout(predicate::str::contains("1 items"))
        .stdout(predicate::str::contains("(1 ignored)"));
}

#[test]
fn test_list_show_ignored_reveals_suppressed() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: visible item\n// TODO: hidden item todo-scan:ignore\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--show-ignored",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("visible item"))
        .stdout(predicate::str::contains("Ignored items"))
        .stdout(predicate::str::contains("hidden item"))
        .stdout(predicate::str::contains("(1 ignored)"));
}

#[test]
fn test_list_ignore_next_line_works_e2e() {
    let dir = setup_project(&[(
        "main.rs",
        "// todo-scan:ignore-next-line\n// TODO: suppressed\n// TODO: visible\n",
    )]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("visible"))
        .stdout(predicate::str::contains("suppressed").not())
        .stdout(predicate::str::contains("1 items"))
        .stdout(predicate::str::contains("(1 ignored)"));
}

#[test]
fn test_list_no_ignored_shows_no_suffix() {
    let dir = setup_project(&[("main.rs", "// TODO: just a normal todo\n")]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 items in 1 files"))
        .stdout(predicate::str::contains("ignored").not());
}

#[test]
fn test_list_ignore_strips_marker_from_message() {
    let dir = setup_project(&[("main.rs", "// TODO: fix this todo-scan:ignore\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--show-ignored",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"message\": \"fix this\""))
        .stdout(predicate::str::contains("todo-scan:ignore").not());
}

// --- Detail level tests ---

#[test]
fn test_list_detail_minimal_hides_author_and_issue() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO(alice): fix issue #123\n// FIXME(bob): broken thing\n",
    )]);

    let output = todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "minimal",
        ])
        .assert()
        .success();

    // Minimal should NOT show (@author) or (#issue)
    output
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("FIXME"))
        .stdout(predicate::str::contains("(@alice)").not())
        .stdout(predicate::str::contains("(@bob)").not())
        .stdout(predicate::str::contains("(#123)").not());
}

#[test]
fn test_list_detail_default_is_normal() {
    // Omitting --detail should give the same as --detail normal
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    // Without --detail flag
    let without = todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    // With --detail normal
    let with_normal = todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "normal",
        ])
        .output()
        .unwrap();

    assert_eq!(without.stdout, with_normal.stdout);
}

#[test]
fn test_list_detail_minimal_json_only_core_fields() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    let output = todo_scan()
        .args([
            "list",
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

    // Core fields must be present
    assert!(item["file"].is_string());
    assert!(item["line"].is_number());
    assert!(item["tag"].is_string());
    assert!(item["message"].is_string());

    // Non-core fields must be absent in minimal
    assert!(
        item.get("author").is_none(),
        "author should not be in minimal JSON"
    );
    assert!(
        item.get("issue_ref").is_none(),
        "issue_ref should not be in minimal JSON"
    );
    assert!(
        item.get("priority").is_none(),
        "priority should not be in minimal JSON"
    );
    assert!(
        item.get("deadline").is_none(),
        "deadline should not be in minimal JSON"
    );
}

#[test]
fn test_list_detail_full_json_includes_match_key() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    let output = todo_scan()
        .args([
            "list",
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
    let item = &json["items"][0];

    // Full mode should include match_key
    assert!(
        item.get("match_key").is_some(),
        "match_key should be present in full JSON"
    );
    assert!(item["match_key"].as_str().unwrap().contains("TODO"));

    // Full mode should also include context even without -C flag
    assert!(
        item.get("context").is_some(),
        "context should be auto-included in full JSON"
    );
}

#[test]
fn test_list_detail_full_text_auto_context() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n    let z = 3;\n}\n",
    )]);

    // Full detail without -C flag should still show context
    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "full",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("let y = 2"));
}

#[test]
fn test_list_json_contains_id_field_at_normal_detail() {
    let dir = setup_project(&[("main.rs", "// TODO: stable id test\n")]);

    let output = todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["id"].as_str().unwrap(), "main.rs:TODO:stable id test");
}

#[test]
fn test_list_json_contains_id_field_at_minimal_detail() {
    let dir = setup_project(&[("main.rs", "// TODO: minimal id test\n")]);

    let output = todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--detail",
            "minimal",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let item = &json["items"][0];
    assert!(
        item["id"].is_string(),
        "id field must be present in minimal detail"
    );
}

#[test]
fn test_list_json_full_detail_has_both_id_and_match_key() {
    let dir = setup_project(&[("main.rs", "// TODO: full detail test\n")]);

    let output = todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--detail",
            "full",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let item = &json["items"][0];
    assert!(item["id"].is_string(), "id field must be present");
    assert!(
        item["match_key"].is_string(),
        "match_key must still be present in full detail"
    );
    assert_eq!(item["id"].as_str(), item["match_key"].as_str());
}

// --- Text format context lines (list -C) ---

#[test]
fn test_list_text_context_lines() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n}\n",
    )]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap(), "-C", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("let x = 1"))
        .stdout(predicate::str::contains("let y = 2"))
        .stdout(predicate::str::contains("TODO"))
        .stdout(predicate::str::contains("→"));
}

#[test]
fn test_list_text_context_lines_with_group_by_tag() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n}\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "-C",
            "1",
            "--group-by",
            "tag",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (1 items)"))
        .stdout(predicate::str::contains("→"));
}

// --- Text format with deadlines ---

#[test]
fn test_list_text_shows_deadline() {
    let dir = setup_project(&[("main.rs", "// TODO(2099-06-15): future deadline task\n")]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("[deadline: 2099-06-15]"));
}

#[test]
fn test_list_text_shows_expired_deadline() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-01-01): expired task\n")]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("[expired: 2020-01-01]"));
}

// --- Text format with author and issue_ref ---

#[test]
fn test_list_text_shows_author_and_issue_ref() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix bug #42\n")]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("@alice"))
        .stdout(predicate::str::contains("#42"));
}

// --- Show-ignored with group-by tag ---

#[test]
fn test_list_show_ignored_with_group_by_tag() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: visible\n// FIXME: hidden todo-scan:ignore\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--show-ignored",
            "--group-by",
            "tag",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ignored items"))
        .stdout(predicate::str::contains("FIXME"));
}

// --- Text format minimal detail suppresses author/issue/deadline ---

#[test]
fn test_list_text_minimal_suppresses_metadata() {
    let dir = setup_project(&[("main.rs", "// TODO(alice, 2099-06-15): fix this bug\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--detail",
            "minimal",
        ])
        .assert()
        .success()
        // Minimal mode should not show author annotation or deadline
        .stdout(predicate::str::contains("@alice").not())
        .stdout(predicate::str::contains("deadline").not())
        .stdout(predicate::str::contains("TODO"));
}

// --- Sort by tag ---

#[test]
fn test_list_sort_by_tag() {
    let dir = setup_project(&[(
        "main.rs",
        "// NOTE: low severity\n// BUG: high severity\n// TODO: medium severity\n",
    )]);

    let output = todo_scan()
        .args([
            "list",
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
    // Should be sorted by severity descending: BUG > TODO > NOTE
    assert_eq!(items[0]["tag"].as_str().unwrap(), "BUG");
    assert_eq!(items[1]["tag"].as_str().unwrap(), "TODO");
    assert_eq!(items[2]["tag"].as_str().unwrap(), "NOTE");
}

// --- Sort by priority ---

#[test]
fn test_list_sort_by_priority() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: normal\n// TODO!!: urgent\n// TODO!: high\n",
    )]);

    let output = todo_scan()
        .args([
            "list",
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
    // Should be sorted by priority descending: urgent > high > normal
    assert_eq!(items[0]["priority"].as_str().unwrap(), "urgent");
    assert_eq!(items[1]["priority"].as_str().unwrap(), "high");
    assert_eq!(items[2]["priority"].as_str().unwrap(), "normal");
}

// --- Full detail level with auto-context ---

#[test]
fn test_list_full_detail_auto_context_json() {
    let dir = setup_project(&[(
        "main.rs",
        "fn main() {\n    let x = 1;\n    // TODO: fix this\n    let y = 2;\n}\n",
    )]);

    let output = todo_scan()
        .args([
            "list",
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
    // Full detail should include match_key
    assert!(
        items[0].get("match_key").is_some(),
        "full detail should include match_key"
    );
    // Full detail should auto-include context
    assert!(
        items[0].get("context").is_some(),
        "full detail should auto-include context"
    );
}

// --- Scanner exclude_dirs config ---

#[test]
fn test_list_exclude_dirs_config() {
    let dir = setup_project(&[
        ("src/lib.rs", "// TODO: keep this\n"),
        ("vendor/dep.rs", "// TODO: exclude this\n"),
        ("node_modules/pkg.js", "// TODO: exclude this too\n"),
        (
            ".todo-scan.toml",
            r#"
exclude_dirs = ["vendor", "node_modules"]
"#,
        ),
    ]);

    todo_scan()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("keep this"))
        .stdout(predicate::str::contains("exclude this").not())
        .stdout(predicate::str::contains("exclude this too").not())
        .stdout(predicate::str::contains("1 items"));
}

// --- List filter by normal priority ---

#[test]
fn test_list_filter_normal_priority() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO!!: urgent\n// TODO!: high\n// TODO: normal one\n// TODO: normal two\n",
    )]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--priority",
            "normal",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("normal one"))
        .stdout(predicate::str::contains("normal two"))
        .stdout(predicate::str::contains("urgent").not())
        .stdout(predicate::str::contains("2 items"));
}

// --- Diff text format with expired deadline ---

#[test]
fn test_list_text_search_expired_deadline_in_group_by_tag() {
    let dir = setup_project(&[("main.rs", "// TODO(2020-01-01): old expired task\n")]);

    todo_scan()
        .args([
            "list",
            "--root",
            dir.path().to_str().unwrap(),
            "--group-by",
            "tag",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("[expired: 2020-01-01]"));
}
