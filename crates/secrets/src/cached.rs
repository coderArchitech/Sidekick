use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use secrecy::SecretString;
use sidekick_core::SidekickResult;

use crate::KeyStore;

/// Remembers what another store returned, for the life of the app.
///
/// Every read of the macOS keychain by an app it doesn't recognise (each new
/// local build) can show a password prompt, once per key. Without this, the
/// Settings window and session options read every provider's key each time
/// they open. With it, each key is read at most once per run. Saving or
/// removing a key updates the cache, so it never goes stale within the app.
/// A failed read is not cached, so the next attempt asks again.
pub struct CachedStore<S> {
    inner: S,
    cache: Mutex<HashMap<String, Option<SecretString>>>,
}

impl<S> CachedStore<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, HashMap<String, Option<SecretString>>> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<S: KeyStore> KeyStore for CachedStore<S> {
    fn set(&self, account: &str, secret: &SecretString) -> SidekickResult<()> {
        self.inner.set(account, secret)?;
        self.cache()
            .insert(account.to_owned(), Some(secret.clone()));
        Ok(())
    }

    fn get(&self, account: &str) -> SidekickResult<Option<SecretString>> {
        if let Some(cached) = self.cache().get(account) {
            return Ok(cached.clone());
        }
        let value = self.inner.get(account)?;
        self.cache().insert(account.to_owned(), value.clone());
        Ok(value)
    }

    fn delete(&self, account: &str) -> SidekickResult<()> {
        self.inner.delete(account)?;
        self.cache().insert(account.to_owned(), None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use secrecy::ExposeSecret;
    use sidekick_core::{ErrorCode, SidekickError};

    use super::*;
    use crate::MemoryStore;

    /// Counts reads, and can fail them, like a keychain that prompts.
    #[derive(Default)]
    struct Counting {
        store: MemoryStore,
        reads: AtomicUsize,
        fail: std::sync::atomic::AtomicBool,
    }

    impl KeyStore for &Counting {
        fn set(&self, a: &str, s: &SecretString) -> SidekickResult<()> {
            self.store.set(a, s)
        }
        fn get(&self, a: &str) -> SidekickResult<Option<SecretString>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(SidekickError::new(ErrorCode::PermissionDenied, "denied"));
            }
            self.store.get(a)
        }
        fn delete(&self, a: &str) -> SidekickResult<()> {
            self.store.delete(a)
        }
    }

    fn key(s: &str) -> SecretString {
        SecretString::from(s.to_owned())
    }

    #[test]
    fn each_key_is_read_from_the_keychain_once() {
        let inner = Counting::default();
        inner.store.set("anthropic", &key("sk-ant-1234")).unwrap();
        let cached = CachedStore::new(&inner);
        for _ in 0..5 {
            assert!(cached.status("anthropic").unwrap().saved);
            assert!(!cached.status("openai").unwrap().saved);
        }
        assert_eq!(inner.reads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn saving_and_removing_keep_the_cache_current() {
        let inner = Counting::default();
        let cached = CachedStore::new(&inner);
        assert!(cached.get("deepgram").unwrap().is_none());
        cached.set("deepgram", &key("dg-5678")).unwrap();
        assert_eq!(
            cached.get("deepgram").unwrap().unwrap().expose_secret(),
            "dg-5678"
        );
        cached.delete("deepgram").unwrap();
        assert!(cached.get("deepgram").unwrap().is_none());
        assert!(inner.store.get("deepgram").unwrap().is_none());
        assert_eq!(inner.reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_failed_read_is_tried_again_next_time() {
        let inner = Counting::default();
        inner.store.set("anthropic", &key("sk-ant-1234")).unwrap();
        let cached = CachedStore::new(&inner);
        inner.fail.store(true, Ordering::SeqCst);
        assert!(cached.get("anthropic").is_err());
        inner.fail.store(false, Ordering::SeqCst);
        assert!(cached.get("anthropic").unwrap().is_some());
        assert_eq!(inner.reads.load(Ordering::SeqCst), 2);
    }
}
