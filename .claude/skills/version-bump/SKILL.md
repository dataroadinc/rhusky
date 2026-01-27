---
name: version-bump
description:
  Bump version in Cargo.toml using cargo-version-info bump command
---

# Version Bump Skill

Use this skill when bumping the version.

## Important: Use cargo version-info bump

**Always use `cargo version-info bump`** for version management.

**Never use `cog bump`** - it creates local tags which conflict with
the CI workflow that creates tags after tests pass.

## Bump Commands

```bash
# Patch bump: 0.0.1 -> 0.0.2
cargo version-info bump --patch

# Minor bump: 0.1.0 -> 0.2.0
cargo version-info bump --minor

# Major bump: 1.0.0 -> 2.0.0
cargo version-info bump --major
```

## What the Bump Command Does

1. Updates version in `Cargo.toml`
2. Updates version in `Cargo.lock`
3. Updates version badges in `README.md`
4. Creates a git commit with message:
   `chore(version): bump X.Y.Z -> A.B.C`

The bump command uses hunk-level selective staging, so it only commits
version-related changes. Any other uncommitted work remains unstaged.

## What the Bump Command Does NOT Do

- Does NOT create git tags (CI creates tags after merge)
- Does NOT push to remote (you must push manually)

## Workflow

1. Run `cargo version-info bump --patch` (or --minor/--major)
2. Push the branch or create a PR
3. Merge to main
4. CI detects version change, creates tag, publishes release

## Checking Current Version

```bash
# Get version from Cargo.toml
cargo version-info current

# Get computed build version (includes git SHA in dev)
cargo version-info build-version

# Check if version changed since last tag
cargo version-info changed
```

## Dry Run

To see what would change without making changes:

```bash
# Check current version
cargo version-info current

# Calculate what next patch would be
cargo version-info next
```

## After Bumping

After running bump, verify the commit includes all version-related files:

```bash
git log -1 --oneline
git diff HEAD~1 --stat
git status
```

**Important**: Check that all files modified by pre-bump hooks are
included in the commit. If `git status` shows uncommitted version
changes (from hooks), amend the commit:

```bash
git add <missing-files>
git commit --amend --no-edit
```

Then add those files to `additional_files` in Cargo.toml to prevent
this in future bumps:

```toml
[package.metadata.version-info]
additional_files = [
    "path/to/file"
]
```

Then push when ready:

```bash
git push origin <branch>
```
