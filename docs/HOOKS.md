# Hook & Integration Recipes

Copy-paste recipes for integrating todox into git hooks, CI pipelines, and Claude Code workflows.

## 1. Git Pre-commit Hook

Create `.git/hooks/pre-commit` (or add to an existing one):

```bash
#!/usr/bin/env bash
set -euo pipefail

# --- todox lint: reject malformed TODO comments ---
echo "todox: linting TODO comments..."
todox lint --no-bare-tags --uppercase-tag --require-colon
lint_status=$?
if [ $lint_status -ne 0 ]; then
  echo "todox lint failed. Fix the TODO formatting issues above."
  exit 1
fi

# --- todox check: enforce thresholds ---
echo "todox: checking TODO thresholds..."
todox check --max 100 --block-tags BUG,FIXME
check_status=$?
if [ $check_status -ne 0 ]; then
  echo "todox check failed. Reduce TODOs or adjust thresholds."
  exit 1
fi

echo "todox: all checks passed."
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
    todox-lint:
      run: todox lint --no-bare-tags --uppercase-tag --require-colon
    todox-check:
      run: todox check --max 100 --block-tags BUG,FIXME
```

## 2. GitHub Actions CI Gate

### Basic workflow

```yaml
# .github/workflows/todox.yml
name: TODO Gate

on:
  pull_request:
  push:
    branches: [main]

jobs:
  todox:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Required for todox diff and --since

      - name: Install todox
        run: cargo install todox

      - name: Lint TODO format
        run: todox lint --no-bare-tags --uppercase-tag --require-colon

      - name: Check TODO thresholds
        run: todox check --max 100 --block-tags BUG,FIXME

      - name: Block new TODOs in PR
        if: github.event_name == 'pull_request'
        run: todox check --max-new 0 --since origin/${{ github.base_ref }}
```

### With SARIF upload and PR diff

```yaml
# .github/workflows/todox.yml
name: TODO Gate

on:
  pull_request:
  push:
    branches: [main]

jobs:
  todox:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install todox
        run: cargo install todox

      - name: Check TODO thresholds
        run: todox check --max 100 --block-tags BUG,FIXME

      - name: Upload SARIF to Code Scanning
        if: always()
        run: todox list --format sarif > todox.sarif

      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: todox.sarif

      - name: PR diff summary
        if: github.event_name == 'pull_request'
        run: |
          echo "## TODO Diff" >> "$GITHUB_STEP_SUMMARY"
          echo '```' >> "$GITHUB_STEP_SUMMARY"
          todox diff origin/${{ github.base_ref }} >> "$GITHUB_STEP_SUMMARY"
          echo '```' >> "$GITHUB_STEP_SUMMARY"

      - name: Generate HTML report
        if: always()
        run: todox report --output todox-report.html

      - name: Upload report artifact
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: todox-report
          path: todox-report.html
```

## 3. Claude Code Hooks

Add these hooks to `.claude/settings.json` to run todox automatically during Claude Code sessions.

### Lint on file write

Run `todox lint` whenever Claude edits or creates a file, catching malformed TODOs immediately:

```jsonc
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "file_path=$(echo '$TOOL_INPUT' | jq -r '.file_path') && dir=$(dirname \"$file_path\") && todox lint --root \"$dir\" --no-bare-tags --uppercase-tag --require-colon"
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
            "command": "todox diff HEAD~1 2>/dev/null || true"
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
            "command": "file_path=$(echo '$TOOL_INPUT' | jq -r '.file_path') && dir=$(dirname \"$file_path\") && todox lint --root \"$dir\" --no-bare-tags --uppercase-tag --require-colon"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "todox diff HEAD~1 2>/dev/null || true"
          }
        ]
      }
    ]
  }
}
```

## 4. Claude Code CLAUDE.md Snippet

Add this to your project's `CLAUDE.md` to instruct Claude Code to use todox during development:

````markdown
## TODO Hygiene

- Run `todox lint --no-bare-tags --uppercase-tag --require-colon` before committing
- Run `todox check --max 100 --block-tags BUG,FIXME` to verify thresholds
- Use `todox diff main` to review TODO changes before opening a PR
- Format TODO comments as: `TAG(author): message #issue-ref`
- Tags must be uppercase with a colon separator
- Do not leave bare tags (e.g., `// TODO` with no message)
````

## Reference

| Command | Purpose |
|---|---|
| `todox lint` | Check TODO formatting rules |
| `todox check` | Enforce count/tag thresholds |
| `todox diff <ref>` | Compare TODOs against a git ref |
| `todox list` | List all TODOs |
| `todox report` | Generate HTML dashboard |

See `todox <command> --help` for all available flags.
