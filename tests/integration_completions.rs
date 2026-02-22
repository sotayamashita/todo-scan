use assert_cmd::Command;
use predicates::prelude::*;

fn todo_scan() -> Command {
    assert_cmd::cargo_bin_cmd!("todo-scan")
}

#[test]
fn test_completions_bash() {
    todo_scan()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("todo-scan"));
}

#[test]
fn test_completions_zsh() {
    todo_scan()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("todo-scan"));
}

#[test]
fn test_completions_fish() {
    todo_scan()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -c todo-scan"));
}
