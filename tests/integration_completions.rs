use assert_cmd::Command;
use predicates::prelude::*;

fn todox() -> Command {
    Command::cargo_bin("todox").unwrap()
}

#[test]
fn test_completions_bash() {
    todox()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("todox"));
}

#[test]
fn test_completions_zsh() {
    todox()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("todox"));
}

#[test]
fn test_completions_fish() {
    todox()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -c todox"));
}
