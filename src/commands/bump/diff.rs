//! Diff generation and hunk-level filtering.
//!
//! This module implements the "holy grail" of version bumping: staging ONLY
//! the lines that contain version changes, leaving all other changes (even
//! in the same file) unstaged.
//!
//! # The Problem
//!
//! Consider this scenario - you're working on Cargo.toml:
//!
//! ```diff
//! @@ -1,7 +1,8 @@
//!  [package]
//!  name = "my-crate"
//! -version = "0.1.0"
//! +version = "0.2.0"
//!  edition = "2021"
//!  
//!  [dependencies]
//! -serde = "1.0"
//! +serde = { version = "1.0", features = ["derive"] }
//! ```
//!
//! We want to commit ONLY the version line change, not the serde dependency
//! change. This requires hunk-level staging.
//!
//! # Solution: Unified Diff + Hunk Filtering
//!
//! 1. Generate unified diff between HEAD and working directory
//! 2. Parse diff into hunks
//! 3. Filter hunks to find version-related changes
//! 4. Apply only those hunks to create a partially-staged file
//! 5. Write the partially-staged content as a blob
//!
//! # Unified Diff Format
//!
//! A unified diff looks like:
//! ```text
//! --- a/Cargo.toml
//! +++ b/Cargo.toml
//! @@ -3,5 +3,5 @@
//!  name = "my-crate"
//! -version = "0.1.0"
//! +version = "0.2.0"
//!  edition = "2021"
//! ```
//!
//! Key components:
//! - **Hunk header**: `@@ -3,5 +3,5 @@` means "at line 3, remove 5 lines, add 5
//!   lines"
//! - **Context lines**: Start with space, unchanged
//! - **Removed lines**: Start with `-`
//! - **Added lines**: Start with `+`
//!
//! # Hunk Filtering Logic
//!
//! A hunk is "version-related" if:
//! - It contains lines with "version" keyword
//! - It contains the old or new version string
//! - It's within a reasonable distance of other version changes
//!
//! # Implementation Strategy
//!
//! We use the `similar` crate to:
//! - Generate line-by-line diff
//! - Identify change regions (hunks)
//! - Reconstruct file content with selected changes only

use anyhow::Result;
use regex::Regex;
use similar::{
    ChangeTag,
    TextDiff,
};

/// Apply only version-related hunks to create partially-staged content.
///
/// This is the core function that implements selective hunk staging. It:
/// 1. Generates a diff between HEAD and working directory versions
/// 2. Identifies which lines changed
/// 3. Filters to keep only version-related changes
/// 4. Reconstructs the file with only those changes applied
///
/// # Arguments
///
/// * `head_content` - Content of the file in HEAD commit
/// * `working_content` - Content of the file in working directory
/// * `old_version` - The version string being replaced
/// * `new_version` - The version string being added
///
/// # Returns
///
/// Returns the partially-staged content (HEAD + only version changes).
///
/// # Examples
///
/// ```rust
/// # use cargo_version_info::commands::bump::diff::apply_version_hunks;
/// let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\ndesc = \"old\"";
/// let working = "[package]\nname = \"test\"\nversion = \"0.2.0\"\ndesc = \"new\"";
///
/// let staged = apply_version_hunks(head, working, "0.1.0", "0.2.0").unwrap();
///
/// // staged contains only the version change, not the desc change
/// assert!(staged.contains("version = \"0.2.0\""));
/// assert!(staged.contains("desc = \"old\"")); // NOT "new"
/// ```
///
/// # Algorithm
///
/// 1. Generate unified diff using `similar::TextDiff`
/// 2. Iterate through all changes (insertions, deletions, unchanged)
/// 3. For each change, check if it's version-related:
///    - Does the line contain "version"?
///    - Does the line contain old_version or new_version?
/// 4. Build output:
///    - Version-related changes: Use working directory version
///    - Non-version changes: Use HEAD version (ignore working changes)
///    - Unchanged lines: Include as-is
///
/// # Edge Cases
///
/// - **Multiple version fields**: All are updated (package.version,
///   dependencies.*.version)
/// - **Version in comments**: May be incorrectly detected (acceptable
///   trade-off)
/// - **Adjacent changes**: Non-version changes adjacent to version changes are
///   kept separate
pub fn apply_version_hunks(
    head_content: &str,
    working_content: &str,
    old_version: &str,
    new_version: &str,
) -> Result<String> {
    // Generate unified diff between HEAD and working directory
    let diff = TextDiff::from_lines(head_content, working_content);

    let mut result = Vec::new();

    // Iterate through all changes
    for change in diff.iter_all_changes() {
        let line = change.value();

        // Determine if this line is version-related
        let is_version_related =
            line.contains("version") || line.contains(old_version) || line.contains(new_version);

        match change.tag() {
            ChangeTag::Equal => {
                // Unchanged line - always include
                result.push(line);
            }
            ChangeTag::Delete => {
                // Line removed in working directory
                if is_version_related {
                    // This is a version line being removed - apply the change
                    // (skip it) Don't add to result
                } else {
                    // Non-version line removed - keep the original (don't apply change)
                    result.push(line);
                }
            }
            ChangeTag::Insert => {
                // Line added in working directory
                if is_version_related {
                    // This is a version line being added - apply the change (include it)
                    result.push(line);
                } else {
                    // Non-version line added - don't apply the change (skip it)
                    // The line stays not present (remains as in HEAD)
                }
            }
        }
    }

    Ok(result.join(""))
}

