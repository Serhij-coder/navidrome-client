use crate::{AtomicStore, Error, MAX_BYTES};

/// Origin-scoped single-key localStorage replacement. Quota/security/disabled
/// storage failures are explicit. No multi-key transaction or secret storage.
pub struct BrowserStore {
    storage: web_sys::Storage,
    key: &'static str,
}
impl BrowserStore {
    pub const KEY: &'static str = "feishin-rust:dev:p02:profiles:v1";
    pub const IMPORT_KEY: &'static str = "feishin-rust:dev:p02:imports:v1";
    pub const PLAYLIST_BACKUP_KEY: &'static str = "feishin-rust:dev:p03:playlist-backups:v1";
    pub fn open_playlist_backups() -> Result<Self, Error> {
        Self::open_key(Self::PLAYLIST_BACKUP_KEY)
    }
    pub const STATE_KEY: &'static str = "feishin-rust:dev:p02:state:v1";
    pub fn open_state() -> Result<Self, Error> {
        Self::open_key(Self::STATE_KEY)
    }
    pub fn open() -> Result<Self, Error> {
        Self::open_key(Self::KEY)
    }
    pub fn open_imports() -> Result<Self, Error> {
        Self::open_key(Self::IMPORT_KEY)
    }
    fn open_key(key: &'static str) -> Result<Self, Error> {
        let storage = web_sys::window()
            .ok_or(Error::Unavailable)?
            .local_storage()
            .map_err(|_| Error::Unavailable)?
            .ok_or(Error::Unavailable)?;
        Ok(Self { storage, key })
    }
}
impl AtomicStore for BrowserStore {
    fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
        let value = self
            .storage
            .get_item(self.key)
            .map_err(|_| Error::Unavailable)?;
        match value {
            Some(value) if value.len() > MAX_BYTES => Err(Error::TooLarge),
            Some(value) => Ok(Some(value.into_bytes())),
            None => Ok(None),
        }
    }
    fn replace(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidDocument)?;
        self.storage
            .set_item(self.key, text)
            .map_err(|_| Error::Unavailable)
    }
}
