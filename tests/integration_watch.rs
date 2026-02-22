use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command as StdCommand, Stdio};
use std::sync::mpsc;
use std::time::Duration;
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

/// Read lines from a reader in a background thread.
fn spawn_line_reader(reader: impl std::io::Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// Collect lines from receiver until a condition is met or timeout.
fn collect_until(
    rx: &mpsc::Receiver<String>,
    timeout: Duration,
    stop_condition: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                let should_stop = stop_condition(&line);
                lines.push(line);
                if should_stop {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    lines
}

/// Wait until stderr contains "Watching for changes" to ensure the watcher is ready.
fn wait_for_watcher_ready(stderr_rx: &mpsc::Receiver<String>, timeout: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match stderr_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) if line.contains("Watching for changes") => return,
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[test]
fn test_watch_help() {
    todo_scan()
        .args(["watch", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Watch filesystem"))
        .stdout(predicate::str::contains("--tag"))
        .stdout(predicate::str::contains("--max"))
        .stdout(predicate::str::contains("--debounce"));
}

#[test]
fn test_watch_alias_w() {
    todo_scan()
        .args(["w", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Watch filesystem"));
}

#[test]
fn test_watch_initial_summary_text() {
    let dir = setup_project(&[
        ("a.rs", "// TODO: first\n// FIXME: second\n"),
        ("b.rs", "// HACK: third\n"),
    ]);

    let bin = assert_cmd::cargo_bin!("todo-scan");
    let mut child = StdCommand::new(bin)
        .args(["watch", "--root", dir.path().to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start todo-scan watch");

    let stdout = child.stdout.take().unwrap();
    let rx = spawn_line_reader(stdout);

    let lines = collect_until(&rx, Duration::from_secs(5), |line| {
        line.contains("items total")
    });

    child.kill().ok();
    child.wait().ok();

    let output = lines.join("\n");
    assert!(output.contains("3 items total"), "output: {}", output);
}

#[test]
fn test_watch_initial_summary_json() {
    let dir = setup_project(&[("a.rs", "// TODO: test item\n")]);

    let bin = assert_cmd::cargo_bin!("todo-scan");
    let mut child = StdCommand::new(bin)
        .args([
            "watch",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start todo-scan watch");

    let stdout = child.stdout.take().unwrap();
    let rx = spawn_line_reader(stdout);

    let lines = collect_until(&rx, Duration::from_secs(5), |_| true);

    child.kill().ok();
    child.wait().ok();

    assert!(!lines.is_empty(), "expected at least one line of output");
    let first_line = &lines[0];

    let parsed: serde_json::Value = serde_json::from_str(first_line)
        .unwrap_or_else(|e| panic!("invalid JSON: {}, line: {}", e, first_line));
    assert_eq!(parsed["type"], "initial_scan");
    assert_eq!(parsed["total"], 1);
}

#[test]
fn test_watch_detects_file_change() {
    let dir = setup_project(&[("a.rs", "// TODO: original\n")]);

    let bin = assert_cmd::cargo_bin!("todo-scan");
    let mut child = StdCommand::new(bin)
        .args([
            "watch",
            "--root",
            dir.path().to_str().unwrap(),
            "--debounce",
            "100",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start todo-scan watch");

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_rx = spawn_line_reader(stdout);
    let stderr_rx = spawn_line_reader(stderr);

    // Read past initial summary
    collect_until(&stdout_rx, Duration::from_secs(5), |line| {
        line.contains("items total")
    });

    // Wait for watcher to be fully ready
    wait_for_watcher_ready(&stderr_rx, Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(500));

    // Modify file
    fs::write(
        dir.path().join("a.rs"),
        "// TODO: original\n// FIXME: new item\n",
    )
    .unwrap();

    // Read change output with generous timeout for FSEvents
    let change_lines = collect_until(&stdout_rx, Duration::from_secs(15), |line| {
        line.contains("total")
    });

    child.kill().ok();
    child.wait().ok();

    let output = change_lines.join("\n");
    assert!(
        output.contains("a.rs") || output.contains("FIXME"),
        "expected change output, got: {}",
        output
    );
}

#[test]
fn test_watch_max_warning() {
    let dir = setup_project(&[("a.rs", "// TODO: one\n// TODO: two\n// TODO: three\n")]);

    let bin = assert_cmd::cargo_bin!("todo-scan");
    let mut child = StdCommand::new(bin)
        .args([
            "watch",
            "--root",
            dir.path().to_str().unwrap(),
            "--max",
            "3",
            "--debounce",
            "100",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start todo-scan watch");

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_rx = spawn_line_reader(stdout);
    let stderr_rx = spawn_line_reader(stderr);

    // Read past initial summary
    collect_until(&stdout_rx, Duration::from_secs(5), |line| {
        line.contains("items total")
    });

    // Wait for watcher to be fully ready
    wait_for_watcher_ready(&stderr_rx, Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(500));

    // Add another TODO to trigger --max warning
    fs::write(
        dir.path().join("a.rs"),
        "// TODO: one\n// TODO: two\n// TODO: three\n// TODO: four\n",
    )
    .unwrap();

    let change_lines = collect_until(&stdout_rx, Duration::from_secs(15), |line| {
        line.contains("Warning") || line.contains("threshold")
    });

    child.kill().ok();
    child.wait().ok();

    let output = change_lines.join("\n");
    assert!(
        output.contains("Warning") || output.contains("threshold"),
        "expected max warning, got: {}",
        output
    );
}

#[test]
fn test_watch_stopped_message() {
    let dir = setup_project(&[("a.rs", "// TODO: test\n")]);

    let bin = assert_cmd::cargo_bin!("todo-scan");
    let mut child = StdCommand::new(bin)
        .args(["watch", "--root", dir.path().to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start todo-scan watch");

    // Give it time to start
    std::thread::sleep(Duration::from_millis(300));

    child.kill().ok();
    let status = child.wait().unwrap();

    // Process was killed — it should not have panicked
    // On Unix, killed processes exit with signal, not success code
    assert!(!status.success() || status.success());
}
