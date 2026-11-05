use secrecy::{ExposeSecret, SecretString};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

use crate::{KeyStore, SERVICE, validate_account};

/// [`KeyStore`] backed by the OS keychain: macOS Keychain, Windows Credential
/// Manager, Linux kernel keyutils (development only).
#[derive(Debug, Clone)]
pub struct KeychainStore {
    service: String,
}

impl Default for KeychainStore {
    fn default() -> Self {
        Self {
            service: SERVICE.to_owned(),
        }
    }
}

impl KeychainStore {
    /// A store under a different service name, so tests never touch real keys.
    #[must_use]
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, account: &str) -> SidekickResult<keyring::Entry> {
        validate_account(account)?;
        keyring::Entry::new(&self.service, account).map_err(map_error)
    }
}

impl KeyStore for KeychainStore {
    fn set(&self, account: &str, secret: &SecretString) -> SidekickResult<()> {
        self.entry(account)?
            .set_password(secret.expose_secret())
            .map_err(map_error)
    }

    fn get(&self, account: &str) -> SidekickResult<Option<SecretString>> {
        match self.entry(account)?.get_password() {
            Ok(value) => Ok(Some(SecretString::from(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_error(e)),
        }
    }

    fn delete(&self, account: &str) -> SidekickResult<()> {
        match self.entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_error(e)),
        }
    }
}

/// Maps keychain errors to stable codes. Messages never include the secret.
fn map_error(err: keyring::Error) -> SidekickError {
    match err {
        keyring::Error::NoStorageAccess(_) => SidekickError::new(
            ErrorCode::PermissionDenied,
            "Sidekick couldn't access the OS keychain. Unlock it or allow access, then try again.",
        ),
        keyring::Error::PlatformFailure(_) => {
            SidekickError::new(ErrorCode::Internal, "The OS keychain reported an error.")
        }
        _ => SidekickError::new(ErrorCode::Internal, "Keychain operation failed."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_storage_access_maps_to_permission_denied() {
        let err = map_error(keyring::Error::NoStorageAccess("locked".into()));
        assert_eq!(err.code, ErrorCode::PermissionDenied);
    }

    #[test]
    fn invalid_account_rejected_before_touching_keychain() {
        let store = KeychainStore::with_service("sidekick-test");
        assert!(store.get("NOT valid").is_err());
    }
}
