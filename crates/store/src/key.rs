use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{DB_KEY_ACCOUNT, KeyStore, SecretString};

/// Returns the database key from the keychain (tech doc 4). The key is 64 hex
/// characters.
///
/// A new random 256-bit key is created only when there is no database yet.
/// If the database exists but its key is gone (deleted, or the keychain was
/// reset), this fails with `KEY_MISSING` instead: a new key can't unlock the
/// old file, and saving one would replace any chance of restoring the original
/// key from a keychain backup.
pub fn database_key(keys: &dyn KeyStore, database_exists: bool) -> SidekickResult<SecretString> {
    if let Some(existing) = keys.get(DB_KEY_ACCOUNT)? {
        return Ok(existing);
    }
    if database_exists {
        return Err(SidekickError::new(
            ErrorCode::KeyMissing,
            "The key that unlocks Sidekick's data is missing from the keychain.",
        ));
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| SidekickError::internal("Couldn't generate a database key."))?;
    let key = SecretString::from(hex::encode(bytes));
    bytes.fill(0);
    keys.set(DB_KEY_ACCOUNT, &key)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_secrets::{ExposeSecret, MemoryStore};

    #[test]
    fn creates_once_then_reuses() {
        let keys = MemoryStore::default();
        let first = database_key(&keys, false).unwrap();
        // The database now exists; the saved key is reused.
        let second = database_key(&keys, true).unwrap();
        assert_eq!(first.expose_secret(), second.expose_secret());
        assert_eq!(first.expose_secret().len(), 64);
        assert!(first.expose_secret().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn keys_are_random() {
        let a = database_key(&MemoryStore::default(), false).unwrap();
        let b = database_key(&MemoryStore::default(), false).unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
    }

    #[test]
    fn never_replaces_the_key_of_an_existing_database() {
        let keys = MemoryStore::default();
        let err = database_key(&keys, true).unwrap_err();
        assert_eq!(err.code, ErrorCode::KeyMissing);
        // Nothing was saved: a restored keychain entry can still be used.
        assert!(!keys.status(DB_KEY_ACCOUNT).unwrap().saved);
    }
}
