# Pick Next Issue

Analyze open GitHub issues and recommend the best issue to work on next.

## Steps

1. **Fetch open issues with full details:**

```bash
gh issue list --state open --json number,title,labels,body --limit 50
```

2. **Check current branch and working tree state:**

```bash
git status
git branch --show-current
```

3. **Analyze each issue for:**
   - **Dependencies**: Read issue body for references to other issues (`#N`, `depends on`, `requires`, `after`). An issue is blocked if its dependency is still open.
   - **Complexity**: Estimate from issue body (new subcommand = high, flag addition = medium, docs/chore = low)
   - **Priority signals**: Labels, whether other issues depend on it (blocker for others = higher priority), standalone vs dependency chain
   - **Current state**: Whether a branch already exists for it (`git branch --list '*issue-keyword*'`)

4. **Categorize issues into tiers:**
   - **Tier 1 — Ready now**: No open dependencies, standalone, low-to-medium complexity
   - **Tier 2 — Ready but complex**: No open dependencies, high complexity
   - **Tier 3 — Blocked**: Has open dependencies that must be completed first

5. **Present a recommendation table** sorted by tier, then by priority:

```
| Tier | Issue | Title | Complexity | Blocked By |
|------|-------|-------|------------|------------|
```

6. **Recommend the top 3 issues** with a brief rationale for each, explaining why it should be picked next.

7. **Ask the user** which issue they want to work on. Once they choose, tell them to run `/implement <issue-number>` to start.
