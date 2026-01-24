//! Commit signing for version bump commits.
//!
//! This module provides GPG and SSH commit signing using pure Rust crates.
//! It reads signing configuration from git config and handles both signing
//! formats transparently.
//!
//! # Supported Formats
//!
//! - **SSH**: Default format when signing is enabled. Uses ssh-agent or key
//!   files.
//! - **GPG/OpenPGP**: Currently not implemented (requires external `gpg`).
//!
//! # Git Config Keys
//!
//! | Key | Type | Default | Description |
//! |-----|------|---------|-------------|
//! | `commit.gpgsign` | bool | false | Enable commit signing |
//! | `gpg.format` | string | "ssh" | Format: "openpgp" or "ssh" |
//! | `user.signingkey` | string | - | Key ID (GPG) or path (SSH) |
//!
//! # Error Handling
//!
//! | Scenario | Behavior |
//! |----------|----------|
//! | Signing not configured | Silent - unsigned commit |
//! | SSH agent unavailable | Try key file, then fail |
//! | GPG signing requested | Error - use SSH instead |
//! | Key not found | **Error** - fail the bump |
//! | Signing failed | **Error** - fail the bump |

use std::path::Path;

use anyhow::{
    Context,
    Result,
};
use bstr::ByteSlice;

/// The signing format to use.
///
/// SSH is the default (differs from Git's GPG default).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SigningFormat {
    /// GPG/OpenPGP signing via gpg-agent.
    Gpg,
    /// SSH signing via ssh-agent or key file.
    #[default]
    Ssh,
}

/// Configuration for commit signing.
#[derive(Debug, Clone, Default)]
pub struct SigningConfig {
    /// Whether signing is enabled (from `commit.gpgsign`).
    pub enabled: bool,
    /// The signing format (from `gpg.format`).
    pub format: SigningFormat,
    /// The signing key identifier (from `user.signingkey`).
    /// For SSH: path to public key file or key fingerprint.
    /// For GPG: key ID or email.
    pub signing_key: Option<String>,
}

/// Read signing configuration from git repository config.
///
/// Reads the following config keys:
/// - `commit.gpgsign`: Whether to sign commits (bool)
/// - `gpg.format`: Signing format, "openpgp" or "ssh" (default: "ssh")
/// - `user.signingkey`: The key to use for signing
///
/// # Arguments
///
/// * `repo` - The git repository to read config from
///
/// # Returns
///
/// Returns the signing configuration. If signing is not configured,
/// returns a config with `enabled: false`.
pub fn read_signing_config(repo: &gix::Repository) -> SigningConfig {
    let config = repo.config_snapshot();

    // Read commit.gpgsign (bool, default false)
    let enabled = config.boolean("commit.gpgsign").unwrap_or(false);

    // Read gpg.format (string, default "ssh" for us)
    let format = config
        .string("gpg.format")
        .map(|s| {
            let s_str = s.to_str_lossy();
            match s_str.as_ref() {
                "openpgp" => SigningFormat::Gpg,
                "ssh" => SigningFormat::Ssh,
                _ => SigningFormat::default(),
            }
        })
        .unwrap_or_default();

    // Read user.signingkey
    let signing_key = config.string("user.signingkey").map(|s| s.to_string());

    SigningConfig {
        enabled,
        format,
        signing_key,
    }
}

/// Sign a commit payload using the configured signing method.
///
/// # Arguments
///
/// * `config` - The signing configuration
/// * `payload` - The commit payload to sign (raw commit data without gpgsig
///   header)
///
/// # Returns
///
/// - `Ok(Some(signature))` - Signed successfully
/// - `Ok(None)` - Signing not enabled/configured
/// - `Err(e)` - Signing was configured but failed critically (missing key)
///
/// # Signature Format
///
/// The returned signature is formatted for inclusion in a git commit
/// `gpgsig` header. Each line after the first is prefixed with a space.
pub fn sign_commit_payload(config: &SigningConfig, payload: &[u8]) -> Result<Option<Vec<u8>>> {
    if !config.enabled {
        return Ok(None);
    }

    let signing_key = match &config.signing_key {
        Some(key) => key,
        None => {
            // No signing key configured - this is an error if signing is
            // enabled
            anyhow::bail!(
                "Commit signing is enabled but no signing key is configured.\n\
                 Please set user.signingkey in git config:\n  \
                 git config user.signingkey <key-path-or-id>"
            );
        }
    };

    let signature = match config.format {
        SigningFormat::Ssh => sign_with_ssh(signing_key, payload)?,
        SigningFormat::Gpg => {
            anyhow::bail!(
                "GPG signing is not yet implemented in cargo-version-info.\n\
                 Please use SSH signing instead:\n  \
                 git config gpg.format ssh\n  \
                 git config user.signingkey ~/.ssh/id_ed25519.pub"
            );
        }
    };

    Ok(Some(format_signature_for_header(&signature)))
}

