---
name: todox
description: >
  Use todox CLI to scan, diff, and gate TODO/FIXME/HACK/XXX/BUG/NOTE comments
  in codebases. Use when scanning for TODO comments, listing TODOs with filters,
  comparing TODOs between git refs, running CI quality gates on TODO counts,
  configuring .todox.toml, or setting up pre-merge checks for TODO hygiene.
---

# todox

Scan TODO/FIXME/HACK/XXX/BUG/NOTE comments in source code. Three commands: `list` (scan), `diff` (compare git refs), `check` (CI gate).

Prefer `--format json` for all commands to enable structured parsing.

## Commands

### list — Scan and display TODOs

```bash
todox list --format json
todox list --format json --tag TODO --tag FIXME
todox list --format json --sort priority
```

Flags:
- `--tag <TAG>` — Filter by tag (repeatable)
- `--sort <file|tag|priority>` — Sort order (default: `file`)
- `--format <text|json>` — Output format

JSON output:

```json
{
  "items": [
    {
      "file": "src/main.rs",
      "line": 10,
      "tag": "TODO",
      "message": "implement feature X",
      "author": "alice",
      "issue_ref": "#123",
      "priority": "normal"
    }
  ],
  "files_scanned": 42
}
```

Priority values: `"normal"` (default), `"high"` (`!`), `"urgent"` (`!!`).

Count TODOs:

```bash
todox list --format json | jq '.items | length'
```

### diff — Compare TODOs between git refs

```bash
todox diff main --format json
todox diff HEAD~5 --format json --tag BUG
```

Arguments:
- `<GIT_REF>` — Base git ref to compare against (required)

Flags:
- `--tag <TAG>` — Filter by tag (repeatable)
- `--format <text|json>` — Output format

JSON output:

```json
{
  "entries": [
    {
      "status": "added",
      "item": { "file": "src/parser.rs", "line": 45, "tag": "FIXME", "message": "broken parsing logic", "author": "bob", "issue_ref": null, "priority": "urgent" }
    }
  ],
  "added_count": 3,
  "removed_count": 1,
  "base_ref": "main"
}
```

Count new TODOs since main:

```bash
todox diff main --format json | jq '.added_count'
```

### check — CI quality gate

```bash
todox check
todox check --max 50
todox check --block-tags BUG --block-tags HACK
todox check --max-new 0 --since main
```

Flags:
- `--max <N>` — Fail if total TODOs exceed N
- `--block-tags <TAG>` — Fail if any matching tag exists (repeatable)
- `--max-new <N>` — Fail if new TODOs since `--since` exceed N
- `--since <GIT_REF>` — Base ref for `--max-new` comparison
- `--format <text|json>` — Output format

JSON output:

```json
{
  "passed": false,
  "total": 150,
  "violations": [
    { "rule": "max", "message": "Total TODOs (150) exceeds max (100)" },
    { "rule": "block_tags", "message": "Blocked tag BUG found in src/api.rs:23" }
  ]
}
```

## Global Flags

All commands accept:
- `--root <PATH>` — Project root (default: cwd)
- `--format <text|json>` — Output format (default: `text`)
- `--config <PATH>` — Config file path (default: `.todox.toml` searched upward)

## Exit Codes

| Code | Meaning |
|------|---------|
| `0`  | Success / check passed |
| `1`  | Check failed (violations found) |
| `2`  | Error (invalid args, parse failure) |

## Configuration — .todox.toml

```toml
# Tags to scan (default: all six)
tags = ["TODO", "FIXME", "HACK", "XXX", "BUG", "NOTE"]

# Directories to exclude
exclude_dirs = ["vendor", "third_party", "node_modules"]

# File patterns to exclude (regex)
exclude_patterns = [".*\\.min\\.js$", ".*generated.*"]

[check]
max = 100
max_new = 0
block_tags = ["BUG"]
```

## Common Patterns

**PR pre-merge gate — block BUG tags and cap new TODOs:**

```bash
todox check --block-tags BUG --max-new 0 --since main --format json
```

**List all high/urgent priority TODOs:**

```bash
todox list --format json | jq '[.items[] | select(.priority != "normal")]'
```

**Detect new TODOs on current branch:**

```bash
todox diff main --format json | jq '[.entries[] | select(.status == "added")]'
```

**GitHub Actions CI step:**

```yaml
- name: TODO gate
  run: todox check --max 100 --block-tags BUG --max-new 0 --since origin/main
```
