---
name: testing
description:
  Run tests, linting, and formatting checks for rhusky
---

# Testing Skill

Use this skill when running tests, checking code quality, or verifying
the codebase compiles and passes all checks.

## Running Tests

```bash
# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

## Formatting

Formatting requires the nightly toolchain:

```bash
# Check formatting (CI mode)
cargo +nightly fmt --all -- --check

# Apply formatting fixes
cargo +nightly fmt --all
```

## Linting with Clippy

Clippy also requires nightly with strict settings:

```bash
cargo +nightly clippy --all-targets --all-features -- -D warnings -W missing-docs
```

This enforces:

- All warnings as errors (`-D warnings`)
- Documentation on all public items (`-W missing-docs`)
- Max 120 lines per function
- Max nesting depth of 5

## Full Check Workflow

Run all checks in sequence:

```bash
cargo +nightly fmt --all -- --check && \
cargo +nightly clippy --all-targets --all-features -- -D warnings -W missing-docs && \
cargo test
```

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Check only (faster, no codegen)
cargo check
```

## Git Hooks (Automatic Checks)

Git hooks in `.githooks/` run automatically on commit:

- **pre-commit**: Runs fmt and clippy on Rust files
- **commit-msg**: Validates conventional commit format with scope
- **post-commit**: Verifies commit signature

Activate hooks: `git config core.hooksPath .githooks`

## Common Issues

### Nightly toolchain not installed

```bash
rustup install nightly
```

### Missing docs warning

All public items need documentation. Add doc comments:

```rust
/// Brief description of the function.
pub fn my_function() {}
```
