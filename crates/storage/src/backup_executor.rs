//! Shared checks/conversion for the single-owner backup boundary.
use crate::playlist_backups::{Backup, Error, MAX_SONGS, Repository};
use feishin_domain::*;

pub(crate) fn failure(cause: BackupError, may_have_written: bool) -> BackupFailure {
    BackupFailure {
        cause,
        may_have_written,
    }
}
pub(crate) fn map_error(error: Error) -> BackupFailure {
    use crate::Error as StoreError;
    let cause = match error {
        Error::InvalidDocument => BackupError::Corrupt,
        Error::UnsupportedVersion => BackupError::UnsupportedVersion,
        Error::InvalidBackup => BackupError::Invalid,
        Error::TooLarge | Error::Capacity => BackupError::Capacity,
        Error::Stale => BackupError::Conflict,
        Error::Exhausted => BackupError::Exhausted,
        Error::Store(_) => BackupError::Unavailable,
    };
    failure(cause, matches!(error, Error::Store(StoreError::Io)))
}
impl From<Error> for BackupFailure {
    fn from(error: Error) -> Self {
        map_error(error)
    }
}
pub(crate) fn prepare(
    backup: PlaylistBackup,
    clock: &dyn Clock,
    context: OperationContext<'_>,
) -> Result<Backup, BackupFailure> {
    context
        .check(clock.now())
        .map_err(|e| failure(BackupError::Context(e), false))?;
    context
        .check_items(backup.songs.len())
        .map_err(|e| failure(BackupError::Context(e), false))?;
    // Bound queued allocations before converting or serializing. This is a lower
    // bound on JSON size; the worker enforces the actual escaped envelope limit.
    if backup.songs.len() > MAX_SONGS
        || backup
            .songs
            .iter()
            .map(|s| s.as_str().len())
            .chain([
                backup.playlist.server.as_str().len(),
                backup.playlist.id.as_str().len(),
                backup.name.len(),
            ])
            .try_fold(0usize, usize::checked_add)
            .is_none_or(|n| n > crate::MAX_BYTES.min(context.limits.request_bytes))
    {
        return Err(failure(BackupError::Capacity, false));
    }
    let value = Backup {
        server_id: backup.playlist.server.as_str().into(),
        playlist_id: backup.playlist.id.as_str().into(),
        name: backup.name,
        song_ids: backup
            .songs
            .into_iter()
            .map(|s| s.as_str().to_owned())
            .collect(),
        captured_at_unix_ms: backup.captured_at.0,
    };
    if !value.valid() {
        return Err(failure(BackupError::Invalid, false));
    }
    Ok(value)
}
pub(crate) fn save<S: crate::AtomicStore>(
    repository: &mut Repository<S>,
    backup: &Backup,
    max_bytes: usize,
    can_start: impl Fn() -> Result<(), BackupFailure>,
) -> Result<Revision, BackupFailure> {
    can_start()?;
    let snapshot = repository.load().map_err(map_error)?;
    can_start()?;
    repository.save_guarded(backup, snapshot.revision, max_bytes, can_start)
}
pub(crate) fn finish(
    result: Result<Revision, BackupFailure>,
    clock: &dyn Clock,
    context: OperationContext<'_>,
) -> Result<BackupReceipt, BackupFailure> {
    let may_have_written = result
        .as_ref()
        .map_or_else(|e| e.may_have_written, |_| true);
    context
        .check(clock.now())
        .map_err(|e| failure(BackupError::Context(e), may_have_written))?;
    result.map(|revision| BackupReceipt {
        context: context.correlation,
        revision,
    })
}

#[cfg(target_arch = "wasm32")]
pub struct BrowserPlaylistBackups {
    repository: Repository<crate::BrowserStore>,
}
#[cfg(target_arch = "wasm32")]
impl BrowserPlaylistBackups {
    pub fn open() -> Result<Self, BackupFailure> {
        Ok(Self {
            repository: Repository::new(
                crate::BrowserStore::open_playlist_backups()
                    .map_err(|e| map_error(Error::from(e)))?,
            ),
        })
    }
}
#[cfg(target_arch = "wasm32")]
impl PlaylistBackupStore for BrowserPlaylistBackups {
    fn save<'a>(
        &'a mut self,
        backup: PlaylistBackup,
        clock: &'a dyn Clock,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<BackupReceipt, BackupFailure>> {
        Box::pin(async move {
            let value = prepare(backup, clock, context)?;
            let result = save(
                &mut self.repository,
                &value,
                context.limits.request_bytes,
                || {
                    context
                        .check(clock.now())
                        .map_err(|e| failure(BackupError::Context(e), false))
                },
            );
            finish(result, clock, context)
        })
    }
}
