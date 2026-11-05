use sidekick_core::{SidekickError, SidekickResult};
use sidekick_secrets::{DB_KEY_ACCOUNT, KeyStore, SecretString};

/// Returns the database key from the keychain, creating a random 256-bit key on
/// first run (tech doc 4). The key is 64 hex characters.
pub fn database_key(keys: &dyn KeyStore) -> SidekickResult<SecretString> {
    if let Some(existing) = keys.get(DB_KEY_ACCOUNT)? {
        return Ok(existing);
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
        let first = database_key(&keys).unwrap();
        let second = database_key(&keys).unwrap();
        assert_eq!(first.expose_secret(), second.expose_secret());
        assert_eq!(first.expose_secret().len(), 64);
        assert!(first.expose_secret().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn keys_are_random() {
        let a = database_key(&MemoryStore::default()).unwrap();
        let b = database_key(&MemoryStore::default()).unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
    }
}
