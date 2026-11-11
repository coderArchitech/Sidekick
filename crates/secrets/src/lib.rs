//! Secret storage for Sidekick (tech doc 11.4, ADR-008).
//!
//! - Keys live only in the OS keychain under service `sidekick`, account `<name>`
//!   (for example `anthropic` or `db-key`).
//! - Keys are held as [`SecretString`] (zeroed on drop, redacted in `Debug`).
//! - Callers outside the gateway only ever see [`KeyStatus`], which holds at most
//!   the last 4 characters.
//!
//! [`KeyStore`] is the seam: production uses [`KeychainStore`], tests and the
//! mock-provider mode use [`MemoryStore`].

mod cached;
mod keychain;
mod memory;

pub use cached::CachedStore;
pub use keychain::KeychainStore;
pub use memory::MemoryStore;
pub use secrecy::{ExposeSecret, SecretString};

use serde::Serialize;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

/// Keychain service name shared by every Sidekick secret.
pub const SERVICE: &str = "sidekick";

/// Account name of the database encryption key (tech doc 4).
pub const DB_KEY_ACCOUNT: &str = "db-key";

/// Storage for named secrets. Implementations must never log secret values.
pub trait KeyStore: Send + Sync {
    /// Saves or replaces the secret for `account`.
    fn set(&self, account: &str, secret: &SecretString) -> SidekickResult<()>;
    /// Returns the secret, or `None` if nothing is saved.
    fn get(&self, account: &str) -> SidekickResult<Option<SecretString>>;
    /// Removes the secret. Removing a missing secret is not an error.
    fn delete(&self, account: &str) -> SidekickResult<()>;

    /// Whether a secret is saved, with only its last 4 characters (KEY-5).
    fn status(&self, account: &str) -> SidekickResult<KeyStatus> {
        Ok(match self.get(account)? {
            Some(secret) => KeyStatus {
                saved: true,
                last4: Some(last4(&secret)),
            },
            None => KeyStatus {
                saved: false,
                last4: None,
            },
        })
    }
}

/// What the UI may know about a saved key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub saved: bool,
    /// Last 4 characters, or fewer if the key is shorter. Never more.
    pub last4: Option<String>,
}

/// Last 4 characters of a secret (by `char`, so multi-byte input can't panic).
#[must_use]
pub fn last4(secret: &SecretString) -> String {
    let chars: Vec<char> = secret.expose_secret().chars().collect();
    let start = chars.len().saturating_sub(4);
    chars[start..].iter().collect()
}

/// Account names are fixed identifiers like `anthropic` or `db-key`. Rejecting
/// anything else stops user input from addressing unrelated keychain entries.
pub fn validate_account(account: &str) -> SidekickResult<()> {
    let ok = !account.is_empty()
        && account.len() <= 64
        && account
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(SidekickError::new(
            ErrorCode::Internal,
            "Invalid secret name",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(s: &str) -> SecretString {
        SecretString::from(s.to_owned())
    }

    #[test]
    fn last4_shows_only_the_end() {
        assert_eq!(last4(&secret("sk-ant-1234567890abcd")), "abcd");
    }

    #[test]
    fn last4_handles_short_and_multibyte_keys() {
        assert_eq!(last4(&secret("ab")), "ab");
        assert_eq!(last4(&secret("")), "");
        assert_eq!(last4(&secret("ключ-éèàü")), "éèàü");
    }

    #[test]
    fn account_names_are_restricted() {
        assert!(validate_account("anthropic").is_ok());
        assert!(validate_account("db-key").is_ok());
        assert!(validate_account("").is_err());
        assert!(validate_account("Anthropic").is_err());
        assert!(validate_account("../etc").is_err());
        assert!(validate_account(&"a".repeat(65)).is_err());
    }

    #[test]
    fn key_status_never_serialises_more_than_last4() {
        let store = MemoryStore::default();
        store
            .set("openai", &secret("sk-proj-SECRETVALUE9876"))
            .unwrap();
        let json = serde_json::to_string(&store.status("openai").unwrap()).unwrap();
        assert_eq!(json, r#"{"saved":true,"last4":"9876"}"#);
        assert!(!json.contains("SECRETVALUE"));
    }
}
