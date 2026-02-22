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
fn test_relate_proximity_detection() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: fix input validation\n// FIXME: broken input handling\nfn main() {}\n",
    )]);

    todox()
        .args(["relate", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("proximity"))
        .stdout(predicate::str::contains("relationships"));
}

#[test]
fn test_relate_keyword_detection() {
    let dir = setup_project(&[
        ("src/auth.rs", "// TODO: implement authentication check\n"),
        (
            "src/db.rs",
            "// FIXME: authentication bypass vulnerability\n",
        ),
    ]);

    todox()
        .args([
            "relate",
            "--root",
            dir.path().to_str().unwrap(),
            "--min-score",
            "0.01",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("shared_keyword"));
}

#[test]
fn test_relate_cross_reference_same_issue() {
    let dir = setup_project(&[
        ("src/auth.rs", "// TODO(alice): fix login flow #42\n"),
        ("src/api.rs", "// FIXME(alice): validate tokens #42\n"),
    ]);

    todox()
        .args([
            "relate",
            "--root",
            dir.path().to_str().unwrap(),
            "--min-score",
            "0.01",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("same_issue:#42"));
}

#[test]
fn test_relate_cluster_output() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: fix authentication\n// FIXME: broken authentication\nfn main() {}\n",
    )]);

    todox()
        .args([
            "relate",
            "--cluster",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cluster 1"));
}

#[test]
fn test_relate_for_filter() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: fix auth\n// FIXME: broken auth\n// NOTE: unrelated note far away\nfn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\nfn f() {}\nfn g() {}\nfn h() {}\nfn i() {}\nfn j() {}\nfn k() {}\nfn l() {}\nfn m() {}\nfn n() {}\nfn o() {}\nfn p() {}\nfn q() {}\n// TODO: another unrelated thing\n",
    )]);

    todox()
        .args([
            "relate",
            "--for",
            "main.rs:1",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Relationships for main.rs:1"));
}

#[test]
fn test_relate_min_score_threshold() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: fix alpha\n// FIXME: broken beta\nfn main() {}\n",
    )]);

    // Very high min_score should filter out everything
    todox()
        .args([
            "relate",
            "--min-score",
            "0.99",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No relationships found"));
}

#[test]
fn test_relate_json_format() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: fix authentication\n// FIXME: broken authentication\nfn main() {}\n",
    )]);

    todox()
        .args([
            "relate",
            "--format",
            "json",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"relationships\""))
        .stdout(predicate::str::contains("\"total_relationships\""))
        .stdout(predicate::str::contains("\"min_score\""));
}

#[test]
fn test_relate_no_relationships() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: alpha something unique\n"),
        ("z.rs", "// NOTE: completely different topic zzz\n"),
    ]);

    todox()
        .args([
            "relate",
            "--root",
            dir.path().to_str().unwrap(),
            "--min-score",
            "0.5",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No relationships found"));
}

#[test]
fn test_relate_empty_scan() {
    let dir = setup_project(&[("main.rs", "fn main() {}\n")]);

    todox()
        .args(["relate", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("No relationships found"));
}

#[test]
fn test_relate_proximity_threshold_flag() {
    let dir = setup_project(&[(
        "main.rs",
        "// TODO: first thing\nfn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\nfn f() {}\nfn g() {}\nfn h() {}\nfn i() {}\nfn j() {}\nfn k() {}\nfn l() {}\nfn m() {}\nfn n() {}\nfn o() {}\nfn p() {}\nfn q() {}\nfn r() {}\nfn s() {}\n// TODO: second thing\n",
    )]);

    // Default proximity=10, items are 20 lines apart → no proximity
    todox()
        .args([
            "relate",
            "--root",
            dir.path().to_str().unwrap(),
            "--min-score",
            "0.5",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No relationships found"));

    // Increase proximity to 25 → should detect
    todox()
        .args([
            "relate",
            "--proximity",
            "25",
            "--root",
            dir.path().to_str().unwrap(),
            "--min-score",
            "0.01",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("proximity"));
}
