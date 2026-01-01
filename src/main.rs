//! Cargo subcommand for unified version management.
//!
//! This tool provides a single source of truth for version operations:
//! - Calculate next version from GitHub releases
//! - Read current version from Cargo.toml
//! - Compare versions
//! - Generate dev versions from git SHA
//! - Generate tag names
//!
//! Replaces scattered version logic in GitHub Actions, bash scripts, and Rust
//! code.

use std::fs;

use anyhow::Result;
use clap::Parser;

mod commands;
mod github;
mod version;

use commands::{
    BadgesArgs,
    BuildVersionArgs,
    ChangedArgs,
    ChangelogArgs,
    CompareArgs,
    CurrentArgs,
    DevArgs,
    DioxusArgs,
    LatestArgs,
    NextArgs,
    PostBumpHookArgs,
    PrLogArgs,
    PreBumpHookArgs,
    RustToolchainArgs,
    TagArgs,
    UpdateReadmeArgs,
};

#[derive(Parser, Debug)]
#[command(
    name = "cargo-version-info",
    about = "Unified version management for Rust projects",
    bin_name = "cargo"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Calculate next patch version from latest GitHub release
    #[command(name = "next")]
    Next(NextArgs),
    /// Get current version from Cargo.toml
    #[command(name = "current")]
    Current(CurrentArgs),
    /// Get latest GitHub release version
    #[command(name = "latest")]
    Latest(LatestArgs),
    /// Generate dev version from git SHA
    #[command(name = "dev")]
    Dev(DevArgs),
    /// Generate tag name (e.g., v0.0.1)
    #[command(name = "tag")]
    Tag(TagArgs),
    /// Compare two versions
    #[command(name = "compare")]
    Compare(CompareArgs),
    /// Get Rust toolchain version from .rust-toolchain.toml
    #[command(name = "rust-toolchain")]
    RustToolchain(RustToolchainArgs),
    /// Get Dioxus version from Cargo.toml
    #[command(name = "dioxus")]
    Dioxus(DioxusArgs),
    /// Determine build version with priority logic
    #[command(name = "build-version")]
    BuildVersion(BuildVersionArgs),
    /// Check if Cargo.toml version changed since last git tag
    #[command(name = "changed")]
    Changed(ChangedArgs),
    /// Pre-bump hook for cog integration (verifies state before bumping)
    #[command(name = "pre-bump-hook")]
    PreBumpHook(PreBumpHookArgs),
    /// Post-bump hook for cog integration (verifies bump succeeded)
    #[command(name = "post-bump-hook")]
    PostBumpHook(PostBumpHookArgs),
    /// Generate changelog from conventional commits
    #[command(name = "changelog")]
    Changelog(ChangelogArgs),
    /// Generate PR log from merged pull requests
    #[command(name = "pr-log")]
    PrLog(PrLogArgs),
    /// Generate badges for quality metrics
    #[command(name = "badges")]
    Badges(BadgesArgs),
    /// Update README with badges
    #[command(name = "update-readme")]
    UpdateReadme(UpdateReadmeArgs),
}

/// Check if any .env* files exist in the current directory.
fn has_env_files() -> bool {
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return false,
    };

    // Check for common .env* file patterns
    let patterns = [".env", ".env.local", ".env.prod", ".env.dev", ".env.test"];

    for pattern in &patterns {
        let path = current_dir.join(pattern);
        if path.exists() && fs::metadata(&path).map(|m| m.is_file()).unwrap_or(false) {
            return true;
        }
    }

    // Also check for .env.{USER} pattern
    if let Ok(user) = std::env::var("USER") {
        let user_env = format!(".env.{}", user);
        let path = current_dir.join(user_env);
        if path.exists() && fs::metadata(&path).map(|m| m.is_file()).unwrap_or(false) {
            return true;
        }
    }

    false
}

fn main() -> Result<()> {
    // Load environment variables from .env* files using dotenvage
    // This allows cargo-version-info to access encrypted secrets like GITHUB_TOKEN
    // stored in .env.local files protected by dotenvage
    // Only attempt to load if .env* files exist to avoid unnecessary warnings
    if has_env_files()
        && let Err(e) = dotenvage::EnvLoader::new().and_then(|loader| loader.load())
    {
        eprintln!("Warning: Failed to load/decrypt env files: {}", e);
        eprintln!("Continuing with existing environment variables...");
    }

    let cli = Cli::parse();

    match cli.command {
        Command::Next(args) => commands::next(args),
        Command::Current(args) => commands::current(args),
        Command::Latest(args) => commands::latest(args),
        Command::Dev(args) => commands::dev(args),
        Command::Tag(args) => commands::tag(args),
        Command::Compare(args) => commands::compare(args),
        Command::RustToolchain(args) => commands::rust_toolchain(args),
        Command::Dioxus(args) => commands::dioxus(args),
        Command::BuildVersion(args) => commands::build_version(args),
        Command::Changed(args) => commands::changed(args),
        Command::PreBumpHook(args) => commands::pre_bump_hook(args),
        Command::PostBumpHook(args) => commands::post_bump_hook(args),
        Command::Changelog(args) => commands::changelog(args),
        Command::PrLog(args) => commands::pr_log(args),
        Command::Badges(args) => commands::badges(args),
        Command::UpdateReadme(args) => commands::update_readme(args),
    }
}
