//! Generate runtime badge.

use anyhow::Result;

/// Show the runtime badge.
pub async fn badge_runtime(package: &cargo_metadata::Package) -> Result<()> {
    // Check dependencies for runtime
    let has_tokio = package.dependencies.iter().any(|dep| dep.name == "tokio");

    if has_tokio {
        let badge_url = "https://img.shields.io/badge/runtime-Tokio-blue";
        let badge_markdown = format!(
            "[![Runtime]({})](docs/adr/0007-async-runtime-tokio.typ)",
            badge_url
        );
        println!("{}", badge_markdown);
    }
    // Future: add other runtimes (async-std, smol, etc.)

    Ok(())
}
