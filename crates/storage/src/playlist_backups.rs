//! Versioned non-secret replacement backups. Repository methods are blocking;
//! native callers must use a bounded worker. Browser storage is a local dispatcher.
use crate::{AtomicStore, MAX_BYTES, bounded_json};
use feishin_domain::{PlaylistId, Revision, Scoped, ServerId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA_VERSION: u64 = 1;
pub const MAX_BACKUPS: usize = 16;
pub const MAX_SONGS: usize = 10_000;
const MAX_ID_BYTES: usize = 8192;
const MAX_UNIX_MS: u64 = 8_640_000_000_000_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Store(crate::Error),
    InvalidDocument,
    UnsupportedVersion,
    InvalidBackup,
    TooLarge,
    Capacity,
    Stale,
    Exhausted,
}
impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        match e {
            crate::Error::TooLarge => Self::TooLarge,
            _ => Self::Store(e),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "playlist backup storage: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Backup {
    pub server_id: String,
    pub playlist_id: String,
    pub name: String,
    pub song_ids: Vec<String>,
    /// Captured UTC Unix milliseconds, never an operation's monotonic clock.
    pub captured_at_unix_ms: u64,
}
impl Backup {
    pub fn identity(&self) -> Scoped<PlaylistId> {
        Scoped {
            server: ServerId::new(&self.server_id),
            id: PlaylistId::new(&self.playlist_id),
        }
    }
    pub(crate) fn valid(&self) -> bool {
        scope_valid(&self.server_id, &self.playlist_id)
            && self.name.len() <= MAX_ID_BYTES
            && self.song_ids.len() <= MAX_SONGS
            && self
                .song_ids
                .iter()
                .all(|id| !id.is_empty() && id.len() <= MAX_ID_BYTES)
            && self.captured_at_unix_ms <= MAX_UNIX_MS
    }
}
fn scope_valid(server: &str, playlist: &str) -> bool {
    !server.trim().is_empty()
        && server.len() <= 128
        && !playlist.is_empty()
        && playlist.len() <= MAX_ID_BYTES
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub revision: Revision,
    pub backups: Vec<Backup>,
}
impl Snapshot {
    pub fn get(&self, identity: &Scoped<PlaylistId>) -> Option<&Backup> {
        self.backups.iter().find(|b| {
            b.server_id == identity.server.as_str() && b.playlist_id == identity.id.as_str()
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema_version: u64,
    revision: u64,
    backups: Vec<Backup>,
}
fn decode(bytes: &[u8]) -> Result<Snapshot, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Error::InvalidDocument)?;
    match value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        Some(SCHEMA_VERSION) => {}
        Some(_) => return Err(Error::UnsupportedVersion),
        None => return Err(Error::InvalidDocument),
    }
    // Decode the original bytes so duplicate members cannot be collapsed by Value.
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| Error::InvalidDocument)?;
    if envelope.schema_version != SCHEMA_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    if envelope.backups.len() > MAX_BACKUPS {
        return Err(Error::Capacity);
    }
    let mut scopes = BTreeSet::new();
    for backup in &envelope.backups {
        if !backup.valid() {
            return Err(Error::InvalidBackup);
        }
        if !scopes.insert((&backup.server_id, &backup.playlist_id)) {
            return Err(Error::InvalidDocument);
        }
    }
    Ok(Snapshot {
        revision: Revision(envelope.revision),
        backups: envelope.backups,
    })
}
pub struct Repository<S> {
    store: S,
}
impl<S: AtomicStore> Repository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
    /// Read-only defaults/recovery. Any corrupt entry protects the complete value;
    /// do not discard an old recovery backup to permit a destructive replacement.
    pub fn load(&mut self) -> Result<Snapshot, Error> {
        match self.store.read()? {
            None => Ok(Snapshot::default()),
            Some(bytes) => decode(&bytes),
        }
    }
    /// Explicit single-owner compare-and-replace. No cross-process/tab CAS claim.
    pub fn save(&mut self, backup: &Backup, expected: Revision) -> Result<Revision, Error> {
        self.save_guarded(backup, expected, MAX_BYTES, || Ok(()))
    }
    pub(crate) fn save_guarded<E: From<Error>>(
        &mut self,
        backup: &Backup,
        expected: Revision,
        limit: usize,
        can_write: impl FnOnce() -> Result<(), E>,
    ) -> Result<Revision, E> {
        if !backup.valid() {
            return Err(Error::InvalidBackup.into());
        }
        let snapshot = self.load()?;
        if snapshot.revision != expected {
            return Err(Error::Stale.into());
        }
        let mut replaced = false;
        let mut rows: Vec<_> = snapshot
            .backups
            .iter()
            .map(|old| {
                if old.server_id == backup.server_id && old.playlist_id == backup.playlist_id {
                    replaced = true;
                    backup
                } else {
                    old
                }
            })
            .collect();
        if !replaced {
            rows.push(backup);
        }
        if rows.len() > MAX_BACKUPS {
            return Err(Error::Capacity.into());
        }
        self.write(expected, &rows, limit, can_write)
    }
    /// Explicit scope removal permits retention cleanup; never evict automatically.
    pub fn remove(
        &mut self,
        identity: &Scoped<PlaylistId>,
        expected: Revision,
    ) -> Result<Revision, Error> {
        if !scope_valid(identity.server.as_str(), identity.id.as_str()) {
            return Err(Error::InvalidBackup);
        }
        let snapshot = self.load()?;
        if snapshot.revision != expected {
            return Err(Error::Stale);
        }
        let rows: Vec<_> = snapshot
            .backups
            .iter()
            .filter(|b| {
                b.server_id != identity.server.as_str() || b.playlist_id != identity.id.as_str()
            })
            .collect();
        if rows.len() == snapshot.backups.len() {
            return Ok(expected);
        }
        self.write(expected, &rows, MAX_BYTES, || Ok(()))
    }
    fn write<E: From<Error>>(
        &mut self,
        current: Revision,
        rows: &[&Backup],
        limit: usize,
        can_write: impl FnOnce() -> Result<(), E>,
    ) -> Result<Revision, E> {
        let revision = current.next().map_err(|_| Error::Exhausted)?;
        #[derive(Serialize)]
        struct View<'a> {
            schema_version: u64,
            revision: u64,
            backups: &'a [&'a Backup],
        }
        let bytes = bounded_json::encode(
            &View {
                schema_version: SCHEMA_VERSION,
                revision: revision.0,
                backups: rows,
            },
            MAX_BYTES.min(limit),
        )
        .map_err(|e| match e {
            bounded_json::EncodeError::TooLarge => Error::TooLarge,
            bounded_json::EncodeError::Invalid => Error::InvalidBackup,
        })?;
        can_write()?;
        self.store.replace(&bytes).map_err(Error::from)?;
        Ok(revision)
    }
}
