//! # Rhusky
//!
//! Git hooks manager for Rust projects. Rhusky sets up Git's
//! `core.hooksPath` to point to your project's hooks directory,
//! enabling shared git hooks across your team.
//!
//! Unlike similar tools, Rhusky is truly idempotent - it will never
//! overwrite your existing hook scripts.
//!
//! ## Usage
//!
//! Add rhusky to your build-dependencies:
//!
//! ```toml
//! [build-dependencies]
//! rhusky = "0.0.6"
//! ```
//!
//! Opt the repository into build-script installation in
//! `.cargo/config.toml`:
//!
//! ```toml
//! [env]
//! RHUSKY_REPOSITORY_ROOT = { value = ".", relative = true, force = true }
//! ```
//!
//! Create a `build.rs` file:
//!
//! ```rust,ignore
//! fn main() {
//!     rhusky::Rhusky::new()
//!         .hooks_dir(".githooks")
//!         .install_from_build_script()
//!         .expect("failed to install repository Git hooks");
//! }
//! ```
//!
//! Create your hooks in `.githooks/`:
//!
//! ```bash
//! mkdir -p .githooks
//! echo '#!/bin/sh\ncargo fmt --check' > .githooks/pre-commit
//! chmod +x .githooks/pre-commit
//! ```
//!
//! ## Features
//!
//! - **Truly idempotent**: Never overwrites existing hooks
//! - **No magic**: Just sets `core.hooksPath` in git config
//! - **CI-aware**: Skips installation in CI environments
//! - **Zero dependencies**: Pure Rust, no external crates

use std::path::{
    Path,
    PathBuf,
};
use std::process::Command;
use std::{
    env,
    fs,
    io,
};

/// Environment variable that explicitly opts a repository into build-script
/// hook installation.
pub const REPOSITORY_ROOT_ENV: &str = "RHUSKY_REPOSITORY_ROOT";

/// Builder for configuring and installing git hooks.
#[derive(Debug, Clone)]
pub struct Rhusky {
    hooks_dir: String,
    skip_env_vars: Vec<String>,
    create_default_hooks: bool,
}

/// Default pre-commit hook script for Rust projects.
///
/// Checks for nightly toolchain, then runs `cargo +nightly fmt --check`
/// and `cargo +nightly clippy` on staged Rust files.
const DEFAULT_PRE_COMMIT_HOOK: &str = r#"#!/bin/sh
# Check for nightly toolchain
if ! rustup run nightly rustc --version > /dev/null 2>&1; then
    echo "Error: Rust nightly toolchain is required but not installed."
    echo "Install with: rustup install nightly"
    exit 1
fi

# Check for Rust files in commit
RUST_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.rs$' || true)
if [ -z "$RUST_FILES" ]; then
    exit 0
fi

echo "Running cargo +nightly fmt --check..."
if ! cargo +nightly fmt -- --check; then
    echo "Format check failed. Run: cargo +nightly fmt"
    exit 1
fi

echo "Running cargo +nightly clippy..."
if ! cargo +nightly clippy --all-targets -- -D warnings; then
    echo "Clippy check failed."
    exit 1
fi
"#;

/// Default commit-msg hook script.
///
/// Validates conventional commit format with mandatory scope.
/// Format: `type(scope): description`
const DEFAULT_COMMIT_MSG_HOOK: &str = r#"#!/bin/sh
# Validate conventional commit format with mandatory scope
# Format: type(scope): description
if ! grep -qE '^[a-z]+\([a-z0-9./-]+\)!?: .+' "$1"; then
    echo "Error: Invalid commit message format."
    echo "Required: type(scope): description"
    echo "Example: feat(api): add user endpoint"
    exit 1
fi
"#;

/// Default post-commit hook script.
///
/// Verifies that commits are signed with GPG or SSH.
const DEFAULT_POST_COMMIT_HOOK: &str = r#"#!/bin/sh
COMMIT_HASH=$(git rev-parse HEAD 2>/dev/null)
if [ -z "$COMMIT_HASH" ]; then
    echo "Error: Failed to get commit hash."
    exit 1
fi

SIG_STATUS=$(git log -1 --format='%G?' "$COMMIT_HASH" 2>/dev/null || echo "N")

