//! Generate crates.io badge.

use anyhow::Result;

use super::common::is_published_on_crates_io;

/// Show the crates.io badge if the project is published there.
pub async fn badge_cratesio(package: &cargo_metadata::Package, no_network: bool) -> Result<()> {
    let package_name = &package.name;

    if is_published_on_crates_io(package_name, package, no_network).await? {
        let badge_url = format!("https://img.shields.io/crates/v/{}", package_name);
        let badge_markdown = format!(
            "[![crates.io]({})](https://crates.io/crates/{})",
            badge_url, package_name
        );
        println!("{}", badge_markdown);
    }

    Ok(())
}
