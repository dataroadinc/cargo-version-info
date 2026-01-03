//! Generate all badges.

use anyhow::Result;

use super::{
    adrs,
    coverage,
    cratesio,
    framework,
    license,
    number_of_tests,
    platform,
    runtime,
    rust_edition,
    rustdocs,
};

/// Generate all badges
pub async fn badge_all(package: &cargo_metadata::Package, no_network: bool) -> Result<()> {
    rustdocs::badge_rustdocs(package, no_network).await?;
    cratesio::badge_cratesio(package, no_network).await?;
    license::badge_license(package).await?;
    rust_edition::badge_rust_edition(package).await?;
    runtime::badge_runtime(package).await?;
    framework::badge_framework(package).await?;
    platform::badge_platform(package).await?;
    adrs::badge_adrs(package).await?;
    coverage::badge_coverage(package).await?;
    number_of_tests::badge_number_of_tests(package).await?;

    Ok(())
}
