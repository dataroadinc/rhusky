# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when
working with code in this repository.

## Project overview

`rhusky` is a git hooks manager for Rust projects. It sets Git's
`core.hooksPath` to point to your project's hooks directory,
enabling shared git hooks across your team.

Key features:

- **Truly idempotent**: Never overwrites existing hook scripts
- **No magic**: Just sets `core.hooksPath` in git config
- **CI-aware**: Skips installation in CI environments
- **Zero dependencies**: Pure Rust, no external crates

Published to crates.io at https://crates.io/crates/rhusky

## Build and test commands

```bash
cargo build                    # Build
cargo test                     # Run tests
cargo install --path .         # Install locally

# Formatting (requires nightly)
cargo +nightly fmt --all -- --check   # Check
cargo +nightly fmt --all              # Apply

# Linting (requires nightly)
cargo +nightly clippy --all-targets --all-features -- -D warnings -W missing-docs
```

## Related projects

This crate is part of a family of Rust projects that share the same
coding standards, tooling, and workflows:

- `cargo-fmt-toml`, `cargo-nightly`, `cargo-plugin-utils`,
  `cargo-propagate-features`, `cargo-version-info` (cargo plugins)
- `dotenvage` (environment variable management)

All projects use identical configurations for rustfmt, clippy,
markdownlint, cocogitto, and git hooks.

## Code style

- **Rust Edition**: 2024, MSRV 1.93.0
- **Formatting**: Uses nightly rustfmt with vertical imports grouped
  by std/external/crate (see `rustfmt.toml`)
- **Clippy**: Nightly with strict settings (max 120 lines/function,
  nesting threshold 5)
- **Disallowed variable names**: foo, bar, baz, qux, i, n
- **Documentation**: All public items must have docs (`-W missing-docs`)

## Architecture

Single-file library (`src/lib.rs`) with a builder pattern:

- `Rhusky` struct - builder for configuring hooks installation
- `get_repo_root()` - finds git repository root via `git rev-parse`
- `set_hooks_path()` - sets `core.hooksPath` via `git config`

Key design decisions:

1. **Never overwrite hooks**: Only sets git config, never creates
   hook files (unlike Sloughi which creates default hooks)
2. **Zero dependencies**: Pure Rust std library only
3. **Builder pattern**: Fluent API with `hooks_dir()` and
   `skip_in_env()` methods

## Version management

Edit `Cargo.toml` manually to bump the version, then run
`cargo update --workspace` to update `Cargo.lock`.

**Workflow:**

1. Create PR with version bump commit
2. Merge PR to main
3. CI detects version change, creates tag
4. CI generates changelog from conventional commits (via Cocogitto)
5. CI creates GitHub Release with changelog as release body
6. CI publishes to crates.io

## Git workflow

- Commits follow Angular Conventional Commits:
  `<type>(<scope>): <subject>`
- Types: feat, fix, docs, refactor, test, style, perf, build, ci,
  chore, revert
- Use lowercase for type, scope, and subject start
- Never bypass git hooks with `--no-verify`
- Never execute `git push` - user must push manually
- Prefer `git rebase` over `git merge` for linear history

### Git hooks

Git hooks in `.githooks/` enforce:

- **pre-commit**: Runs fmt and clippy on Rust files
- **commit-msg**: Validates conventional commit format with mandatory
  scope using Cocogitto (`cog verify`)
- **post-commit**: Verifies commit is signed

Activate hooks: `git config core.hooksPath .githooks`

## Claude Code skills

Skills are defined in `.claude/skills/` and can be invoked with
`/skill-name`:

- `/commit` - Create commits following Angular Conventional Commits
  format with proper scope naming
- `/release-prep` - Prepare a release including version bump,
  testing, and PR creation
- `/version-bump` - Bump version in Cargo.toml
- `/testing` - Run tests, linting, and formatting checks

## Markdown formatting

- Maximum line length: 70 characters
- Use `-` for unordered lists (not `*` or `+`)
- Use sentence case for headers (not Title Case)
- Indent nested lists with 2 spaces
- Surround lists and code blocks with blank lines