/// Sign payload using SSH (agent or file).
fn sign_with_ssh(signing_key: &str, payload: &[u8]) -> Result<Vec<u8>> {
    // Try SSH agent first
    match sign_with_ssh_agent(signing_key, payload) {
        Ok(sig) => return Ok(sig),
        Err(agent_err) => {
            eprintln!(
                "SSH agent signing failed ({}), trying key file...",
                agent_err
            );
        }
    }

    // Fall back to key file
    sign_with_ssh_file(signing_key, payload)
}

/// Sign using SSH agent.
///
/// Connects to the SSH agent via SSH_AUTH_SOCK and requests a signature.
fn sign_with_ssh_agent(signing_key: &str, payload: &[u8]) -> Result<Vec<u8>> {
    use ssh_agent_client_rs::Client;
    use ssh_key::{
        HashAlg,
        SshSig,
    };

    // Get the SSH_AUTH_SOCK path
    let auth_sock =
        std::env::var("SSH_AUTH_SOCK").context("SSH_AUTH_SOCK environment variable not set")?;

    // Connect to the agent
    let mut client =
        Client::connect(Path::new(&auth_sock)).context("Failed to connect to SSH agent")?;

    // List identities to find our key
    let identities = client
        .list_all_identities()
        .context("Failed to list SSH agent identities")?;

    // Find matching key by path, fingerprint, or comment
    let (identity_idx, public_key) = find_matching_identity(&identities, signing_key)?;

    // Sign the data using the agent
    let identity = identities.into_iter().nth(identity_idx).unwrap();
    let signature = client
        .sign(identity, payload)
        .context("SSH agent signing failed")?;

    // Create an SshSig structure (the format git expects)
    let ssh_sig = SshSig::new(
        public_key.key_data().clone(),
        "git",
        HashAlg::Sha512,
        signature,
    )
    .context("Failed to create SSH signature")?;

    // Encode as PEM
    let pem = ssh_sig
        .to_pem(ssh_key::LineEnding::LF)
        .context("Failed to encode SSH signature as PEM")?;

    Ok(pem.into_bytes())
}

