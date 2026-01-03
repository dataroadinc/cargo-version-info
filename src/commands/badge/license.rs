//! Generate license badge.

use anyhow::Result;

/// Show the license badge.
pub async fn badge_license(package: &cargo_metadata::Package) -> Result<()> {
    if let Some(license) = &package.license {
        let license_encoded = license.replace(' ', "%20");
        let badge_url = format!("https://img.shields.io/crates/l/{}", license_encoded);
        let badge_markdown = format!(
            "[![license]({})](https://opensource.org/licenses/{})",
            badge_url, license_encoded
        );
        println!("{}", badge_markdown);
    }

    Ok(())
}
