//! Blocking Secret Service adapter. Execute only on a bounded worker.
use crate::credentials::{CredentialError, CredentialKey, MAX_SECRET_BYTES, PersistentCredentials};
use feishin_domain::Secret;
use secret_service::{EncryptionType, blocking::SecretService};
use std::collections::HashMap;

#[derive(Default)]
pub struct LinuxCredentials {
    connection: Option<zbus::blocking::Connection>,
}
impl LinuxCredentials {
    pub fn new() -> Self {
        Self::default()
    }
    /// The caller owns and configures this connection, including its method timeout.
    pub fn with_connection(connection: zbus::blocking::Connection) -> Self {
        Self {
            connection: Some(connection),
        }
    }
    fn connect(&self) -> Result<SecretService<'_>, CredentialError> {
        match &self.connection {
            Some(c) => SecretService::connect_with_existing(EncryptionType::Dh, c.clone()),
            None => SecretService::connect(EncryptionType::Dh),
        }
        .map_err(map_error)
    }
}
fn map_error(error: secret_service::Error) -> CredentialError {
    match error {
        secret_service::Error::Locked => CredentialError::Locked,
        secret_service::Error::Unavailable
        | secret_service::Error::Zbus(_)
        | secret_service::Error::ZbusFdo(_)
        | secret_service::Error::NoResult => CredentialError::Unavailable,
        _ => CredentialError::Backend,
    }
}
fn attributes(key: &str) -> HashMap<&str, &str> {
    HashMap::from([
        ("application", "feishin-rust"),
        ("scope", "development"),
        ("schema", "credential-v1"),
        ("key", key),
    ])
}
fn find_item<'a>(
    service: &'a SecretService<'a>,
    key: &str,
) -> Result<Option<secret_service::blocking::Item<'a>>, CredentialError> {
    let mut items = service.search_items(attributes(key)).map_err(map_error)?;
    if items.locked.len() + items.unlocked.len() > 1 {
        return Err(CredentialError::Ambiguous);
    }
    if !items.locked.is_empty() {
        return Err(CredentialError::Locked);
    }
    Ok(items.unlocked.pop())
}
impl PersistentCredentials for LinuxCredentials {
    fn persist(&mut self, key: &CredentialKey, secret: &Secret) -> Result<(), CredentialError> {
        if secret.expose().len() > MAX_SECRET_BYTES {
            return Err(CredentialError::TooLarge);
        }
        let service = self.connect()?;
        let key = key.attribute();
        if let Some(item) = find_item(&service, &key)? {
            item.ensure_unlocked().map_err(map_error)?;
            item.set_secret(secret.expose(), "application/octet-stream")
                .map_err(map_error)?;
        } else {
            let collection = service.get_default_collection().map_err(map_error)?;
            collection.ensure_unlocked().map_err(map_error)?;
            collection
                .create_item(
                    "Feishin Rust credential",
                    attributes(&key),
                    secret.expose(),
                    false,
                    "application/octet-stream",
                )
                .map_err(map_error)?;
        }
        Ok(())
    }
    fn load(&mut self, key: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
        let service = self.connect()?;
        let key = key.attribute();
        let Some(item) = find_item(&service, &key)? else {
            return Ok(None);
        };
        item.ensure_unlocked().map_err(map_error)?;
        let bytes = item.get_secret().map_err(map_error)?;
        if bytes.len() > MAX_SECRET_BYTES {
            return Err(CredentialError::TooLarge);
        }
        Ok(Some(Secret::new(bytes)))
    }
    fn delete(&mut self, key: &CredentialKey) -> Result<(), CredentialError> {
        let service = self.connect()?;
        let key = key.attribute();
        if let Some(item) = find_item(&service, &key)? {
            item.ensure_unlocked().map_err(map_error)?;
            item.delete().map_err(map_error)?;
        }
        Ok(())
    }
}
