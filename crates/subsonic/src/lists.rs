//! Complete endpoint-specific local filter/sort/page rules; no generic server paging.
use crate::read_support::validate_identity;
use crate::song::{array, integer, number, object, required_text};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};
use std::cmp::Ordering;

type Result<T> = std::result::Result<T, ExchangeError>;

// Lodash ascending order for the homogeneous source fields used here:
// ordinary values, then null, then undefined. Owner's optional chaining maps
// both missing and null to undefined; other fields retain that distinction.
enum Key {
    Number(f64),
    Text(String),
    Bool(bool),
    Null,
    Missing,
}
impl Key {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Missing, Self::Missing) | (Self::Null, Self::Null) => Ordering::Equal,
            (Self::Missing, _) => Ordering::Greater,
            (_, Self::Missing) => Ordering::Less,
            (Self::Null, _) => Ordering::Greater,
            (_, Self::Null) => Ordering::Less,
            (Self::Number(a), Self::Number(b)) => a.total_cmp(b),
            (Self::Text(a), Self::Text(b)) => a.encode_utf16().cmp(b.encode_utf16()),
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            // A single operation constructs one scalar key type. This branch
            // is unreachable without an internal programming error.
            _ => unreachable!("list keys must have the same scalar type"),
        }
    }
}
fn missing(obj: &Map<String, Value>, field: &str) -> Option<Key> {
    match obj.get(field) {
        None => Some(Key::Missing),
        Some(Value::Null) => Some(Key::Null),
        _ => None,
    }
}
fn numeric_key(obj: &Map<String, Value>, field: &str, whole: bool) -> Result<Key> {
    if let Some(key) = missing(obj, field) {
        return Ok(key);
    }
    let n = if whole {
        integer(obj, field)?.map(|v| v as f64)
    } else {
        number(obj, field)?
    }
    .ok_or(ExchangeError::Malformed)?;
    Ok(Key::Number(if n == 0.0 { 0.0 } else { n }))
}
fn text_key(obj: &Map<String, Value>, field: &str, lower: bool, optional: bool) -> Result<Key> {
    if let Some(key) = missing(obj, field) {
        return Ok(if optional { Key::Missing } else { key });
    }
    let value = required_text(obj, field)?;
    Ok(Key::Text(if lower { value.to_lowercase() } else { value }))
}
fn playlist_key(obj: &Map<String, Value>, sort: PlaylistListSort) -> Result<Key> {
    match sort {
        PlaylistListSort::Duration => numeric_key(obj, "duration", false),
        PlaylistListSort::SongCount => numeric_key(obj, "songCount", true),
        PlaylistListSort::Name => text_key(obj, "name", true, true),
        PlaylistListSort::Owner => text_key(obj, "owner", true, true),
        PlaylistListSort::UpdatedAt => text_key(obj, "changed", false, false),
        PlaylistListSort::Public => match missing(obj, "public") {
            Some(k) => Ok(k),
            None => obj["public"]
                .as_bool()
                .map(Key::Bool)
                .ok_or(ExchangeError::Malformed),
        },
    }
}
fn genre_key(obj: &Map<String, Value>, sort: GenreListSort) -> Result<Key> {
    match sort {
        GenreListSort::AlbumCount => numeric_key(obj, "albumCount", true),
        GenreListSort::SongCount => numeric_key(obj, "songCount", true),
        GenreListSort::Name => Ok(Key::Text(required_text(obj, "value")?.to_lowercase())),
    }
}
pub(super) fn check(
    clock: &dyn Clock,
    context: OperationContext<'_>,
    server: &ServerId,
    start: u64,
    limit: Option<u64>,
    search: Option<&str>,
) -> Result<()> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context.check_items(1).map_err(ExchangeError::Context)?;
    validate_identity(server, "list")?;
    if start > 9_007_199_254_740_991 || limit.is_some_and(|v| v > 9_007_199_254_740_991) {
        return Err(ExchangeError::Malformed);
    }
    if search.is_some_and(|s| s.len() > 8192) {
        return Err(ExchangeError::Context(DomainError::Capacity));
    }
    Ok(())
}
fn values<'a>(
    response: &'a Map<String, Value>,
    parent: &str,
    key: &str,
    context: OperationContext<'_>,
) -> Result<&'a [Value]> {
    let values = match response.get(parent) {
        None | Some(Value::Null) => &[][..],
        Some(v) => array(object(v)?, key, context)?.unwrap_or(&[]),
    };
    context
        .check_items(
            values
                .len()
                .checked_add(1)
                .ok_or(ExchangeError::Context(DomainError::Capacity))?,
        )
        .map_err(ExchangeError::Context)?;
    Ok(values)
}
fn filtered<'a>(values: &'a [Value], field: &str, search: Option<&str>) -> Result<Vec<&'a Value>> {
    let search = search.filter(|s| !s.is_empty()).map(str::to_lowercase);
    values
        .iter()
        .filter_map(|value| {
            let Some(search) = &search else {
                return Some(Ok(value));
            };
            match object(value).and_then(|obj| required_text(obj, field)) {
                Ok(name) => name.to_lowercase().contains(search).then_some(Ok(value)),
                Err(e) => Some(Err(e)),
            }
        })
        .collect()
}
fn sorted<T>(mut values: Vec<(Key, T)>, order: SortOrder) -> Vec<T> {
    // Stable sorting preserves source order on equal keys, including descending.
    values.sort_by(|(a, _), (b, _)| {
        let o = a.cmp(b);
        if order == SortOrder::Desc {
            o.reverse()
        } else {
            o
        }
    });
    values.into_iter().map(|(_, v)| v).collect()
}
pub(super) fn paginate<T>(items: Vec<T>, start: u64, limit: Option<u64>) -> Result<Page<T>> {
    let total =
        u64::try_from(items.len()).map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
    let count = limit.filter(|v| *v != 0).unwrap_or(total);
    let begin = usize::try_from(start.min(total))
        .map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
    let count = usize::try_from(count.min(total.saturating_sub(start)))
        .map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
    Ok(Page {
        items: items.into_iter().skip(begin).take(count).collect(),
        start_index: start,
        total_record_count: Some(total),
    })
}

