//! Portable domain contracts. IDs from different domains cannot be interchanged.
//!
//! ```compile_fail
//! use feishin_domain::{SongId, QueueOccurrenceId};
//! let song = SongId::new("same");
//! let _: QueueOccurrenceId = song;
//! ```
//!
//! ```compile_fail
//! use feishin_domain::{ServerId, DeviceId};
//! let _: DeviceId = ServerId::new("same");
//! ```
//!
//! ```compile_fail
//! use feishin_domain::{PlaylistId, PlaylistEntryId};
//! let _: PlaylistEntryId = PlaylistId::new("same");
//! ```
//!
//! ```compile_fail
//! use feishin_domain::{PlaybackPosition, ListenedDuration, Milliseconds};
//! let _: ListenedDuration = PlaybackPosition(Milliseconds(2));
//! ```
use std::collections::BTreeMap;

macro_rules! id {
    ($($name:ident),+ $(,)?) => {$ (
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
        }
    )+};
}
id!(
    ServerId,
    SongId,
    QueueOccurrenceId,
    PlaylistEntryId,
    DeviceId,
    AlbumId,
    ArtistId,
    PlaylistId,
    GenreId,
    FolderId,
    RadioStationId,
    UserId,
    CollectionId
);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scoped<T> {
    pub server: ServerId,
    pub id: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitError {
    Negative,
    NonFinite,
    Overflow,
}
/// Finite, nonnegative seconds; conversion to integer units is checked.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Seconds(f64);
impl Seconds {
    pub fn new(value: f64) -> Result<Self, UnitError> {
        if !value.is_finite() {
            return Err(UnitError::NonFinite);
        }
        if value < 0.0 {
            return Err(UnitError::Negative);
        }
        Ok(Self(value))
    }
    pub fn get(self) -> f64 {
        self.0
    }
    pub fn to_milliseconds(self) -> Result<Milliseconds, UnitError> {
        Milliseconds::from_seconds(self.0)
    }
    pub fn to_ticks(self) -> Result<Ticks, UnitError> {
        Ticks::from_seconds(self.0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Milliseconds(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ticks(pub u64);
impl Milliseconds {
    /// Fractional milliseconds are truncated towards zero.
    pub fn from_seconds(seconds: f64) -> Result<Self, UnitError> {
        if !seconds.is_finite() {
            return Err(UnitError::NonFinite);
        }
        if seconds < 0.0 {
            return Err(UnitError::Negative);
        }
        let ms = seconds * 1000.0;
        if ms >= u64::MAX as f64 {
            return Err(UnitError::Overflow);
        }
        Ok(Self(ms as u64))
    }
    pub fn to_ticks(self) -> Result<Ticks, UnitError> {
        self.0
            .checked_mul(10_000)
            .map(Ticks)
            .ok_or(UnitError::Overflow)
    }
}
impl Ticks {
    /// Fractional ticks are truncated towards zero.
    pub fn from_seconds(seconds: f64) -> Result<Self, UnitError> {
        if !seconds.is_finite() {
            return Err(UnitError::NonFinite);
        }
        if seconds < 0.0 {
            return Err(UnitError::Negative);
        }
        let ticks = seconds * 10_000_000.0;
        if ticks >= u64::MAX as f64 {
            return Err(UnitError::Overflow);
        }
        Ok(Self(ticks as u64))
    }
    pub fn to_milliseconds(self) -> Milliseconds {
        Milliseconds(self.0 / 10_000)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackPosition(pub Milliseconds);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListenedDuration(pub Milliseconds);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServerFeature {
    AlbumYesNoRatingFilter,
    ArtistImageUpload,
    Bfr,
    InternetRadioImageUpload,
    Jukebox,
    LyricsMultipleStructured,
    LyricsSingleStructured,
    MusicFolderMultiselect,
    OsFormPost,
    OsTranscodeDecision,
    PlaylistImageUpload,
    PlaylistsSmart,
    PublicPlaylist,
    ReportPlayback,
    ServerPlayQueue,
    SharingAlbumSong,
    SimilarSongsMusicFolder,
    Tags,
    TopSongsByArtistId,
    TrackAlbumArtistSearch,
    TrackYesNoRatingFilter,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities(pub BTreeMap<ServerFeature, Vec<u32>>);
impl Capabilities {
    pub fn has(&self, feature: ServerFeature) -> bool {
        self.0.get(&feature).is_some_and(|v| !v.is_empty())
    }
    pub fn has_version(&self, feature: ServerFeature, version: u32) -> bool {
        self.0.get(&feature).is_some_and(|v| v.contains(&version))
    }
}

macro_rules! counter {
    ($($name:ident),+) => {$ (
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(pub u64);
        impl $name { pub fn next(self) -> Result<Self, DomainError> { self.0.checked_add(1).map(Self).ok_or(DomainError::Exhausted) } }
    )+};
}
counter!(Revision, Generation, RequestId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainError {
    Cancelled,
    Deadline,
    Unsupported,
    InvalidInput,
    Authentication,
    Network,
    WireParse,
    Protocol,
    Rejected,
    Storage,
    Playback,
    Stale,
    Capacity,
    Exhausted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestContext {
    pub request: RequestId,
    pub generation: Generation,
    pub deadline: Milliseconds,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueOccurrence {
    pub id: QueueOccurrenceId,
    pub song: Scoped<SongId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    BeginLoad { deadline: Milliseconds },
    Add(QueueOccurrence),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Load { context: RequestContext },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Loaded {
        context: RequestContext,
        items: Vec<QueueOccurrence>,
    },
    LoadFailed {
        context: RequestContext,
        error: DomainError,
    },
}
/// A bounded example owner, not the P05 queue implementation.
pub struct Owner {
    revision: Revision,
    generation: Generation,
    request: RequestId,
    pending: Option<RequestContext>,
    items: Vec<QueueOccurrence>,
    capacity: usize,
}
impl Owner {
    pub fn new(capacity: usize) -> Self {
        Self {
            revision: Revision(0),
            generation: Generation(0),
            request: RequestId(0),
            pending: None,
            items: Vec::new(),
            capacity,
        }
    }
    pub fn items(&self) -> &[QueueOccurrence] {
        &self.items
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn command(&mut self, command: Command) -> Result<Option<Effect>, DomainError> {
        match command {
            Command::BeginLoad { deadline } => {
                let generation = self.generation.next()?;
                let request = self.request.next()?;
                let context = RequestContext {
                    request,
                    generation,
                    deadline,
                };
                self.generation = generation;
                self.request = request;
                self.pending = Some(context);
                Ok(Some(Effect::Load { context }))
            }
            Command::Add(item) => {
                if self.items.len() >= self.capacity {
                    return Err(DomainError::Capacity);
                }
                if self.items.iter().any(|v| v.id == item.id) {
                    return Err(DomainError::InvalidInput);
                }
                let revision = self.revision.next()?;
                self.items.push(item);
                self.revision = revision;
                // A local mutation invalidates an in-flight replacement.
                self.pending = None;
                Ok(None)
            }
        }
    }
    pub fn event(&mut self, event: Event, now: Milliseconds) -> Result<(), DomainError> {
        let context = match &event {
            Event::Loaded { context, .. } | Event::LoadFailed { context, .. } => *context,
        };
        if self.pending != Some(context) {
            return Err(DomainError::Stale);
        }
        if now > context.deadline {
            self.pending = None;
            return Err(DomainError::Deadline);
        }
        let items = match event {
            Event::Loaded { items, .. } => items,
            Event::LoadFailed { error, .. } => {
                self.pending = None;
                return Err(error);
            }
        };
        if items.len() > self.capacity {
            self.pending = None;
            return Err(DomainError::Capacity);
        }
        let mut ids = std::collections::BTreeSet::new();
        if items.iter().any(|item| !ids.insert(&item.id)) {
            self.pending = None;
            return Err(DomainError::InvalidInput);
        }
        let revision = self.revision.next()?;
        self.items = items;
        self.revision = revision;
        self.pending = None;
        Ok(())
    }
}

mod catalog;
mod models;
mod ports;
pub use catalog::*;
pub use models::*;
pub use ports::*;

impl ServerFeature {
    pub const ALL: &'static [Self] = &[
        Self::AlbumYesNoRatingFilter,
        Self::ArtistImageUpload,
        Self::Bfr,
        Self::InternetRadioImageUpload,
        Self::Jukebox,
        Self::LyricsMultipleStructured,
        Self::LyricsSingleStructured,
        Self::MusicFolderMultiselect,
        Self::OsFormPost,
        Self::OsTranscodeDecision,
        Self::PlaylistImageUpload,
        Self::PlaylistsSmart,
        Self::PublicPlaylist,
        Self::ReportPlayback,
        Self::ServerPlayQueue,
        Self::SharingAlbumSong,
        Self::SimilarSongsMusicFolder,
        Self::Tags,
        Self::TopSongsByArtistId,
        Self::TrackAlbumArtistSearch,
        Self::TrackYesNoRatingFilter,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlbumYesNoRatingFilter => "albumYesNoRatingFilter",
            Self::ArtistImageUpload => "artistImageUpload",
            Self::Bfr => "bfr",
            Self::InternetRadioImageUpload => "internetRadioImageUpload",
            Self::Jukebox => "jukebox",
            Self::LyricsMultipleStructured => "lyricsMultipleStructured",
            Self::LyricsSingleStructured => "lyricsSingleStructured",
            Self::MusicFolderMultiselect => "musicFolderMultiselect",
            Self::OsFormPost => "osFormPost",
            Self::OsTranscodeDecision => "osTranscodeDecision",
            Self::PlaylistImageUpload => "playlistImageUpload",
            Self::PlaylistsSmart => "playlistsSmart",
            Self::PublicPlaylist => "publicPlaylist",
            Self::ReportPlayback => "reportPlayback",
            Self::ServerPlayQueue => "serverPlayQueue",
            Self::SharingAlbumSong => "sharingAlbumSong",
            Self::SimilarSongsMusicFolder => "similarSongsMusicFolder",
            Self::Tags => "tags",
            Self::TopSongsByArtistId => "topSongsByArtistId",
            Self::TrackAlbumArtistSearch => "trackAlbumArtistSearch",
            Self::TrackYesNoRatingFilter => "trackYesNoRatingFilter",
        }
    }
}
