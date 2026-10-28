//! Application-owned service boundaries. Concrete adapters enforce cancellation,
//! deadlines and payload limits before I/O and after completion.
use crate::*;
use std::{future::Future, pin::Pin};

/// Browser implementations may capture Rc or browser handles. No universal Send bound.
pub type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
pub trait Cancellation {
    fn is_cancelled(&self) -> bool;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationLimits {
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub items: usize,
}
#[derive(Clone, Copy)]
pub struct OperationContext<'a> {
    pub correlation: RequestContext,
    pub limits: OperationLimits,
    pub cancellation: &'a dyn Cancellation,
}
impl OperationContext<'_> {
    pub fn check(&self, now: Milliseconds) -> Result<(), DomainError> {
        if self.cancellation.is_cancelled() {
            return Err(DomainError::Cancelled);
        }
        if now > self.correlation.deadline {
            return Err(DomainError::Deadline);
        }
        Ok(())
    }
    pub fn check_request_size(&self, size: usize) -> Result<(), DomainError> {
        check_size(size, self.limits.request_bytes)
    }
    pub fn check_response_size(&self, size: usize) -> Result<(), DomainError> {
        check_size(size, self.limits.response_bytes)
    }
    pub fn check_items(&self, size: usize) -> Result<(), DomainError> {
        check_size(size, self.limits.items)
    }
}
fn check_size(size: usize, limit: usize) -> Result<(), DomainError> {
    if size > limit {
        Err(DomainError::Capacity)
    } else {
        Ok(())
    }
}

/// Normalized read boundary, not the full P03/P04 operation facade.
pub trait Backend {
    fn song<'a>(
        &'a self,
        id: Scoped<SongId>,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Song, DomainError>>;
    fn songs<'a>(
        &'a self,
        query: SongListQuery,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Page<Song>, DomainError>>;
    fn capabilities<'a>(
        &'a self,
        server: ServerId,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Capabilities, DomainError>>;
}
/// Opaque protocol payload exchange. HTTP/URL/header/status DTOs stay in adapters;
/// this primitive is not an unauthenticated URL-fetching service.
pub trait Transport {
    fn request<'a>(
        &'a self,
        request: Vec<u8>,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Vec<u8>, DomainError>>;
}
pub trait Clock {
    /// Monotonic time in the same epoch used by operation deadlines.
    fn now(&self) -> Milliseconds;
}
pub trait Random {
    fn next_u64(&mut self) -> u64;
}
/// Versioned non-secret DTO bytes only. Atomicity/recovery belong to adapters.
pub trait Persistence {
    fn load<'a>(
        &'a self,
        key: &'a str,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Option<Vec<u8>>, DomainError>>;
    fn save<'a>(
        &'a self,
        key: &'a str,
        value: Vec<u8>,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>>;
    fn remove<'a>(
        &'a self,
        key: &'a str,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>>;
}
/// Deliberately does not implement Debug, avoiding accidental credential logs.
pub struct Secret(Vec<u8>);
impl Secret {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}
pub trait SecretStore {
    fn get<'a>(
        &'a self,
        key: &'a str,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<Option<Secret>, DomainError>>;
    fn set<'a>(
        &'a self,
        key: &'a str,
        value: Secret,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>>;
    fn remove<'a>(
        &'a self,
        key: &'a str,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackCapabilities {
    pub pause: bool,
    pub seek: bool,
    pub gapless: bool,
    pub crossfade: bool,
    pub speed: bool,
    pub filters: bool,
    pub audio_analysis: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackCapability {
    Pause,
    Seek,
    Gapless,
    Crossfade,
    Speed,
    Filters,
    AudioAnalysis,
}
impl PlaybackCapabilities {
    /// Gate the operation before dispatch; a feature flag does not implement it.
    pub fn require(self, feature: PlaybackCapability) -> Result<(), DomainError> {
        let supported = match feature {
            PlaybackCapability::Pause => self.pause,
            PlaybackCapability::Seek => self.seek,
            PlaybackCapability::Gapless => self.gapless,
            PlaybackCapability::Crossfade => self.crossfade,
            PlaybackCapability::Speed => self.speed,
            PlaybackCapability::Filters => self.filters,
            PlaybackCapability::AudioAnalysis => self.audio_analysis,
        };
        if supported {
            Ok(())
        } else {
            Err(DomainError::Unsupported)
        }
    }
    pub fn check_command(self, command: &PlaybackCommand) -> Result<(), DomainError> {
        match command {
            PlaybackCommand::Pause | PlaybackCommand::Resume => {
                self.require(PlaybackCapability::Pause)
            }
            PlaybackCommand::Seek(_) => self.require(PlaybackCapability::Seek),
            PlaybackCommand::Play(_) | PlaybackCommand::Stop => Ok(()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackCommand {
    Play(QueueOccurrence),
    Pause,
    Resume,
    Stop,
    Seek(PlaybackPosition),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackEventKind {
    Ready,
    Playing,
    Paused,
    Stopped,
    Ended,
    Position(PlaybackPosition),
    Failed(DomainError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackEvent {
    pub context: RequestContext,
    pub occurrence: QueueOccurrenceId,
    pub kind: PlaybackEventKind,
}
/// Sources and credentials are resolved inside the engine adapter, not logged in commands.
pub trait Playback {
    fn capabilities(&self) -> PlaybackCapabilities;
    fn command<'a>(
        &'a self,
        command: PlaybackCommand,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<(), DomainError>>;
    /// Returns one correlated event, with deadline/cancellation rather than a blocking wait.
    fn next_event<'a>(
        &'a self,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<PlaybackEvent, DomainError>>;
}
