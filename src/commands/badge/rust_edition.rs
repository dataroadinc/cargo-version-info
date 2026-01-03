//! Generate Rust edition badge.

use anyhow::Result;

/// Show the Rust edition badge.
pub async fn badge_rust_edition(package: &cargo_metadata::Package) -> Result<()> {
    let edition_str = package.edition.as_str();
    let badge_url = format!(
        "https://img.shields.io/badge/rust%20edition-{}-orange",
        edition_str
    );
    let badge_markdown = format!("[![Rust Edition]({})](Cargo.toml)", badge_url);
    println!("{}", badge_markdown);

    Ok(())
}