case "$SIG_STATUS" in
    G) echo "Commit is signed." ;;
    N)
        echo "Error: Commit $COMMIT_HASH is not signed!"
        echo "Configure signing: git config commit.gpgsign true"
        exit 1
        ;;
    *)
        echo "Error: Invalid signature (status: $SIG_STATUS)"
        exit 1
        ;;
esac
"#;

impl Default for Rhusky {
    fn default() -> Self {
        Self::new()
    }
}

impl Rhusky {
    /// Create a new Rhusky instance with default configuration.
    ///
    /// Defaults:
    /// - hooks_dir: `.githooks`
    /// - Skips installation when `CI` env var is set
    /// - Does not create default hooks
    #[must_use]
    pub fn new() -> Self {
        Self {
            hooks_dir: ".githooks".to_string(),
            skip_env_vars: vec!["CI".to_string()],
            create_default_hooks: false,
        }
    }

    /// Set a custom directory for git hooks.
    ///
    /// Default is `.githooks`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rhusky::Rhusky;
    ///
    /// let _rhusky = Rhusky::new().hooks_dir(".git-hooks");
    /// ```
    #[must_use]
    pub fn hooks_dir(mut self, path: &str) -> Self {
        self.hooks_dir = path.to_string();
        self
    }

    /// Skip installation when the specified environment variable is set.
    ///
    /// By default, installation is skipped when `CI` is set.
    /// Call this method to add additional env vars to skip on.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rhusky::Rhusky;
    ///
    /// let _rhusky = Rhusky::new()
    ///     .skip_in_env("GITHUB_ACTIONS")
    ///     .skip_in_env("GITLAB_CI");
    /// ```
    #[must_use]
    pub fn skip_in_env(mut self, var: &str) -> Self {
        self.skip_env_vars.push(var.to_string());
        self
    }

    /// Create default hook scripts if they don't exist.
    ///
    /// Creates pre-commit, commit-msg, and post-commit hooks with
    /// sensible defaults for Rust projects:
    ///
    /// - **pre-commit**: Runs `cargo +nightly fmt --check` and `cargo +nightly
    ///   clippy` on staged Rust files
    /// - **commit-msg**: Validates conventional commit format with mandatory
    ///   scope (e.g., `feat(api): add endpoint`)
    /// - **post-commit**: Verifies the commit is signed
    ///
    /// Existing hooks are never overwritten.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rhusky::Rhusky;
    ///
    /// let _rhusky = Rhusky::new().with_default_hooks();
    /// ```
    #[must_use]
    pub fn with_default_hooks(mut self) -> Self {
        self.create_default_hooks = true;
        self
    }

    /// Install git hooks by setting `core.hooksPath`.
    ///
    /// This method:
    /// 1. Checks if we're in a git repository
    /// 2. Creates the hooks directory if it doesn't exist
    /// 3. Sets `core.hooksPath` to point to the hooks directory
    ///
    /// **Important**: This method never overwrites existing hook files.
    /// You are responsible for creating and maintaining your hook scripts.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Not in a git repository
    /// - Cannot create the hooks directory
    /// - Cannot set git config
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn main() {
    ///     rhusky::Rhusky::new()
    ///         .install()
    ///         .expect("failed to install Git hooks");
    /// }
    /// ```
    pub fn install(&self) -> io::Result<()> {
        if self.should_skip_for_environment() {
            return Ok(());
        }

        let current_dir = env::current_dir()?;
        self.install_from(&current_dir)
    }

    /// Install Git hooks when invoked from an owning package's build script.
    ///
    /// Installation only runs when [`REPOSITORY_ROOT_ENV`] is set. Configure
    /// that variable in the owning repository's `.cargo/config.toml` with a
    /// config-relative path. Published dependencies do not load their source
    /// repository's Cargo configuration, so they cannot alter a downstream
    /// checkout.
    ///
    /// An opted-in repository returns any installation error to the caller.
    /// Build scripts should treat that error as fatal so a broken repository
    /// hook configuration cannot pass silently.
    ///
    /// # Errors
    ///
    /// Returns an error when the opted-in root is empty, is outside a Git
    /// repository, or its hook configuration cannot be installed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn main() {
    ///     rhusky::Rhusky::new()
    ///         .install_from_build_script()
    ///         .expect("failed to install repository Git hooks");
    /// }
    /// ```
    pub fn install_from_build_script(&self) -> io::Result<()> {
        if self.should_skip_for_environment() {
            return Ok(());
        }

        let Some(repository_root) = env::var_os(REPOSITORY_ROOT_ENV) else {
            return Ok(());
        };
        if repository_root.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{REPOSITORY_ROOT_ENV} is empty"),
            ));
        }

        self.install_from(Path::new(&repository_root))
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "failed to install hooks for {}={}: {error}",
                        REPOSITORY_ROOT_ENV,
                        Path::new(&repository_root).display()
                    ),
                )
            })
    }

    fn should_skip_for_environment(&self) -> bool {
        self.skip_env_vars
            .iter()
            .any(|var| env::var_os(var).is_some())
    }

    fn install_from(&self, start_dir: &Path) -> io::Result<()> {
        let repo_root = get_repo_root(start_dir)?;

        // Create hooks directory if it doesn't exist
        let hooks_path = repo_root.join(&self.hooks_dir);
        if !hooks_path.exists() {
            fs::create_dir_all(&hooks_path)?;
        }

        // Create default hooks if requested
        if self.create_default_hooks {
            create_hook_if_missing(&hooks_path, "pre-commit", DEFAULT_PRE_COMMIT_HOOK)?;
            create_hook_if_missing(&hooks_path, "commit-msg", DEFAULT_COMMIT_MSG_HOOK)?;
            create_hook_if_missing(&hooks_path, "post-commit", DEFAULT_POST_COMMIT_HOOK)?;
        }

        set_hooks_path(&repo_root, &self.hooks_dir)
    }
}

