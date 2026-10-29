//! Atomic apply/backup/rollback for reviewed imports. The source copy is never stored.
//! Blocking worker API; application shell adoption remains a separate operation.
use crate::{
    AtomicStore, Error, MAX_BYTES,
    imports::{self, CopiedInputs, ImportError, ImportPreview, MigrationData},
};
use serde::{Deserialize, Serialize};

pub const JOURNAL_SCHEMA: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalError {
    Storage(Error),
    Import(ImportError),
    InvalidJournal,
    UnsupportedSchema,
    Conflict,
    NoBackup,
    Exhausted,
}
impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "import journal: {self:?}")
    }
}
impl std::error::Error for JournalError {}
impl From<Error> for JournalError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}
impl From<ImportError> for JournalError {
    fn from(error: ImportError) -> Self {
        Self::Import(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u64,
    pub data: Option<MigrationData>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema_version: u64,
    current: Snapshot,
    backup: Option<Snapshot>,
}
impl Default for Envelope {
    fn default() -> Self {
        Self {
            schema_version: JOURNAL_SCHEMA,
            current: Snapshot {
                revision: 0,
                data: None,
            },
            backup: None,
        }
    }
}

/// Candidate and exact target baseline are immutable after preparation.
/// The bytes of an imported source are not retained by this object.
pub struct PreparedImport {
    expected: Option<Vec<u8>>,
    preview: ImportPreview,
}
impl PreparedImport {
    pub fn preview(&self) -> &ImportPreview {
        &self.preview
    }
}
pub struct PreparedRollback {
    expected: Option<Vec<u8>>,
    restored: Option<MigrationData>,
}
impl PreparedRollback {
    pub fn restored(&self) -> Option<&MigrationData> {
        self.restored.as_ref()
    }
}

pub struct JournalRepository<S> {
    store: S,
}
impl<S: AtomicStore> JournalRepository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
    pub(crate) fn snapshot_and_bytes(
        &mut self,
    ) -> Result<(Snapshot, Option<Vec<u8>>), JournalError> {
        let bytes = self.store.read()?;
        let snapshot = decode(bytes.as_deref())?.current;
        Ok((snapshot, bytes))
    }
    pub fn load(&mut self) -> Result<Snapshot, JournalError> {
        let bytes = self.store.read()?;
        Ok(decode(bytes.as_deref())?.current)
    }
    /// Pure projection plus target-baseline capture. Does not write either store.
    pub fn prepare(&mut self, input: &CopiedInputs<'_>) -> Result<PreparedImport, JournalError> {
        let expected = self.store.read()?;
        let mut envelope = decode(expected.as_deref())?;
        let empty = MigrationData::default();
        let preview = imports::preview(input, envelope.current.data.as_ref().unwrap_or(&empty))?;
        // Apply must fit current plus its one-level backup before asking a caller
        // to approve a candidate. Actual apply repeats validation and the guard.
        if envelope.current.data.as_ref() != Some(preview.data()) {
            replace_current(&mut envelope, Some(preview.data().clone()))?;
            encode(&envelope)?;
        }
        Ok(PreparedImport { expected, preview })
    }
    pub fn apply(&mut self, prepared: &PreparedImport) -> Result<Snapshot, JournalError> {
        let mut envelope = self.guarded(&prepared.expected)?;
        imports::validate_data(prepared.preview.data())?;
        if envelope.current.data.as_ref() == Some(prepared.preview.data()) {
            return Ok(envelope.current);
        }
        replace_current(&mut envelope, Some(prepared.preview.data().clone()))?;
        let bytes = encode(&envelope)?;
        self.store.replace(&bytes)?;
        Ok(envelope.current)
    }
    pub fn prepare_rollback(&mut self) -> Result<PreparedRollback, JournalError> {
        let expected = self.store.read()?;
        let envelope = decode(expected.as_deref())?;
        let restored = envelope.backup.ok_or(JournalError::NoBackup)?.data;
        Ok(PreparedRollback { expected, restored })
    }
    /// Restores the reviewed backup with a new revision, retaining the replaced
    /// current value as the next one-level backup. No multi-file transaction.
    pub fn rollback(&mut self, prepared: &PreparedRollback) -> Result<Snapshot, JournalError> {
        let mut envelope = self.guarded(&prepared.expected)?;
        let backup = envelope.backup.as_ref().ok_or(JournalError::NoBackup)?;
        if backup.data != prepared.restored {
            return Err(JournalError::Conflict);
        }
        replace_current(&mut envelope, prepared.restored.clone())?;
        let bytes = encode(&envelope)?;
        self.store.replace(&bytes)?;
        Ok(envelope.current)
    }
    fn guarded(&mut self, expected: &Option<Vec<u8>>) -> Result<Envelope, JournalError> {
        let bytes = self.store.read()?;
        if &bytes != expected {
            return Err(JournalError::Conflict);
        }
        decode(bytes.as_deref())
    }
}
fn replace_current(
    envelope: &mut Envelope,
    data: Option<MigrationData>,
) -> Result<(), JournalError> {
    let revision = envelope
        .current
        .revision
        .checked_add(1)
        .ok_or(JournalError::Exhausted)?;
    let old = std::mem::replace(&mut envelope.current, Snapshot { revision, data });
    envelope.backup = Some(old);
    Ok(())
}
fn encode(envelope: &Envelope) -> Result<Vec<u8>, JournalError> {
    crate::bounded_json::encode(envelope, MAX_BYTES).map_err(|error| match error {
        crate::bounded_json::EncodeError::TooLarge => JournalError::Storage(Error::TooLarge),
        crate::bounded_json::EncodeError::Invalid => JournalError::InvalidJournal,
    })
}
fn decode(bytes: Option<&[u8]>) -> Result<Envelope, JournalError> {
    let Some(bytes) = bytes else {
        return Ok(Envelope::default());
    };
    if bytes.len() > MAX_BYTES {
        return Err(JournalError::Storage(Error::TooLarge));
    }
    let raw: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| JournalError::InvalidJournal)?;
    let version = raw
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(JournalError::InvalidJournal)?;
    if version != JOURNAL_SCHEMA {
        return Err(JournalError::UnsupportedSchema);
    }
    let envelope: Envelope =
        serde_json::from_value(raw).map_err(|_| JournalError::InvalidJournal)?;
    for snapshot in std::iter::once(&envelope.current).chain(envelope.backup.iter()) {
        if snapshot.revision == 0 && snapshot.data.is_some() {
            return Err(JournalError::InvalidJournal);
        }
        if let Some(data) = &snapshot.data {
            imports::validate_data(data).map_err(|_| JournalError::InvalidJournal)?;
        }
    }
    if match &envelope.backup {
        Some(backup) => backup.revision.checked_add(1) != Some(envelope.current.revision),
        None => envelope.current.revision != 0,
    } {
        return Err(JournalError::InvalidJournal);
    }
    Ok(envelope)
}
