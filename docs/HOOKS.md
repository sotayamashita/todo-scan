# Hook & Integration Recipes

Copy-paste recipes for integrating todo-scan into git hooks, CI pipelines, and Claude Code workflows.

## 1. Git Pre-commit Hook

Create `.git/hooks/pre-commit` (or add to an existing one):

```bash
#!/usr/bin/env bash
set -euo pipefail

# --- todo-scan lint: reject malformed TODO comments ---
echo "todo-scan: linting TODO comments..."
todo-scan lint --no-bare-tags --uppercase-tag --require-colon
lint_status=$?
if [ $lint_status -ne 0 ]; then
  echo "todo-scan lint failed. Fix the TODO formatting issues above."
  exit 1
fi

# --- todo-scan check: enforce thresholds ---
echo "todo-scan: checking TODO thresholds..."
todo-scan check --max 100 --block-tags BUG,FIXME
check_status=$?
if [ $check_status -ne 0 ]; then
  echo "todo-scan check failed. Reduce TODOs or adjust thresholds."
  exit 1
fi

echo "todo-scan: all checks passed."
```

Make it executable:

```bash
chmod +x .git/hooks/pre-commit
```

> **Tip:** If you use a hook manager like [husky](https://typicode.github.io/husky/) or [lefthook](https://github.com/evilmartians/lefthook), add the commands to your config file instead of editing `.git/hooks/` directly.

### Lefthook example

```yaml
# .lefthook.yml
pre-commit:
  commands:
    todo-scan-lint:
      run: todo-scan lint --no-bare-tags --uppercase-tag --require-colon
    todo-scan-check:
      run: todo-scan check --max 100 --block-tags BUG,FIXME
```

## 2. GitHub Actions CI Gate

### Basic workflow

```yaml
# .github/workflows/todo-scan.yml
name: TODO Gate

on:
  pull_request:
  push:
    branches: [main]

jobs:
  todo-scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Required for todo-scan diff and --since

      - name: Install todo-scan
        run: cargo install todo-scan

      - name: Lint TODO format
        run: todo-scan lint --no-bare-tags --uppercase-tag --require-colon

      - name: Check TODO thresholds
        run: todo-scan check --max 100 --block-tags BUG,FIXME

      - name: Block new TODOs in PR
        if: github.event_name == 'pull_request'
        run: todo-scan check --max-new 0 --since origin/${{ github.base_ref }}
```

### With SARIF upload and PR diff

```yaml
# .github/workflows/todo-scan.yml
name: TODO Gate

on:
  pull_request:
  push:
    branches: [main]

jobs:
  todo-scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install todo-scan
        run: cargo install todo-scan

      - name: Check TODO thresholds
        run: todo-scan check --max 100 --block-tags BUG,FIXME

      - name: Upload SARIF to Code Scanning
        if: always()
        run: todo-scan list --format sarif > todo-scan.sarif

      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: todo-scan.sarif

      - name: PR diff summary
        if: github.event_name == 'pull_request'
        run: |
          echo "## TODO Diff" >> "$GITHUB_STEP_SUMMARY"
          echo '```' >> "$GITHUB_STEP_SUMMARY"
          todo-scan diff origin/${{ github.base_ref }} >> "$GITHUB_STEP_SUMMARY"
          echo '```' >> "$GITHUB_STEP_SUMMARY"

      - name: Generate HTML report
        if: always()
        run: todo-scan report --output todo-scan-report.html

      - name: Upload report artifact
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: todo-scan-report
          path: todo-scan-report.html
```

## 3. Claude Code Hooks

Add these hooks to `.claude/settings.json` to run todo-scan automatically during Claude Code sessions.

### Lint on file write

Run `todo-scan lint` whenever Claude edits or creates a file, catching malformed TODOs immediately:

```jsonc
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "file_path=$(echo '$TOOL_INPUT' | jq -r '.file_path') && dir=$(dirname \"$file_path\") && todo-scan lint --root \"$dir\" --no-bare-tags --uppercase-tag --require-colon"
          }
        ]
      }
    ]
  }
}
```

### Diff summary on stop

Show a TODO diff summary when a Claude Code session ends, so you can see what changed:

```jsonc
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "todo-scan diff HEAD~1 2>/dev/null || true"
          }
        ]
      }
    ]
  }
}
```

### Combined example

A complete `.claude/settings.json` with both hooks:

```jsonc
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "file_path=$(echo '$TOOL_INPUT' | jq -r '.file_path') && dir=$(dirname \"$file_path\") && todo-scan lint --root \"$dir\" --no-bare-tags --uppercase-tag --require-colon"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "todo-scan diff HEAD~1 2>/dev/null || true"
          }
        ]
      }
    ]
  }
}
```

## 4. Claude Code CLAUDE.md Snippet

Add this to your project's `CLAUDE.md` to instruct Claude Code to use todo-scan during development:

````markdown
## TODO Hygiene

- Run `todo-scan lint --no-bare-tags --uppercase-tag --require-colon` before committing
- Run `todo-scan check --max 100 --block-tags BUG,FIXME` to verify thresholds
- Use `todo-scan diff main` to review TODO changes before opening a PR
- Format TODO comments as: `TAG(author): message #issue-ref`
- Tags must be uppercase with a colon separator
- Do not leave bare tags (e.g., `// TODO` with no message)
````

## Reference

| Command | Purpose |
|---|---|
| `todo-scan lint` | Check TODO formatting rules |
| `todo-scan check` | Enforce count/tag thresholds |
| `todo-scan diff <ref>` | Compare TODOs against a git ref |
| `todo-scan list` | List all TODOs |
| `todo-scan report` | Generate HTML dashboard |

See `todo-scan <command> --help` for all available flags.