/// Get the root directory of the git repository.
fn get_repo_root(start_dir: &Path) -> io::Result<PathBuf> {
    let output = Command::new("git")
        .current_dir(start_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Not in a git repository",
        ));
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(path))
}

/// Set git's core.hooksPath configuration.
fn set_hooks_path(repo_root: &Path, hooks_dir: &str) -> io::Result<()> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["config", "core.hooksPath", hooks_dir])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Failed to set core.hooksPath: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

/// Create a hook file if it doesn't already exist.
///
/// On Unix systems, the file is made executable (mode 0o755).
fn create_hook_if_missing(hooks_dir: &Path, name: &str, content: &str) -> io::Result<()> {
    let hook_path = hooks_dir.join(name);

    // Never overwrite existing hooks
    if hook_path.exists() {
        return Ok(());
    }

    fs::write(&hook_path, content)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Builder Tests ====================

    #[test]
    fn test_builder_defaults() {
        let rhusky = Rhusky::new();
        assert_eq!(rhusky.hooks_dir, ".githooks");
        assert!(rhusky.skip_env_vars.contains(&"CI".to_string()));
        assert!(!rhusky.create_default_hooks);
    }

    #[test]
    fn test_builder_custom_hooks_dir() {
        let rhusky = Rhusky::new().hooks_dir(".git-hooks");
        assert_eq!(rhusky.hooks_dir, ".git-hooks");
    }

    #[test]
    fn test_builder_skip_in_env() {
        let rhusky = Rhusky::new()
            .skip_in_env("GITHUB_ACTIONS")
            .skip_in_env("GITLAB_CI");
        assert!(rhusky.skip_env_vars.contains(&"CI".to_string()));
        assert!(rhusky.skip_env_vars.contains(&"GITHUB_ACTIONS".to_string()));
        assert!(rhusky.skip_env_vars.contains(&"GITLAB_CI".to_string()));
    }

    #[test]
    fn test_builder_with_default_hooks() {
        let rhusky = Rhusky::new().with_default_hooks();
        assert!(rhusky.create_default_hooks);
    }

    #[test]
    fn test_builder_chaining() {
        let rhusky = Rhusky::new()
            .hooks_dir(".hooks")
            .skip_in_env("BUILD_ENV")
            .skip_in_env("TEST_ENV");
        assert_eq!(rhusky.hooks_dir, ".hooks");
        assert_eq!(rhusky.skip_env_vars.len(), 3); // CI + 2 custom
    }

    #[test]
    fn test_default_trait() {
        let default_rhusky = Rhusky::default();
        let new_rhusky = Rhusky::new();
        assert_eq!(default_rhusky.hooks_dir, new_rhusky.hooks_dir);
        assert_eq!(default_rhusky.skip_env_vars, new_rhusky.skip_env_vars);
        assert_eq!(
            default_rhusky.create_default_hooks,
            new_rhusky.create_default_hooks
        );
    }

    #[test]
    fn test_clone_trait() {
        let original = Rhusky::new().hooks_dir(".custom").with_default_hooks();
        let cloned = original.clone();
        assert_eq!(original.hooks_dir, cloned.hooks_dir);
        assert_eq!(original.skip_env_vars, cloned.skip_env_vars);
        assert_eq!(original.create_default_hooks, cloned.create_default_hooks);
    }

    #[test]
    fn test_debug_trait() {
        let rhusky = Rhusky::new();
        let debug_str = format!("{:?}", rhusky);
        assert!(debug_str.contains("Rhusky"));
        assert!(debug_str.contains(".githooks"));
    }
}

