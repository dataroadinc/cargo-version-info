//! Generate framework badge.

use anyhow::Result;

/// Show the framework badge.
pub async fn badge_framework(package: &cargo_metadata::Package) -> Result<()> {
    // Check dependencies for framework
    let has_axum = package.dependencies.iter().any(|dep| dep.name == "axum");

    if has_axum {
        let badge_url = "https://img.shields.io/badge/web%20framework-Axum-blueviolet";
        let badge_markdown = format!(
            "[![Framework]({})](docs/adr/0008-web-framework-axum.typ)",
            badge_url
        );
        println!("{}", badge_markdown);
    }
    // Future: add other frameworks (actix-web, warp, etc.)

    Ok(())
}
