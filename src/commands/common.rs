//! Common helper functions shared across commands.

use std::env;

use anyhow::{
    Context,
    Result,
};
use cargo_metadata::MetadataCommand;

/// Detect GitHub repository from environment or git remote.
#[allow(clippy::disallowed_methods)] // CLI tool needs direct env access
pub fn detect_repo() -> Result<(String, String)> {
    // Try GITHUB_REPOSITORY env var first (set by GitHub Actions)
    if let Ok(repo) = env::var("GITHUB_REPOSITORY") {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }

    // Try to detect from git remote
    let repo = gix::discover(".").context("Failed to discover git repository")?;
    let remote = repo
        .find_default_remote(gix::remote::Direction::Fetch)
        .context("Failed to find default remote")?
        .context("No default remote found")?;

    let remote_url = remote
        .url(gix::remote::Direction::Fetch)
        .context("Failed to get remote URL")?;

    // Parse git@github.com:owner/repo.git or https://github.com/owner/repo.git
    let url_str = remote_url.to_string();
    if let Some(rest) = url_str.strip_prefix("git@github.com:") {
        let rest_trimmed: &str = rest.strip_suffix(".git").unwrap_or(rest);
        let parts: Vec<&str> = rest_trimmed.split('/').collect();
        if parts.len() >= 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    } else if let Some(rest) = url_str.strip_prefix("https://github.com/") {
        let rest_trimmed: &str = rest.strip_suffix(".git").unwrap_or(rest);
        let parts: Vec<&str> = rest_trimmed.split('/').collect();
        if parts.len() >= 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }

    anyhow::bail!(
        "Could not detect GitHub repository. Set GITHUB_REPOSITORY or use --owner/--repo flags"
    );
}

/// Get owner and repo from args or environment.
pub fn get_owner_repo(owner: Option<String>, repo: Option<String>) -> Result<(String, String)> {
    match (owner, repo) {
        (Some(o), Some(r)) => Ok((o, r)),
        (Some(_), None) | (None, Some(_)) => {
            anyhow::bail!("Both --owner and --repo must be provided together");
        }
        (None, None) => detect_repo(),
    }
}

/// Get package version from a specific manifest path using cargo_metadata.
pub fn get_package_version_from_manifest(manifest_path: &std::path::Path) -> Result<String> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .with_context(|| {
            format!(
                "Failed to get cargo metadata for {}",
                manifest_path.display()
            )
        })?;

    // Check if we're in a workspace and get workspace package version
    if !metadata.workspace_members.is_empty() {
        // Find the workspace package (usually the root package or first member)
        if let Some(root_package) = metadata.root_package() {
            // If root package exists and is in workspace, use it
            if metadata.workspace_members.contains(&root_package.id) {
                return Ok(root_package.version.to_string());
            }
        }
        // Otherwise, find first workspace member package
        if let Some(first_member_id) = metadata.workspace_members.first()
            && let Some(workspace_package) = metadata
                .packages
                .iter()
                .find(|pkg| &pkg.id == first_member_id)
        {
            return Ok(workspace_package.version.to_string());
        }
    }

    // Fall back to root package
    let root_package = metadata
        .root_package()
        .context("No package found in metadata")?;

    Ok(root_package.version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_owner_repo_both_provided() {
        let result = get_owner_repo(Some("owner".to_string()), Some("repo".to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ("owner".to_string(), "repo".to_string()));
    }

    #[test]
    fn test_get_owner_repo_only_owner() {
        let result = get_owner_repo(Some("owner".to_string()), None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Both --owner and --repo must be provided")
        );
    }

    #[test]
    fn test_get_owner_repo_only_repo() {
        let result = get_owner_repo(None, Some("repo".to_string()));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Both --owner and --repo must be provided")
        );
    }
}
