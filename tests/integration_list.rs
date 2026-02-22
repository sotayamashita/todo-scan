use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn todox() -> Command {
    assert_cmd::cargo_bin_cmd!("todox")
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

    todox()
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

    todox()
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

    todox()
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

    todox()
        .args(["ls", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"));
}

#[test]
fn test_list_empty_project() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todox()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 items"));
}

#[test]
fn test_list_with_author_and_issue() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): fix issue #123\n")]);

    todox()
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

    todox()
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
    todox()
        .args(["list", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items"));

    // Verify correct messages via JSON output
    todox()
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

    todox()
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

    todox()
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
        .stdout(predicate::str::contains("::notice::todox: 2 items found"));
}

#[test]
fn test_list_sarif_format() {
    let dir = setup_project(&[("main.rs", "// TODO: sarif test\n")]);

    todox()
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
        .stdout(predicate::str::contains("\"ruleId\": \"todox/TODO\""))
        .stdout(predicate::str::contains("\"text\": \"sarif test\""));
}

#[test]
fn test_list_markdown_format() {
    let dir = setup_project(&[("main.rs", "// TODO(alice): implement feature #42\n")]);

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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

    todox()
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
    todox()
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
    todox()
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

    todox()
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

    todox()
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
    todox()
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