/// Fetch/filter/sort/normalize the entire playlist list before local pagination.
/// The source ignores exclude_smart_playlists for this backend.
pub async fn get_playlists(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &PlaylistListQuery,
) -> Result<Page<Playlist>> {
    check(
        clock,
        context,
        &query.server,
        query.start_index,
        query.limit,
        query.search_term.as_deref(),
    )?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getPlaylists.view",
            &[],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = (|| {
        let raw = values(&response, "playlists", "playlist", context)?;
        let rows = filtered(raw, "name", query.search_term.as_deref())?;
        let keyed = rows
            .into_iter()
            .map(|v| {
                let obj = object(v)?;
                Ok((playlist_key(obj, query.sort_by)?, obj))
            })
            .collect::<Result<Vec<_>>>()?;
        let items = sorted(keyed, query.sort_order)
            .into_iter()
            .map(|v| crate::playlist::normalize(v, &query.server))
            .collect::<Result<Vec<_>>>()?;
        paginate(items, query.start_index, query.limit)
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

/// Count the filtered raw playlist rows without normalizing metadata or paging.
pub async fn get_playlist_count(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
    search: Option<&str>,
) -> Result<u64> {
    check(clock, context, server, 0, None, search)?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getPlaylists.view",
            &[],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = values(&response, "playlists", "playlist", context)
        .and_then(|raw| filtered(raw, "name", search))
        .and_then(|v| {
            u64::try_from(v.len()).map_err(|_| ExchangeError::Context(DomainError::Capacity))
        });
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

/// Fetch the full genre collection; music-folder selections have no effect here.
pub async fn get_genres(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &GenreListQuery,
) -> Result<Page<Genre>> {
    check(
        clock,
        context,
        &query.server,
        query.start_index,
        query.limit,
        query.search_term.as_deref(),
    )?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(base, auth, "getGenres.view", &[], PostPreference::Get, None)?,
    )
    .await?;
    let result = (|| {
        let raw = values(&response, "genres", "genre", context)?;
        let rows = filtered(raw, "value", query.search_term.as_deref())?;
        let keyed = rows
            .into_iter()
            .map(|v| {
                let obj = object(v)?;
                Ok((genre_key(obj, query.sort_by)?, obj))
            })
            .collect::<Result<Vec<_>>>()?;
        let items = sorted(keyed, query.sort_order)
            .into_iter()
            .map(|obj| {
                let name = required_text(obj, "value")?;
                Ok(Genre {
                    server: query.server.clone(),
                    server_type: ServerType::Subsonic,
                    id: GenreId::new(name.clone()),
                    name,
                    album_count: integer(obj, "albumCount")?,
                    song_count: integer(obj, "songCount")?,
                    image_id: None,
                    image_url: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        paginate(items, query.start_index, query.limit)
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