/// Find a matching identity from the SSH agent.
///
/// Returns the index of the identity and the public key for creating SshSig.
fn find_matching_identity(
    identities: &[ssh_agent_client_rs::Identity<'static>],
    signing_key: &str,
) -> Result<(usize, ssh_key::PublicKey)> {
    use ssh_agent_client_rs::Identity;
    use ssh_key::PublicKey;

    // If signing_key is a path to a public key file, read it
    let target_fingerprint =
        if signing_key.ends_with(".pub") || signing_key.contains('/') || signing_key.contains('\\')
        {
            // Try to read as a public key file
            let pub_key_path = if signing_key.ends_with(".pub") {
                signing_key.to_string()
            } else {
                format!("{}.pub", signing_key)
            };

            if let Ok(pub_key) = PublicKey::read_openssh_file(Path::new(&pub_key_path)) {
                Some(pub_key.fingerprint(ssh_key::HashAlg::Sha256))
            } else {
                None
            }
        } else {
            // Might be a fingerprint directly
            None
        };

    for (idx, identity) in identities.iter().enumerate() {
        // Extract public key from identity
        // Identity is an enum containing Box<Cow<'_, PublicKey/Certificate>>
        let public_key: PublicKey = match identity {
            Identity::PublicKey(pk) => pk.as_ref().clone().into_owned(),
            Identity::Certificate(cert) => {
                // Build a public key from the certificate
                let cert_ref: &ssh_key::Certificate = &cert.as_ref().clone();
                PublicKey::new(cert_ref.public_key().clone(), "")
            }
        };

        let fingerprint = public_key.fingerprint(ssh_key::HashAlg::Sha256);

        // Match by fingerprint
        if let Some(ref target_fp) = target_fingerprint
            && fingerprint == *target_fp
        {
            return Ok((idx, public_key));
        }

        // Match by fingerprint string (SHA256:...)
        let fp_str = fingerprint.to_string();
        if fp_str.contains(signing_key) || signing_key.contains(&fp_str) {
            return Ok((idx, public_key));
        }

        // Match by comment
        let comment = public_key.comment();
        if !comment.is_empty() && (comment.contains(signing_key) || signing_key.contains(comment)) {
            return Ok((idx, public_key));
        }
    }

    // Build list of available keys for error message
    let available_keys: Vec<String> = identities
        .iter()
        .map(|identity| {
            let pk: PublicKey = match identity {
                Identity::PublicKey(pk) => pk.as_ref().clone().into_owned(),
                Identity::Certificate(cert) => {
                    let cert_ref: &ssh_key::Certificate = &cert.as_ref().clone();
                    PublicKey::new(cert_ref.public_key().clone(), "")
                }
            };
            let comment = pk.comment();
            if comment.is_empty() {
                pk.fingerprint(ssh_key::HashAlg::Sha256).to_string()
            } else {
                comment.to_string()
            }
        })
        .collect();

    anyhow::bail!(
        "No matching SSH key found in agent for '{}'.\n\
         Available keys: {:?}",
        signing_key,
        available_keys
    );
}

/// Sign using SSH key file directly.
///
/// This is a fallback when SSH agent is not available.
fn sign_with_ssh_file(signing_key: &str, payload: &[u8]) -> Result<Vec<u8>> {
    use ssh_key::{
        HashAlg,
        PrivateKey,
        SshSig,
    };

    // Determine the private key path
    let private_key_path = if signing_key.ends_with(".pub") {
        // Remove .pub extension to get private key path
        signing_key.trim_end_matches(".pub").to_string()
    } else {
        signing_key.to_string()
    };

    // Try to load the private key
    let private_key = PrivateKey::read_openssh_file(Path::new(&private_key_path))
        .with_context(|| format!("Failed to read SSH private key from '{}'", private_key_path))?;

    // Check if the key is encrypted
    if private_key.is_encrypted() {
        anyhow::bail!(
            "SSH key '{}' is encrypted. Please use ssh-agent or an unencrypted key.\n\
             Add the key to ssh-agent with: ssh-add {}",
            private_key_path,
            private_key_path
        );
    }

    // Create the signature
    let ssh_sig = SshSig::sign(&private_key, "git", HashAlg::Sha512, payload)
        .context("Failed to create SSH signature")?;

    // Encode as PEM
    let pem = ssh_sig
        .to_pem(ssh_key::LineEnding::LF)
        .context("Failed to encode SSH signature as PEM")?;

    Ok(pem.into_bytes())
}

/// Format a signature for inclusion in a git commit gpgsig header.
///
/// Git expects the signature to be formatted with each line after the first
/// indented with a space character. This handles the multi-line nature of
/// PEM-formatted signatures.
///
/// # Example Output
///
/// ```text
/// -----BEGIN SSH SIGNATURE-----
///  <base64 line 1>
///  <base64 line 2>
///  ...
///  -----END SSH SIGNATURE-----
/// ```
fn format_signature_for_header(signature: &[u8]) -> Vec<u8> {
    let sig_str = String::from_utf8_lossy(signature);
    let mut result = Vec::new();
    let mut first_line = true;

    for line in sig_str.lines() {
        if first_line {
            result.extend_from_slice(line.as_bytes());
            first_line = false;
        } else {
            result.push(b'\n');
            result.push(b' ');
            result.extend_from_slice(line.as_bytes());
        }
    }

    result
}

/// Build the commit payload that needs to be signed.
///
/// This creates the raw commit object content that git signs. The format is:
/// ```text
/// tree <tree-sha>
/// parent <parent-sha>
/// author <name> <email> <timestamp> <offset>
/// committer <name> <email> <timestamp> <offset>
///
/// <commit message>
/// ```
///
/// Note: The gpgsig header is NOT included in the payload - it's added
/// separately after signing.
pub fn build_commit_payload(
    tree_id: &gix::ObjectId,
    parent_id: gix::Id,
    author: &gix::actor::Signature,
    committer: &gix::actor::Signature,
    message: &str,
) -> Vec<u8> {
    let mut payload = Vec::new();

    // tree <sha>
    payload.extend_from_slice(b"tree ");
    payload.extend_from_slice(tree_id.to_string().as_bytes());
    payload.push(b'\n');

    // parent <sha>
    payload.extend_from_slice(b"parent ");
    payload.extend_from_slice(parent_id.to_string().as_bytes());
    payload.push(b'\n');

    // author <signature>
    payload.extend_from_slice(b"author ");
    write_signature(&mut payload, author);
    payload.push(b'\n');

    // committer <signature>
    payload.extend_from_slice(b"committer ");
    write_signature(&mut payload, committer);
    payload.push(b'\n');

    // blank line before message
    payload.push(b'\n');

    // commit message
    payload.extend_from_slice(message.as_bytes());

    payload
}

/// Write a git signature (author/committer) to the payload buffer.
fn write_signature(buf: &mut Vec<u8>, sig: &gix::actor::Signature) {
    // Format: Name <email> timestamp offset
    buf.extend_from_slice(&sig.name);
    buf.extend_from_slice(b" <");
    buf.extend_from_slice(&sig.email);
    buf.extend_from_slice(b"> ");
    buf.extend_from_slice(sig.time.seconds.to_string().as_bytes());
    buf.push(b' ');

    // Format offset as +HHMM or -HHMM
    let offset_minutes = sig.time.offset;
    let sign = if offset_minutes >= 0 { '+' } else { '-' };
    let abs_offset = offset_minutes.abs();
    let hours = abs_offset / 60;
    let minutes = abs_offset % 60;
    buf.extend_from_slice(format!("{}{:02}{:02}", sign, hours, minutes).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_format_default() {
        assert_eq!(SigningFormat::default(), SigningFormat::Ssh);
    }

    #[test]
    fn test_signing_config_default() {
        let config = SigningConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.format, SigningFormat::Ssh);
        assert!(config.signing_key.is_none());
    }

    #[test]
    fn test_format_signature_for_header() {
        let signature = b"-----BEGIN SSH SIGNATURE-----\nline1\nline2\n-----END SSH SIGNATURE-----";
        let formatted = format_signature_for_header(signature);
        let result = String::from_utf8_lossy(&formatted);

        assert!(result.starts_with("-----BEGIN SSH SIGNATURE-----"));
        assert!(result.contains("\n line1"));
        assert!(result.contains("\n line2"));
    }

    #[test]
    fn test_write_signature_positive_offset() {
        let mut buf = Vec::new();
        let sig = gix::actor::Signature {
            name: "Test User".into(),
            email: "test@example.com".into(),
            time: gix::date::Time {
                seconds: 1700000000,
                offset: 60, // UTC+1
            },
        };

        write_signature(&mut buf, &sig);
        let result = String::from_utf8_lossy(&buf);

        assert!(result.contains("Test User <test@example.com>"));
        assert!(result.contains("1700000000"));
        assert!(result.contains("+0100"));
    }

    #[test]
    fn test_write_signature_negative_offset() {
        let mut buf = Vec::new();
        let sig = gix::actor::Signature {
            name: "Test User".into(),
            email: "test@example.com".into(),
            time: gix::date::Time {
                seconds: 1700000000,
                offset: -300, // UTC-5
            },
        };

        write_signature(&mut buf, &sig);
        let result = String::from_utf8_lossy(&buf);

        assert!(result.contains("Test User <test@example.com>"));
        assert!(result.contains("1700000000"));
        assert!(result.contains("-0500"));
    }

    #[test]
    fn test_sign_disabled() {
        let config = SigningConfig::default();
        let result = sign_commit_payload(&config, b"test payload").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_sign_enabled_no_key() {
        let config = SigningConfig {
            enabled: true,
            format: SigningFormat::Ssh,
            signing_key: None,
        };
        let result = sign_commit_payload(&config, b"test payload");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("signing key"));
    }

    #[test]
    fn test_sign_gpg_not_implemented() {
        let config = SigningConfig {
            enabled: true,
            format: SigningFormat::Gpg,
            signing_key: Some("ABCD1234".to_string()),
        };
        let result = sign_commit_payload(&config, b"test payload");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented")
        );
    }
}
