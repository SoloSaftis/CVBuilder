//! Optional AI help for CVBuilder with the person's own API key: Claude, ChatGPT, Gemini or
//! Grok. Keys are kept in the operating system's keychain (Windows Credential Manager, macOS
//! Keychain, Secret Service on Linux) through `craft-ai-native`, never in the project file.
pub mod providers;
pub mod tasks;

use craft_ai_core::{SecretRef, SecretString};
use craft_ai_native::secrets::{KeychainStore, SecretStore};

pub use providers::{AiFailure, Endpoint, SERVICES, Service, test_connection};

/// Saved API keys. Falls back to keeping the key only while the app is open when the computer
/// has no keychain (some Linux setups); the app says so.
pub struct Keys {
    store: KeychainStore,
    session: std::collections::HashMap<Service, SecretString>,
}

impl Default for Keys {
    fn default() -> Self {
        Keys { store: KeychainStore::with_service("cvbuilder"), session: Default::default() }
    }
}

impl Keys {
    pub fn keychain_available() -> bool {
        KeychainStore::is_available()
    }

    fn reference(service: Service) -> Option<SecretRef> {
        SecretRef::new(service.secret_name()).ok()
    }

    pub fn get(&self, service: Service) -> Option<SecretString> {
        if let Some(key) = self.session.get(&service) {
            return Some(key.clone());
        }
        let reference = Self::reference(service)?;
        self.store.get(&reference).ok().flatten()
    }

    /// Saves the key; returns false when it could only be kept for this session.
    pub fn set(&mut self, service: Service, key: &str) -> bool {
        let secret = SecretString::new(key.trim());
        let saved = Self::reference(service).is_some_and(|r| self.store.set(&r, &secret).is_ok());
        if saved {
            self.session.remove(&service);
        } else {
            self.session.insert(service, secret);
        }
        saved
    }

    pub fn forget(&mut self, service: Service) {
        self.session.remove(&service);
        if let Some(reference) = Self::reference(service) {
            let _ = self.store.delete(&reference);
        }
    }
}
