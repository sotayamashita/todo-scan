# Issue-Driven Development Workflow

This document defines the development workflow for the todox project. All development follows an issue-driven approach using `gh` CLI for GitHub operations.

## Workflow Overview

```mermaid
sequenceDiagram
    participant Dev as Claude Code
    participant GH as GitHub (gh CLI)
    participant Repo as Local Repository
    participant CI as CI / Tests

    Note over Dev,CI: 1. Pick an Issue
    Dev->>GH: gh issue list
    GH-->>Dev: Open issues
    Dev->>GH: gh issue view <number>
    GH-->>Dev: Issue details

    Note over Dev,CI: 2. Create a Branch
    Dev->>Repo: git switch main && git pull --rebase
    Dev->>Repo: git switch -c <type>/<description>

    Note over Dev,CI: 3. Post Implementation Plan
    Dev->>Dev: Create plan (Plan mode)
    Dev->>GH: gh issue comment <number> --body "plan"
    GH-->>Dev: Plan posted to issue

    Note over Dev,CI: 4. Implement with TDD
    loop Red-Green-Refactor
        Dev->>Repo: Write failing test
        Dev->>CI: cargo test (Red)
        CI-->>Dev: FAIL
        Dev->>Repo: Implement code
        Dev->>CI: cargo test (Green)
        CI-->>Dev: PASS
        Dev->>Repo: Refactor
        Dev->>CI: cargo test (Refactor)
        CI-->>Dev: PASS
    end

    Note over Dev,CI: 5. Update README (if features changed)
    Dev->>Dev: Run feature-writing skill on Features section
    Dev->>Repo: Update README.md

    Note over Dev,CI: 6. Commit
    Dev->>Dev: /commit
    Dev->>Repo: Committed via commit skill

    Note over Dev,CI: 7. Create a Pull Request
    Dev->>Repo: git push -u origin HEAD
    Dev->>GH: gh pr create --body "Closes #N"
    GH-->>Dev: PR URL
    GH->>CI: Trigger CI checks
    CI-->>GH: CI status

    Note over Dev,CI: 8. Merge
    Dev->>GH: gh pr merge --rebase --delete-branch
    GH->>GH: Auto-close linked issue
    GH-->>Dev: Merged & issue closed
```

## Rules

- **No work without an issue** — every change must be linked to a GitHub issue
- **Use `gh` CLI** for all GitHub operations (issues, PRs, project board)
- **Follow TDD** — write failing tests first, then implement, then refactor
- **Post the plan to the issue** — share the implementation plan as a comment before coding
- **Link PRs to issues** with `Closes #<number>` for auto-closing
- **Update README** when adding or changing user-facing features — use the `feature-writing` skill for the Features section
- **Use `/commit`** for all commits — the commit skill handles staging, message formatting, and validation

## Workflow Steps

### 1. Pick an Issue

Browse open issues and select one to work on.

```bash
# List open issues
gh issue list

# View a specific issue
gh issue view <number>

# Check the project board
gh project item-list --owner <owner> --format json
```

- Read the issue description, acceptance criteria, and any linked discussions
- Check that the issue is not already assigned or in progress
- Assign yourself to the issue before starting work

### 2. Create a Branch

Create a feature branch from `main` with a descriptive name.

```bash
# Ensure you're on the latest main
git switch main
git pull --rebase

# Create a branch (one branch per issue)
git switch -c <type>/<description>
```

Branch naming conventions:
- `feature/<description>` — new functionality
- `fix/<description>` — bug fixes
- `refactor/<description>` — code restructuring
- `docs/<description>` — documentation changes
- `chore/<description>` — maintenance tasks

### 3. Post Implementation Plan

After creating a branch, use Plan mode to design the implementation approach. Then post the plan as a comment on the issue using `gh` CLI. The plan must be written in English.

```bash
# Post the implementation plan to the issue
gh issue comment <number> --body "$(cat <<'EOF'
## Implementation Plan

### Overview
Brief description of the approach.

### Changes
- **file_or_module** — what will be changed and why
- **file_or_module** — what will be changed and why

### Testing Strategy
- What tests will be added or updated
EOF
)"
```

- Use Plan mode to explore the codebase and design the approach before writing code
- Write the plan in English for consistency across the project
- Wait for feedback on the plan if working with a team before proceeding to implementation
- The plan serves as documentation for the rationale behind implementation decisions

### 4. Implement with TDD

Follow the Red-Green-Refactor cycle:

1. **Red** — Write a failing test that defines the expected behavior
2. **Green** — Write the minimum code to make the test pass
3. **Refactor** — Clean up the code while keeping tests green

```bash
# Run tests continuously during development
cargo test

# Run specific tests
cargo test <test_name>

# Check formatting
cargo fmt --check

# Type-check
cargo check
```

### 5. Update README (if features changed)

When a change adds or modifies user-facing features, update the Features section of `README.md` before committing. Use the `feature-writing` skill to draft the update.

- **When to update**: new commands, new flags, changed behavior, new integrations
- **When to skip**: internal optimizations, refactoring, test-only changes, docs-only changes
- Use the `feature-writing` skill (`/feature-writing`) to ensure consistent messaging

### 6. Commit

Use the `/commit` skill for all commits. It handles staging, conventional commit message formatting, and validation automatically.

```
/commit
```

The commit skill produces messages in conventional format with issue references:
- `feat(scanner): add support for custom tag patterns (#12)`
- `fix(diff): handle binary files gracefully (#8)`
- `test(check): add threshold validation tests (#15)`
- `docs(readme): update installation instructions (#3)`

### 7. Create a Pull Request

Push the branch and create a PR that links to the issue.

```bash
# Push the branch
git push -u origin HEAD

# Create a PR linking to the issue
gh pr create \
  --title "type(scope): description" \
  --body "$(cat <<'EOF'
## Summary

Brief description of changes.

Closes #<issue-number>

## Test Plan

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing completed
EOF
)"
```

- The `Closes #<number>` keyword in the PR body auto-closes the issue on merge
- Ensure the PR title follows conventional commit format
- Include a test plan in the PR description

### 8. Merge

After review and CI passes, merge the PR.

```bash
# Merge with rebase strategy and delete the branch
gh pr merge --rebase --delete-branch

# Verify the issue was auto-closed
gh issue view <number>
```

- Use `--rebase` to maintain a linear commit history
- Use `--delete-branch` to clean up the feature branch
- Verify the linked issue was automatically closed after merge
