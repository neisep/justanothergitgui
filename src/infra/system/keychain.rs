use keyring::Entry;

pub fn entry(service: &str, user: &str) -> Result<Entry, String> {
    Entry::new(service, user)
        .map_err(|error| format!("Could not access system keychain: {}", error))
}
