//! Bounded transformations of retained column and Auto DJ fields from copied historical inputs.
//! Versions 14 through 34 are supported; other settings fields are outside this module.
use crate::imports::{Alignment, AutoDj, Column, ColumnId, ColumnView, ImportError, ListId, Pin};
use serde_json::{Number, Value};
use std::collections::BTreeSet;

/// Migrate one whole, already decoded column array without mutating it on failure.
pub fn migrate_columns(
    columns: &mut Vec<Column>,
    list: ListId,
    view: ColumnView,
    version: u64,
) -> Result<bool, ImportError> {
    if !(14..=34).contains(&version) {
        return Err(ImportError::UnsupportedVersion);
    }
    let mut ids = BTreeSet::new();
    if columns.len() > 64
        || columns.iter().any(|c| {
            !ids.insert(c.id)
                || !c
                    .width
                    .as_f64()
                    .is_some_and(|w| w.is_finite() && (0.0..=16384.0).contains(&w))
        })
    {
        return Err(ImportError::InvalidData);
    }
    let mut result = columns.clone();
    let song_tables = matches!(
        list,
        ListId::Song | ListId::PlaylistSong | ListId::QueueSong | ListId::AlbumDetail
    );
    let title_tables =
        song_tables || matches!(list, ListId::Album | ListId::FullScreen | ListId::SideQueue);
    let group_tables = matches!(
        list,
        ListId::Song
            | ListId::Folder
            | ListId::PlaylistSong
            | ListId::AlbumArtistSong
            | ListId::GenreSong
            | ListId::QueueSong
            | ListId::FullScreen
            | ListId::SideQueue
    );
    let date_tables = group_tables || list == ListId::AlbumDetail;
    if view == ColumnView::Table {
        if version <= 14 && song_tables {
            insert(
                &mut result,
                ColumnId::BitDepth,
                Alignment::Center,
                100,
                None,
                None,
                false,
            );
            insert(
                &mut result,
                ColumnId::SampleRate,
                Alignment::Center,
                100,
                None,
                None,
                false,
            );
        }
        if version <= 20 && title_tables {
            insert(
                &mut result,
                ColumnId::TitleArtist,
                Alignment::Start,
                300,
                None,
                Some(ColumnId::TitleCombined),
                true,
            );
        }
        if version <= 21 && title_tables {
            insert(
                &mut result,
                ColumnId::Composer,
                Alignment::Start,
                300,
                None,
                Some(ColumnId::Artist),
                true,
            );
        }
        if version <= 26 && group_tables {
            insert(
                &mut result,
                ColumnId::AlbumGroup,
                Alignment::Start,
                240,
                Some(Pin::Left),
                None,
                false,
            );
        }
    }
    if version < 29
        && ((view == ColumnView::Table && date_tables)
            || (view == ColumnView::Detail && list == ListId::Album))
    {
        insert(
            &mut result,
            ColumnId::Year,
            Alignment::Center,
            200,
            None,
            Some(ColumnId::ReleaseYear),
            false,
        );
        insert(
            &mut result,
            ColumnId::Date,
            Alignment::Center,
            240,
            None,
            Some(ColumnId::ReleaseDate),
            false,
        );
    }
    if result.len() > 64 {
        return Err(ImportError::InvalidData);
    }
    let changed = *columns != result;
    *columns = result;
    Ok(changed)
}

fn insert(
    columns: &mut Vec<Column>,
    id: ColumnId,
    align: Alignment,
    width: u64,
    pinned: Option<Pin>,
    anchor: Option<ColumnId>,
    after: bool,
) {
    if columns.iter().any(|c| c.id == id) {
        return;
    }
    let position = anchor
        .and_then(|id| columns.iter().position(|c| c.id == id))
        .map_or(columns.len(), |i| i + usize::from(after));
    columns.insert(
        position,
        Column {
            id,
            align,
            auto_size: Some(false),
            is_enabled: false,
            pinned,
            width: Number::from(width),
        },
    );
}

/// Normalize a supplied legacy Auto DJ object; current objects remain strict.
pub fn migrate_auto_dj(value: &Value, version: u64) -> Result<AutoDj, ImportError> {
    if !(14..=34).contains(&version) {
        return Err(ImportError::UnsupportedVersion);
    }
    let mut value = value.clone();
    if version < 28 {
        let object = value.as_object_mut().ok_or(ImportError::InvalidData)?;
        let defaults = [
            ("albumStrategy", serde_json::json!("similar")),
            ("allowDuplicates", serde_json::json!(false)),
            ("enabled", serde_json::json!(false)),
            ("itemCount", serde_json::json!(5)),
            ("mode", serde_json::json!("songs")),
            ("onlySimilar", serde_json::json!(false)),
            ("songStrategy", serde_json::json!("similar")),
            ("timing", serde_json::json!(1)),
        ];
        for (key, default) in defaults {
            object.entry(key).or_insert(default);
        }
        if !matches!(
            object.get("mode").and_then(Value::as_str),
            Some("albums" | "songs")
        ) {
            object.insert("mode".into(), serde_json::json!("songs"));
        }
        for key in ["albumStrategy", "songStrategy"] {
            let normalized = if object.get(key).and_then(Value::as_str) == Some("library_random") {
                "library_random"
            } else {
                "similar"
            };
            object.insert(key.into(), serde_json::json!(normalized));
        }
    }
    let result: AutoDj = serde_json::from_value(value).map_err(|_| ImportError::InvalidData)?;
    if !result.item_count.as_f64().is_some_and(f64::is_finite)
        || !result.timing.as_f64().is_some_and(f64::is_finite)
    {
        return Err(ImportError::InvalidData);
    }
    Ok(result)
}
