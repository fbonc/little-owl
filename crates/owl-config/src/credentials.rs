use owl_provider::ProviderId;

use crate::{Error, Result};

const DEFAULT_KEYCHAIN_SERVICE: &str = "little-owl";

pub trait CredentialStore: Send + Sync {
    fn get(&self, provider: &ProviderId) -> Result<Option<String>>;

    fn set(&self, provider: &ProviderId, api_key: &str) -> Result<()>;

    fn remove(&self, provider: &ProviderId) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct KeychainCredentialStore {
    service: String,
}

impl KeychainCredentialStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl Default for KeychainCredentialStore {
    fn default() -> Self {
        Self::new(DEFAULT_KEYCHAIN_SERVICE)
    }
}

#[cfg(target_os = "macos")]
impl CredentialStore for KeychainCredentialStore {
    fn get(&self, provider: &ProviderId) -> Result<Option<String>> {
        use security_framework::passwords::{PasswordOptions, generic_password};
        use security_framework_sys::base::errSecItemNotFound;

        let options = PasswordOptions::new_generic_password(&self.service, provider.as_str());
        match generic_password(options) {
            Ok(password) => String::from_utf8(password)
                .map(Some)
                .map_err(|error| Error::Credential(error.to_string())),
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(error) => Err(Error::Credential(error.to_string())),
        }
    }

    fn set(&self, provider: &ProviderId, api_key: &str) -> Result<()> {
        security_framework::passwords::set_generic_password(
            &self.service,
            provider.as_str(),
            api_key.as_bytes(),
        )
        .map_err(|error| Error::Credential(error.to_string()))
    }

    fn remove(&self, provider: &ProviderId) -> Result<()> {
        use security_framework_sys::base::errSecItemNotFound;

        match security_framework::passwords::delete_generic_password(
            &self.service,
            provider.as_str(),
        ) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(error) => Err(Error::Credential(error.to_string())),
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl CredentialStore for KeychainCredentialStore {
    fn get(&self, _provider: &ProviderId) -> Result<Option<String>> {
        Err(Error::Credential(
            "Keychain credential storage is only supported on macOS".into(),
        ))
    }

    fn set(&self, _provider: &ProviderId, _api_key: &str) -> Result<()> {
        Err(Error::Credential(
            "Keychain credential storage is only supported on macOS".into(),
        ))
    }

    fn remove(&self, _provider: &ProviderId) -> Result<()> {
        Err(Error::Credential(
            "Keychain credential storage is only supported on macOS".into(),
        ))
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads and writes the user's macOS Keychain"]
    fn live_keychain_round_trip() {
        let store = KeychainCredentialStore::new(format!("little-owl-test-{}", std::process::id()));
        let provider = ProviderId::new("test-provider");

        store.remove(&provider).expect("clear previous credential");
        assert_eq!(store.get(&provider).expect("read missing credential"), None);

        store.set(&provider, "test-secret").expect("set credential");
        assert_eq!(
            store.get(&provider).expect("read credential").as_deref(),
            Some("test-secret")
        );

        store.remove(&provider).expect("remove credential");
        assert_eq!(store.get(&provider).expect("read removed credential"), None);
    }
}
