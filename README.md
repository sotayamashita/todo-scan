# todox

[![CI](https://github.com/sotayamashita/todox/actions/workflows/ci.yml/badge.svg)](https://github.com/sotayamashita/todox/actions/workflows/ci.yml) [![codecov](https://codecov.io/gh/sotayamashita/todox/graph/badge.svg)](https://codecov.io/gh/sotayamashita/todox) [![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/sotayamashita/todox)

> [!WARNING]
> **This is an experiment.** This repository exists to explore what AI can and cannot do across the entire software development lifecycle — and where human judgment remains essential. All code, issues, discussions, pull requests, and code reviews are authored and managed exclusively by [Claude Code](https://docs.anthropic.com/en/docs/claude-code) with no human review. Use this project at your own risk. The maintainers assume no responsibility for any issues arising from its use.

> [!NOTE]
> This project used the prompt from the **"Start your first agent team"** section of the [Claude Code Agent Teams documentation](https://code.claude.com/docs/en/agent-teams) as-is:
>
> ```
> I'm designing a CLI tool that helps developers track TODO comments across
> their codebase. Create an agent team to explore this from different angles: one
> teammate on UX, one on technical architecture, one playing devil's advocate.
> ```

Track TODO/FIXME/HACK comments in your codebase with git-aware diff and CI gate.

## Features

**`todox list`**

TODO comments scatter across hundreds of files, making it hard to know what's outstanding. `todox list` scans your entire codebase and displays every TODO, FIXME, HACK, XXX, BUG, and NOTE comment with color-coded tags, and supports flexible grouping (`--group-by file|tag|priority|author|dir`) and filtering by priority, author, path glob, and result limit. Run `todox list` or use the short alias `todox ls`.

**`todox diff <ref>`**

New TODOs slip into pull requests unnoticed while resolved ones go unrecognized. `todox diff` compares the current working tree against any git ref and shows exactly which TODOs were added or removed. Run `todox diff main` to compare against your main branch.

**`todox stats`**

A flat list of TODOs makes it hard to see the big picture — whether tech debt is growing, who owns the most items, and which files are hotspots. `todox stats` provides a dashboard summary with tag and author breakdowns, priority distribution, and top files by TODO count. Add `--since <ref>` to see the trend of added and removed items over time.

**`todox check`**

Without enforcement, TODO debt grows silently until it becomes unmanageable. `todox check` acts as a CI gate that fails the build when TODO counts exceed a threshold, forbidden tags appear, too many new TODOs are introduced, or deadlines have expired. Run `todox check --max 100 --block-tags BUG` in your CI pipeline, or `todox check --expired` to catch overdue TODOs.

**`todox context <file>:<line>`**

TODO lists show file:line references but lack surrounding code, forcing you to open files to understand what each TODO refers to. `todox context` displays the code around a specific line with related TODOs in the same file, and the `-C N` flag on `list` and `diff` adds inline context to every item. Run `todox context src/main.rs:25` or `todox list -C 3` to see code in context.

**CI-ready output formats**

Plain text output requires extra tooling to integrate with CI dashboards and PR workflows. todox supports `--format github-actions` for inline PR annotations, `--format sarif` for GitHub's [Code Scanning](https://docs.github.com/en/code-security/code-scanning) tab via SARIF (Static Analysis Results Interchange Format), and `--format markdown` for PR comment bot tables. Add `--format github-actions` to any command to get started.

### What it detects

Tags: `TODO`, `FIXME`, `HACK`, `XXX`, `BUG`, `NOTE` (case-insensitive)

```
// TODO: basic task
// FIXME(alice): broken parsing logic
// BUG: !! crashes on empty input       ← priority: urgent
// TODO: fix layout issue #123          ← issue ref extracted
// HACK(bob): workaround for JIRA-456   ← author + issue ref
// TODO(2025-06-01): migrate to v2 API   ← deadline (YYYY-MM-DD)
// TODO(alice, 2025-Q2): refactor auth   ← author + deadline (quarter)
```

## Installation

```bash
cargo install --path .
```

## Usage

### List TODOs

```bash
# List all TODOs in current directory
todox list

# Short alias
todox ls

# Filter by tag
todox list --tag FIXME
todox list --tag TODO --tag BUG

# Filter by priority, author, or path
todox list --priority urgent
todox list --author alice
todox list --path "src/**"

# Combine filters
todox list --priority urgent --author alice --path "src/**"

# Limit results
todox list --limit 10

# Group by tag, priority, author, or directory (default: file)
todox list --group-by tag
todox list --group-by priority
todox list --group-by author
todox list --group-by dir

# Sort by priority or tag severity
todox list --sort priority
todox list --sort tag

# JSON output
todox list --format json
```

### Show context around TODOs

```bash
# Show code context around a specific line (default: 5 lines)
todox context src/main.rs:25

# Custom context window
todox context src/main.rs:25 -C 3

# JSON output with related TODOs
todox context src/main.rs:25 --format json

# Add context lines to list output
todox list -C 3
todox list -C 2 --format json

# Add context lines to diff output
todox diff main -C 2
```

### Diff against a git ref

```bash
# Compare against main branch
todox diff main

# Compare against recent commits
todox diff HEAD~3

# Filter diff by tag
todox diff main --tag FIXME

# JSON output
todox diff main --format json
```

### Stats dashboard

```bash
# Show tag/priority/author/hotspot summary
todox stats

# Show trend compared to a git ref
todox stats --since main

# JSON output
todox stats --format json
```

### CI gate

```bash
# Fail if total TODOs exceed 100
todox check --max 100

# Fail if any FIXME or BUG tags exist
todox check --block-tags FIXME,BUG

# Fail if new TODOs were added since main
todox check --max-new 0 --since main

# Fail if any TODOs have expired deadlines
todox check --expired

# Combine rules
todox check --max 50 --block-tags BUG --max-new 0 --since main --expired
```

Exit codes: `0` = pass, `1` = fail, `2` = error.

### Global flags

| Flag | Description |
|---|---|
| `--root <path>` | Set the project root directory (default: current directory) |
| `--format <format>` | Output format: `text`, `json`, `github-actions`, `sarif`, `markdown` (default: text) |
| `--config <path>` | Path to config file (default: auto-discover `.todox.toml`) |

### Output formats

```bash
# GitHub Actions annotations — inline warnings/errors in PR diffs
todox list --format github-actions
todox check --max 100 --format github-actions

# SARIF — upload to GitHub Code Scanning / Security tab
todox list --format sarif > results.sarif

# Markdown — tables for PR comment bots
todox diff main --format markdown
```

## Configuration

Create a `.todox.toml` in your project root. The file is discovered by searching upward from the current directory.

```toml
# Tags to scan for (default: all supported tags)
tags = ["TODO", "FIXME", "HACK", "XXX", "BUG", "NOTE"]

# Directories to exclude from scanning
exclude_dirs = ["vendor", "third_party"]

# Regex patterns to exclude files
exclude_patterns = [".*\\.min\\.js$", ".*generated.*"]

[check]
# Maximum total TODOs allowed
max = 100

# Maximum new TODOs allowed (requires --since)
max_new = 0

# Tags that cause check to fail immediately
block_tags = ["BUG"]

# Fail if any TODOs have expired deadlines
expired = true
```

All fields are optional. Unspecified values use sensible defaults.

## Agent Skill

todox provides a [Claude Code skill](https://docs.anthropic.com/en/docs/claude-code/skills) that enables AI coding agents to automatically use todox commands for TODO tracking, CI gate configuration, and code quality checks.

### Install with [skills CLI](https://github.com/vercel-labs/skills)

```bash
npx skills add sotayamashita/todox
```

### Manual install

```bash
cp -r .claude/skills/todox ~/.claude/skills/
```

## CI Integration

### GitHub Actions

```yaml
- name: Check TODOs
  run: |
    todox check --max 100 --block-tags BUG,FIXME
```

### GitHub Actions with inline annotations

```yaml
- name: Check TODOs with annotations
  run: |
    todox check --max 100 --format github-actions
```

### SARIF upload to Code Scanning

```yaml
- name: Scan TODOs
  run: todox list --format sarif > todox.sarif
- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: todox.sarif
```

### PR review with diff

> **Note:** `todox diff` and `todox check --since` need access to the base ref's git history.
> `actions/checkout@v4` uses `fetch-depth: 1` (shallow clone) by default, which means the base
> SHA is not available. Set `fetch-depth: 0` to fetch the full history.

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0  # Required for todox to access the base ref

- name: Check new TODOs
  run: |
    todox check --max-new 0 --since origin/main
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run against a project
cargo run -- list --root /path/to/project
```
