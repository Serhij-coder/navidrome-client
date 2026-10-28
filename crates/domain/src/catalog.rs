#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryItem {
    Album,
    AlbumArtist,
    Artist,
    Folder,
    Genre,
    Playlist,
    PlaylistSong,
    QueueSong,
    RadioStation,
    Song,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerType {
    Jellyfin,
    Navidrome,
    Subsonic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplicitStatus {
    Clean,
    Explicit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalSource {
    Lastfm,
    Musicbrainz,
    Spotify,
    Theaudiodb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalType {
    Id,
    Link,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenreListSort {
    AlbumCount,
    Name,
    SongCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageType {
    Backdrop,
    Logo,
    Primary,
    Screenshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagListSort {
    AlbumCount,
    Name,
    SongCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlbumListSort {
    AlbumArtist,
    Artist,
    CommunityRating,
    CriticRating,
    Duration,
    ExplicitStatus,
    Favorited,
    Id,
    Name,
    PlayCount,
    Random,
    Rating,
    RecentlyAdded,
    RecentlyPlayed,
    ReleaseDate,
    SongCount,
    SortName,
    Year,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongListSort {
    Album,
    AlbumArtist,
    Artist,
    Bpm,
    Channels,
    Comment,
    Duration,
    ExplicitStatus,
    Favorited,
    Genre,
    Id,
    Name,
    PlayCount,
    Random,
    Rating,
    RecentlyAdded,
    RecentlyPlayed,
    ReleaseDate,
    ReleaseYear,
    SortName,
    Year,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlbumArtistListSort {
    Album,
    AlbumCount,
    Duration,
    Favorited,
    Name,
    PlayCount,
    Random,
    Rating,
    RecentlyAdded,
    ReleaseDate,
    SongCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtistListSort {
    Album,
    AlbumCount,
    Duration,
    Favorited,
    Name,
    PlayCount,
    Random,
    Rating,
    RecentlyAdded,
    ReleaseDate,
    SongCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistListSort {
    Duration,
    Name,
    Owner,
    Public,
    SongCount,
    UpdatedAt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioListSort {
    Id,
    Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserListSort {
    Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Played {
    All,
    Never,
    Played,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricSource {
    Genius,
    Lrclib,
    Netease,
    Simpmusic,
}

impl LibraryItem {
    pub const ALL: &'static [Self] = &[
        Self::Album,
        Self::AlbumArtist,
        Self::Artist,
        Self::Folder,
        Self::Genre,
        Self::Playlist,
        Self::PlaylistSong,
        Self::QueueSong,
        Self::RadioStation,
        Self::Song,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::AlbumArtist => "albumArtist",
            Self::Artist => "artist",
            Self::Folder => "folder",
            Self::Genre => "genre",
            Self::Playlist => "playlist",
            Self::PlaylistSong => "playlistSong",
            Self::QueueSong => "queueSong",
            Self::RadioStation => "radioStation",
            Self::Song => "song",
        }
    }
}

impl ServerType {
    pub const ALL: &'static [Self] = &[Self::Jellyfin, Self::Navidrome, Self::Subsonic];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jellyfin => "jellyfin",
            Self::Navidrome => "navidrome",
            Self::Subsonic => "subsonic",
        }
    }
}

impl SortOrder {
    pub const ALL: &'static [Self] = &[Self::Asc, Self::Desc];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

impl ExplicitStatus {
    pub const ALL: &'static [Self] = &[Self::Clean, Self::Explicit];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "CLEAN",
            Self::Explicit => "EXPLICIT",
        }
    }
}

impl ExternalSource {
    pub const ALL: &'static [Self] = &[
        Self::Lastfm,
        Self::Musicbrainz,
        Self::Spotify,
        Self::Theaudiodb,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lastfm => "LASTFM",
            Self::Musicbrainz => "MUSICBRAINZ",
            Self::Spotify => "SPOTIFY",
            Self::Theaudiodb => "THEAUDIODB",
        }
    }
}

impl ExternalType {
    pub const ALL: &'static [Self] = &[Self::Id, Self::Link];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Id => "ID",
            Self::Link => "LINK",
        }
    }
}

impl GenreListSort {
    pub const ALL: &'static [Self] = &[Self::AlbumCount, Self::Name, Self::SongCount];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlbumCount => "albumCount",
            Self::Name => "name",
            Self::SongCount => "songCount",
        }
    }
}

impl ImageType {
    pub const ALL: &'static [Self] = &[Self::Backdrop, Self::Logo, Self::Primary, Self::Screenshot];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Backdrop => "BACKDROP",
            Self::Logo => "LOGO",
            Self::Primary => "PRIMARY",
            Self::Screenshot => "SCREENSHOT",
        }
    }
}

impl TagListSort {
    pub const ALL: &'static [Self] = &[Self::AlbumCount, Self::Name, Self::SongCount];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlbumCount => "albumCount",
            Self::Name => "name",
            Self::SongCount => "songCount",
        }
    }
}

