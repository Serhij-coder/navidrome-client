//! Normalized library values derived from Feishin 7d6ad318 shared/types/domain-types.ts.
//! These are not wire DTOs. Adapters validate and normalize source values.
use crate::*;
use std::collections::BTreeMap;

/// Optional nullable source fields distinguish omission from explicit null.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field<T> {
    Absent,
    Null,
    Value(T),
}

/// Structured extension data (smart-playlist rules), interpreted by adapters.
#[derive(Clone, Debug, PartialEq)]
pub enum MetadataValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<MetadataValue>),
    Object(BTreeMap<String, MetadataValue>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YearRange {
    pub min: i32,
    pub max: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GainInfo {
    pub album: Option<f64>,
    pub track: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelatedArtist {
    pub id: ArtistId,
    pub image_id: Option<String>,
    pub image_url: Option<String>,
    pub name: String,
    pub user_favorite: bool,
    pub user_rating: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelatedAlbumArtist {
    pub id: ArtistId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Genre {
    pub server: ServerId,
    pub server_type: ServerType,
    pub album_count: Option<u64>,
    pub id: GenreId,
    pub image_id: Option<String>,
    pub image_url: Option<String>,
    pub name: String,
    pub song_count: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MusicFolder {
    pub server: ServerId,
    pub id: FolderId,
    pub name: String,
}

/// An unlocalized role filter choice; a server can also return an empty named role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtistRoleChoice {
    AllArtists,
    Role(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomSongQuery {
    pub genre: Option<String>,
    pub limit: Option<u64>,
    pub max_year: Option<i32>,
    pub min_year: Option<i32>,
    pub music_folder_ids: Option<Vec<FolderId>>,
    pub played: Played,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimilarSongsQuery {
    pub count: Option<u64>,
    pub music_folder_ids: Option<Vec<FolderId>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct User {
    pub server: ServerId,
    pub created_at: Option<String>,
    pub email: Option<String>,
    pub id: UserId,
    pub is_admin: Option<bool>,
    pub last_login_at: Option<String>,
    pub name: String,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Album {
    pub server: ServerId,
    pub server_type: ServerType,
    pub album_artist_name: String,
    pub album_artists: Vec<RelatedArtist>,
    pub artists: Vec<RelatedArtist>,
    pub blur_hash: Option<String>,
    pub comment: Option<String>,
    pub created_at: String,
    pub discs: Option<BTreeMap<u32, String>>,
    pub dominant_color: Option<String>,
    pub duration: Option<Milliseconds>,
    pub explicit_status: Option<ExplicitStatus>,
    pub gain: Option<GainInfo>,
    pub genres: Vec<Genre>,
    pub id: AlbumId,
    pub image_id: Option<String>,
    pub image_url: Option<String>,
    pub is_compilation: Option<bool>,
    pub last_played_at: Option<String>,
    pub mbz_id: Option<String>,
    pub mbz_release_group_id: Option<String>,
    pub missing: Option<bool>,
    pub name: String,
    pub original_date: Option<String>,
    pub original_year: Option<i32>,
    pub participants: Option<BTreeMap<String, Vec<RelatedArtist>>>,
    pub peak: Option<GainInfo>,
    pub play_count: Option<u64>,
    pub rated_at: Option<String>,
    pub record_labels: Vec<String>,
    pub release_date: Option<String>,
    pub release_type: Option<String>,
    pub release_types: Vec<String>,
    pub release_year: Option<i32>,
    pub size: Option<u64>,
    pub song_count: Option<u64>,
    pub songs: Option<Vec<Song>>,
    pub sort_name: String,
    pub starred_at: Option<String>,
    pub tags: Option<BTreeMap<String, Vec<String>>>,
    pub thumb_hash: Option<String>,
    pub track_year_range: Option<YearRange>,
    pub updated_at: String,
    pub user_favorite: bool,
    pub user_rating: Option<f64>,
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlbumArtist {
    pub server: ServerId,
    pub server_type: ServerType,
    pub album_count: Option<u64>,
    pub biography: Option<String>,
    pub blur_hash: Option<String>,
    pub dominant_color: Option<String>,
    pub duration: Option<Milliseconds>,
    pub genres: Vec<Genre>,
    pub id: ArtistId,
    pub image_id: Option<String>,
    pub image_url: Option<String>,
    pub last_played_at: Option<String>,
    pub mbz: Option<String>,
    pub missing: Option<bool>,
    pub name: String,
    pub play_count: Option<u64>,
    pub rated_at: Option<String>,
    pub similar_artists: Option<Vec<RelatedArtist>>,
    pub song_count: Option<u64>,
    pub starred_at: Option<String>,
    pub thumb_hash: Option<String>,
    pub uploaded_image: Option<String>,
    pub user_favorite: bool,
    pub user_rating: Option<f64>,
}

/// Normalized artist detail; absent albums differ from an explicitly empty list.
#[derive(Clone, Debug, PartialEq)]
pub struct AlbumArtistDetail {
    pub artist: AlbumArtist,
    pub albums: Option<Vec<Album>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    pub server: ServerId,
    pub server_type: ServerType,
    pub blur_hash: Option<String>,
    pub description: Option<String>,
    pub dominant_color: Option<String>,
    pub duration: Option<Milliseconds>,
    pub evaluated_at: Option<String>,
    pub genres: Vec<Genre>,
    pub id: PlaylistId,
    pub image_id: Option<String>,
    pub image_url: Option<String>,
    pub name: String,
    pub owner: Option<String>,
    pub owner_id: Option<UserId>,
    pub public: Option<bool>,
    pub rules: Field<BTreeMap<String, MetadataValue>>,
    pub size: Option<u64>,
    pub song_count: Option<u64>,
    pub sync: Field<bool>,
    pub thumb_hash: Option<String>,
    pub uploaded_image: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub server: ServerId,
    pub server_type: ServerType,
    pub album: Option<String>,
    pub album_artist_name: String,
    pub album_artists: Vec<RelatedArtist>,
    pub album_id: AlbumId,
    pub artist_name: String,
    pub artists: Vec<RelatedArtist>,
    pub bit_depth: Option<u64>,
    pub bit_rate: u64,
    pub blur_hash: Option<String>,
    pub bpm: Option<f64>,
    pub channels: Option<u64>,
    pub codec: Option<String>,
    pub comment: Option<String>,
    pub compilation: Option<bool>,
    pub container: Option<String>,
    pub created_at: String,
    pub date: Option<String>,
    pub disc_number: u64,
    pub disc_subtitle: Option<String>,
    pub duration: Milliseconds,
    pub explicit_status: Option<ExplicitStatus>,
    pub folder_id: Option<FolderId>,
    pub gain: Option<GainInfo>,
    pub genres: Vec<Genre>,
    pub id: SongId,
    pub image_id: Option<String>,
    pub image_url: Option<String>,
    pub is_resumable: Option<bool>,
    pub last_played_at: Option<String>,
    pub library_id: Option<u64>,
    pub library_name: Option<String>,
    pub lyrics: Option<String>,
    pub mbz_album_id: Option<String>,
    pub mbz_album_type: Option<String>,
    pub mbz_recording_id: Option<String>,
    pub mbz_release_group_id: Option<String>,
    pub mbz_track_id: Option<String>,
    pub missing: Option<bool>,
    pub name: String,
    pub original_date: Option<String>,
    pub original_year: Option<i32>,
    pub participants: Option<BTreeMap<String, Vec<RelatedArtist>>>,
    pub path: Option<String>,
    pub peak: Option<GainInfo>,
    pub play_count: u64,
    pub playlist_item_id: Option<PlaylistEntryId>,
    pub release_date: Option<String>,
    pub release_year: Option<i32>,
    pub resume_position_ms: Option<PlaybackPosition>,
    pub sample_rate: Option<u64>,
    pub size: u64,
    pub sort_name: String,
    pub tags: Option<BTreeMap<String, Vec<String>>>,
    pub thumb_hash: Option<String>,
    pub track_number: u64,
    pub track_subtitle: Option<String>,
    pub updated_at: String,
    pub user_favorite: bool,
    pub user_rating: Option<f64>,
    pub year: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InternetRadioStation {
    pub server: ServerId,
    pub homepage_url: Option<String>,
    pub id: RadioStationId,
    pub image_id: Field<String>,
    pub image_url: Field<String>,
    pub name: String,
    pub stream_url: String,
    pub thumb_hash: Field<String>,
    pub uploaded_image: Field<String>,
}

/// The artist category is distinct from an album artist even when metadata matches.
#[derive(Clone, Debug, PartialEq)]
pub struct Artist(pub AlbumArtist);
#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    pub identity: Scoped<FolderId>,
    pub server_type: ServerType,
    pub name: String,
    pub parent_id: Option<FolderId>,
    pub image_id: Field<String>,
    pub image_url: Field<String>,
    pub children: Option<FolderChildren>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FolderChildren {
    pub folders: Vec<Folder>,
    pub songs: Vec<Song>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct QueueSong {
    pub occurrence: QueueOccurrenceId,
    pub song: Song,
    pub context_playlist: Field<PlaylistId>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlaylistSong {
    pub entry: PlaylistEntryId,
    pub song: Song,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub start_index: u64,
    pub total_record_count: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongSummary {
    pub identity: Scoped<SongId>,
    pub name: String,
    pub duration: Milliseconds,
}

/// Common normalized list dimensions. None and empty lists remain distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListQuery<S, F> {
    pub server: ServerId,
    pub sort_by: S,
    pub sort_order: SortOrder,
    pub start_index: u64,
    pub limit: Option<u64>,
    pub search_term: Option<String>,
    pub filter: F,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlbumFilter {
    pub artist_ids: Option<Vec<ArtistId>>,
    pub compilation: Option<bool>,
    pub favorite: Option<bool>,
    pub genre_ids: Option<Vec<GenreId>>,
    pub min_year: Option<i32>,
    pub max_year: Option<i32>,
    pub music_folder_ids: Option<Vec<FolderId>>,
    pub has_rating: Option<bool>,
    pub is_recently_played: Option<bool>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SongFilter {
    pub album_artist_ids: Option<Vec<ArtistId>>,
    pub album_ids: Option<Vec<AlbumId>>,
    pub artist_ids: Option<Vec<ArtistId>>,
    pub favorite: Option<bool>,
    pub genre_ids: Option<Vec<GenreId>>,
    pub has_rating: Option<bool>,
    pub image_size: Option<u32>,
    pub min_year: Option<i32>,
    pub max_year: Option<i32>,
    pub music_folder_ids: Option<Vec<FolderId>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtistFilter {
    pub favorite: Option<bool>,
    pub music_folder_ids: Option<Vec<FolderId>>,
    pub role: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlbumArtistFilter {
    pub favorite: Option<bool>,
    pub music_folder_ids: Option<Vec<FolderId>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenreFilter {
    pub music_folder_ids: Option<Vec<FolderId>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaylistFilter {
    pub exclude_smart_playlists: Option<bool>,
}
pub type AlbumListQuery = ListQuery<AlbumListSort, AlbumFilter>;
pub type SongListQuery = ListQuery<SongListSort, SongFilter>;
pub type ArtistListQuery = ListQuery<ArtistListSort, ArtistFilter>;
pub type AlbumArtistListQuery = ListQuery<AlbumArtistListSort, AlbumArtistFilter>;
pub type GenreListQuery = ListQuery<GenreListSort, GenreFilter>;
pub type PlaylistListQuery = ListQuery<PlaylistListSort, PlaylistFilter>;
pub type UserListQuery = ListQuery<UserListSort, ()>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderQuery {
    pub identity: Scoped<FolderId>,
    pub sort_by: SongListSort,
    pub sort_order: SortOrder,
    pub music_folder_ids: Option<Vec<FolderId>>,
    pub search_term: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagListQuery {
    pub server: ServerId,
    pub folder: Option<FolderId>,
    pub tag_name: Option<String>,
    pub item_type: CollectionType,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionType {
    Album,
    Song,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedCollection {
    pub id: CollectionId,
    pub name: String,
    pub item_type: CollectionType,
    pub filter_query_string: String,
}

/// Non-secret server profile. URLs must be validated at the adapter/service boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerProfile {
    pub id: ServerId,
    pub name: String,
    pub server_type: ServerType,
    pub url: String,
    pub username: String,
    pub user_id: Option<UserId>,
    pub version: Option<String>,
    pub features: Option<Capabilities>,
    pub is_admin: Option<bool>,
    pub music_folder_ids: Option<Vec<FolderId>>,
    pub enable_audiobooks: Option<bool>,
    pub prefer_instant_mix: Option<bool>,
    pub prefer_remote_url: Option<bool>,
    pub remote_url: Option<String>,
    pub save_password: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub options: Vec<TagOption>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagOption {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumInfo {
    pub image_url: Option<String>,
    pub notes: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AlbumArtistInfo {
    pub biography: Field<String>,
    pub image_url: Field<String>,
    pub similar_artists: Option<Vec<RelatedArtist>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanStatus {
    pub count: u64,
    pub folder_count: u64,
    pub last_scan: Option<String>,
    pub scanning: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynchronizedLyricLine {
    pub cue_lines: Option<Vec<SyncedCueLine>>,
    pub start: Milliseconds,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncedCueLine {
    pub agent_id: Option<String>,
    pub end: Milliseconds,
    pub index: u64,
    pub start: Milliseconds,
    pub value: String,
    pub words: Vec<SyncedWordCue>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncedWordCue {
    pub end: Milliseconds,
    pub start: Milliseconds,
    pub text: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricRole {
    Background,
    Group,
    Main,
    Voice,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricAgent {
    pub id: String,
    pub name: Option<String>,
    pub role: LyricRole,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricsKind {
    Main,
    Pronunciation,
    Translation,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lyrics {
    Plain(String),
    Synchronized(Vec<SynchronizedLyricLine>),
}
/// Signed offsets permit advancing as well as delaying lyrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LyricOffsetMs(pub i64);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricsMetadata {
    pub artist: String,
    pub name: String,
    pub offset: Field<LyricOffsetMs>,
    pub remote: bool,
    pub source: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullLyricsMetadata {
    pub metadata: LyricsMetadata,
    pub lyrics: Lyrics,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructuredLyrics {
    Synchronized {
        metadata: LyricsMetadata,
        lyrics: Vec<SynchronizedLyricLine>,
        language: String,
        agents: Option<Vec<LyricAgent>>,
        kind: Option<LyricsKind>,
    },
    Unsynchronized {
        metadata: LyricsMetadata,
        lyrics: String,
        language: String,
    },
}

/// Independent result-family windows for the shared upstream search contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchQuery {
    pub server: ServerId,
    pub album_artist_limit: Option<u64>,
    pub album_artist_start_index: Option<u64>,
    pub album_limit: Option<u64>,
    pub album_start_index: Option<u64>,
    pub music_folder_ids: Option<Vec<FolderId>>,
    pub query: Option<String>,
    pub song_limit: Option<u64>,
    pub song_start_index: Option<u64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResults {
    pub album_artists: Vec<AlbumArtist>,
    pub albums: Vec<Album>,
    pub songs: Vec<Song>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FavoriteSongMode {
    #[default]
    Favorite,
    Rating,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FavoriteSongListQuery {
    pub artist: Scoped<ArtistId>,
    pub limit: Option<u64>,
    pub mode: FavoriteSongMode,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TopSongMode {
    #[default]
    Community,
    Personal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopSongListQuery {
    pub artist: Scoped<ArtistId>,
    pub artist_name: String,
    pub limit: Option<u64>,
    pub mode: TopSongMode,
}

/// A favorite request groups one supported identity family. Playlist songs use
/// their song IDs, never positional playlist-entry or queue-occurrence IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FavoriteTarget {
    Albums(Vec<AlbumId>),
    AlbumArtists(Vec<ArtistId>),
    Artists(Vec<ArtistId>),
    Songs(Vec<SongId>),
    PlaylistSongs(Vec<SongId>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FavoriteMutation {
    pub server: ServerId,
    pub target: FavoriteTarget,
}
/// Rating's source controller forwards every ID without an item-type branch.
/// Contextual song variants still carry song IDs rather than occurrence IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryEntityId {
    Album(AlbumId),
    AlbumArtist(ArtistId),
    Artist(ArtistId),
    Folder(FolderId),
    Genre(GenreId),
    Playlist(PlaylistId),
    PlaylistSong(SongId),
    QueueSong(SongId),
    RadioStation(RadioStationId),
    Song(SongId),
}
impl LibraryEntityId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Album(id) => id.as_str(),
            Self::AlbumArtist(id) | Self::Artist(id) => id.as_str(),
            Self::Folder(id) => id.as_str(),
            Self::Genre(id) => id.as_str(),
            Self::Playlist(id) => id.as_str(),
            Self::PlaylistSong(id) | Self::QueueSong(id) | Self::Song(id) => id.as_str(),
            Self::RadioStation(id) => id.as_str(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct RatingMutation {
    pub server: ServerId,
    pub items: Vec<LibraryEntityId>,
    pub rating: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedPlaylist {
    pub identity: Scoped<PlaylistId>,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistMetadataMutation {
    pub playlist: Scoped<PlaylistId>,
    pub name: String,
    pub comment: Option<String>,
    pub public: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistSongsMutation {
    pub playlist: Scoped<PlaylistId>,
    pub songs: Vec<SongId>,
}
/// A wire removal position, not a durable entry identity or a mutation handle
/// derived automatically from an old playlist snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaylistIndex(pub u64);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistPositionsMutation {
    pub playlist: Scoped<PlaylistId>,
    pub positions: Vec<PlaylistIndex>,
}