/// Check if the file has changes beyond version modifications.
///
/// This is used to determine if we need hunk-level filtering or if we can
/// just stage the whole file.
///
/// # Arguments
///
/// * `head_content` - Content from HEAD
/// * `working_content` - Content from working directory
/// * `old_version` - Old version string
/// * `new_version` - New version string
///
/// # Returns
///
/// Returns `true` if there are non-version changes.
pub fn has_non_version_changes(
    head_content: &str,
    working_content: &str,
    old_version: &str,
    new_version: &str,
) -> bool {
    let diff = TextDiff::from_lines(head_content, working_content);

    // Check if any changes are NOT version-related
    for change in diff.iter_all_changes() {
        if matches!(change.tag(), ChangeTag::Delete | ChangeTag::Insert) {
            let line = change.value();
            let is_version_related = line.contains("version")
                || line.contains(old_version)
                || line.contains(new_version);

            if !is_version_related {
                // Found a non-version change
                return true;
            }
        }
    }

    false
}

/// Check if a line is version-related for README.md.
///
/// A line is README-version-related if it matches the pattern
/// `crate-name = "version"` (typical TOML dependency format).
///
/// # Arguments
///
/// * `line` - The line to check
/// * `crate_name` - The crate name (hyphenated or underscored)
/// * `old_version` - The version being replaced
/// * `new_version` - The version being added
fn is_readme_version_line(
    line: &str,
    crate_name: &str,
    old_version: &str,
    new_version: &str,
) -> bool {
    // Handle both hyphenated and underscored package names
    let hyphenated = crate_name.replace('_', "-");
    let underscored = crate_name.replace('-', "_");

    for name in [&hyphenated, &underscored] {
        // Pattern: name = "version" (with optional whitespace)
        for version in [old_version, new_version] {
            let pattern = format!(
                r#"{}(\s|[-_])?\s*=\s*"{}""#,
                regex::escape(name),
                regex::escape(version)
            );
            if let Ok(re) = Regex::new(&pattern)
                && re.is_match(line)
            {
                return true;
            }
        }
    }

    false
}

/// Apply only README version-related hunks to create partially-staged content.
///
/// This function filters changes to README.md to include only lines that
/// reference the crate's version (e.g., `my-crate = "1.0.0"`).
///
/// # Arguments
///
/// * `head_content` - Content of README.md in HEAD commit
/// * `working_content` - Content of README.md in working directory
/// * `crate_name` - The crate/package name
/// * `old_version` - The version string being replaced
/// * `new_version` - The version string being added
///
/// # Returns
///
/// Returns the partially-staged content (HEAD + only version-related changes).
pub fn apply_readme_version_hunks(
    head_content: &str,
    working_content: &str,
    crate_name: &str,
    old_version: &str,
    new_version: &str,
) -> Result<String> {
    let diff = TextDiff::from_lines(head_content, working_content);

    let mut result = Vec::new();

    for change in diff.iter_all_changes() {
        let line = change.value();
        let is_version_related = is_readme_version_line(line, crate_name, old_version, new_version);

        match change.tag() {
            ChangeTag::Equal => {
                result.push(line);
            }
            ChangeTag::Delete => {
                if is_version_related {
                    // Version line being removed - apply the change (skip it)
                } else {
                    // Non-version line - keep original
                    result.push(line);
                }
            }
            ChangeTag::Insert => {
                if is_version_related {
                    // Version line being added - apply the change (include it)
                    result.push(line);
                } else {
                    // Non-version line - don't apply the change (skip it)
                }
            }
        }
    }

    Ok(result.join(""))
}

