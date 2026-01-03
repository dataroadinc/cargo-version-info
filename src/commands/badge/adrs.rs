//! Generate ADRs badge.

use anyhow::Result;

/// Show the ADRs badge.
pub async fn badge_adrs(package: &cargo_metadata::Package) -> Result<()> {
    let manifest_dir = package
        .manifest_path
        .as_std_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    // Check if docs/adr/ directory exists
    let adr_dir = manifest_dir.join("docs/adr");
    let has_adrs = tokio::fs::metadata(&adr_dir).await.is_ok();

    if has_adrs {
        let badge_url = "https://img.shields.io/badge/ADRs-index-informational";
        let badge_markdown = format!("[![ADRs]({})](docs/adr/index.typ)", badge_url);
        println!("{}", badge_markdown);
    }

    Ok(())
}
