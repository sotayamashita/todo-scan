# todo-scan

[![CI](https://github.com/sotayamashita/todo-scan/actions/workflows/ci.yml/badge.svg)](https://github.com/sotayamashita/todo-scan/actions/workflows/ci.yml) 
[![codecov](https://codecov.io/gh/sotayamashita/todo-scan/graph/badge.svg)](https://codecov.io/gh/sotayamashita/todo-scan)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/sotayamashita/todo-scan)

A CI gate that fails your build when TODO comments exceed thresholds, so technical debt stays visible and under control.

### Why not `grep -r TODO .`?

- **No CI enforcement** — grep finds TODOs but can't fail a build when the count crosses a limit
- **No git awareness** — grep can't show which TODOs were added in a branch or since a specific commit
- **No structure** — grep gives raw text; todo-scan extracts tags, authors, priorities, issue refs, and deadlines
- **No format enforcement** — grep can't reject malformed TODOs like `todo fix this` in a pre-merge check

### Quick start

```sh
cargo install --path .

# See what you have
todo-scan list

# Set a ceiling and enforce it in CI
todo-scan check --max 100
```

### Key commands

| Command | What it does |
|---|---|
| `todo-scan check --max 100` | CI gate — fails the build when TODO count exceeds the threshold |
| `todo-scan diff main` | Shows TODOs added/removed since a git ref — useful in PR review |
| `todo-scan lint` | Enforces consistent TODO formatting (uppercase, colon, author) |

## Features

1. **Discover**
     - [Scan & List TODOs](#scan--list-todos)
     - [Search TODOs](#search-todos)
     - [Inline Code Context](#inline-code-context)
     - [Progressive Detail Levels](#progressive-detail-levels)
2. **Analyze**
     - [Diff Against Git Refs](#diff-against-git-refs)
     - [Dashboard & Statistics](#dashboard--statistics)
     - [Git Blame Integration](#git-blame-integration)
     - [Discover TODO Relationships](#discover-todo-relationships)
3. **Enforce**
     - [Inline Suppression](#inline-suppression)
     - [Lint TODO Format](#lint-todo-format)
     - [Clean Stale & Duplicate TODOs](#clean-stale--duplicate-todos)
     - [CI Quality Gate](#ci-quality-gate)
4. **Scale**
     - [Workspace-Aware Scanning](#workspace-aware-scanning)
     - [Per-Package CI Gate](#per-package-ci-gate)
     - [Single Package Scope](#single-package-scope)
5. **Report & Integrate**
     - [HTML Report Generation](#html-report-generation)
     - [CI Output Formats](#ci-output-formats)
     - [Claude Code Task Export](#claude-code-task-export)
6. **Productivity**
     - [Real-time File Watching](#real-time-file-watching)
     - [Interactive Setup](#interactive-setup)
     - [Shell Completions](#shell-completions)

### Scan & List TODOs

🔥 **Problem**

TODO comments scatter across hundreds of files, making it hard to know what's outstanding.

🌱 **Solution**

`todo-scan list` scans your entire codebase and displays every TODO, FIXME, HACK, XXX, BUG, and NOTE comment with color-coded tags, flexible grouping (`--group-by file|tag|priority|author|dir`), and filtering by priority, author, path glob, and result limit.

🎁 **Outcome**

One command gives you a complete, filterable inventory of all technical debt markers in your project.

```sh
todo-scan list --group-by tag --priority high
```

### Search TODOs

🔥 **Problem**

Scrolling through `todo-scan list` output or manually grepping to find specific TODOs is impractical in large codebases with hundreds of items.

🌱 **Solution**

`todo-scan search` filters TODO comments by message text or issue reference using case-insensitive substring matching, with `--exact` for case-sensitive searches and `-C` for context lines.

🎁 **Outcome**

You can instantly find relevant TODOs without scrolling through hundreds of items.

```sh
todo-scan search "migration" --author alice
```

### Inline Code Context

🔥 **Problem**

TODO lists show file:line references but lack surrounding code, forcing you to open files to understand what each TODO refers to.

🌱 **Solution**

`todo-scan context` displays the code around a specific line with related TODOs in the same file, and the `-C N` flag on `list` and `diff` adds inline context to every item.

🎁 **Outcome**

You understand what each TODO refers to without leaving the terminal.

```sh
todo-scan context src/main.rs:25 -C 3
```

### Progressive Detail Levels

🔥 **Problem**

Humans scanning a terminal need compact output, while AI agents need full metadata — but every command outputs the same level of detail regardless of the consumer.

🌱 **Solution**

`--detail minimal|normal|full` controls information density across `list`, `diff`, and `search`: `minimal` shows only file, line, tag, and message; `normal` (default) preserves current behavior; `full` injects `match_key` and auto-collects surrounding source context.

🎁 **Outcome**

One flag adapts todo-scan output from quick human glances to rich machine-readable payloads without separate commands.

```sh
todo-scan list --detail minimal
todo-scan search "migration" --detail full --format json
```

### Diff Against Git Refs

🔥 **Problem**

New TODOs slip into pull requests unnoticed while resolved ones go unrecognized.

🌱 **Solution**

`todo-scan diff` compares the current working tree against any git ref and shows exactly which TODOs were added or removed.

🎁 **Outcome**

Every PR review shows precisely what TODO debt changed, making it impossible to sneak in untracked work.

```sh
todo-scan diff main
```

### Dashboard & Statistics

🔥 **Problem**

A flat list of TODOs makes it hard to see the big picture — whether tech debt is growing, who owns the most items, and which files are hotspots.

🌱 **Solution**

`todo-scan stats` provides a dashboard summary with tag and author breakdowns, priority distribution, and top files by TODO count, with `--since <ref>` for trend analysis.

🎁 **Outcome**

You get an at-a-glance view of your project's technical debt health and trends.

```sh
todo-scan stats --since main
```

### Compressed Brief Summary

🔥 **Problem**

AI agents and developers working in token-constrained environments need a quick TODO landscape overview without consuming excessive context window or screen space.

🌱 **Solution**

`todo-scan brief` produces a compressed 2-4 line summary showing total counts, priority breakdown, and the single most urgent item, with optional `--since <ref>` for trend info and `--budget N` to cap output lines.

🎁 **Outcome**

You get the essential TODO health signal in minimal output, ideal for CI summaries and AI agent context.

```sh
todo-scan brief --since main --budget 2
```

### Git Blame Integration

🔥 **Problem**

TODO comments lack accountability — you can't tell who wrote them or when without manually running `git blame`.

🌱 **Solution**

`todo-scan blame` enriches each TODO with git blame metadata including author, commit date, and age in days, and flags items older than a configurable threshold as stale.

🎁 **Outcome**

Every TODO has clear ownership and age, making it easy to prioritize and assign cleanup work.

```sh
todo-scan blame --sort age --min-age 90d
```

### Discover TODO Relationships

🔥 **Problem**

TODOs in large codebases form implicit dependency chains, but existing tools treat each item in isolation.

🌱 **Solution**

`todo-scan relate` discovers relationships between TODO comments using same-file proximity, shared keywords, cross-references (same issue or author), and tag similarity, scoring each pair on a 0–1 scale.

🎁 **Outcome**

Related TODOs surface as actionable clusters, revealing hidden patterns in your technical debt.

```sh
todo-scan relate --cluster
```

### Inline Suppression

🔥 **Problem**

Some TODO comments are intentional or false positives, but the only way to exclude them is file-level patterns in `.todo-scan.toml`, which is too coarse.

🌱 **Solution**

Add `todo-scan:ignore` at the end of a TODO line to suppress that specific item, or place `todo-scan:ignore-next-line` on the line above to suppress the following TODO. Suppressed items are excluded from counts, checks, and output by default. Use `--show-ignored` to reveal them.

🎁 **Outcome**

You get fine-grained, inline control over false positives without maintaining exclusion lists in config files.

```
// TODO: this is tracked normally
// TODO: known false positive todo-scan:ignore
// todo-scan:ignore-next-line
// FIXME: suppressed item
```

### Lint TODO Format

🔥 **Problem**

TODO comments in team codebases drift in format — inconsistent casing, missing colons, missing authors — degrading scanner reliability and code hygiene.

🌱 **Solution**

`todo-scan lint` enforces configurable formatting rules (uppercase tags, colons, author attribution, issue references, message length) and exits with code 1 on violations, making it CI-ready out of the box.

🎁 **Outcome**

Every TODO in your codebase follows consistent formatting, improving both machine parseability and human readability.

```sh
todo-scan lint --require-author TODO,FIXME
```

### Clean Stale & Duplicate TODOs

🔥 **Problem**

TODOs accumulate faster than they resolve, and no amount of listing or linting reduces the pile.

🌱 **Solution**

`todo-scan clean` identifies TODOs referencing closed GitHub issues (stale) and those with identical messages across files (duplicates).

🎁 **Outcome**

You get an actionable cleanup list that targets the lowest-hanging fruit first.

```sh
todo-scan clean --check
```

### CI Quality Gate

🔥 **Problem**

Without enforcement, TODO debt grows silently until it becomes unmanageable.

🌱 **Solution**

`todo-scan check` acts as a CI gate that fails the build when TODO counts exceed a threshold, forbidden tags appear, too many new TODOs are introduced, or deadlines have expired.

🎁 **Outcome**

Your CI pipeline automatically prevents TODO debt from spiraling out of control.

```sh
todo-scan check --max 100 --block-tags BUG
```

### Workspace-Aware Scanning

🔥 **Problem**

Monorepos lack per-package TODO visibility — you can't tell which packages are accumulating debt without manually scanning each one.

🌱 **Solution**

`todo-scan workspace list` auto-detects your workspace format (Cargo, npm, pnpm, Nx, Go workspaces), scans each package independently, and displays a summary table with TODO counts, configured thresholds, and pass/fail status.

🎁 **Outcome**

Every package's TODO health is visible at a glance, making it easy to spot where debt concentrates.

```sh
todo-scan workspace list
```

### Per-Package CI Gate

🔥 **Problem**

A single global `--max` threshold doesn't work for monorepos where packages have different maturity levels.

🌱 **Solution**

`todo-scan check --workspace` evaluates per-package thresholds defined in `[workspace.packages.<name>]` config sections, failing the build if any package exceeds its individual limit or uses forbidden tags.

🎁 **Outcome**

Each package enforces its own TODO budget, matching reality instead of a one-size-fits-all limit.

```sh
todo-scan check --workspace
```

### Single Package Scope

🔥 **Problem**

Sometimes you only need to see TODOs in one package without the noise from the rest of the monorepo.

🌱 **Solution**

The `--package` flag on `list`, `check`, and `diff` scopes the scan to a single workspace package.

🎁 **Outcome**

You get focused results for just the package you're working on.

```sh
todo-scan list --package core
```

### HTML Report Generation

🔥 **Problem**

Presenting TODO metrics to stakeholders requires manual data collection and slide preparation.

🌱 **Solution**

`todo-scan report` generates a self-contained HTML dashboard with summary cards, trend charts from git history, tag/priority/age distribution, author breakdowns, and a sortable items table — zero external dependencies.

🎁 **Outcome**

You get a shareable, presentation-ready report droppable into any CI pipeline as an artifact.

```sh
todo-scan report --output debt.html --history 20
```

### CI Output Formats

🔥 **Problem**

Plain text output requires extra tooling to integrate with CI dashboards and PR workflows.

🌱 **Solution**

todo-scan supports `--format github-actions` for inline PR annotations, `--format sarif` for GitHub's [Code Scanning](https://docs.github.com/en/code-security/code-scanning) tab via SARIF, and `--format markdown` for PR comment bot tables.

🎁 **Outcome**

todo-scan integrates natively with your CI pipeline without any glue scripts.

```sh
todo-scan list --format github-actions
```

### Claude Code Task Export

🔥 **Problem**

Bridging TODO scanning with AI task orchestration requires manually parsing `todo-scan list --format json` output and constructing TaskCreate calls.

🌱 **Solution**

`todo-scan tasks` exports scanned TODOs as Claude Code Task-compatible JSON with action-verb subjects, code context in descriptions, and priority-based ordering.

🎁 **Outcome**

Your TODOs become AI-assignable tasks with a single command.

```sh
todo-scan tasks --dry-run
```

### Real-time File Watching

🔥 **Problem**

Re-running `todo-scan list` after every edit breaks flow when actively cleaning up TODO debt.

🌱 **Solution**

`todo-scan watch` monitors the filesystem and shows real-time TODO additions and removals as files change, with optional `--max` threshold warnings.

🎁 **Outcome**

You see the impact of your cleanup work instantly without switching context.

```sh
todo-scan watch
```

### Interactive Setup

🔥 **Problem**

New users must manually create `.todo-scan.toml` from documentation, slowing onboarding.

🌱 **Solution**

`todo-scan init` walks you through an interactive setup that detects your project type (Rust, Node, Go, Python), suggests appropriate exclude directories, and lets you choose which tags to track.

🎁 **Outcome**

You go from zero to a working configuration in seconds, not minutes of documentation reading.

```sh
todo-scan init
```

### Shell Completions

🔥 **Problem**

Shell completions are table stakes for CLI tools but require manual setup.

🌱 **Solution**

`todo-scan completions` generates completion scripts for bash, zsh, fish, elvish, and PowerShell and outputs them to stdout for easy installation.

🎁 **Outcome**

Tab completion works out of the box for every major shell.

```sh
todo-scan completions fish > ~/.config/fish/completions/todo-scan.fish
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
// TODO: false positive todo-scan:ignore     ← suppressed from output
// todo-scan:ignore-next-line                ← suppresses the line below
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

> **Note:** Detection is line-based. Multi-line constructs (Python docstrings, heredocs) are not supported. Tags must appear as standalone words — `todo-scan` and `TODOS` will not match `TODO`.

### Supported workspace formats

todo-scan auto-detects monorepo/workspace layouts by checking for these manifest files in order:

| Format | Manifest File | Member Field |
|--------|--------------|--------------|
| Cargo  | `Cargo.toml` | `[workspace] members` |
| npm    | `package.json` | `"workspaces"` array |
| pnpm   | `pnpm-workspace.yaml` | `packages` list |
| Nx     | `workspace.json` | `"projects"` map |
| Go     | `go.work` | `use` directives |

Glob patterns in member lists (e.g., `packages/*`, `crates/*`) are expanded automatically. You can also define packages manually in `.todo-scan.toml` with `[workspace]` configuration.

## Installation

```bash
cargo install --path .
```

## Usage

### List TODOs

```bash
# List all TODOs in current directory
todo-scan list

# Short alias
todo-scan ls

# Filter by tag
todo-scan list --tag FIXME
todo-scan list --tag TODO --tag BUG

# Filter by priority, author, or path
todo-scan list --priority urgent
todo-scan list --author alice
todo-scan list --path "src/**"

# Combine filters
todo-scan list --priority urgent --author alice --path "src/**"

# Limit results
todo-scan list --limit 10

# Group by tag, priority, author, or directory (default: file)
todo-scan list --group-by tag
todo-scan list --group-by priority
todo-scan list --group-by author
todo-scan list --group-by dir

# Sort by priority or tag severity
todo-scan list --sort priority
todo-scan list --sort tag

# JSON output
todo-scan list --format json
```

### Search TODOs

```bash
# Search by message text (case-insensitive)
todo-scan search "migration"

# Short alias
todo-scan s "migration"

# Case-sensitive exact match
todo-scan search "TODO" --exact

# Search by issue reference
todo-scan search "#123"

# Combine with filters
todo-scan search "fix" --author alice --tag FIXME --path "src/**"

# Show context lines around matches
todo-scan search "bug" -C 3

# JSON output with query metadata
todo-scan search "fix" --format json
```

### Show context around TODOs

```bash
# Show code context around a specific line (default: 5 lines)
todo-scan context src/main.rs:25

# Custom context window
todo-scan context src/main.rs:25 -C 3

# JSON output with related TODOs
todo-scan context src/main.rs:25 --format json

# Add context lines to list output
todo-scan list -C 3
todo-scan list -C 2 --format json

# Add context lines to diff output
todo-scan diff main -C 2
```

### Diff against a git ref

```bash
# Compare against main branch
todo-scan diff main

# Compare against recent commits
todo-scan diff HEAD~3

# Filter diff by tag
todo-scan diff main --tag FIXME

# JSON output
todo-scan diff main --format json
```

### Blame — TODO age and ownership

```bash
# Show all TODOs with git blame metadata
todo-scan blame

# Sort by age (oldest first)
todo-scan blame --sort age

# Filter by author (substring match)
todo-scan blame --author alice

# Filter by minimum age
todo-scan blame --min-age 90d

# Set stale threshold (default: 365 days)
todo-scan blame --stale-threshold 180d

# Filter by tag or path
todo-scan blame --tag TODO
todo-scan blame --path "src/**"

# JSON output
todo-scan blame --format json
```

### Stats dashboard

```bash
# Show tag/priority/author/hotspot summary
todo-scan stats

# Show trend compared to a git ref
todo-scan stats --since main

# JSON output
todo-scan stats --format json
```

### Brief summary

```bash
# Compressed summary (2-4 lines)
todo-scan brief

# With trend info compared to a git ref
todo-scan brief --since main

# Limit output to N lines
todo-scan brief --budget 1

# JSON output
todo-scan brief --format json
```

### Lint TODO formatting

```bash
# Check formatting with sensible defaults (uppercase, colon, no bare tags)
todo-scan lint

# Require author for specific tags
todo-scan lint --require-author TODO,FIXME

# Require issue reference for BUG tags
todo-scan lint --require-issue-ref BUG

# Enforce max message length
todo-scan lint --max-message-length 120

# Combine rules
todo-scan lint --require-author TODO --require-issue-ref BUG --max-message-length 120

# JSON output
todo-scan lint --format json
```

Exit codes: `0` = pass, `1` = fail, `2` = error.

### Clean — stale issues and duplicates

```bash
# Dry-run: show stale and duplicate TODOs (always exit 0)
todo-scan clean

# CI gate: exit 1 if any violations found
todo-scan clean --check

# Only flag issues closed more than 30 days ago
todo-scan clean --since 30d

# JSON output
todo-scan clean --format json
```

Exit codes (with `--check`): `0` = pass, `1` = fail, `2` = error. Without `--check`, always exits `0`.

### HTML report

```bash
# Generate report with default settings (todo-scan-report.html)
todo-scan report

# Custom output path
todo-scan report --output debt-report.html

# Sample more commits for trend chart
todo-scan report --history 20

# Skip history analysis (faster)
todo-scan report --history 0

# Set stale threshold
todo-scan report --stale-threshold 180d
```

### CI gate

```bash
# Fail if total TODOs exceed 100
todo-scan check --max 100

# Fail if any FIXME or BUG tags exist
todo-scan check --block-tags FIXME,BUG

# Fail if new TODOs were added since main
todo-scan check --max-new 0 --since main

# Fail if any TODOs have expired deadlines
todo-scan check --expired

# Combine rules
todo-scan check --max 50 --block-tags BUG --max-new 0 --since main --expired
```

Exit codes: `0` = pass, `1` = fail, `2` = error.

### Workspace — monorepo support

```bash
# List all packages with TODO counts
todo-scan workspace list

# Short aliases
todo-scan ws ls

# JSON output
todo-scan workspace list --format json

# Scope any command to a single package
todo-scan list --package core
todo-scan check --max 50 --package cli
todo-scan diff main --package core

# Per-package CI gate (uses [workspace.packages.*] config)
todo-scan check --workspace
```

### Relate — TODO relationships and clusters

```bash
# Discover all relationships (text output)
todo-scan relate

# JSON output
todo-scan relate --format json

# Group related TODOs into clusters
todo-scan relate --cluster

# Show TODOs related to a specific item
todo-scan relate --for src/auth.rs:42

# Set minimum relationship score (default: 0.3)
todo-scan relate --min-score 0.5

# Adjust proximity threshold (default: 10 lines)
todo-scan relate --proximity 20

# Combine options
todo-scan relate --cluster --min-score 0.4 --format json
```

### Export as Claude Code Tasks

```bash
# Preview tasks as JSON to stdout
todo-scan tasks --dry-run

# Write individual task files to a directory
todo-scan tasks --output ~/.claude/tasks/my-sprint/

# Filter by tag, priority, author, or path
todo-scan tasks --dry-run --tag BUG --priority urgent
todo-scan tasks --dry-run --author alice --path "src/**"

# Only TODOs added since a git ref
todo-scan tasks --dry-run --since main

# Control context lines in task descriptions (default: 3)
todo-scan tasks --dry-run -C 5

# JSON output
todo-scan tasks --dry-run --format json
```

### Global flags

| Flag | Description |
|---|---|
| `--root <path>` | Set the project root directory (default: current directory) |
| `--format <format>` | Output format: `text`, `json`, `github-actions`, `sarif`, `markdown` (default: text) |
| `--config <path>` | Path to config file (default: auto-discover `.todo-scan.toml`) |
| `--show-ignored` | Show items suppressed by `todo-scan:ignore` markers |

### Output formats

```bash
# GitHub Actions annotations — inline warnings/errors in PR diffs
todo-scan list --format github-actions
todo-scan check --max 100 --format github-actions

# SARIF — upload to GitHub Code Scanning / Security tab
todo-scan list --format sarif > results.sarif

# Markdown — tables for PR comment bots
todo-scan diff main --format markdown
```

### Quick start

```bash
# Interactive setup — generates .todo-scan.toml
todo-scan init

# Non-interactive with defaults
todo-scan init --yes
```

### Shell completions

```bash
# Bash
todo-scan completions bash > ~/.local/share/bash-completion/completions/todo-scan

# Zsh
todo-scan completions zsh > ~/.zfunc/_todo-scan

# Fish
todo-scan completions fish > ~/.config/fish/completions/todo-scan.fish
```

## Configuration

Create a `.todo-scan.toml` in your project root (or run `todo-scan init`). The file is discovered by searching upward from the current directory.

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

A machine-readable JSON Schema is available at [`schema/todo-scan.schema.json`](schema/todo-scan.schema.json) for editor validation and autocompletion (e.g., [Taplo](https://taplo.tamasfe.dev/), [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml)).

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

Per-package check thresholds for `todo-scan check --workspace`.

| Field | Type | Default | Description |
|---|---|---|---|
| `max` | `integer` | _(none)_ | Maximum TODOs allowed for this package |
| `block_tags` | `string[]` | `[]` | Tags that cause check to fail for this package |

## Agent Skill

todo-scan provides a [Claude Code plugin](https://docs.anthropic.com/en/docs/claude-code/skills) that enables AI coding agents to automatically use todo-scan commands for TODO tracking, CI gate configuration, and code quality checks.

### Install from plugin marketplace (recommended)

```bash
/plugin marketplace add sotayamashita/todo-scan
```

### Install with [skills CLI](https://github.com/vercel-labs/skills)

```bash
npx skills add sotayamashita/todo-scan
```

### Manual install

```bash
cp -r skills/todo-scan ~/.claude/skills/
```

## CI Integration

### GitHub Actions

```yaml
- name: Check TODOs
  run: |
    todo-scan check --max 100 --block-tags BUG,FIXME
```

### GitHub Actions with inline annotations

```yaml
- name: Check TODOs with annotations
  run: |
    todo-scan check --max 100 --format github-actions
```

### SARIF upload to Code Scanning

```yaml
- name: Scan TODOs
  run: todo-scan list --format sarif > todo-scan.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: todo-scan.sarif
```

### HTML report artifact

```yaml
- name: Generate TODO report
  run: todo-scan report --output todo-scan-report.html

- name: Upload TODO report
  uses: actions/upload-artifact@v4
  with:
    name: todo-scan-report
    path: todo-scan-report.html
```

### PR review with diff

> **Note:** `todo-scan diff` and `todo-scan check --since` need access to the base ref's git history.
> `actions/checkout@v4` uses `fetch-depth: 1` (shallow clone) by default, which means the base
> SHA is not available. Set `fetch-depth: 0` to fetch the full history.

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0  # Required for todo-scan to access the base ref

- name: Check new TODOs
  run: |
    todo-scan check --max-new 0 --since origin/main
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
