//! Single-owner active durable state. Blocking operations belong on a bounded worker.
use crate::{
    AtomicStore, Error, MAX_BYTES, Preferences, Recovery,
    imports::{self, MigrationData},
    journal::{JournalError, JournalRepository},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    Storage(Error),
    Journal(JournalError),
    InvalidState,
    UnsupportedSchema,
    Conflict,
    EmptyStage,
    NoBackup,
    Exhausted,
}
impl From<Error> for StateError {
    fn from(value: Error) -> Self {
        Self::Storage(value)
    }
}
impl From<JournalError> for StateError {
    fn from(value: JournalError) -> Self {
        Self::Journal(value)
    }
}
impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "active state: {self:?}")
    }
}
impl std::error::Error for StateError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u64,
    pub data: MigrationData,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedState {
    pub snapshot: Snapshot,
    pub recovered: Vec<Recovery>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema_version: u64,
    current: Snapshot,
    backup: Option<Snapshot>,
}

/// Immutable reviewed operation, bound to exact source and target baselines.
pub struct PreparedState {
    expected: Option<Vec<u8>>,
    legacy: Option<Option<Vec<u8>>>,
    stage: Option<Option<Vec<u8>>>,
    before: Snapshot,
    after: Snapshot,
    encoded: Option<Vec<u8>>,
}
impl PreparedState {
    pub fn before(&self) -> &Snapshot {
        &self.before
    }
    pub fn after(&self) -> &Snapshot {
        &self.after
    }
}
type Baseline = (
    Option<Vec<u8>>,
    Option<Option<Vec<u8>>>,
    Envelope,
    Vec<Recovery>,
);
pub struct StateRepository<A, J, P> {
    active: A,
    journal: JournalRepository<J>,
    legacy: P,
}
impl<A: AtomicStore, J: AtomicStore, P: AtomicStore> StateRepository<A, J, P> {
    pub fn new(active: A, journal: J, legacy: P) -> Self {
        Self {
            active,
            journal: JournalRepository::new(journal),
            legacy,
        }
    }
    pub fn journal_mut(&mut self) -> &mut JournalRepository<J> {
        &mut self.journal
    }
    fn baseline(&mut self) -> Result<Baseline, StateError> {
        let bytes = self.active.read()?;
        if let Some(raw) = &bytes {
            return Ok((bytes.clone(), None, decode(raw)?, vec![]));
        }
        let legacy = self.legacy.read()?;
        let loaded = match &legacy {
            Some(raw) => crate::decode(raw)?,
            None => crate::Loaded {
                data: Default::default(),
                recovered: vec![],
            },
        };
        let data = MigrationData {
            profile: loaded.data,
            ..Default::default()
        };
        Ok((
            None,
            Some(legacy),
            Envelope {
                schema_version: 1,
                current: Snapshot { revision: 0, data },
                backup: None,
            },
            loaded.recovered,
        ))
    }
    pub fn load(&mut self) -> Result<LoadedState, StateError> {
        let (_, _, env, recovered) = self.baseline()?;
        Ok(LoadedState {
            snapshot: env.current,
            recovered,
        })
    }
    pub fn prepare_save(&mut self, data: &MigrationData) -> Result<PreparedState, StateError> {
        // Reject oversized caller-owned input before duplicating its allocations.
        imports::validate_data(data).map_err(|_| StateError::InvalidState)?;
        self.prepare(data.clone(), None, false)
    }
    pub fn prepare_preferences(
        &mut self,
        expected_revision: u64,
        preferences: Preferences,
    ) -> Result<PreparedState, StateError> {
        let (expected, legacy, mut env, _) = self.baseline()?;
        if env.current.revision != expected_revision {
            return Err(StateError::Conflict);
        }
        let mut data = env.current.data.clone();
        data.profile.preferences = preferences;
        Self::make(expected, legacy, None, &mut env, data, false)
    }
    pub fn prepare_adoption(&mut self) -> Result<PreparedState, StateError> {
        let (stage, bytes) = self.journal.snapshot_and_bytes()?;
        self.prepare(
            stage.data.ok_or(StateError::EmptyStage)?,
            Some(bytes),
            false,
        )
    }
    pub fn prepare_rollback(&mut self) -> Result<PreparedState, StateError> {
        let (expected, legacy, mut env, _) = self.baseline()?;
        let data = env
            .backup
            .as_ref()
            .ok_or(StateError::NoBackup)?
            .data
            .clone();
        Self::make(expected, legacy, None, &mut env, data, true)
    }
    fn prepare(
        &mut self,
        data: MigrationData,
        stage: Option<Option<Vec<u8>>>,
        force: bool,
    ) -> Result<PreparedState, StateError> {
        let (expected, legacy, mut env, _) = self.baseline()?;
        Self::make(expected, legacy, stage, &mut env, data, force)
    }
    fn make(
        expected: Option<Vec<u8>>,
        legacy: Option<Option<Vec<u8>>>,
        stage: Option<Option<Vec<u8>>>,
        env: &mut Envelope,
        data: MigrationData,
        force: bool,
    ) -> Result<PreparedState, StateError> {
        imports::validate_data(&data).map_err(|_| StateError::InvalidState)?;
        let before = env.current.clone();
        let encoded = if !force && expected.is_some() && before.data == data {
            None
        } else {
            let revision = before
                .revision
                .checked_add(1)
                .ok_or(StateError::Exhausted)?;
            env.current = Snapshot { revision, data };
            env.backup = Some(before.clone());
            Some(encode(env)?)
        };
        Ok(PreparedState {
            expected,
            legacy,
            stage,
            before,
            after: env.current.clone(),
            encoded,
        })
    }
    pub fn apply(&mut self, prepared: &PreparedState) -> Result<Snapshot, StateError> {
        if self.active.read()? != prepared.expected {
            return Err(StateError::Conflict);
        }
        if let Some(expected) = &prepared.legacy
            && self.legacy.read()? != *expected
        {
            return Err(StateError::Conflict);
        }
        if let Some(expected) = &prepared.stage
            && self.journal.snapshot_and_bytes()?.1 != *expected
        {
            return Err(StateError::Conflict);
        }
        if let Some(bytes) = &prepared.encoded {
            self.active.replace(bytes)?;
        }
        Ok(prepared.after.clone())
    }
}
fn encode(env: &Envelope) -> Result<Vec<u8>, StateError> {
    crate::bounded_json::encode(env, MAX_BYTES).map_err(|error| match error {
        crate::bounded_json::EncodeError::TooLarge => StateError::Storage(Error::TooLarge),
        crate::bounded_json::EncodeError::Invalid => StateError::InvalidState,
    })
}
fn decode(bytes: &[u8]) -> Result<Envelope, StateError> {
    if bytes.len() > MAX_BYTES {
        return Err(StateError::Storage(Error::TooLarge));
    }
    let raw: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| StateError::InvalidState)?;
    let version = raw
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(StateError::InvalidState)?;
    if version != 1 {
        return Err(StateError::UnsupportedSchema);
    }
    let env: Envelope = serde_json::from_value(raw).map_err(|_| StateError::InvalidState)?;
    if env.current.revision == 0
        || env
            .backup
            .as_ref()
            .is_none_or(|b| b.revision.checked_add(1) != Some(env.current.revision))
    {
        return Err(StateError::InvalidState);
    }
    for snapshot in std::iter::once(&env.current).chain(env.backup.iter()) {
        imports::validate_data(&snapshot.data).map_err(|_| StateError::InvalidState)?;
    }
    Ok(env)
}
