//! Generate badges for quality metrics.
//!
//! This command generates various badges (tests, coverage, quality metrics)
//! that can be included in markdown documents.
//!
//! # Examples
//!
//! ```bash
//! # Generate all badges
//! cargo version-info badge all
//!
//! # Generate docs.rs badge (only if published)
//! cargo version-info badge rustdocs
//!
//! # Generate crates.io badge (only if published)
//! cargo version-info badge cratesio
//!
//! # Generate license badge
//! cargo version-info badge license
//!
//! # Generate Rust edition badge
//! cargo version-info badge rust-edition
//!
//! # Generate runtime badge
//! cargo version-info badge runtime
//!
//! # Generate framework badge
//! cargo version-info badge framework
//!
//! # Generate platform badge
//! cargo version-info badge platform
//!
//! # Generate ADRs badge
//! cargo version-info badge ADRs
//!
//! # Generate coverage badge (requires cargo-llvm-cov)
//! cargo version-info badge coverage
//!
//! # Generate number of tests badge
//! cargo version-info badge number-of-tests
//!
//! # Use heuristics instead of network requests
//! cargo version-info badge all --no-network
//! cargo version-info badge rustdocs --no-network
//! ```

mod adrs;
mod all;
mod common;
mod coverage;
mod cratesio;
mod framework;
mod license;
mod number_of_tests;
mod platform;
mod runtime;
mod rust_edition;
mod rustdocs;

use anyhow::{
    Context,
    Result,
};
use clap::{
    Parser,
    Subcommand,
};

/// Arguments for the `badge` command.
#[derive(Parser, Debug)]
pub struct BadgeArgs {
    /// Skip network requests and use heuristics to guess if crate is published.
    ///
    /// When set, checks:
    /// - `publish` field in Cargo.toml
    /// - Whether any GitHub workflow files contain "cargo publish"
    /// - Whether LICENSE file exists
    #[arg(long)]
    pub no_network: bool,

    /// The badge subcommand to execute.
    #[command(subcommand)]
    pub subcommand: BadgeSubcommand,
}

/// Subcommands for the badge command.
#[derive(Subcommand, Debug)]
pub enum BadgeSubcommand {
    /// Generate all badges (including rustdocs and cratesio if published).
    All,
    /// Show the docs.rs badge if the project is published there, otherwise no
    /// output.
    Rustdocs,
    /// Show the crates.io badge if the project is published there, otherwise no
    /// output.
    Cratesio,
    /// Show the license badge.
    License,
    /// Show the Rust edition badge.
    #[command(name = "rust-edition")]
    RustEdition,
    /// Show the runtime badge (Tokio, etc.).
    Runtime,
    /// Show the framework badge (Axum, etc.).
    Framework,
    /// Show the platform badge (Fly.io, Vercel, etc.).
    Platform,
    /// Show the ADRs badge if docs/adr/ exists.
    ADRs,
    /// Show the test coverage badge (requires cargo-llvm-cov).
    Coverage,
    /// Show the number of tests badge.
    #[command(name = "number-of-tests")]
    NumberOfTests,
}

/// Generate badges for quality metrics.
pub fn badge(args: BadgeArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    rt.block_on(badge_async(args))
}

/// Async entry point for badge generation.
async fn badge_async(args: BadgeArgs) -> Result<()> {
    // Detect package from Cargo's context (working directory when
    // --manifest-path is used)
    let package = find_package().await?;

    match args.subcommand {
        BadgeSubcommand::All => all::badge_all(&package, args.no_network).await,
        BadgeSubcommand::Rustdocs => rustdocs::badge_rustdocs(&package, args.no_network).await,
        BadgeSubcommand::Cratesio => cratesio::badge_cratesio(&package, args.no_network).await,
        BadgeSubcommand::License => license::badge_license(&package).await,
        BadgeSubcommand::RustEdition => rust_edition::badge_rust_edition(&package).await,
        BadgeSubcommand::Runtime => runtime::badge_runtime(&package).await,
        BadgeSubcommand::Framework => framework::badge_framework(&package).await,
        BadgeSubcommand::Platform => platform::badge_platform(&package).await,
        BadgeSubcommand::ADRs => adrs::badge_adrs(&package).await,
        BadgeSubcommand::Coverage => coverage::badge_coverage(&package).await,
        BadgeSubcommand::NumberOfTests => number_of_tests::badge_number_of_tests(&package).await,
    }
}

/// Find the Cargo package using cargo_metadata.
///
/// This automatically respects Cargo's `--manifest-path` option when running
/// as a cargo subcommand.
///
/// Returns the package that corresponds to the current context:
/// - If `--manifest-path` is specified, finds the package matching that path
/// - Otherwise, finds the package in the current working directory
/// - Falls back to the root package if no match is found
async fn find_package() -> Result<cargo_metadata::Package> {
    use cargo_metadata::MetadataCommand;

    // Use cargo_metadata which automatically respects --manifest-path
    let metadata = tokio::task::spawn_blocking(|| MetadataCommand::new().exec())
        .await
        .context("Failed to spawn blocking task")?
        .context("Failed to get cargo metadata")?;

    // Try to find the package in the current working directory
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let current_manifest = current_dir.join("Cargo.toml");

    // Canonicalize current manifest path and all package paths, then find match
    let (canonical_current, packages_with_paths) = tokio::task::spawn_blocking({
        let packages = metadata.packages.clone();
        let current = current_manifest.clone();
        move || {
            let canonical_current = current.canonicalize().ok();
            let packages_with_paths: Vec<_> = packages
                .iter()
                .filter_map(|pkg| {
                    pkg.manifest_path
                        .as_std_path()
                        .canonicalize()
                        .ok()
                        .map(|p| (pkg.clone(), p))
                })
                .collect();
            (canonical_current, packages_with_paths)
        }
    })
    .await
    .context("Failed to spawn blocking task")?;

    if let Some(ref canonical) = canonical_current
        && let Some((pkg, _)) = packages_with_paths
            .iter()
            .find(|(_, pkg_path)| pkg_path == canonical)
    {
        return Ok(pkg.clone());
    }

    // Fallback to root package (workspace root or single package)
    metadata
        .root_package()
        .cloned()
        .context("No package found in metadata")
}
