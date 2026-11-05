//! Round trip against the real OS keychain. Ignored by default because it
//! needs an unlocked keychain; CI runs it on Windows and macOS with `--ignored`.

use sidekick_secrets::{ExposeSecret, KeyStore, KeychainStore, SecretString};

#[test]
#[ignore = "touches the real OS keychain"]
fn real_keychain_round_trip() {
    let store = KeychainStore::with_service("sidekick-ci-test");
    let account = "roundtrip-test";
    store.delete(account).unwrap();

    store
        .set(account, &SecretString::from("first-1234".to_owned()))
        .unwrap();
    store
        .set(account, &SecretString::from("second-5678".to_owned()))
        .unwrap();
    assert_eq!(
        store.get(account).unwrap().unwrap().expose_secret(),
        "second-5678"
    );
    assert_eq!(
        store.status(account).unwrap().last4.as_deref(),
        Some("5678")
    );

    store.delete(account).unwrap();
    assert!(store.get(account).unwrap().is_none());
}
