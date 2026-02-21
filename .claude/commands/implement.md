# Implement Issue

Implement GitHub issue #$ARGUMENTS following the development workflow defined in `docs/DEVELOPMENT_WORKFLOW.md`. Execute each step in order — do not skip steps.

## Prerequisites

Before starting, read these files:
- `docs/DEVELOPMENT_WORKFLOW.md` — the authoritative workflow definition
- `CLAUDE.md` — project conventions and build commands

## Step 1: Pick the Issue

```bash
gh issue view $ARGUMENTS
```

- Read the issue description, acceptance criteria, and any linked discussions
- Identify dependencies on other issues — if a dependency is still open, stop and inform the user
- Assign yourself to the issue:

```bash
gh issue edit $ARGUMENTS --add-assignee @me
```

## Step 2: Create a Branch

```bash
git switch main
git pull --rebase
```

Determine the branch type from the issue title/labels:
- `feature/` for new functionality (label: `enhancement`)
- `fix/` for bug fixes (label: `bug`)
- `refactor/` for restructuring
- `docs/` for documentation
- `chore/` for maintenance
- `perf/` for performance

Create the branch:

```bash
git switch -c <type>/<short-description>
```

## Step 3: Post Implementation Plan

Enter Plan mode to design the implementation approach. After the plan is approved:

- Post the plan as a comment on the issue:

```bash
gh issue comment $ARGUMENTS --body "$(cat <<'EOF'
## Implementation Plan

### Overview
<brief description>

### Changes
- **module** — what and why

### Testing Strategy
- What tests will be added
EOF
)"
```

## Step 4: Implement with TDD

Use the `/tdd-workflow` skill to drive the implementation:

```
/tdd-workflow
```

This skill enforces Kent Beck's 5-step cycle: Test List → Write Test → Make Pass → Refactor → Repeat.

Run tests with:

```bash
cargo test
cargo fmt --check
cargo check
```

## Step 5: Update README (if features changed)

If the change adds or modifies user-facing features (new commands, new flags, changed behavior):

- Use the `/feature-writing` skill to draft the README update
- Skip this step for internal optimizations, refactoring, test-only changes, or docs-only changes

## Step 6: Commit

Use the `/commit` skill:

```
/commit
```

The commit message must:
- Follow conventional commit format: `type(scope): description (#$ARGUMENTS)`
- Reference the issue number

## Step 7: Create a Pull Request

```bash
git push -u origin HEAD
```

Create the PR linking to the issue:

```bash
gh pr create \
  --title "type(scope): description" \
  --body "$(cat <<'EOF'
## Summary

<brief description of changes>

Closes #$ARGUMENTS

## Test Plan

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing completed
EOF
)"
```

## Step 8: Merge

After CI passes:

```bash
gh pr merge --rebase --delete-branch
```

Verify the issue was auto-closed:

```bash
gh issue view $ARGUMENTS
```
