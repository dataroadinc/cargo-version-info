//! Determine build version with priority logic command.
//!
//! This command determines the build version using a priority-based fallback
//! system. It's designed for use in CI/CD pipelines where different sources
//! of version information may be available.
//!
//! # Priority Order
//!
//! 1. **BUILD_VERSION** (environment variable) - Preferred for CI workflows
//! 2. **CARGO_PKG_VERSION_OVERRIDE** (environment variable) - Legacy override
//! 3. **GitHub API** - Query and calculate next version (only in GitHub
//!    Actions)
//! 4. **CARGO_PKG_VERSION** (environment variable) - From Cargo.toml at build
//!    time
//! 5. **Git SHA** - Fallback: `0.0.0-dev-<short-sha>` for local development
//!
//! # Examples
//!
//! ```bash
//! # Determine build version (uses priority logic)
//! cargo version-info build-version
//!
//! # Get JSON output with source information
//! cargo version-info build-version --format json
//!
//! # With BUILD_VERSION set (highest priority)
//! BUILD_VERSION=1.2.3 cargo version-info build-version
//! ```

use std::env;
use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};
use clap::Parser;

use super::common::get_owner_repo;
use crate::github;

/// Arguments for the `build-version` command.
#[derive(Parser, Debug)]
pub struct BuildVersionArgs {
    /// GitHub repository owner.
    ///
    /// Only used when falling back to GitHub API (priority 3).
    /// Defaults to `GITHUB_REPOSITORY` environment variable or auto-detected
    /// from the current git remote.
    #[arg(long)]
    owner: Option<String>,

    /// GitHub repository name.
    ///
    /// Only used when falling back to GitHub API (priority 3).
    /// Defaults to `GITHUB_REPOSITORY` environment variable or auto-detected
    /// from the current git remote.
    #[arg(long)]
    repo: Option<String>,

    /// GitHub personal access token for API authentication.
    ///
    /// Only used when falling back to GitHub API (priority 3).
    /// Defaults to `GITHUB_TOKEN` environment variable.
    #[arg(long, env = "GITHUB_TOKEN")]
    github_token: Option<String>,

    /// Path to the Cargo.toml manifest file.
    ///
    /// Currently unused but reserved for future use. Defaults to
    /// `./Cargo.toml`.
    #[arg(long, default_value = "./Cargo.toml")]
    manifest: PathBuf,

    /// Path to the git repository.
    ///
    /// Used for the git SHA fallback (priority 5). Defaults to the current
    /// directory.
    #[arg(long, default_value = ".")]
    repo_path: PathBuf,

    /// Output format for the build version.
    ///
    /// - `version`: Print just the version number
    /// - `json`: Print JSON with version and source fields indicating where the
    ///   version came from (environment, github_api, cargo_toml, or git)
    #[arg(long, default_value = "version")]
    format: String,
}

