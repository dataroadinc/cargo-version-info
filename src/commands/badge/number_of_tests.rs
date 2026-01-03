//! Generate number of tests badge.

use std::process::Command;

use anyhow::{
    Context,
    Result,
};
use serde::{
    Deserialize,
    Serialize,
};

use super::common;

/// Show the number of tests badge.
pub async fn badge_number_of_tests(package: &cargo_metadata::Package) -> Result<()> {
    let test_count = get_test_count(package).await?;

    if let Some(count) = test_count {
        let badge_url = format!("https://img.shields.io/badge/tests-{}-blue", count);
        let badge_markdown = format!("[![Tests]({})](tests/)", badge_url);
        println!("{}", badge_markdown);
    }

    Ok(())
}

/// Cache entry for test count results.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestCountCache {
    /// Package name
    package: String,
    /// Cache key (git commit hash or file mtime)
    cache_key: String,
    /// Test count
    test_count: u32,
}

/// Get the number of tests in the package.
/// Uses cache if available and valid.
async fn get_test_count(package: &cargo_metadata::Package) -> Result<Option<u32>> {
    // Try to load from cache first
    if let Some(cached) = load_test_count_cache(package).await? {
        let current_key = common::compute_cache_key(package).await?;
        if cached.cache_key == current_key && package.name == cached.package {
            return Ok(Some(cached.test_count));
        }
    }

    // Use cargo test --no-run --message-format=json to count tests
    let output_result = tokio::task::spawn_blocking({
        let package_name = package.name.clone();
        move || {
            Command::new("cargo")
                .args([
                    "test",
                    "--package",
                    &package_name,
                    "--no-run",
                    "--message-format=json",
                ])
                .output()
        }
    })
    .await
    .context("Failed to spawn blocking task")?;

    let output = match output_result {
        Ok(out) => out,
        Err(_) => return Ok(None),
    };

    if !output.status.success() {
        return Ok(None);
    }

    // Parse JSON messages to count test artifacts
    let stdout = String::from_utf8(output.stdout).context("Failed to parse cargo test output")?;

    let mut test_count = 0;
    let package_id_prefix = format!("{}@", package.name);
    for line in stdout.lines() {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        // Look for compiler artifacts that are test executables for our package
        if json.get("reason") != Some(&serde_json::Value::String("compiler-artifact".to_string())) {
            continue;
        }

        // Check if this is for our package
        let is_our_package = json
            .get("package_id")
            .and_then(|id| id.as_str())
            .map(|id| id.starts_with(&package_id_prefix))
            .unwrap_or(false);

        if !is_our_package {
            continue;
        }

        // Check if it's a test target with an executable
        let is_test = json
            .get("target")
            .and_then(|t| t.get("kind"))
            .and_then(|k| k.as_array())
            .map(|kinds| kinds.contains(&serde_json::Value::String("test".to_string())))
            .unwrap_or(false);

        if !is_test {
            continue;
        }

        // Count test executables
        if let Some(executable) = json.get("executable")
            && executable.is_string()
        {
            test_count += 1;
        }
    }

    // If we got a count from JSON parsing, use it
    if test_count > 0 {
        // Save to cache
        save_test_count_cache(package, test_count).await?;
        return Ok(Some(test_count));
    }

    // Alternative: count by running test binaries with --list flag
    // First ensure tests are compiled, then run with --list to get test names
    let list_output_result = tokio::task::spawn_blocking({
        let package_name = package.name.clone();
        move || {
            // First compile tests
            let compile_status = Command::new("cargo")
                .args(["test", "--package", &package_name, "--no-run"])
                .status();

            if compile_status.is_err() || !compile_status.unwrap().success() {
                return Err(std::io::Error::other("Failed to compile tests"));
            }

            // Then run with --list to get test names
            Command::new("cargo")
                .args(["test", "--package", &package_name, "--", "--list"])
                .output()
        }
    })
    .await
    .context("Failed to spawn blocking task")?;

    if let Ok(list_output) = list_output_result
        && list_output.status.success()
    {
        let list_stdout = String::from_utf8(list_output.stdout)
            .context("Failed to parse cargo test --list output")?;

        // Count lines that are test names (format: "test_name: test")
        let count = list_stdout
            .lines()
            .filter(|line| line.contains(": test"))
            .count() as u32;

        if count > 0 {
            // Save to cache
            save_test_count_cache(package, count).await?;
            return Ok(Some(count));
        }
    }

    Ok(None)
}

/// Load test count from cache.
async fn load_test_count_cache(
    _package: &cargo_metadata::Package,
) -> Result<Option<TestCountCache>> {
    let cache_path = common::get_badge_cache_path("test-count")?;

    if !cache_path.exists() {
        return Ok(None);
    }

    let contents = tokio::fs::read_to_string(&cache_path)
        .await
        .context("Failed to read cache file")?;

    let cache: TestCountCache =
        serde_json::from_str(&contents).context("Failed to parse cache file")?;

    Ok(Some(cache))
}

/// Save test count to cache.
async fn save_test_count_cache(package: &cargo_metadata::Package, test_count: u32) -> Result<()> {
    let cache_key = common::compute_cache_key(package).await?;
    let cache = TestCountCache {
        package: package.name.to_string(),
        cache_key,
        test_count,
    };

    let cache_path = common::get_badge_cache_path("test-count")?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = cache_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create cache directory")?;
    }

    let json = serde_json::to_string_pretty(&cache).context("Failed to serialize cache")?;

    tokio::fs::write(&cache_path, json)
        .await
        .context("Failed to write cache file")?;

    Ok(())
}
