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

1. **Discover**
     - [Scan & List TODOs](#scan--list-todos) · [Search TODOs](#search-todos) · [Inline Code Context](#inline-code-context)
2. **Analyze**
     - [Diff Against Git Refs](#diff-against-git-refs) · [Dashboard & Statistics](#dashboard--statistics) · [Git Blame Integration](#git-blame-integration) · [Discover TODO Relationships](#discover-todo-relationships)
3. **Enforce**
     - [Inline Suppression](#inline-suppression) · [Lint TODO Format](#lint-todo-format) · [Clean Stale & Duplicate TODOs](#clean-stale--duplicate-todos) · [CI Quality Gate](#ci-quality-gate)
4. **Scale**
     - [Workspace-Aware Scanning](#workspace-aware-scanning) · [Per-Package CI Gate](#per-package-ci-gate) · [Single Package Scope](#single-package-scope)
5. **Report & Integrate**
     - [HTML Report Generation](#html-report-generation) · [CI Output Formats](#ci-output-formats) · [Claude Code Task Export](#claude-code-task-export)
6. **Productivity**
     - [Real-time File Watching](#real-time-file-watching) · [Interactive Setup](#interactive-setup) · [Shell Completions](#shell-completions)

### Scan & List TODOs

🔥 **Problem**

TODO comments scatter across hundreds of files, making it hard to know what's outstanding.

🌱 **Solution**

`todox list` scans your entire codebase and displays every TODO, FIXME, HACK, XXX, BUG, and NOTE comment with color-coded tags, flexible grouping (`--group-by file|tag|priority|author|dir`), and filtering by priority, author, path glob, and result limit.

🎁 **Outcome**

One command gives you a complete, filterable inventory of all technical debt markers in your project.

```sh
todox list --group-by tag --priority high
```

### Search TODOs

🔥 **Problem**

Scrolling through `todox list` output or manually grepping to find specific TODOs is impractical in large codebases with hundreds of items.

🌱 **Solution**

`todox search` filters TODO comments by message text or issue reference using case-insensitive substring matching, with `--exact` for case-sensitive searches and `-C` for context lines.

🎁 **Outcome**

You can instantly find relevant TODOs without scrolling through hundreds of items.

```sh
todox search "migration" --author alice
```

### Inline Code Context

🔥 **Problem**

TODO lists show file:line references but lack surrounding code, forcing you to open files to understand what each TODO refers to.

🌱 **Solution**

`todox context` displays the code around a specific line with related TODOs in the same file, and the `-C N` flag on `list` and `diff` adds inline context to every item.

🎁 **Outcome**

You understand what each TODO refers to without leaving the terminal.

```sh
todox context src/main.rs:25 -C 3
```

### Diff Against Git Refs

🔥 **Problem**

New TODOs slip into pull requests unnoticed while resolved ones go unrecognized.

🌱 **Solution**

`todox diff` compares the current working tree against any git ref and shows exactly which TODOs were added or removed.

🎁 **Outcome**

Every PR review shows precisely what TODO debt changed, making it impossible to sneak in untracked work.

```sh
todox diff main
```

### Dashboard & Statistics

🔥 **Problem**

A flat list of TODOs makes it hard to see the big picture — whether tech debt is growing, who owns the most items, and which files are hotspots.

🌱 **Solution**

`todox stats` provides a dashboard summary with tag and author breakdowns, priority distribution, and top files by TODO count, with `--since <ref>` for trend analysis.

🎁 **Outcome**

You get an at-a-glance view of your project's technical debt health and trends.

```sh
todox stats --since main
```

### Git Blame Integration

🔥 **Problem**

TODO comments lack accountability — you can't tell who wrote them or when without manually running `git blame`.

🌱 **Solution**

`todox blame` enriches each TODO with git blame metadata including author, commit date, and age in days, and flags items older than a configurable threshold as stale.

🎁 **Outcome**

Every TODO has clear ownership and age, making it easy to prioritize and assign cleanup work.

```sh
todox blame --sort age --min-age 90d
```

### Discover TODO Relationships

🔥 **Problem**

TODOs in large codebases form implicit dependency chains, but existing tools treat each item in isolation.

🌱 **Solution**

`todox relate` discovers relationships between TODO comments using same-file proximity, shared keywords, cross-references (same issue or author), and tag similarity, scoring each pair on a 0–1 scale.

🎁 **Outcome**

Related TODOs surface as actionable clusters, revealing hidden patterns in your technical debt.

```sh
todox relate --cluster
```

### Inline Suppression

🔥 **Problem**

Some TODO comments are intentional or false positives, but the only way to exclude them is file-level patterns in `.todox.toml`, which is too coarse.

🌱 **Solution**

Add `todox:ignore` at the end of a TODO line to suppress that specific item, or place `todox:ignore-next-line` on the line above to suppress the following TODO. Suppressed items are excluded from counts, checks, and output by default. Use `--show-ignored` to reveal them.

🎁 **Outcome**

You get fine-grained, inline control over false positives without maintaining exclusion lists in config files.

```
// TODO: this is tracked normally
// TODO: known false positive todox:ignore
// todox:ignore-next-line
// FIXME: suppressed item
```

### Lint TODO Format

🔥 **Problem**

TODO comments in team codebases drift in format — inconsistent casing, missing colons, missing authors — degrading scanner reliability and code hygiene.

🌱 **Solution**

`todox lint` enforces configurable formatting rules (uppercase tags, colons, author attribution, issue references, message length) and exits with code 1 on violations, making it CI-ready out of the box.

🎁 **Outcome**

Every TODO in your codebase follows consistent formatting, improving both machine parseability and human readability.

```sh
todox lint --require-author TODO,FIXME
```

### Clean Stale & Duplicate TODOs

🔥 **Problem**

TODOs accumulate faster than they resolve, and no amount of listing or linting reduces the pile.

🌱 **Solution**

`todox clean` identifies TODOs referencing closed GitHub issues (stale) and those with identical messages across files (duplicates).

🎁 **Outcome**

You get an actionable cleanup list that targets the lowest-hanging fruit first.

```sh
todox clean --check
```

### CI Quality Gate

🔥 **Problem**

Without enforcement, TODO debt grows silently until it becomes unmanageable.

🌱 **Solution**

`todox check` acts as a CI gate that fails the build when TODO counts exceed a threshold, forbidden tags appear, too many new TODOs are introduced, or deadlines have expired.

🎁 **Outcome**

Your CI pipeline automatically prevents TODO debt from spiraling out of control.

```sh
todox check --max 100 --block-tags BUG
```

### Workspace-Aware Scanning

🔥 **Problem**

Monorepos lack per-package TODO visibility — you can't tell which packages are accumulating debt without manually scanning each one.

🌱 **Solution**

`todox workspace list` auto-detects your workspace format (Cargo, npm, pnpm, Nx, Go workspaces), scans each package independently, and displays a summary table with TODO counts, configured thresholds, and pass/fail status.

🎁 **Outcome**

Every package's TODO health is visible at a glance, making it easy to spot where debt concentrates.

```sh
todox workspace list
```

### Per-Package CI Gate

🔥 **Problem**

A single global `--max` threshold doesn't work for monorepos where packages have different maturity levels.

🌱 **Solution**

`todox check --workspace` evaluates per-package thresholds defined in `[workspace.packages.<name>]` config sections, failing the build if any package exceeds its individual limit or uses forbidden tags.

🎁 **Outcome**

Each package enforces its own TODO budget, matching reality instead of a one-size-fits-all limit.

```sh
todox check --workspace
```

### Single Package Scope

🔥 **Problem**

Sometimes you only need to see TODOs in one package without the noise from the rest of the monorepo.

🌱 **Solution**

The `--package` flag on `list`, `check`, and `diff` scopes the scan to a single workspace package.

🎁 **Outcome**

You get focused results for just the package you're working on.

```sh
todox list --package core
```

### HTML Report Generation

🔥 **Problem**

Presenting TODO metrics to stakeholders requires manual data collection and slide preparation.

🌱 **Solution**

`todox report` generates a self-contained HTML dashboard with summary cards, trend charts from git history, tag/priority/age distribution, author breakdowns, and a sortable items table — zero external dependencies.

🎁 **Outcome**

You get a shareable, presentation-ready report droppable into any CI pipeline as an artifact.

```sh
todox report --output debt.html --history 20
```

### CI Output Formats

🔥 **Problem**

Plain text output requires extra tooling to integrate with CI dashboards and PR workflows.

🌱 **Solution**

todox supports `--format github-actions` for inline PR annotations, `--format sarif` for GitHub's [Code Scanning](https://docs.github.com/en/code-security/code-scanning) tab via SARIF, and `--format markdown` for PR comment bot tables.

🎁 **Outcome**

todox integrates natively with your CI pipeline without any glue scripts.

```sh
todox list --format github-actions
```

### Claude Code Task Export

🔥 **Problem**

Bridging TODO scanning with AI task orchestration requires manually parsing `todox list --format json` output and constructing TaskCreate calls.

🌱 **Solution**

`todox tasks` exports scanned TODOs as Claude Code Task-compatible JSON with action-verb subjects, code context in descriptions, and priority-based ordering.

🎁 **Outcome**

Your TODOs become AI-assignable tasks with a single command.

```sh
todox tasks --dry-run
```

### Real-time File Watching

🔥 **Problem**

Re-running `todox list` after every edit breaks flow when actively cleaning up TODO debt.

🌱 **Solution**

`todox watch` monitors the filesystem and shows real-time TODO additions and removals as files change, with optional `--max` threshold warnings.

🎁 **Outcome**

You see the impact of your cleanup work instantly without switching context.

```sh
todox watch
```

### Interactive Setup

🔥 **Problem**

New users must manually create `.todox.toml` from documentation, slowing onboarding.

🌱 **Solution**

`todox init` walks you through an interactive setup that detects your project type (Rust, Node, Go, Python), suggests appropriate exclude directories, and lets you choose which tags to track.

🎁 **Outcome**

You go from zero to a working configuration in seconds, not minutes of documentation reading.

```sh
todox init
```

### Shell Completions

🔥 **Problem**

Shell completions are table stakes for CLI tools but require manual setup.

🌱 **Solution**

`todox completions` generates completion scripts for bash, zsh, fish, elvish, and PowerShell and outputs them to stdout for easy installation.

🎁 **Outcome**

Tab completion works out of the box for every major shell.

```sh
todox completions fish > ~/.config/fish/completions/todox.fish
```

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
// TODO: false positive todox:ignore     ← suppressed from output
// todox:ignore-next-line                ← suppresses the line below
// FIXME: suppressed item
```

### Supported comment syntax

The scanner uses line-based heuristic comment detection, not a language parser. The following comment prefixes are recognized:

| Prefix | Languages |
|--------|-----------|
| `//`   | Rust, C/C++, Java, Go, JavaScript, TypeScript, Swift, Kotlin, C# |
| `#`    | Python, Ruby, Shell, YAML, TOML, Perl, R |
| `/* `  | C/C++, Java, JavaScript, CSS (block comment start) |
| ` * `  | Block comment continuation lines |
| `--`   | SQL, Haskell, Lua, Ada |
| `<!--` | HTML, XML, Markdown |
| `;`    | Lisp, Clojure, Assembly, INI |
| `(*`   | OCaml, Pascal, F# |
| `{-`   | Haskell (block) |
| `%`    | LaTeX, Erlang, MATLAB |

> **Note:** Detection is line-based. Multi-line constructs (Python docstrings, heredocs) are not supported. Tags must appear as standalone words — `todox` and `TODOS` will not match `TODO`.

### Supported workspace formats

todox auto-detects monorepo/workspace layouts by checking for these manifest files in order:

| Format | Manifest File | Member Field |
|--------|--------------|--------------|
| Cargo  | `Cargo.toml` | `[workspace] members` |
| npm    | `package.json` | `"workspaces"` array |
| pnpm   | `pnpm-workspace.yaml` | `packages` list |
| Nx     | `workspace.json` | `"projects"` map |
| Go     | `go.work` | `use` directives |

Glob patterns in member lists (e.g., `packages/*`, `crates/*`) are expanded automatically. You can also define packages manually in `.todox.toml` with `[workspace]` configuration.

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

### Search TODOs

```bash
# Search by message text (case-insensitive)
todox search "migration"

# Short alias
todox s "migration"

# Case-sensitive exact match
todox search "TODO" --exact

# Search by issue reference
todox search "#123"

# Combine with filters
todox search "fix" --author alice --tag FIXME --path "src/**"

# Show context lines around matches
todox search "bug" -C 3

# JSON output with query metadata
todox search "fix" --format json
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

### Blame — TODO age and ownership

```bash
# Show all TODOs with git blame metadata
todox blame

# Sort by age (oldest first)
todox blame --sort age

# Filter by author (substring match)
todox blame --author alice

# Filter by minimum age
todox blame --min-age 90d

# Set stale threshold (default: 365 days)
todox blame --stale-threshold 180d

# Filter by tag or path
todox blame --tag TODO
todox blame --path "src/**"

# JSON output
todox blame --format json
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

### Lint TODO formatting

```bash
# Check formatting with sensible defaults (uppercase, colon, no bare tags)
todox lint

# Require author for specific tags
todox lint --require-author TODO,FIXME

# Require issue reference for BUG tags
todox lint --require-issue-ref BUG

# Enforce max message length
todox lint --max-message-length 120

# Combine rules
todox lint --require-author TODO --require-issue-ref BUG --max-message-length 120

# JSON output
todox lint --format json
```

Exit codes: `0` = pass, `1` = fail, `2` = error.

### Clean — stale issues and duplicates

```bash
# Dry-run: show stale and duplicate TODOs (always exit 0)
todox clean

# CI gate: exit 1 if any violations found
todox clean --check

# Only flag issues closed more than 30 days ago
todox clean --since 30d

# JSON output
todox clean --format json
```

Exit codes (with `--check`): `0` = pass, `1` = fail, `2` = error. Without `--check`, always exits `0`.

### HTML report

```bash
# Generate report with default settings (todox-report.html)
todox report

# Custom output path
todox report --output debt-report.html

# Sample more commits for trend chart
todox report --history 20

# Skip history analysis (faster)
todox report --history 0

# Set stale threshold
todox report --stale-threshold 180d
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

### Workspace — monorepo support

```bash
# List all packages with TODO counts
todox workspace list

# Short aliases
todox ws ls

# JSON output
todox workspace list --format json

# Scope any command to a single package
todox list --package core
todox check --max 50 --package cli
todox diff main --package core

# Per-package CI gate (uses [workspace.packages.*] config)
todox check --workspace
```

### Relate — TODO relationships and clusters

```bash
# Discover all relationships (text output)
todox relate

# JSON output
todox relate --format json

# Group related TODOs into clusters
todox relate --cluster

# Show TODOs related to a specific item
todox relate --for src/auth.rs:42

# Set minimum relationship score (default: 0.3)
todox relate --min-score 0.5

# Adjust proximity threshold (default: 10 lines)
todox relate --proximity 20

# Combine options
todox relate --cluster --min-score 0.4 --format json
```

### Export as Claude Code Tasks

```bash
# Preview tasks as JSON to stdout
todox tasks --dry-run

# Write individual task files to a directory
todox tasks --output ~/.claude/tasks/my-sprint/

# Filter by tag, priority, author, or path
todox tasks --dry-run --tag BUG --priority urgent
todox tasks --dry-run --author alice --path "src/**"

# Only TODOs added since a git ref
todox tasks --dry-run --since main

# Control context lines in task descriptions (default: 3)
todox tasks --dry-run -C 5

# JSON output
todox tasks --dry-run --format json
```

### Global flags

| Flag | Description |
|---|---|
| `--root <path>` | Set the project root directory (default: current directory) |
| `--format <format>` | Output format: `text`, `json`, `github-actions`, `sarif`, `markdown` (default: text) |
| `--config <path>` | Path to config file (default: auto-discover `.todox.toml`) |
| `--show-ignored` | Show items suppressed by `todox:ignore` markers |

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

### Quick start

```bash
# Interactive setup — generates .todox.toml
todox init

# Non-interactive with defaults
todox init --yes
```

### Shell completions

```bash
# Bash
todox completions bash > ~/.local/share/bash-completion/completions/todox

# Zsh
todox completions zsh > ~/.zfunc/_todox

# Fish
todox completions fish > ~/.config/fish/completions/todox.fish
```

## Configuration

Create a `.todox.toml` in your project root (or run `todox init`). The file is discovered by searching upward from the current directory.

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

[blame]
# Days threshold for marking TODOs as stale (default: 365d)
stale_threshold = "180d"

[clean]
# Enable stale issue detection (default: true)
stale_issues = true

# Enable duplicate detection (default: true)
duplicates = true

# Only flag issues closed longer than this duration (default: disabled)
# since = "30d"

[workspace]
# Disable automatic workspace detection (default: true)
# auto_detect = false

# Per-package check thresholds
[workspace.packages.core]
max = 50
block_tags = ["BUG"]

[workspace.packages.cli]
max = 20

[lint]
# Reject TODOs with empty message (default: true)
no_bare_tags = true

# Enforce uppercase tag names (default: true)
uppercase_tag = true

# Enforce colon after tag (default: true)
require_colon = true

# Enforce max message character count (default: disabled)
# max_message_length = 120

# Require (author) for specified tags (default: disabled)
# require_author = ["TODO", "FIXME"]

# Require issue ref for specified tags (default: disabled)
# require_issue_ref = ["BUG"]
```

All fields are optional. Unspecified values use sensible defaults.

A machine-readable JSON Schema is available at [`schema/todox.schema.json`](schema/todox.schema.json) for editor validation and autocompletion (e.g., [Taplo](https://taplo.tamasfe.dev/), [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml)).

### Configuration Reference

#### Top-level fields

| Field | Type | Default | Description |
|---|---|---|---|
| `tags` | `string[]` | `["TODO","FIXME","HACK","XXX","BUG","NOTE"]` | Tag keywords to scan for |
| `exclude_dirs` | `string[]` | `[]` | Directory names to skip during scanning |
| `exclude_patterns` | `string[]` | `[]` | Regex patterns; matching file paths are excluded |

#### `[check]` section

| Field | Type | Default | Description |
|---|---|---|---|
| `max` | `integer` | _(none)_ | Maximum total TODOs allowed |
| `max_new` | `integer` | _(none)_ | Maximum new TODOs allowed (requires `--since`) |
| `block_tags` | `string[]` | `[]` | Tags that cause `check` to fail immediately |
| `expired` | `boolean` | _(none)_ | Fail if any TODOs have expired deadlines |

#### `[blame]` section

| Field | Type | Default | Description |
|---|---|---|---|
| `stale_threshold` | `string` | `"365d"` | Duration threshold for marking TODOs as stale |

#### `[clean]` section

| Field | Type | Default | Description |
|---|---|---|---|
| `stale_issues` | `boolean` | `true` | Enable stale issue detection via `gh` CLI |
| `duplicates` | `boolean` | `true` | Enable duplicate TODO detection |
| `since` | `string` | _(none)_ | Only flag issues closed longer than this duration (e.g., `"30d"`) |

#### `[lint]` section

| Field | Type | Default | Description |
|---|---|---|---|
| `no_bare_tags` | `boolean` | `true` | Reject TODOs with empty message |
| `uppercase_tag` | `boolean` | `true` | Enforce uppercase tag names |
| `require_colon` | `boolean` | `true` | Enforce colon after tag |
| `max_message_length` | `integer` | _(none)_ | Enforce max message character count |
| `require_author` | `string[]` | _(none)_ | Require `(author)` for specified tags |
| `require_issue_ref` | `string[]` | _(none)_ | Require issue ref for specified tags |

#### `[workspace]` section

| Field | Type | Default | Description |
|---|---|---|---|
| `auto_detect` | `boolean` | `true` | Enable automatic workspace detection |

#### `[workspace.packages.<name>]` section

Per-package check thresholds for `todox check --workspace`.

| Field | Type | Default | Description |
|---|---|---|---|
| `max` | `integer` | _(none)_ | Maximum TODOs allowed for this package |
| `block_tags` | `string[]` | `[]` | Tags that cause check to fail for this package |

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

### HTML report artifact

```yaml
- name: Generate TODO report
  run: todox report --output todox-report.html

- name: Upload TODO report
  uses: actions/upload-artifact@v4
  with:
    name: todox-report
    path: todox-report.html
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
