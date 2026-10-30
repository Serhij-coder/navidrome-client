//! Normalized recovery boundary. Wire DTOs and platform storage stay in adapters.
use crate::{
    Clock, DomainError, LocalFuture, OperationContext, PlaylistId, RequestContext, Revision,
    Scoped, SongId,
};

/// UTC Unix milliseconds, distinct from monotonic operation deadlines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnixMilliseconds(pub u64);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistBackup {
    pub playlist: Scoped<PlaylistId>,
    pub name: String,
    pub songs: Vec<SongId>,
    pub captured_at: UnixMilliseconds,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupError {
    Context(DomainError),
    Invalid,
    Capacity,
    Corrupt,
    UnsupportedVersion,
    Conflict,
    Exhausted,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupFailure {
    pub cause: BackupError,
    /// The durable value may have changed. No destructive server work may follow
    /// any failure; reload/reconcile instead of automatically retrying.
    pub may_have_written: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupReceipt {
    pub context: RequestContext,
    pub revision: Revision,
}
/// Single-owner backup capture. Native implementations must move blocking I/O
/// off the caller; browser implementations may use local non-Send futures.
/// Caller scheduling must wake on deadline/cancellation. Implementations check
/// every poll; an in-flight OS write cannot be forcibly interrupted.
pub trait PlaylistBackupStore {
    fn save<'a>(
        &'a mut self,
        backup: PlaylistBackup,
        clock: &'a dyn Clock,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<BackupReceipt, BackupFailure>>;
}
