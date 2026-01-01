//! GitHub API integration for version queries.

use anyhow::{
    Context,
    Result,
};

use crate::version::{
    format_version,
    increment_patch,
    parse_version,
};

/// Get the latest published release version from GitHub.
///
/// Uses the GitHub API via octocrab if GITHUB_TOKEN is set,
/// otherwise falls back to gh CLI.
pub async fn get_latest_release_version(
    owner: &str,
    repo: &str,
    github_token: Option<&str>,
) -> Result<Option<String>> {
    // Try GitHub API first if token is available
    if let Some(token) = github_token
        && let Ok(version) = get_latest_release_via_api(owner, repo, token).await
    {
        return Ok(Some(version));
    }

    // Fallback to gh CLI
    get_latest_release_via_cli(owner, repo)
}

/// Get latest release via GitHub API.
async fn get_latest_release_via_api(owner: &str, repo: &str, token: &str) -> Result<String> {
    let octocrab = octocrab::OctocrabBuilder::new()
        .personal_token(token.to_string())
        .build()
        .context("Failed to create GitHub API client")?;

    let releases = octocrab
        .repos(owner, repo)
        .releases()
        .list()
        .per_page(1)
        .send()
        .await
        .context("Failed to query GitHub releases")?;

    let release = releases.items.first().context("No releases found")?;

    let tag_name = release.tag_name.as_str();
    let version = tag_name.strip_prefix('v').unwrap_or(tag_name);
    let version = version.strip_prefix('V').unwrap_or(version);

    Ok(version.to_string())
}

/// Get latest release via gh CLI.
fn get_latest_release_via_cli(owner: &str, repo: &str) -> Result<Option<String>> {
    use std::process::Command;

    let output = Command::new("gh")
        .args([
            "release",
            "list",
            "--repo",
            &format!("{}/{}", owner, repo),
            "--exclude-drafts",
            "--limit",
            "1",
            "--json",
            "tagName",
            "--jq",
            ".[0].tagName",
        ])
        .output()
        .context("Failed to execute gh CLI. Is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no releases found") || stderr.is_empty() {
            return Ok(None);
        }
        anyhow::bail!("gh CLI failed: {}", stderr);
    }

    let tag_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tag_name.is_empty() {
        return Ok(None);
    }

    let version = tag_name.strip_prefix('v').unwrap_or(&tag_name);
    let version = version.strip_prefix('V').unwrap_or(version);

    Ok(Some(version.to_string()))
}

/// Calculate next patch version from latest GitHub release.
pub async fn calculate_next_version(
    owner: &str,
    repo: &str,
    github_token: Option<&str>,
) -> Result<(String, String)> {
    // Get latest release
    let latest_version_str = match get_latest_release_version(owner, repo, github_token).await? {
        Some(v) => v,
        None => {
            // No releases yet, start at 0.0.1
            return Ok(("0.0.0".to_string(), "0.0.1".to_string()));
        }
    };

    let (major, minor, patch) = parse_version(&latest_version_str)
        .with_context(|| format!("Failed to parse latest version: {}", latest_version_str))?;

    let (major, minor, patch) = increment_patch(major, minor, patch);
    let next_version = format_version(major, minor, patch);

    Ok((latest_version_str, next_version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_get_latest_release_via_cli() {
        // This test requires gh CLI and network access
        // Only run manually
        if let Ok(Some(version)) = get_latest_release_via_cli("rust-lang", "rust") {
            println!("Latest rust release: {}", version);
        }
    }
}
