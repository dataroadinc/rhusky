# Rhusky

[![Crates.io](https://img.shields.io/crates/v/rhusky.svg)](https://crates.io/crates/rhusky)
[![Documentation](https://docs.rs/rhusky/badge.svg)](https://docs.rs/rhusky)
[![CI](https://github.com/dataroadinc/rhusky/actions/workflows/ci.yml/badge.svg)](https://github.com/dataroadinc/rhusky/actions/workflows/ci.yml)
[![License: CC BY-SA 4.0](https://img.shields.io/badge/License-CC_BY--SA_4.0-lightgrey.svg)](https://creativecommons.org/licenses/by-sa/4.0/)

Git hooks manager for Rust projects. Like Husky, but for Rust.

Inspired by [Sloughi](https://github.com/01walid/sloughi) and the
Node.js [Husky](https://typicode.github.io/husky/) project.

## Why Rhusky?

- **Truly idempotent**: Never overwrites your existing hook scripts
- **No magic**: Just sets `core.hooksPath` in git config
- **CI-aware**: Skips installation in CI environments
- **Zero dependencies**: Pure Rust, no external crates

## Installation

Add rhusky to your build-dependencies:

```toml
[build-dependencies]
rhusky = "0.0.1"
```

Create a `build.rs` file at the root of your project:

```rust
fn main() {
    rhusky::Rhusky::new()
        .hooks_dir(".githooks")       // Custom hooks directory (default: ".githooks")
        .skip_in_env("GITHUB_ACTIONS") // Skip when this env var is set (CI always skipped)
        .with_default_hooks()          // Create default hooks if they don't exist
        .install()
        .ok();
}
```

Create your hooks directory and add your hooks:

```bash
mkdir -p .githooks
cat > .githooks/pre-commit << 'EOF'
#!/bin/sh
cargo fmt --check
cargo clippy -- -D warnings
EOF
chmod +x .githooks/pre-commit
```

## Configuration

### Custom hooks directory

```rust
rhusky::Rhusky::new()
    .hooks_dir(".git-hooks")  // default is ".githooks"
    .install()
    .ok();
```

### Skip in CI environments

By default, installation is skipped when the `CI` environment
variable is set. Add more:

```rust
rhusky::Rhusky::new()
    .skip_in_env("GITHUB_ACTIONS")
    .skip_in_env("GITLAB_CI")
    .install()
    .ok();
```

### Default hooks

Optionally create default hook scripts for Rust projects:

```rust
rhusky::Rhusky::new()
    .with_default_hooks()
    .install()
    .ok();
```

This creates three hooks (if they don't already exist):

- **pre-commit**: Runs `cargo +nightly fmt --check` and
  `cargo +nightly clippy` on staged Rust files
- **commit-msg**: Validates conventional commit format with
  mandatory scope (e.g., `feat(api): add endpoint`)
- **post-commit**: Verifies the commit is signed

Existing hooks are never overwritten.

## How it works

Rhusky sets Git's `core.hooksPath` configuration to point to your
hooks directory. This tells Git to look for hooks in that directory
instead of the default `.git/hooks/`.

Unlike other tools, Rhusky:

1. **Never overwrites existing hooks** - truly idempotent
2. **Optionally creates defaults** - use `with_default_hooks()` or
   manage your own
3. **Minimal by default** - just sets git config unless you opt in

## Recommended hooks

### pre-commit

```bash
#!/bin/sh
cargo +nightly fmt --check
cargo +nightly clippy --all-targets -- -D warnings
```

### commit-msg

```bash
#!/bin/sh
# Verify conventional commit format with Cocogitto
if command -v cog &> /dev/null; then
    cog verify --file "$1"
fi
```

## Comparison with similar tools

| Feature | Rhusky | [Sloughi] | [cargo-husky] | [husky-rs] |
| ------- | ------ | --------- | ------------- | ---------- |
| Sets `core.hooksPath` | Yes | Yes | No | No |
| Creates hook files | Optional | Yes | Yes | Yes |
| Overwrites existing hooks | Never | [Yes][sloughi-issue] | Yes | Yes |
| Zero dependencies | Yes | Yes | No | No |
| Customizable hooks dir | Yes | Yes | Limited | Limited |
| CI-aware | Yes | Yes | No | No |

[Sloughi]: https://crates.io/crates/sloughi
[cargo-husky]: https://crates.io/crates/cargo-husky
[husky-rs]: https://crates.io/crates/husky
[sloughi-issue]: https://github.com/01walid/sloughi/issues/2

### Sloughi

[Sloughi](https://github.com/01walid/sloughi) was the inspiration
for Rhusky. It uses the same `core.hooksPath` approach but creates
default hook files on every `cargo build`, overwriting any custom
hooks you've written. Rhusky fixes this by never creating or
modifying hook files.

### cargo-husky

[cargo-husky](https://github.com/rhysd/cargo-husky) copies hook
scripts into `.git/hooks/` rather than using `core.hooksPath`. This
means hooks aren't easily shareable via version control and can
conflict with other tools that manage `.git/hooks/`.

### husky-rs

[husky-rs](https://github.com/pksunkara/husky-rs) also copies hooks
into `.git/hooks/`. Like cargo-husky, it doesn't use the modern
`core.hooksPath` approach and overwrites existing hooks.

## License

[![CC BY-SA 4.0](https://licensebuttons.net/l/by-sa/4.0/88x31.png)](https://creativecommons.org/licenses/by-sa/4.0/)
