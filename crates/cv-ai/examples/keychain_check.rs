//! Round-trips a dummy secret through the OS keychain under a throwaway service name.
use craft_ai_core::{SecretRef, SecretString};
use craft_ai_native::secrets::{KeychainStore, SecretStore};

fn main() {
    let store = KeychainStore::with_service("cvbuilder-selftest");
    let key = SecretRef::new("selftest").expect("ref");
    println!("keychain available: {}", KeychainStore::is_available());
    store.set(&key, &SecretString::new("dummy-value")).expect("set");
    let back = store.get(&key).expect("get").map(|s| s.expose() == "dummy-value");
    store.delete(&key).expect("delete");
    let gone = store.get(&key).expect("get after delete").is_none();
    println!("stored and read back: {back:?}; deleted: {gone}");
}