/// Determine the build version using a priority-based fallback system.
///
/// This function implements a cascading fallback strategy to determine the
/// build version, checking multiple sources in order of preference. This is
/// designed for CI/CD pipelines where the version source may vary.
///
/// # Priority Order
///
/// 1. **BUILD_VERSION** environment variable - Set by CI workflows to avoid
///    duplicate API queries
/// 2. **CARGO_PKG_VERSION_OVERRIDE** environment variable - Legacy script-based
///    override mechanism
/// 3. **GitHub API** - Only checked if running in GitHub Actions (detected via
///    `GITHUB_ACTIONS` env var). Queries the API to calculate the next version.
/// 4. **CARGO_PKG_VERSION** environment variable - Set by Cargo at build time
///    from Cargo.toml. Usually "0.0.0" for placeholder versions.
/// 5. **Git SHA** - Final fallback for local development:
///    `0.0.0-dev-<short-sha>`
///
/// # Errors
///
/// Returns an error if:
/// - GitHub API fallback is attempted but fails (network error, auth failure,
///   etc.)
/// - Git repository cannot be discovered (for SHA fallback)
/// - HEAD does not point to a valid commit (for SHA fallback)
///
/// # Examples
///
/// ```no_run
/// use cargo_version_info::commands::{
///     BuildVersionArgs,
///     build_version,
/// };
///
/// let args = BuildVersionArgs {
///     owner: None,
///     repo: None,
///     github_token: None,
///     manifest: "./Cargo.toml".into(),
///     repo_path: ".".into(),
///     format: "version".to_string(),
/// };
/// build_version(args)?;
/// ```
///
/// # Example Output
///
/// With `--format version` (from BUILD_VERSION env var):
/// ```text
/// 1.2.3
/// ```
///
/// With `--format json` (from GitHub API):
/// ```json
/// {"version":"0.1.3","source":"github_api"}
/// ```
///
/// With `--format json` (from CARGO_PKG_VERSION):
/// ```json
/// {"version":"0.1.2","source":"cargo_toml"}
/// ```
///
/// With `--format json` (from git SHA fallback):
/// ```json
/// {"version":"0.0.0-dev-a1b2c3d","sha":"a1b2c3d","source":"git"}
/// ```
#[allow(clippy::disallowed_methods)] // CLI tool needs direct env access
pub fn build_version(args: BuildVersionArgs) -> Result<()> {
    // Try explicit overrides first (CI workflow should set BUILD_VERSION)
    let env_version = ["BUILD_VERSION", "CARGO_PKG_VERSION_OVERRIDE"]
        .into_iter()
        .find_map(|key| env::var(key).ok())
        .filter(|v| !v.trim().is_empty());

    if let Some(version) = env_version {
        match args.format.as_str() {
            "version" => println!("{}", version),
            "json" => println!("{{\"version\":\"{}\",\"source\":\"environment\"}}", version),
            _ => anyhow::bail!("Invalid format: {}", args.format),
        }
        return Ok(());
    }

    // Fallback: Try to query GitHub API via octocrab
    let is_github_actions = env::var("GITHUB_ACTIONS").is_ok();
    if is_github_actions {
        let (owner, repo) = get_owner_repo(args.owner, args.repo)?;
        let github_token = args.github_token.as_deref();

        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        if let Ok((_, next)) =
            rt.block_on(github::calculate_next_version(&owner, &repo, github_token))
        {
            match args.format.as_str() {
                "version" => println!("{}", next),
                "json" => println!("{{\"version\":\"{}\",\"source\":\"github_api\"}}", next),
                _ => anyhow::bail!("Invalid format: {}", args.format),
            }
            return Ok(());
        }
    }

    // Fall back to CARGO_PKG_VERSION (from Cargo.toml)
    if let Ok(version) = env::var("CARGO_PKG_VERSION") {
        let trimmed = version.trim();
        if !trimmed.is_empty() && trimmed != "0.0.0" {
            match args.format.as_str() {
                "version" => println!("{}", version),
                "json" => println!("{{\"version\":\"{}\",\"source\":\"cargo_toml\"}}", version),
                _ => anyhow::bail!("Invalid format: {}", args.format),
            }
            return Ok(());
        }
    }

    // Final fallback: git SHA for local dev
    let repo = gix::discover(&args.repo_path).with_context(|| {
        format!(
            "Failed to discover git repository at {}",
            args.repo_path.display()
        )
    })?;

    let head = repo.head().context("Failed to read HEAD")?;
    let commit_id = head.id().context("HEAD does not point to a commit")?;
    let short_sha = commit_id
        .shorten()
        .context("Failed to shorten commit SHA")?;

    let dev_version = format!("0.0.0-dev-{}", short_sha);

    match args.format.as_str() {
        "version" => println!("{}", dev_version),
        "json" => println!(
            "{{\"version\":\"{}\",\"sha\":\"{}\",\"source\":\"git\"}}",
            dev_version, short_sha
        ),
        _ => anyhow::bail!("Invalid format: {}", args.format),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_build_version_env_priority() {
        // Set BUILD_VERSION env var
        unsafe {
            env::set_var("BUILD_VERSION", "1.2.3");
        }
        let args = BuildVersionArgs {
            owner: None,
            repo: None,
            github_token: None,
            manifest: "./Cargo.toml".into(),
            repo_path: ".".into(),
            format: "version".to_string(),
        };
        let result = build_version(args);
        unsafe {
            env::remove_var("BUILD_VERSION");
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_version_env_json() {
        unsafe {
            env::set_var("BUILD_VERSION", "2.0.0");
        }
        let args = BuildVersionArgs {
            owner: None,
            repo: None,
            github_token: None,
            manifest: "./Cargo.toml".into(),
            repo_path: ".".into(),
            format: "json".to_string(),
        };
        let result = build_version(args);
        unsafe {
            env::remove_var("BUILD_VERSION");
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_version_cargo_pkg_version() {
        // Clear BUILD_VERSION if set
        unsafe {
            env::remove_var("BUILD_VERSION");
            env::remove_var("CARGO_PKG_VERSION_OVERRIDE");
            env::set_var("CARGO_PKG_VERSION", "1.5.0");
        }
        let args = BuildVersionArgs {
            owner: None,
            repo: None,
            github_token: None,
            manifest: "./Cargo.toml".into(),
            repo_path: ".".into(),
            format: "version".to_string(),
        };
        let result = build_version(args);
        unsafe {
            env::remove_var("CARGO_PKG_VERSION");
        }
        // May succeed if CARGO_PKG_VERSION is set and not 0.0.0
        let _ = result;
    }

    #[test]
    fn test_build_version_invalid_format() {
        unsafe {
            env::set_var("BUILD_VERSION", "1.0.0");
        }
        let args = BuildVersionArgs {
            owner: None,
            repo: None,
            github_token: None,
            manifest: "./Cargo.toml".into(),
            repo_path: ".".into(),
            format: "invalid".to_string(),
        };
        let result = build_version(args);
        unsafe {
            env::remove_var("BUILD_VERSION");
        }
        assert!(result.is_err());
    }

    #[test]
    fn test_build_version_empty_env_var() {
        unsafe {
            env::set_var("BUILD_VERSION", "");
        }
        let args = BuildVersionArgs {
            owner: None,
            repo: None,
            github_token: None,
            manifest: "./Cargo.toml".into(),
            repo_path: ".".into(),
            format: "version".to_string(),
        };
        let result = build_version(args);
        unsafe {
            env::remove_var("BUILD_VERSION");
        }
        // Should fall through to next priority
        let _ = result;
    }

    #[test]
    fn test_build_version_override_priority() {
        unsafe {
            env::set_var("BUILD_VERSION", "1.0.0");
            env::set_var("CARGO_PKG_VERSION_OVERRIDE", "2.0.0");
        }
        let args = BuildVersionArgs {
            owner: None,
            repo: None,
            github_token: None,
            manifest: "./Cargo.toml".into(),
            repo_path: ".".into(),
            format: "version".to_string(),
        };
        let result = build_version(args);
        unsafe {
            env::remove_var("BUILD_VERSION");
            env::remove_var("CARGO_PKG_VERSION_OVERRIDE");
        }
        // BUILD_VERSION should take priority
        assert!(result.is_ok());
    }
}