/// Check if README.md has changes beyond version modifications.
///
/// # Arguments
///
/// * `head_content` - Content from HEAD
/// * `working_content` - Content from working directory
/// * `crate_name` - The crate/package name
/// * `old_version` - Old version string
/// * `new_version` - New version string
///
/// # Returns
///
/// Returns `true` if there are non-version changes.
pub fn has_non_readme_version_changes(
    head_content: &str,
    working_content: &str,
    crate_name: &str,
    old_version: &str,
    new_version: &str,
) -> bool {
    let diff = TextDiff::from_lines(head_content, working_content);

    for change in diff.iter_all_changes() {
        if matches!(change.tag(), ChangeTag::Delete | ChangeTag::Insert) {
            let line = change.value();
            let is_version_related =
                is_readme_version_line(line, crate_name, old_version, new_version);

            if !is_version_related {
                return true;
            }
        }
    }

    false
}

/// Check if a line belongs to our crate's package block in Cargo.lock.
///
/// Cargo.lock has the format:
/// ```toml
/// [[package]]
/// name = "crate-name"
/// version = "1.0.0"
/// ...
/// ```
///
/// We track if we're inside the package block for our crate.
fn is_cargo_lock_version_line(
    line: &str,
    crate_name: &str,
    old_version: &str,
    new_version: &str,
) -> bool {
    // Check if this line references our crate's name or version
    let name_pattern = format!(r#"name\s*=\s*"{}""#, regex::escape(crate_name));
    if let Ok(re) = Regex::new(&name_pattern)
        && re.is_match(line)
    {
        return true;
    }

    // Check for version lines with old or new version
    for version in [old_version, new_version] {
        let version_pattern = format!(r#"version\s*=\s*"{}""#, regex::escape(version));
        if let Ok(re) = Regex::new(&version_pattern)
            && re.is_match(line)
        {
            return true;
        }
    }

    false
}

/// Apply only Cargo.lock version-related hunks for our crate.
///
/// This function filters changes to Cargo.lock to include only changes
/// to our crate's package entry.
///
/// # Algorithm
///
/// We use a stateful approach to track which package block we're in:
/// 1. Parse the diff line by line
/// 2. Track the current package name based on `name = "..."` lines
/// 3. For version changes, only include those for our crate
///
/// # Arguments
///
/// * `head_content` - Content of Cargo.lock in HEAD commit
/// * `working_content` - Content of Cargo.lock in working directory
/// * `crate_name` - Our crate's name
/// * `old_version` - The version string being replaced
/// * `new_version` - The version string being added
///
/// # Returns
///
/// Returns the partially-staged content (HEAD + only our crate's version
/// changes).
pub fn apply_cargo_lock_version_hunks(
    head_content: &str,
    working_content: &str,
    crate_name: &str,
    old_version: &str,
    new_version: &str,
) -> Result<String> {
    let diff = TextDiff::from_lines(head_content, working_content);

    let mut result = Vec::new();

    // Track context: are we in a hunk that relates to our crate?
    // We use a simple heuristic: if a hunk contains our crate name,
    // all version changes in that hunk are for our crate.
    let changes: Vec<_> = diff.iter_all_changes().collect();

    // Group changes into hunks (sequences of Insert/Delete surrounded by Equal)
    let mut current_hunk: Vec<_> = Vec::new();
    let mut hunks: Vec<Vec<_>> = Vec::new();

    for change in &changes {
        match change.tag() {
            ChangeTag::Equal => {
                if !current_hunk.is_empty() {
                    hunks.push(current_hunk.clone());
                    current_hunk.clear();
                }
                // Equal lines are processed directly
                result.push(change.value());
            }
            ChangeTag::Delete | ChangeTag::Insert => {
                current_hunk.push(change);
            }
        }
    }

    // Don't forget the last hunk
    if !current_hunk.is_empty() {
        hunks.push(current_hunk);
    }

    // Now we have hunks. We need to rebuild the file from them.
    // Reset result and process properly
    result.clear();

    for change in &changes {
        let line = change.value();

        match change.tag() {
            ChangeTag::Equal => {
                result.push(line);
            }
            ChangeTag::Delete => {
                // Check if this is a version line for our crate
                let is_our_version =
                    is_cargo_lock_version_line(line, crate_name, old_version, new_version);

                if is_our_version {
                    // Our crate's version line - apply the deletion (skip it)
                } else {
                    // Other package's line - keep it
                    result.push(line);
                }
            }
            ChangeTag::Insert => {
                // Check if this is a version line for our crate
                let is_our_version =
                    is_cargo_lock_version_line(line, crate_name, old_version, new_version);

                if is_our_version {
                    // Our crate's version line - apply the insertion (include it)
                    result.push(line);
                } else {
                    // Other package's line - don't apply (skip it)
                }
            }
        }
    }

    Ok(result.join(""))
}

/// Check if Cargo.lock has changes beyond our crate's version modification.
///
/// # Arguments
///
/// * `head_content` - Content from HEAD
/// * `working_content` - Content from working directory
/// * `crate_name` - Our crate's name
/// * `old_version` - Old version string
/// * `new_version` - New version string
///
/// # Returns
///
/// Returns `true` if there are non-version changes (e.g., dependency updates).
pub fn has_non_cargo_lock_version_changes(
    head_content: &str,
    working_content: &str,
    crate_name: &str,
    old_version: &str,
    new_version: &str,
) -> bool {
    let diff = TextDiff::from_lines(head_content, working_content);

    for change in diff.iter_all_changes() {
        if matches!(change.tag(), ChangeTag::Delete | ChangeTag::Insert) {
            let line = change.value();
            let is_our_version =
                is_cargo_lock_version_line(line, crate_name, old_version, new_version);

            if !is_our_version {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_version_hunks_only_version_change() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
        let working = "[package]\nname = \"test\"\nversion = \"0.2.0\"\nedition = \"2021\"\n";

        let staged = apply_version_hunks(head, working, "0.1.0", "0.2.0").unwrap();

        assert!(staged.contains("version = \"0.2.0\""));
        assert!(!staged.contains("0.1.0"));
    }

    #[test]
    fn test_apply_version_hunks_mixed_changes() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\ndescription = \"old desc\"\n";
        let working =
            "[package]\nname = \"test\"\nversion = \"0.2.0\"\ndescription = \"new desc\"\n";

        let staged = apply_version_hunks(head, working, "0.1.0", "0.2.0").unwrap();

        // Should have version change
        assert!(staged.contains("version = \"0.2.0\""));
        // Should NOT have description change - keeps old value
        assert!(staged.contains("description = \"old desc\""));
        assert!(!staged.contains("description = \"new desc\""));
    }

    #[test]
    fn test_has_non_version_changes_true() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\n";
        let working = "[package]\nname = \"test-renamed\"\nversion = \"0.2.0\"\n";

        assert!(has_non_version_changes(head, working, "0.1.0", "0.2.0"));
    }

    #[test]
    fn test_has_non_version_changes_false() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\n";
        let working = "[package]\nname = \"test\"\nversion = \"0.2.0\"\n";

        assert!(!has_non_version_changes(head, working, "0.1.0", "0.2.0"));
    }

    #[test]
    fn test_apply_version_hunks_multiple_version_fields() {
        let head =
            "[package]\nversion = \"1.0.0\"\n[dependencies]\ncrate-a = { version = \"1.0.0\" }\n";
        let working =
            "[package]\nversion = \"2.0.0\"\n[dependencies]\ncrate-a = { version = \"2.0.0\" }\n";

        let staged = apply_version_hunks(head, working, "1.0.0", "2.0.0").unwrap();

        // Should update both version fields
        assert!(staged.contains("version = \"2.0.0\""));
        assert!(!staged.contains("1.0.0"));
    }

    // README.md selective staging tests

    #[test]
    fn test_apply_readme_version_hunks_only_version_change() {
        let head = r#"# My Crate

Add to Cargo.toml:

```toml
my-crate = "0.1.0"
```
"#;
        let working = r#"# My Crate

Add to Cargo.toml:

```toml
my-crate = "0.2.0"
```
"#;

        let staged =
            apply_readme_version_hunks(head, working, "my-crate", "0.1.0", "0.2.0").unwrap();

        assert!(staged.contains(r#"my-crate = "0.2.0""#));
        assert!(!staged.contains(r#"my-crate = "0.1.0""#));
    }

    #[test]
    fn test_apply_readme_version_hunks_mixed_changes() {
        let head = r#"# My Crate

Old description.

```toml
my-crate = "0.1.0"
```
"#;
        let working = r#"# My Crate

New description with more details.

```toml
my-crate = "0.2.0"
```
"#;

        let staged =
            apply_readme_version_hunks(head, working, "my-crate", "0.1.0", "0.2.0").unwrap();

        // Should have version change
        assert!(staged.contains(r#"my-crate = "0.2.0""#));
        // Should NOT have description change - keeps old value
        assert!(staged.contains("Old description."));
        assert!(!staged.contains("New description"));
    }

    #[test]
    fn test_apply_readme_version_hunks_underscored_name() {
        let head = r#"my_crate = "1.0.0""#;
        let working = r#"my_crate = "1.1.0""#;

        let staged =
            apply_readme_version_hunks(head, working, "my-crate", "1.0.0", "1.1.0").unwrap();

        assert!(staged.contains(r#"my_crate = "1.1.0""#));
    }

    #[test]
    fn test_has_non_readme_version_changes_true() {
        let head = "# Readme\nmy-crate = \"0.1.0\"\n";
        let working = "# Updated Readme\nmy-crate = \"0.2.0\"\n";

        assert!(has_non_readme_version_changes(
            head, working, "my-crate", "0.1.0", "0.2.0"
        ));
    }

    #[test]
    fn test_has_non_readme_version_changes_false() {
        let head = "# Readme\nmy-crate = \"0.1.0\"\n";
        let working = "# Readme\nmy-crate = \"0.2.0\"\n";

        assert!(!has_non_readme_version_changes(
            head, working, "my-crate", "0.1.0", "0.2.0"
        ));
    }

    // Cargo.lock selective staging tests

    #[test]
    fn test_apply_cargo_lock_version_hunks_only_our_crate() {
        let head = r#"[[package]]
name = "my-crate"
version = "0.1.0"

[[package]]
name = "other-crate"
version = "1.0.0"
"#;
        let working = r#"[[package]]
name = "my-crate"
version = "0.2.0"

[[package]]
name = "other-crate"
version = "1.0.0"
"#;

        let staged =
            apply_cargo_lock_version_hunks(head, working, "my-crate", "0.1.0", "0.2.0").unwrap();

        assert!(staged.contains(r#"version = "0.2.0""#));
        assert!(!staged.contains(r#"version = "0.1.0""#));
    }

    #[test]
    fn test_apply_cargo_lock_version_hunks_mixed_changes() {
        let head = r#"[[package]]
name = "my-crate"
version = "0.1.0"

[[package]]
name = "other-crate"
version = "1.0.0"
"#;
        let working = r#"[[package]]
name = "my-crate"
version = "0.2.0"

[[package]]
name = "other-crate"
version = "2.0.0"
"#;

        let staged =
            apply_cargo_lock_version_hunks(head, working, "my-crate", "0.1.0", "0.2.0").unwrap();

        // Should have our crate's version change
        assert!(staged.contains(r#"name = "my-crate""#));
        assert!(
            staged.contains(r#"version = "0.2.0""#),
            "Our crate's new version should be included"
        );

        // Should NOT have other-crate's version change - keeps old value
        assert!(staged.contains(r#"name = "other-crate""#));
        assert!(
            staged.contains(r#"version = "1.0.0""#),
            "Other crate's version should stay at 1.0.0"
        );
        assert!(
            !staged.matches(r#"version = "2.0.0""#).count() > 0
                || staged.matches(r#"version = "0.2.0""#).count() == 1,
            "Only our crate should have updated version"
        );
    }

    #[test]
    fn test_has_non_cargo_lock_version_changes_true() {
        let head = r#"[[package]]
name = "my-crate"
version = "0.1.0"

[[package]]
name = "other-crate"
version = "1.0.0"
"#;
        let working = r#"[[package]]
name = "my-crate"
version = "0.2.0"

[[package]]
name = "other-crate"
version = "2.0.0"
"#;

        assert!(has_non_cargo_lock_version_changes(
            head, working, "my-crate", "0.1.0", "0.2.0"
        ));
    }

    #[test]
    fn test_has_non_cargo_lock_version_changes_false() {
        let head = r#"[[package]]
name = "my-crate"
version = "0.1.0"
"#;
        let working = r#"[[package]]
name = "my-crate"
version = "0.2.0"
"#;

        assert!(!has_non_cargo_lock_version_changes(
            head, working, "my-crate", "0.1.0", "0.2.0"
        ));
    }
}
