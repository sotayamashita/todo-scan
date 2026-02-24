use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn git_command(args: &[&str], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to execute git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }

    let stdout =
        String::from_utf8(output.stdout).with_context(|| "git output is not valid UTF-8")?;

    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_command_success() {
        // `git --version` should succeed in any directory
        let result = git_command(&["--version"], Path::new("."));
        assert!(result.is_ok(), "git --version should succeed: {:?}", result);
    }

    #[test]
    fn test_git_command_failure() {
        // Running `git log` in a non-git directory should fail
        let dir = TempDir::new().unwrap();
        let result = git_command(&["log"], dir.path());
        assert!(result.is_err(), "git log in non-repo should fail");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("git log failed"),
            "error should mention 'git log failed', got: {}",
            err_msg
        );
    }

    #[test]
    fn test_git_command_invalid_args() {
        // Running git with a nonsensical subcommand should fail
        let result = git_command(&["not-a-real-subcommand"], Path::new("."));
        assert!(result.is_err(), "invalid git subcommand should fail");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not-a-real-subcommand"),
            "error should mention the invalid subcommand, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_git_command_returns_stdout() {
        let result = git_command(&["--version"], Path::new("."));
        let stdout = result.unwrap();
        assert!(
            stdout.contains("git version"),
            "stdout should contain 'git version', got: {}",
            stdout
        );
    }
}