#[cfg(test)]
mod integration_tests {
    use std::process::Command;

    use serial_test::serial;
    use tempfile::TempDir;

    use super::*;

    /// Helper to create a temporary git repository.
    fn create_temp_git_repo() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to init git repo");
        // Configure git user for the temp repo (required for some git operations)
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to configure git email");
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to configure git name");
        temp_dir
    }

    /// Helper to get core.hooksPath from a git repo.
    fn get_hooks_path(repo_path: &Path) -> Option<String> {
        let output = Command::new("git")
            .args(["config", "--get", "core.hooksPath"])
            .current_dir(repo_path)
            .output()
            .ok()?;
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    /// Helper to temporarily clear the CI env var for testing.
    /// Returns the original value to restore later.
    fn clear_ci_env() -> Option<String> {
        let ci_value = env::var("CI").ok();
        // SAFETY: We're in single-threaded tests and restoring immediately after
        unsafe { env::remove_var("CI") };
        ci_value
    }

    /// Helper to restore the CI env var after testing.
    fn restore_ci_env(ci_value: Option<String>) {
        // SAFETY: We're in single-threaded tests
        unsafe {
            if let Some(val) = ci_value {
                env::set_var("CI", val);
            }
        }
    }

    /// Restore an environment variable captured before a serial test.
    unsafe fn restore_env_var(name: &str, value: Option<String>) {
        match value {
            Some(original) => {
                // SAFETY: The caller serializes environment mutation.
                unsafe { env::set_var(name, original) };
            }
            None => {
                // SAFETY: The caller serializes environment mutation.
                unsafe { env::remove_var(name) };
            }
        }
    }

    // ==================== Install Tests ====================

    #[test]
    #[serial]
    fn test_install_creates_hooks_directory() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        // Ensure hooks dir doesn't exist
        assert!(!hooks_dir.exists());

        // Change to temp repo and install
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(hooks_dir.exists());
    }

    #[test]
    #[serial]
    fn test_install_sets_hooks_path_config() {
        let temp_repo = create_temp_git_repo();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());

        let hooks_path = get_hooks_path(temp_repo.path());
        assert_eq!(hooks_path, Some(".githooks".to_string()));
    }

    #[test]
    #[serial]
    fn test_install_with_custom_hooks_dir() {
        let temp_repo = create_temp_git_repo();
        let custom_dir = ".my-hooks";
        let hooks_dir = temp_repo.path().join(custom_dir);

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().hooks_dir(custom_dir).install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(hooks_dir.exists());

        let hooks_path = get_hooks_path(temp_repo.path());
        assert_eq!(hooks_path, Some(custom_dir.to_string()));
    }

    #[test]
    #[serial]
    fn test_install_preserves_existing_hooks_directory() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        // Create hooks directory with a file
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_file = hooks_dir.join("pre-commit");
        fs::write(&hook_file, "#!/bin/sh\necho 'test'").unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());

        // Verify existing hook file is preserved
        assert!(hook_file.exists());
        let content = fs::read_to_string(&hook_file).unwrap();
        assert!(content.contains("echo 'test'"));
    }

    #[test]
    #[serial]
    fn test_install_is_idempotent() {
        let temp_repo = create_temp_git_repo();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();

        // Install multiple times
        let result1 = Rhusky::new().install();
        let result2 = Rhusky::new().install();
        let result3 = Rhusky::new().install();

        restore_ci_env(ci_value);
        env::set_current_dir(original_dir).unwrap();

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());

        let hooks_path = get_hooks_path(temp_repo.path());
        assert_eq!(hooks_path, Some(".githooks".to_string()));
    }

    // ==================== Skip Behavior Tests ====================

    #[test]
    #[serial]
    fn test_install_skips_when_ci_env_set() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        // Set CI env var
        let ci_value = env::var("CI").ok();
        // SAFETY: Single-threaded test environment
        unsafe { env::set_var("CI", "true") };

        let result = Rhusky::new().install();

        // Restore CI env var
        // SAFETY: Single-threaded test environment
        unsafe {
            match ci_value {
                Some(val) => env::set_var("CI", val),
                None => env::remove_var("CI"),
            }
        }
        env::set_current_dir(original_dir).unwrap();

        // Install should succeed (by skipping)
        assert!(result.is_ok());

        // But hooks directory should NOT be created
        assert!(!hooks_dir.exists());
    }

    #[test]
    #[serial]
    fn test_install_skips_when_custom_env_set() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        // Clear CI but set custom env var
        let ci_value = clear_ci_env();
        // SAFETY: Single-threaded test environment
        unsafe { env::set_var("RHUSKY_TEST_SKIP", "1") };

        let result = Rhusky::new().skip_in_env("RHUSKY_TEST_SKIP").install();

        // Cleanup
        // SAFETY: Single-threaded test environment
        unsafe { env::remove_var("RHUSKY_TEST_SKIP") };
        restore_ci_env(ci_value);
        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(!hooks_dir.exists());
    }

    #[test]
    #[serial]
    fn test_install_runs_when_skip_env_not_set() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        // Clear all skip env vars
        let ci_value = clear_ci_env();
        // SAFETY: Single-threaded test environment
        unsafe { env::remove_var("RHUSKY_NONEXISTENT_VAR") };

        let result = Rhusky::new()
            .skip_in_env("RHUSKY_NONEXISTENT_VAR")
            .install();

        restore_ci_env(ci_value);
        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(hooks_dir.exists());
    }

    #[test]
    #[serial]
    fn test_build_script_install_skips_without_repository_opt_in() {
        let temp_repo = create_temp_git_repo();
        let original_dir = env::current_dir().unwrap();
        let ci_value = clear_ci_env();
        let repository_root_value = env::var("RHUSKY_REPOSITORY_ROOT").ok();

        env::set_current_dir(temp_repo.path()).unwrap();
        // SAFETY: This serial test restores the process environment below.
        unsafe { env::remove_var("RHUSKY_REPOSITORY_ROOT") };

        let result = Rhusky::new().install_from_build_script();

        // SAFETY: This serial test restores the process environment.
        unsafe {
            restore_env_var("RHUSKY_REPOSITORY_ROOT", repository_root_value);
        }
        restore_ci_env(ci_value);
        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(!temp_repo.path().join(".githooks").exists());
        assert_eq!(get_hooks_path(temp_repo.path()), None);
    }

    #[test]
    #[serial]
    fn test_build_script_install_uses_opted_in_repository_root() {
        let temp_repo = create_temp_git_repo();
        let unrelated_dir = TempDir::new().expect("Failed to create temp directory");
        let original_dir = env::current_dir().unwrap();
        let ci_value = clear_ci_env();
        let repository_root_value = env::var("RHUSKY_REPOSITORY_ROOT").ok();

        env::set_current_dir(unrelated_dir.path()).unwrap();
        // SAFETY: This serial test restores the process environment below.
        unsafe { env::set_var("RHUSKY_REPOSITORY_ROOT", temp_repo.path()) };

        let result = Rhusky::new().install_from_build_script();

        // SAFETY: This serial test restores the process environment.
        unsafe {
            restore_env_var("RHUSKY_REPOSITORY_ROOT", repository_root_value);
        }
        restore_ci_env(ci_value);
        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(temp_repo.path().join(".githooks").exists());
        assert_eq!(
            get_hooks_path(temp_repo.path()),
            Some(".githooks".to_string())
        );
    }

    #[test]
    #[serial]
    fn test_build_script_install_fails_for_opted_in_root_outside_git() {
        let repository_root = TempDir::new().expect("Failed to create temp directory");
        let original_dir = env::current_dir().unwrap();
        let ci_value = clear_ci_env();
        let repository_root_value = env::var("RHUSKY_REPOSITORY_ROOT").ok();

        env::set_current_dir(repository_root.path()).unwrap();
        // SAFETY: This serial test restores the process environment below.
        unsafe { env::set_var("RHUSKY_REPOSITORY_ROOT", repository_root.path()) };

        let result = Rhusky::new().install_from_build_script();

        // SAFETY: This serial test restores the process environment.
        unsafe {
            restore_env_var("RHUSKY_REPOSITORY_ROOT", repository_root_value);
        }
        restore_ci_env(ci_value);
        env::set_current_dir(original_dir).unwrap();

        let error = result.expect_err("opted-in checkout installation must fail");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    // ==================== Error Handling Tests ====================

    #[test]
    #[serial]
    fn test_install_fails_outside_git_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        // Note: NOT initializing git repo

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    #[serial]
    fn test_install_creates_nested_hooks_directory() {
        let temp_repo = create_temp_git_repo();
        let nested_dir = "scripts/git/hooks";
        let hooks_dir = temp_repo.path().join(nested_dir);

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().hooks_dir(nested_dir).install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(hooks_dir.exists());

        let hooks_path = get_hooks_path(temp_repo.path());
        assert_eq!(hooks_path, Some(nested_dir.to_string()));
    }

    // ==================== Default Hooks Tests ====================

    #[test]
    #[serial]
    fn test_install_without_default_hooks_creates_no_hooks() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(hooks_dir.exists());

        // No hook files should be created
        assert!(!hooks_dir.join("pre-commit").exists());
        assert!(!hooks_dir.join("commit-msg").exists());
        assert!(!hooks_dir.join("post-commit").exists());
    }

    #[test]
    #[serial]
    fn test_install_with_default_hooks_creates_hooks() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().with_default_hooks().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(hooks_dir.exists());

        // All default hook files should be created
        assert!(hooks_dir.join("pre-commit").exists());
        assert!(hooks_dir.join("commit-msg").exists());
        assert!(hooks_dir.join("post-commit").exists());

        // Verify hook contents start with shebang
        let pre_commit = fs::read_to_string(hooks_dir.join("pre-commit")).unwrap();
        assert!(pre_commit.starts_with("#!/bin/sh"));

        let commit_msg = fs::read_to_string(hooks_dir.join("commit-msg")).unwrap();
        assert!(commit_msg.starts_with("#!/bin/sh"));

        let post_commit = fs::read_to_string(hooks_dir.join("post-commit")).unwrap();
        assert!(post_commit.starts_with("#!/bin/sh"));
    }

    #[test]
    #[serial]
    fn test_install_with_default_hooks_does_not_overwrite_existing() {
        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        // Create hooks directory with an existing hook
        fs::create_dir_all(&hooks_dir).unwrap();
        let existing_content = "#!/bin/sh\necho 'custom hook'";
        fs::write(hooks_dir.join("pre-commit"), existing_content).unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().with_default_hooks().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());

        // Existing hook should NOT be overwritten
        let pre_commit = fs::read_to_string(hooks_dir.join("pre-commit")).unwrap();
        assert_eq!(pre_commit, existing_content);

        // Other hooks should be created
        assert!(hooks_dir.join("commit-msg").exists());
        assert!(hooks_dir.join("post-commit").exists());
    }

    #[cfg(unix)]
    #[test]
    #[serial]
    fn test_install_with_default_hooks_are_executable() {
        use std::os::unix::fs::PermissionsExt;

        let temp_repo = create_temp_git_repo();
        let hooks_dir = temp_repo.path().join(".githooks");

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_repo.path()).unwrap();

        let ci_value = clear_ci_env();
        let result = Rhusky::new().with_default_hooks().install();
        restore_ci_env(ci_value);

        env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());

        // Check that hooks are executable
        for hook_name in ["pre-commit", "commit-msg", "post-commit"] {
            let hook_path = hooks_dir.join(hook_name);
            let perms = fs::metadata(&hook_path).unwrap().permissions();
            let mode = perms.mode();
            // Check executable bit for owner (0o100)
            assert!(
                mode & 0o100 != 0,
                "{hook_name} should be executable, mode: {mode:o}"
            );
        }
    }
}
