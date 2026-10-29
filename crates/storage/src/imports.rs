//! Bounded projections from explicitly supplied copies. No source discovery or writes.
//! This is a partial import, not a complete settings migration or playback reducer.
use crate::{BackendKind, MAX_BYTES, MAX_PROFILES, Profile, ProfileData};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_QUEUE: usize = 10_000;
pub const MAX_DIAGNOSTICS: usize = 64;
const MAX_COLUMNS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    InputTooLarge,
    OutputTooLarge,
    InvalidDocument,
    UnsupportedVersion,
    InvalidData,
}
impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "copied import: {self:?}")
    }
}
impl std::error::Error for ImportError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationData {
    pub profile: ProfileData,
    pub columns: Vec<ColumnList>,
    pub auto_dj: Option<AutoDj>,
    pub queue: Option<QueueSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsCategory {
    General,
    Lists,
    AutoDJ,
    Hotkeys,
    Playback,
    Lyrics,
    Window,
    Discord,
    Font,
    Css,
    Remote,
    QueryBuilder,
    Tab,
    LyricsDisplay,
    TagEditor,
    Visualizer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Settings(SettingsCategory),
    UnknownSettings,
    Authentication,
    Player,
    Queue,
    Timestamp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    PartialProjection,
    DeferredMigrations,
    HistoricalTransformApplied,
    LegacyDefaultsDeferred,
    UnsupportedCategory,
    UnsupportedFields,
    IgnoredSplitCopy,
    CorruptCategory,
    InvalidPreference,
    InvalidColumns,
    DuplicateColumn,
    CredentialsStripped,
    InvalidProfile,
    DuplicateProfile,
    OrphanSelection,
    QueueRejected,
    MissingSong,
    DuplicateOccurrence,
    OrphansRemoved,
    PriorityRepaired,
    ShuffleRejected,
    CurrentRejected,
    MetadataDropped,
    TimestampRejected,
}
/// Fixed codes and bounded ordinal locations only. Never contains copied keys or values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub category: Category,
    pub code: Code,
    pub list: Option<ListId>,
    pub index: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub partial: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted_diagnostics: usize,
}
impl ImportReport {
    fn new() -> Self {
        Self {
            partial: true,
            diagnostics: vec![],
            omitted_diagnostics: 0,
        }
    }
    fn add(&mut self, category: Category, code: Code, list: Option<ListId>, index: Option<usize>) {
        if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(Diagnostic {
                category,
                code,
                list,
                index,
            });
        } else {
            self.omitted_diagnostics += 1;
        }
    }
    fn note(&mut self, category: Category, code: Code) {
        self.add(category, code, None, None);
    }
}
#[derive(Default)]
pub struct CopiedInputs<'a> {
    pub settings: Option<&'a [u8]>,
    pub auth: Option<&'a [u8]>,
    pub player: Option<&'a [u8]>,
    pub separate_queue: Option<&'a [u8]>,
    pub timestamp: Option<&'a [u8]>,
    pub split_categories: BTreeMap<SettingsCategory, &'a [u8]>,
    /// Copied store_settings_version, strictly parsed by the caller. Missing means source v14.
    pub split_settings_version: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    candidate: MigrationData,
    report: ImportReport,
}
impl ImportPreview {
    pub fn data(&self) -> &MigrationData {
        &self.candidate
    }
    pub fn report(&self) -> &ImportReport {
        &self.report
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListId {
    Album,
    AlbumArtist,
    AlbumArtistAlbum,
    AlbumArtistSong,
    AlbumDetail,
    Artist,
    Folder,
    FullScreen,
    Genre,
    GenreAlbum,
    GenreSong,
    Playlist,
    PlaylistAlbum,
    PlaylistSong,
    QueueSong,
    Radio,
    SideQueue,
    Song,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColumnView {
    Table,
    Detail,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnList {
    pub list: ListId,
    pub view: ColumnView,
    pub columns: Vec<Column>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Alignment {
    Center,
    End,
    Start,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pin {
    Left,
    Right,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Column {
    pub id: ColumnId,
    pub align: Alignment,
    pub auto_size: Option<bool>,
    pub is_enabled: bool,
    pub pinned: Option<Pin>,
    pub width: Number,
}

// Closed vocabulary from pinned shared/types/types.ts TableColumn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ColumnId {
    #[serde(rename = "actions")]
    Actions,
    #[serde(rename = "album")]
    Album,
    #[serde(rename = "albumArtists")]
    AlbumArtist,
    #[serde(rename = "albumCount")]
    AlbumCount,
    #[serde(rename = "albumGroup")]
    AlbumGroup,
    #[serde(rename = "artists")]
    Artist,
    #[serde(rename = "biography")]
    Biography,
    #[serde(rename = "bitDepth")]
    BitDepth,
    #[serde(rename = "bitRate")]
    BitRate,
    #[serde(rename = "bpm")]
    Bpm,
    #[serde(rename = "channels")]
    Channels,
    #[serde(rename = "container")]
    Codec,
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "composer")]
    Composer,
    #[serde(rename = "date")]
    Date,
    #[serde(rename = "createdAt")]
    DateAdded,
    #[serde(rename = "discNumber")]
    DiscNumber,
    #[serde(rename = "duration")]
    Duration,
    #[serde(rename = "genres")]
    Genre,
    #[serde(rename = "genreBadge")]
    GenreBadge,
    #[serde(rename = "id")]
    Id,
    #[serde(rename = "imageUrl")]
    Image,
    #[serde(rename = "lastPlayedAt")]
    LastPlayed,
    #[serde(rename = "__layoutFill")]
    LayoutFill,
    #[serde(rename = "username")]
    Owner,
    #[serde(rename = "path")]
    Path,
    #[serde(rename = "playCount")]
    PlayCount,
    #[serde(rename = "playlistReorder")]
    PlaylistReorder,
    #[serde(rename = "releaseDate")]
    ReleaseDate,
    #[serde(rename = "releaseYear")]
    ReleaseYear,
    #[serde(rename = "rowIndex")]
    RowIndex,
    #[serde(rename = "sampleRate")]
    SampleRate,
    #[serde(rename = "size")]
    Size,
    #[serde(rename = "skip")]
    Skip,
    #[serde(rename = "songCount")]
    SongCount,
    #[serde(rename = "name")]
    Title,
    #[serde(rename = "titleArtist")]
    TitleArtist,
    #[serde(rename = "titleCombined")]
    TitleCombined,
    #[serde(rename = "trackNumber")]
    TrackNumber,
    #[serde(rename = "userFavorite")]
    UserFavorite,
    #[serde(rename = "userRating")]
    UserRating,
    #[serde(rename = "year")]
    Year,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoDjStrategy {
    Similar,
    LibraryRandom,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoDjMode {
    Songs,
    Albums,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AutoDj {
    pub album_strategy: AutoDjStrategy,
    pub allow_duplicates: bool,
    pub enabled: bool,
    pub item_count: Number,
    pub mode: AutoDjMode,
    pub only_similar: bool,
    pub song_strategy: AutoDjStrategy,
    pub timing: Number,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueOccurrence {
    pub occurrence_id: String,
    pub song_id: String,
    pub server_id: String,
    pub title: String,
    /// Source normalized song.duration is already milliseconds; checked truncation.
    pub duration_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumePosition {
    pub occurrence_id: String,
    pub milliseconds: u64,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueSnapshot {
    pub occurrences: Vec<QueueOccurrence>,
    pub priority: Vec<String>,
    /// Permutation of occurrence IDs. Empty means source shuffle disabled/rejected.
    pub shuffled: Vec<String>,
    pub current_occurrence: Option<String>,
    pub position: Option<ResumePosition>,
}

impl QueueOccurrence {
    pub fn occurrence_id(&self) -> feishin_domain::QueueOccurrenceId {
        feishin_domain::QueueOccurrenceId::new(self.occurrence_id.clone())
    }
    pub fn song(&self) -> feishin_domain::Scoped<feishin_domain::SongId> {
        feishin_domain::Scoped {
            server: feishin_domain::ServerId::new(self.server_id.clone()),
            id: feishin_domain::SongId::new(self.song_id.clone()),
        }
    }
    pub fn duration(&self) -> feishin_domain::Milliseconds {
        feishin_domain::Milliseconds(self.duration_ms)
    }
}
impl ResumePosition {
    pub fn occurrence_id(&self) -> feishin_domain::QueueOccurrenceId {
        feishin_domain::QueueOccurrenceId::new(self.occurrence_id.clone())
    }
    pub fn position(&self) -> feishin_domain::PlaybackPosition {
        feishin_domain::PlaybackPosition(feishin_domain::Milliseconds(self.milliseconds))
    }
}

fn parse(bytes: &[u8]) -> Result<Value, ImportError> {
    serde_json::from_slice(bytes).map_err(|_| ImportError::InvalidDocument)
}
fn object(v: &Value) -> Result<&serde_json::Map<String, Value>, ImportError> {
    v.as_object().ok_or(ImportError::InvalidDocument)
}
fn version(v: &Value, supported: &[u64]) -> Result<u64, ImportError> {
    let ver = v
        .get("version")
        .and_then(Value::as_u64)
        .ok_or(ImportError::InvalidDocument)?;
    if supported.contains(&ver) {
        Ok(ver)
    } else {
        Err(ImportError::UnsupportedVersion)
    }
}
fn bounded(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max
}
fn seconds_ms(v: &Value) -> Option<u64> {
    let seconds = v.as_f64()?;
    let ms = seconds * 1000.0;
    (seconds.is_finite() && seconds >= 0.0 && ms < u64::MAX as f64).then_some(ms as u64)
}
fn milliseconds(v: &Value) -> Option<u64> {
    let ms = v.as_f64()?;
    (ms.is_finite() && ms >= 0.0 && ms < u64::MAX as f64).then_some(ms as u64)
}
fn finite(n: &Number) -> bool {
    n.as_f64().is_some_and(f64::is_finite)
}
fn valid_columns(columns: &[Column]) -> bool {
    let mut ids = BTreeSet::new();
    columns.len() <= MAX_COLUMNS
        && columns.iter().all(|c| {
            ids.insert(c.id)
                && c.width
                    .as_f64()
                    .is_some_and(|w| w.is_finite() && (0.0..=16384.0).contains(&w))
        })
}
/// Revalidates durable values too, including old journal revisions. No silent repair.
pub fn validate_data(data: &MigrationData) -> Result<(), ImportError> {
    let p = &data.profile;
    let mut ids = BTreeSet::new();
    if p.profiles.len() > MAX_PROFILES
        || !p.preferences.valid()
        || p.profiles.iter().any(|p| !p.valid() || !ids.insert(&p.id))
        || p.current_server
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
    {
        return Err(ImportError::InvalidData);
    }
    let mut lists = BTreeSet::new();
    if data.columns.len() > 36
        || data
            .columns
            .iter()
            .any(|c| !lists.insert((c.list, c.view)) || !valid_columns(&c.columns))
        || data
            .auto_dj
            .as_ref()
            .is_some_and(|dj| !finite(&dj.item_count) || !finite(&dj.timing))
    {
        return Err(ImportError::InvalidData);
    }
    if let Some(q) = &data.queue {
        let mut occurrences = BTreeSet::new();
        if q.occurrences.len() > MAX_QUEUE
            || q.occurrences.iter().any(|o| {
                !bounded(&o.occurrence_id, 128)
                    || !bounded(&o.song_id, 128)
                    || !bounded(&o.server_id, 128)
                    || o.title.len() > 1024
                    || !occurrences.insert(&o.occurrence_id)
            })
        {
            return Err(ImportError::InvalidData);
        }
        let mut priorities = BTreeSet::new();
        let mut shuffled = BTreeSet::new();
        if q.priority.len() > q.occurrences.len()
            || q.priority
                .iter()
                .any(|id| !occurrences.contains(id) || !priorities.insert(id))
            || (!q.shuffled.is_empty()
                && (q.shuffled.len() != occurrences.len()
                    || q.shuffled
                        .iter()
                        .any(|id| !occurrences.contains(id) || !shuffled.insert(id))))
            || q.current_occurrence
                .as_ref()
                .is_some_and(|id| !occurrences.contains(id))
            || q.position
                .as_ref()
                .is_some_and(|pos| q.current_occurrence.as_ref() != Some(&pos.occurrence_id))
        {
            return Err(ImportError::InvalidData);
        }
    }
    if serde_json::to_vec(data)
        .map_err(|_| ImportError::InvalidData)?
        .len()
        > MAX_BYTES
    {
        return Err(ImportError::OutputTooLarge);
    }
    Ok(())
}

pub fn preview(
    input: &CopiedInputs<'_>,
    base: &MigrationData,
) -> Result<ImportPreview, ImportError> {
    let total = [
        input.settings,
        input.auth,
        input.player,
        input.separate_queue,
        input.timestamp,
    ]
    .into_iter()
    .flatten()
    .map(<[u8]>::len)
    .chain(input.split_categories.values().map(|v| v.len()))
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or(ImportError::InputTooLarge)?;
    if total > MAX_INPUT_BYTES {
        return Err(ImportError::InputTooLarge);
    }
    validate_data(base)?;
    let mut candidate = base.clone();
    let mut report = ImportReport::new();
    report.note(Category::UnknownSettings, Code::PartialProjection);
    let mut legacy_settings = false;
    let mut auto_dj_supplied = false;
    // A supplied general split copy makes split storage authoritative, as in source.
    if input
        .split_categories
        .contains_key(&SettingsCategory::General)
        || input.settings.is_none()
    {
        if !input.split_categories.is_empty() {
            let ver = input.split_settings_version.unwrap_or(14);
            if !(14..=34).contains(&ver) {
                return Err(ImportError::UnsupportedVersion);
            }
            if ver < 34 {
                report.note(Category::UnknownSettings, Code::DeferredMigrations);
            }
            legacy_settings |= ver < 28;
            for (&category, bytes) in &input.split_categories {
                auto_dj_supplied |= category == SettingsCategory::AutoDJ;
                match parse(bytes) {
                    Ok(value) => {
                        project_category(category, &value, ver, &mut candidate, &mut report)
                    }
                    Err(_) => report.note(Category::Settings(category), Code::CorruptCategory),
                }
            }
        }
    } else if let Some(bytes) = input.settings {
        let value = parse(bytes)?;
        let ver = value
            .get("version")
            .and_then(Value::as_u64)
            .ok_or(ImportError::InvalidDocument)?;
        if !(14..=34).contains(&ver) {
            return Err(ImportError::UnsupportedVersion);
        }
        let state = value.get("state").unwrap_or(&value);
        let state = object(state)?;
        if ver < 34 {
            report.note(Category::UnknownSettings, Code::DeferredMigrations);
        }
        legacy_settings |= ver < 28;
        for (key, value) in state {
            if key == "version" {
                continue;
            }
            match serde_json::from_value::<SettingsCategory>(Value::String(key.clone())) {
                Ok(category) => {
                    auto_dj_supplied |= category == SettingsCategory::AutoDJ;
                    project_category(category, value, ver, &mut candidate, &mut report);
                }
                Err(_) => report.note(Category::UnknownSettings, Code::UnsupportedCategory),
            }
        }
        if !input.split_categories.is_empty() {
            let split_version = input.split_settings_version.unwrap_or(14);
            if !(14..=34).contains(&split_version) {
                return Err(ImportError::UnsupportedVersion);
            }
            if split_version < 34 {
                report.note(Category::UnknownSettings, Code::DeferredMigrations);
            }
            for (&category, bytes) in &input.split_categories {
                let key = serde_json::to_value(category).map_err(|_| ImportError::InvalidData)?;
                // Source's legacy split helper never overwrites an existing split Auto DJ.
                // Retain combined Auto DJ too when that separate category is absent (ADR-009).
                if category != SettingsCategory::AutoDJ
                    && key.as_str().is_some_and(|key| state.contains_key(key))
                {
                    report.note(Category::Settings(category), Code::IgnoredSplitCopy);
                    continue;
                }
                legacy_settings |= split_version < 28;
                auto_dj_supplied |= category == SettingsCategory::AutoDJ;
                match parse(bytes) {
                    Ok(value) => project_category(
                        category,
                        &value,
                        split_version,
                        &mut candidate,
                        &mut report,
                    ),
                    Err(_) => report.note(Category::Settings(category), Code::CorruptCategory),
                }
            }
        }
    }
    if legacy_settings && !auto_dj_supplied {
        report.note(
            Category::Settings(SettingsCategory::AutoDJ),
            Code::LegacyDefaultsDeferred,
        );
    }
    if let Some(bytes) = input.auth {
        project_auth(&parse(bytes)?, &mut candidate, &mut report)?;
    }
    let mut imported_queue = false;
    if let Some(bytes) = input.player {
        let value = parse(bytes)?;
        version(&value, &[3, 4])?;
        let state = object(value.get("state").ok_or(ImportError::InvalidDocument)?)?;
        report.note(Category::Player, Code::UnsupportedFields);
        let separate = input.separate_queue.map(parse);
        let queue = match &separate {
            Some(Ok(v)) => Some(v),
            Some(Err(_)) => {
                report.note(Category::Queue, Code::QueueRejected);
                None
            }
            None => state.get("queue"),
        };
        if let Some(queue) = queue {
            match project_queue(queue, state.get("player"), &mut report) {
                Some(q) => {
                    candidate.queue = Some(q);
                    imported_queue = true;
                }
                None => report.note(Category::Queue, Code::QueueRejected),
            }
        }
    } else if input.separate_queue.is_some() {
        // A queue without its versioned player envelope cannot establish current identity.
        report.note(Category::Queue, Code::QueueRejected);
    }
    if let Some(bytes) = input.timestamp {
        let value = parse(bytes)?;
        let raw = if value.is_object() {
            version(&value, &[1])?;
            value.get("state").and_then(|s| s.get("timestamp"))
        } else {
            Some(&value)
        };
        let ms = raw.and_then(seconds_ms);
        if let (true, Some(ms), Some(queue)) = (imported_queue, ms, &mut candidate.queue)
            && let Some(current) = &queue.current_occurrence
        {
            queue.position = Some(ResumePosition {
                occurrence_id: current.clone(),
                milliseconds: ms,
            });
        } else {
            report.note(Category::Timestamp, Code::TimestampRejected);
        }
    }
    candidate.columns.sort_by_key(|c| (c.list, c.view));
    validate_data(&candidate)?;
    Ok(ImportPreview { candidate, report })
}

fn project_category(
    category: SettingsCategory,
    value: &Value,
    version: u64,
    data: &mut MigrationData,
    report: &mut ImportReport,
) {
    let cat = Category::Settings(category);
    match category {
        SettingsCategory::General => {
            let Some(general) = value.as_object() else {
                report.note(cat, Code::CorruptCategory);
                return;
            };
            if let Some(language) = general.get("language") {
                if let Some(language) = language.as_str() {
                    let mut prefs = data.profile.preferences.clone();
                    prefs.language = language.into();
                    if prefs.valid() {
                        data.profile.preferences = prefs;
                    } else {
                        report.note(cat, Code::InvalidPreference);
                    }
                } else {
                    report.note(cat, Code::InvalidPreference);
                }
            }
            if let Some(follow) = general.get("followSystemTheme") {
                if let Some(follow) = follow.as_bool() {
                    data.profile.preferences.follow_system_theme = follow;
                } else {
                    report.note(cat, Code::InvalidPreference);
                }
            }
            if general
                .keys()
                .any(|k| !matches!(k.as_str(), "language" | "followSystemTheme"))
            {
                report.note(cat, Code::UnsupportedFields);
            }
        }
        SettingsCategory::AutoDJ => {
            // Legacy aliases normalize into the closed target schema; unknown fields never survive.
            match crate::settings_migration::migrate_auto_dj(value, version) {
                Ok(dj) => {
                    if version < 28 {
                        report.note(cat, Code::HistoricalTransformApplied);
                    }
                    data.auto_dj = Some(dj);
                }
                _ => report.note(cat, Code::CorruptCategory),
            }
        }
        SettingsCategory::Lists => project_lists(value, version, data, report),
        _ => report.note(cat, Code::UnsupportedCategory),
    }
}
fn project_lists(value: &Value, version: u64, data: &mut MigrationData, report: &mut ImportReport) {
    let cat = Category::Settings(SettingsCategory::Lists);
    let Some(lists) = value.as_object() else {
        report.note(cat, Code::CorruptCategory);
        return;
    };
    for (key, config) in lists {
        let Ok(list) = serde_json::from_value::<ListId>(Value::String(key.clone())) else {
            report.note(cat, Code::UnsupportedFields);
            continue;
        };
        let Some(config) = config.as_object() else {
            report.add(cat, Code::InvalidColumns, Some(list), None);
            continue;
        };
        for (view, key) in [(ColumnView::Table, "table"), (ColumnView::Detail, "detail")] {
            let Some(raw) = config.get(key) else {
                continue;
            };
            let Some(raw) = raw.as_object() else {
                report.add(cat, Code::InvalidColumns, Some(list), None);
                continue;
            };
            let Some(columns) = raw.get("columns") else {
                continue;
            };
            let Ok(mut columns) = serde_json::from_value::<Vec<Column>>(columns.clone()) else {
                report.add(cat, Code::InvalidColumns, Some(list), None);
                continue;
            };
            let mut seen = BTreeSet::new();
            let duplicate = columns.iter().any(|c| !seen.insert(c.id));
            if !valid_columns(&columns) {
                report.add(
                    cat,
                    if duplicate {
                        Code::DuplicateColumn
                    } else {
                        Code::InvalidColumns
                    },
                    Some(list),
                    None,
                );
                continue;
            }
            match crate::settings_migration::migrate_columns(&mut columns, list, view, version) {
                Ok(true) => report.add(cat, Code::HistoricalTransformApplied, Some(list), None),
                Ok(false) => {}
                Err(_) => {
                    report.add(cat, Code::InvalidColumns, Some(list), None);
                    continue;
                }
            }
            let next = ColumnList {
                list,
                view,
                columns,
            };
            if let Some(old) = data
                .columns
                .iter_mut()
                .find(|c| c.list == list && c.view == view)
            {
                *old = next;
            } else {
                data.columns.push(next);
            }
        }
        // Grid/layout/filter settings and non-column table/detail options remain unported.
        report.add(cat, Code::UnsupportedFields, Some(list), None);
    }
}
fn project_auth(
    value: &Value,
    data: &mut MigrationData,
    report: &mut ImportReport,
) -> Result<(), ImportError> {
    version(value, &[2])?;
    let state = object(value.get("state").ok_or(ImportError::InvalidDocument)?)?;
    let cat = Category::Authentication;
    report.note(cat, Code::CredentialsStripped);
    report.note(cat, Code::UnsupportedFields);
    let servers = state
        .get("serverList")
        .and_then(Value::as_object)
        .ok_or(ImportError::InvalidDocument)?;
    if servers.len() > MAX_PROFILES {
        return Err(ImportError::InputTooLarge);
    }
    let mut profiles = vec![];
    for (index, (key, raw)) in servers.iter().enumerate() {
        let projected = (|| {
            let id = raw.get("id")?.as_str()?;
            if key != id {
                return None;
            }
            Some(Profile {
                id: id.into(),
                name: raw.get("name")?.as_str()?.into(),
                endpoint: raw.get("url")?.as_str()?.into(),
                backend: serde_json::from_value::<BackendKind>(raw.get("type")?.clone()).ok()?,
            })
        })();
        match projected {
            Some(p) if p.valid() => {
                if profiles.iter().any(|old: &Profile| old.id == p.id) {
                    report.add(cat, Code::DuplicateProfile, None, Some(index));
                } else {
                    profiles.push(p);
                }
            }
            _ => report.add(cat, Code::InvalidProfile, None, Some(index)),
        }
    }
    data.profile.profiles = profiles;
    data.profile.current_server = state
        .get("currentServer")
        .and_then(|s| s.get("id"))
        .and_then(Value::as_str)
        .filter(|id| data.profile.profiles.iter().any(|p| p.id == *id))
        .map(str::to_owned);
    if state.get("currentServer").is_some_and(|s| !s.is_null())
        && data.profile.current_server.is_none()
    {
        report.note(cat, Code::OrphanSelection);
    }
    Ok(())
}
fn project_queue(
    value: &Value,
    player: Option<&Value>,
    report: &mut ImportReport,
) -> Option<QueueSnapshot> {
    let default = value.get("default")?.as_array()?;
    let songs = value.get("songs")?.as_object()?;
    if default.len() > MAX_QUEUE || songs.len() > MAX_QUEUE {
        return None;
    }
    let cat = Category::Queue;
    let mut queue = QueueSnapshot::default();
    let mut seen = BTreeSet::new();
    let raw_ids: Vec<Option<&str>> = default.iter().map(Value::as_str).collect();
    for (index, id) in raw_ids.iter().enumerate() {
        let Some(id) = id.filter(|s| bounded(s, 128)) else {
            report.add(cat, Code::MissingSong, None, Some(index));
            continue;
        };
        if !seen.insert(id) {
            report.add(cat, Code::DuplicateOccurrence, None, Some(index));
            continue;
        }
        let song = songs.get(id);
        let projected = (|| {
            let song = song?;
            if song
                .get("_uniqueId")
                .is_some_and(|v| v.as_str() != Some(id))
            {
                return None;
            }
            let song_id = song.get("id")?.as_str()?;
            let server_id = song.get("_serverId")?.as_str()?;
            let title = song.get("name")?.as_str()?;
            if !bounded(song_id, 128) || !bounded(server_id, 128) || title.len() > 1024 {
                return None;
            }
            Some(QueueOccurrence {
                occurrence_id: id.into(),
                song_id: song_id.into(),
                server_id: server_id.into(),
                title: title.into(),
                duration_ms: milliseconds(song.get("duration")?)?,
            })
        })();
        match projected {
            Some(song) => queue.occurrences.push(song),
            None => report.add(cat, Code::MissingSong, None, Some(index)),
        }
    }
    if songs.keys().any(|id| !seen.contains(id.as_str())) {
        report.note(cat, Code::OrphansRemoved);
    }
    report.note(cat, Code::MetadataDropped);
    let live: BTreeSet<&str> = queue
        .occurrences
        .iter()
        .map(|o| o.occurrence_id.as_str())
        .collect();
    let mut priority_seen = BTreeSet::new();
    if let Some(priority) = value.get("priority").and_then(Value::as_array) {
        if priority.len() > MAX_QUEUE {
            return None;
        }
        for id in priority {
            if let Some(id) = id
                .as_str()
                .filter(|s| live.contains(s) && priority_seen.insert(*s))
            {
                queue.priority.push(id.into());
            } else {
                report.note(cat, Code::PriorityRepaired);
            }
        }
    } else {
        report.note(cat, Code::PriorityRepaired);
    }
    let shuffle = player
        .and_then(|p| p.get("shuffle"))
        .and_then(Value::as_str);
    let mut order_valid = true;
    match shuffle {
        Some("none" | "album") => {}
        Some("track")
            if value
                .get("shuffled")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty) => {}
        Some("track") => {
            let shuffled = value.get("shuffled").and_then(Value::as_array);
            let mut used = BTreeSet::new();
            if let Some(shuffled) =
                shuffled.filter(|s| s.len() == raw_ids.len() && s.len() <= MAX_QUEUE)
            {
                for index in shuffled {
                    if let Some(index) = index.as_u64().and_then(|n| usize::try_from(n).ok())
                        && used.insert(index)
                        && let Some(Some(id)) = raw_ids.get(index)
                        && live.contains(id)
                    {
                        queue.shuffled.push((*id).into());
                    } else {
                        order_valid = false;
                        break;
                    }
                }
                if queue.shuffled.len() != live.len() {
                    order_valid = false;
                }
            } else {
                order_valid = false;
            }
        }
        _ => {
            order_valid = false;
        }
    }
    if !order_valid {
        queue.shuffled.clear();
        report.note(cat, Code::ShuffleRejected);
    }
    let index = player
        .and_then(|p| p.get("index"))
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok());
    // Source getQueueOrder compacts missing map entries before getCurrentSong indexes
    // them (player.store.ts:1101-1119, 982-994). Once target validation removes or
    // deduplicates a default entry, neither its raw nor compacted index is trusted.
    // Keep the recovered occurrences, but require an explicit later selection.
    let default_order_recovered = queue.occurrences.len() != raw_ids.len();
    queue.current_occurrence =
        if order_valid && (!queue.shuffled.is_empty() || !default_order_recovered) {
            index.and_then(|index| {
                if !queue.shuffled.is_empty() {
                    queue.shuffled.get(index).cloned()
                } else {
                    raw_ids
                        .get(index)
                        .and_then(|id| *id)
                        .filter(|id| live.contains(id))
                        .map(str::to_owned)
                }
            })
        } else {
            None
        };
    if queue.current_occurrence.is_none() {
        report.note(cat, Code::CurrentRejected);
    }
    Some(queue)
}
