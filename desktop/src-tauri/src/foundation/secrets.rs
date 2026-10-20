//! Thin wrapper over the OS keychain (Windows Credential Manager / Linux Secret Service).
//! Secrets must never be stored in SQLite, settings or logs (Rules.md §4).

use super::{
    errors::{PrekuError, Result},
    validate,
};

const SERVICE: &str = "app.preku.desktop";
/// Set to a folder to run Preku as a brand-new user for testing: data goes there, and
/// secrets go to a separate keychain service so the real ones (the phone pairing key,
/// calendar links, tokens) are never read or replaced.
pub const TEST_PROFILE_ENV: &str = "PREKU_DATA_DIR";

fn service() -> &'static str {
    if std::env::var_os(TEST_PROFILE_ENV).is_some() {
        "app.preku.desktop.test-profile"
    } else {
        SERVICE
    }
}

fn entry(key: &str) -> Result<keyring::Entry> {
    validate::key("Secret", key)?;
    keyring::Entry::new(service(), key).map_err(secret_error)
}

fn secret_error(err: keyring::Error) -> PrekuError {
    PrekuError::Secret(err.to_string())
}

/// Returns the secret, or `None` if it was never stored.
pub fn get(key: &str) -> Result<Option<String>> {
    match entry(key)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(secret_error(err)),
    }
}

pub fn set(key: &str, value: &str) -> Result<()> {
    entry(key)?.set_password(value).map_err(secret_error)
}

/// Deletes the secret. Deleting a missing secret is not an error.
pub fn delete(key: &str) -> Result<()> {
    match entry(key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(secret_error(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_key_before_touching_keychain() {
        assert!(matches!(get("Bad Key"), Err(PrekuError::InvalidInput(_))));
    }

    /// Needs a real OS keychain, which CI runners don't have: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn round_trip_with_os_keychain() {
        let key = "test.round-trip";
        set(key, "s3cret").unwrap();
        assert_eq!(get(key).unwrap().as_deref(), Some("s3cret"));
        delete(key).unwrap();
        assert_eq!(get(key).unwrap(), None);
    }
}
