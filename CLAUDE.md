# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                  # Debug build
cargo build --release        # Release build
cargo test                   # Run all tests (unit + integration)
cargo test scanner           # Run tests matching "scanner"
cargo test --test integration_list  # Run a specific integration test file
cargo fmt                    # Format code
cargo check                  # Type-check without building
cargo run -- list            # Run list command on current directory
cargo run -- diff main       # Diff TODOs against main branch
cargo run -- check --max 100 # CI gate check
cargo llvm-cov --summary-only          # Show coverage summary
cargo llvm-cov --ignore-run-fail --summary-only --fail-under-lines 88  # CI coverage gate
cargo llvm-cov --html                  # Generate HTML coverage report in target/llvm-cov/html/
```

## Architecture

todo-scan is a Rust CLI tool that scans codebases for TODO/FIXME/HACK/XXX/BUG/NOTE comments and provides listing, diffing, and CI gating capabilities.

### Module Responsibilities

- **main.rs** - Entry point, clap CLI dispatch to `cmd_list()`, `cmd_diff()`, `cmd_check()`
- **cli.rs** - CLI argument definitions using clap derive macros (three subcommands: list, diff, check)
- **model.rs** - Core data types: `Tag`, `TodoItem`, `ScanResult`, `DiffResult`, `CheckResult`, `DiffEntry`
- **scanner.rs** - Directory walking (via `ignore` crate for .gitignore support), file reading, regex matching to extract TODO items
- **config.rs** - Loads `.todo-scan.toml` config, provides defaults, builds the scan regex pattern
- **diff.rs** - Retrieves file contents at a git ref via `git show`/`git ls-tree`, computes added/removed/modified TODOs using `match_key()`
- **check.rs** - Validates against thresholds: `--max` (total count), `--block-tags` (forbidden tags), `--max-new` (new items limit)
- **output.rs** - Text (colored, grouped by file) and JSON (serde) output formatting

### Data Flow

1. **list**: scan directory → collect `TodoItem`s → sort/filter → print
2. **diff**: scan current + scan at git ref → compare by `match_key()` → produce `DiffEntry` (added/removed/modified) → print
3. **check**: scan (+ optional diff) → evaluate `CheckViolation`s → PASS/FAIL with exit code (0=pass, 1=fail, 2=error)

### Tag Pattern

The scanner regex matches: `TAG[(author)][!{1,2}]: message` where `!` = high priority, `!!` = urgent. Issue references (`#123`, `JIRA-456`) are extracted from the message text.

## Integration Tests

Tests in `tests/` use `assert_cmd` + `tempfile` to create temporary directories with fixture files and run the binary as a subprocess. Each test file corresponds to a subcommand.

## Development Workflow

**IMPORTANT: Before starting any development task, read and follow the workflow defined in `docs/DEVELOPMENT_WORKFLOW.md`.**

Key rules:
- All development is issue-driven — no work without a corresponding issue
- Use `gh` CLI for all GitHub operations (issues, PRs, project board)
- Follow TDD: write failing tests first, then implement
- Link PRs to issues with `Closes #<number>` for auto-closing
