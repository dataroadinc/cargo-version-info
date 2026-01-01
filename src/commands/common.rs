//! Common helper functions shared across commands.

use std::env;

use anyhow::{
    Context,
    Result,
};

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

/// Extract version from `[workspace.package]` section.
pub fn extract_workspace_version(content: &str) -> Option<String> {
    // Simple regex-like parsing for [workspace.package] version
    let workspace_section_start = content.find("[workspace.package]")?;
    let workspace_section = &content[workspace_section_start..];

    // Find version = "..." or version = { workspace = true }
    for line in workspace_section.lines() {
        if let Some(version_start) = line.find("version") {
            let after_version = &line[version_start + 7..];
            if let Some(quote_start) = after_version.find('"') {
                let version_content = &after_version[quote_start + 1..];
                if let Some(quote_end) = version_content.find('"') {
                    return Some(version_content[..quote_end].to_string());
                }
            }
        }
    }

    None
}

/// Extract version from `[package]` section.
pub fn extract_package_version(content: &str) -> Result<String> {
    // Simple parsing for version = "..."
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version")
            && let Some(quote_start) = trimmed.find('"')
        {
            let after_quote = &trimmed[quote_start + 1..];
            if let Some(quote_end) = after_quote.find('"') {
                return Ok(after_quote[..quote_end].to_string());
            }
        }
    }

    anyhow::bail!("No version found in `[package]` section");
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

    #[test]
    fn test_extract_workspace_version() {
        let content = r#"
[workspace.package]
version = "0.1.2"
"#;
        assert_eq!(
            extract_workspace_version(content),
            Some("0.1.2".to_string())
        );
    }

    #[test]
    fn test_extract_workspace_version_with_spaces() {
        let content = r#"
[workspace.package]
version = "1.2.3"
"#;
        assert_eq!(
            extract_workspace_version(content),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn test_extract_workspace_version_not_found() {
        let content = r#"
[package]
version = "0.1.2"
"#;
        assert_eq!(extract_workspace_version(content), None);
    }

    #[test]
    fn test_extract_package_version() {
        let content = r#"
[package]
name = "test"
version = "0.1.2"
"#;
        assert_eq!(
            extract_package_version(content).unwrap(),
            "0.1.2".to_string()
        );
    }

    #[test]
    fn test_extract_package_version_with_spaces() {
        let content = r#"
[package]
name = "test"
version = "1.2.3"
"#;
        assert_eq!(
            extract_package_version(content).unwrap(),
            "1.2.3".to_string()
        );
    }

    #[test]
    fn test_extract_package_version_not_found() {
        let content = r#"
[package]
name = "test"
"#;
        assert!(extract_package_version(content).is_err());
    }

    #[test]
    fn test_extract_workspace_version_precedence() {
        // Workspace version should be found even if package version exists
        let content = r#"
[workspace.package]
version = "0.1.0"

[package]
name = "test"
version = "0.1.2"
"#;
        assert_eq!(
            extract_workspace_version(content),
            Some("0.1.0".to_string())
        );
    }
}
