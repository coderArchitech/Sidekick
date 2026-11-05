use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use secrecy::{ExposeSecret, SecretString};
use sidekick_core::SidekickResult;

use crate::{KeyStore, validate_account};

/// In-memory [`KeyStore`] for tests and `SIDEKICK_MOCK_PROVIDERS=1`.
/// Values are `SecretString`, so `Debug` output stays redacted.
#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: Mutex<HashMap<String, SecretString>>,
}

impl KeyStore for MemoryStore {
    fn set(&self, account: &str, secret: &SecretString) -> SidekickResult<()> {
        validate_account(account)?;
        let copy = SecretString::from(secret.expose_secret().to_owned());
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(account.to_owned(), copy);
        Ok(())
    }

    fn get(&self, account: &str) -> SidekickResult<Option<SecretString>> {
        validate_account(account)?;
        Ok(self
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(account)
            .map(|s| SecretString::from(s.expose_secret().to_owned())))
    }

    fn delete(&self, account: &str) -> SidekickResult<()> {
        validate_account(account)?;
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(s: &str) -> SecretString {
        SecretString::from(s.to_owned())
    }

    #[test]
    fn set_get_overwrite_delete() {
        let store = MemoryStore::default();
        assert!(store.get("anthropic").unwrap().is_none());
        store.set("anthropic", &secret("one")).unwrap();
        store.set("anthropic", &secret("two")).unwrap();
        assert_eq!(
            store.get("anthropic").unwrap().unwrap().expose_secret(),
            "two"
        );
        store.delete("anthropic").unwrap();
        assert!(store.get("anthropic").unwrap().is_none());
        // Deleting again is fine.
        store.delete("anthropic").unwrap();
    }

    #[test]
    fn providers_are_independent() {
        let store = MemoryStore::default();
        store.set("anthropic", &secret("a-key")).unwrap();
        store.set("openai", &secret("o-key")).unwrap();
        store.delete("anthropic").unwrap();
        assert_eq!(
            store.get("openai").unwrap().unwrap().expose_secret(),
            "o-key"
        );
        assert!(!store.status("anthropic").unwrap().saved);
    }

    #[test]
    fn debug_output_is_redacted() {
        let store = MemoryStore::default();
        store.set("deepgram", &secret("dg-SUPERSECRET")).unwrap();
        let debug = format!("{store:?}");
        assert!(!debug.contains("SUPERSECRET"), "{debug}");
    }

    #[test]
    fn rejects_invalid_account() {
        let store = MemoryStore::default();
        assert!(store.set("Bad Name", &secret("x")).is_err());
    }
}
