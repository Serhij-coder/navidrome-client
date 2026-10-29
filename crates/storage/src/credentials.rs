//! Explicit, bounded session credentials and opt-in persistence.
use crate::{BackendKind, Profile};
use feishin_domain::Secret;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const MAX_SECRET_BYTES: usize = 64 * 1024;
pub const MAX_SESSION_ENTRIES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    InvalidKey,
    MissingSession,
    TooLarge,
    Capacity,
    Unsupported,
    Locked,
    Unavailable,
    Ambiguous,
    Backend,
}
impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "credential operation: {self:?}")
    }
}
impl std::error::Error for CredentialError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSlot {
    SessionTokens,
    LoginPassword,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialKey([u8; 32]);
impl CredentialKey {
    pub fn new(
        namespace: &str,
        profile: &Profile,
        account: &str,
        slot: CredentialSlot,
    ) -> Result<Self, CredentialError> {
        if namespace.trim().is_empty()
            || namespace.len() > 128
            || account.trim().is_empty()
            || account.len() > 256
            || !profile.valid()
        {
            return Err(CredentialError::InvalidKey);
        }
        let endpoint =
            url::Url::parse(&profile.endpoint).map_err(|_| CredentialError::InvalidKey)?;
        let backend = match profile.backend {
            BackendKind::Navidrome => "navidrome",
            BackendKind::Subsonic => "subsonic",
            BackendKind::Jellyfin => "jellyfin",
        };
        let slot = match slot {
            CredentialSlot::SessionTokens => "session-tokens",
            CredentialSlot::LoginPassword => "login-password",
        };
        let mut digest = Sha256::new();
        for part in [
            "feishin-rust:credential:v1",
            namespace,
            &profile.id,
            backend,
            endpoint.as_str(),
            account,
            slot,
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        Ok(Self(digest.finalize().into()))
    }
    pub fn attribute(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}
/// Synchronous persistence. Call on a worker, never the UI or async executor.
pub trait PersistentCredentials {
    fn persist(&mut self, key: &CredentialKey, secret: &Secret) -> Result<(), CredentialError>;
    fn load(&mut self, key: &CredentialKey) -> Result<Option<Secret>, CredentialError>;
    fn delete(&mut self, key: &CredentialKey) -> Result<(), CredentialError>;
}
#[derive(Default)]
pub struct NoPersistence;
impl PersistentCredentials for NoPersistence {
    fn persist(&mut self, _: &CredentialKey, _: &Secret) -> Result<(), CredentialError> {
        Err(CredentialError::Unsupported)
    }
    fn load(&mut self, _: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
        Err(CredentialError::Unsupported)
    }
    fn delete(&mut self, _: &CredentialKey) -> Result<(), CredentialError> {
        Ok(())
    }
}
pub struct CredentialOwner<B = NoPersistence> {
    backend: B,
    session: HashMap<CredentialKey, Secret>,
}
impl Default for CredentialOwner<NoPersistence> {
    fn default() -> Self {
        Self::new()
    }
}
impl CredentialOwner<NoPersistence> {
    pub fn new() -> Self {
        Self::with_backend(NoPersistence)
    }
}
impl<B: PersistentCredentials> CredentialOwner<B> {
    pub fn with_backend(backend: B) -> Self {
        Self {
            backend,
            session: HashMap::new(),
        }
    }
    pub fn put_session(
        &mut self,
        key: CredentialKey,
        secret: Secret,
    ) -> Result<(), CredentialError> {
        if secret.expose().len() > MAX_SECRET_BYTES {
            return Err(CredentialError::TooLarge);
        }
        if !self.session.contains_key(&key) && self.session.len() >= MAX_SESSION_ENTRIES {
            return Err(CredentialError::Capacity);
        }
        self.session.insert(key, secret);
        Ok(())
    }
    pub fn session(&self, key: &CredentialKey) -> Option<&Secret> {
        self.session.get(key)
    }
    pub fn clear_session(&mut self) {
        self.session.clear();
    }
    pub fn remember(&mut self, key: &CredentialKey) -> Result<(), CredentialError> {
        let value = self
            .session
            .get(key)
            .ok_or(CredentialError::MissingSession)?;
        self.backend.persist(key, value)
    }
    pub fn restore_remembered(&mut self, key: &CredentialKey) -> Result<bool, CredentialError> {
        if !self.session.contains_key(key) && self.session.len() >= MAX_SESSION_ENTRIES {
            return Err(CredentialError::Capacity);
        }
        match self.backend.load(key)? {
            Some(secret) => {
                self.put_session(key.clone(), secret)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
    pub fn forget(&mut self, key: &CredentialKey) -> Result<(), CredentialError> {
        self.session.remove(key);
        self.backend.delete(key)
    }
}
