//! Generate docs.rs badge.

use anyhow::Result;

use super::common::is_published_on_docs_rs;

/// Show the docs.rs badge if the project is published there.
pub async fn badge_rustdocs(package: &cargo_metadata::Package, no_network: bool) -> Result<()> {
    let package_name = &package.name;

    if is_published_on_docs_rs(package_name, package, no_network).await? {
        let badge_url = format!("https://img.shields.io/docsrs/{}", package_name);
        let badge_markdown = format!(
            "[![docs.rs]({})](https://docs.rs/{})",
            badge_url, package_name
        );
        println!("{}", badge_markdown);
    }

    Ok(())
}