impl AlbumListSort {
    pub const ALL: &'static [Self] = &[
        Self::AlbumArtist,
        Self::Artist,
        Self::CommunityRating,
        Self::CriticRating,
        Self::Duration,
        Self::ExplicitStatus,
        Self::Favorited,
        Self::Id,
        Self::Name,
        Self::PlayCount,
        Self::Random,
        Self::Rating,
        Self::RecentlyAdded,
        Self::RecentlyPlayed,
        Self::ReleaseDate,
        Self::SongCount,
        Self::SortName,
        Self::Year,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlbumArtist => "albumArtist",
            Self::Artist => "artist",
            Self::CommunityRating => "communityRating",
            Self::CriticRating => "criticRating",
            Self::Duration => "duration",
            Self::ExplicitStatus => "explicitStatus",
            Self::Favorited => "favorited",
            Self::Id => "id",
            Self::Name => "name",
            Self::PlayCount => "playCount",
            Self::Random => "random",
            Self::Rating => "rating",
            Self::RecentlyAdded => "recentlyAdded",
            Self::RecentlyPlayed => "recentlyPlayed",
            Self::ReleaseDate => "releaseDate",
            Self::SongCount => "songCount",
            Self::SortName => "sortName",
            Self::Year => "year",
        }
    }
}

impl SongListSort {
    pub const ALL: &'static [Self] = &[
        Self::Album,
        Self::AlbumArtist,
        Self::Artist,
        Self::Bpm,
        Self::Channels,
        Self::Comment,
        Self::Duration,
        Self::ExplicitStatus,
        Self::Favorited,
        Self::Genre,
        Self::Id,
        Self::Name,
        Self::PlayCount,
        Self::Random,
        Self::Rating,
        Self::RecentlyAdded,
        Self::RecentlyPlayed,
        Self::ReleaseDate,
        Self::ReleaseYear,
        Self::SortName,
        Self::Year,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::AlbumArtist => "albumArtist",
            Self::Artist => "artist",
            Self::Bpm => "bpm",
            Self::Channels => "channels",
            Self::Comment => "comment",
            Self::Duration => "duration",
            Self::ExplicitStatus => "explicitStatus",
            Self::Favorited => "favorited",
            Self::Genre => "genre",
            Self::Id => "id",
            Self::Name => "name",
            Self::PlayCount => "playCount",
            Self::Random => "random",
            Self::Rating => "rating",
            Self::RecentlyAdded => "recentlyAdded",
            Self::RecentlyPlayed => "recentlyPlayed",
            Self::ReleaseDate => "releaseDate",
            Self::ReleaseYear => "releaseYear",
            Self::SortName => "sortName",
            Self::Year => "year",
        }
    }
}

impl AlbumArtistListSort {
    pub const ALL: &'static [Self] = &[
        Self::Album,
        Self::AlbumCount,
        Self::Duration,
        Self::Favorited,
        Self::Name,
        Self::PlayCount,
        Self::Random,
        Self::Rating,
        Self::RecentlyAdded,
        Self::ReleaseDate,
        Self::SongCount,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::AlbumCount => "albumCount",
            Self::Duration => "duration",
            Self::Favorited => "favorited",
            Self::Name => "name",
            Self::PlayCount => "playCount",
            Self::Random => "random",
            Self::Rating => "rating",
            Self::RecentlyAdded => "recentlyAdded",
            Self::ReleaseDate => "releaseDate",
            Self::SongCount => "songCount",
        }
    }
}

impl ArtistListSort {
    pub const ALL: &'static [Self] = &[
        Self::Album,
        Self::AlbumCount,
        Self::Duration,
        Self::Favorited,
        Self::Name,
        Self::PlayCount,
        Self::Random,
        Self::Rating,
        Self::RecentlyAdded,
        Self::ReleaseDate,
        Self::SongCount,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::AlbumCount => "albumCount",
            Self::Duration => "duration",
            Self::Favorited => "favorited",
            Self::Name => "name",
            Self::PlayCount => "playCount",
            Self::Random => "random",
            Self::Rating => "rating",
            Self::RecentlyAdded => "recentlyAdded",
            Self::ReleaseDate => "releaseDate",
            Self::SongCount => "songCount",
        }
    }
}

impl PlaylistListSort {
    pub const ALL: &'static [Self] = &[
        Self::Duration,
        Self::Name,
        Self::Owner,
        Self::Public,
        Self::SongCount,
        Self::UpdatedAt,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Duration => "duration",
            Self::Name => "name",
            Self::Owner => "owner",
            Self::Public => "public",
            Self::SongCount => "songCount",
            Self::UpdatedAt => "updatedAt",
        }
    }
}

impl RadioListSort {
    pub const ALL: &'static [Self] = &[Self::Id, Self::Name];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Name => "name",
        }
    }
}

impl UserListSort {
    pub const ALL: &'static [Self] = &[Self::Name];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
        }
    }
}

impl Played {
    pub const ALL: &'static [Self] = &[Self::All, Self::Never, Self::Played];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Never => "never",
            Self::Played => "played",
        }
    }
}

impl LyricSource {
    pub const ALL: &'static [Self] = &[Self::Genius, Self::Lrclib, Self::Netease, Self::Simpmusic];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Genius => "Genius",
            Self::Lrclib => "lrclib.net",
            Self::Netease => "NetEase",
            Self::Simpmusic => "SimpMusic",
        }
    }
}
