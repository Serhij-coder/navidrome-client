//! Complete getAlbumList branches with bounded source-specific local ordering.
use crate::album::normalize_album;
use crate::album_order::{Row, sorted};
use crate::collections::nested_array;
use crate::lists::{check, paginate};
use crate::read_support::Budget;
use crate::song::{array, number, object};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::Value;

type Result<T> = std::result::Result<T, ExchangeError>;
/// Calendar sample is separate from monotonic deadlines; RNG is used only for
/// local random sorting in selected-artist/favorite branches, never auth.
pub struct AlbumReadEnvironment<'a> {
    pub calendar_year: i32,
    pub random: &'a mut dyn Random,
}
fn rows(
    values: &[Value],
    server: &ServerId,
    context: OperationContext<'_>,
    budget: &mut Budget,
) -> Result<Vec<Row>> {
    values
        .iter()
        .map(|v| {
            let obj = object(v)?;
            let album = normalize_album(obj, server, context, budget)?;
            let duration_ms = number(obj, "duration")?.ok_or(ExchangeError::Malformed)? * 1000.0;
            Ok(Row {
                album,
                duration_ms,
                missing_song_count: !obj.contains_key("songCount"),
            })
        })
        .collect()
}
fn page(rows: Vec<Row>, start: u64, total: Option<u64>) -> Page<Album> {
    Page {
        items: rows.into_iter().map(|r| r.album).collect(),
        start_index: start,
        total_record_count: total,
    }
}

/// Caller binds base/auth/scoped query server. No total-count inference from
/// headers on search/general list paths; source totals remain explicitly unknown.
pub async fn get_albums(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &AlbumListQuery,
    environment: &mut AlbumReadEnvironment<'_>,
) -> Result<Page<Album>> {
    check(
        clock,
        context,
        &query.server,
        query.start_index,
        query.limit,
        query.search_term.as_deref(),
    )?;
    for len in [
        query.filter.artist_ids.as_ref().map(Vec::len),
        query.filter.genre_ids.as_ref().map(Vec::len),
        query.filter.music_folder_ids.as_ref().map(Vec::len),
    ]
    .into_iter()
    .flatten()
    {
        context.check_items(len).map_err(ExchangeError::Context)?;
    }
    let result = read(transport, clock, context, base, auth, query, environment).await;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
async fn read(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &AlbumListQuery,
    environment: &mut AlbumReadEnvironment<'_>,
) -> Result<Page<Album>> {
    let mut budget = Budget {
        limit: context.limits.items,
        used: 1,
    };
    let folder = query
        .filter
        .music_folder_ids
        .as_ref()
        .and_then(|v| v.first())
        .map(FolderId::as_str);
    let limit = query.limit.map(|v| v.to_string());
    let offset = query.start_index.to_string();
    if let Some(search) = query.search_term.as_deref().filter(|v| !v.is_empty()) {
        let mut pairs = Vec::new();
        if let Some(limit) = &limit {
            pairs.push(("albumCount", limit.as_str()));
        }
        pairs.extend([
            ("albumOffset", offset.as_str()),
            ("artistCount", "0"),
            ("artistOffset", "0"),
        ]);
        if let Some(folder) = folder {
            pairs.push(("musicFolderId", folder));
        }
        pairs.extend([("query", search), ("songCount", "0"), ("songOffset", "0")]);
        let response = crate::protocol::exchange(
            transport,
            clock,
            context,
            plan_request(
                base,
                auth,
                "search3.view",
                &pairs,
                PostPreference::Get,
                None,
            )?,
        )
        .await?;
        return Ok(page(
            rows(
                nested_array(&response, "searchResult3", "album", context)?,
                &query.server,
                context,
                &mut budget,
            )?,
            query.start_index,
            None,
        ));
    }
    if let Some(artists) = &query.filter.artist_ids {
        let mut result = Vec::new();
        for artist in artists {
            budget.take(1)?;
            let response = crate::protocol::exchange(
                transport,
                clock,
                context,
                plan_request(
                    base,
                    auth,
                    "getArtist.view",
                    &[("id", artist.as_str())],
                    PostPreference::Get,
                    None,
                )?,
            )
            .await;
            let response = match response {
                Ok(v) => v,
                Err(ExchangeError::Http(_)) => continue,
                Err(e) => return Err(e),
            };
            let obj = object(response.get("artist").ok_or(ExchangeError::Malformed)?)?;
            result.extend(rows(
                array(obj, "album", context)?.unwrap_or(&[]),
                &query.server,
                context,
                &mut budget,
            )?);
        }
        let total = u64::try_from(result.len())
            .map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
        return Ok(page(
            sorted(result, query.sort_by, query.sort_order, environment.random),
            0,
            Some(total),
        ));
    }
    if query.filter.favorite == Some(true) {
        let pairs = folder
            .map(|folder| vec![("musicFolderId", folder)])
            .unwrap_or_default();
        let response = crate::protocol::exchange(
            transport,
            clock,
            context,
            plan_request(
                base,
                auth,
                "getStarred.view",
                &pairs,
                PostPreference::Get,
                None,
            )?,
        )
        .await?;
        let rows = rows(
            nested_array(&response, "starred", "album", context)?,
            &query.server,
            context,
            &mut budget,
        )?;
        let rows = sorted(rows, query.sort_by, query.sort_order, environment.random);
        let items = paginate(rows, query.start_index, query.limit)?;
        return Ok(page(
            items.items,
            items.start_index,
            items.total_record_count,
        ));
    }
    let mut kind = match query.sort_by {
        AlbumListSort::AlbumArtist => "alphabeticalByArtist",
        AlbumListSort::Favorited => "starred",
        AlbumListSort::PlayCount => "frequent",
        AlbumListSort::Random => "random",
        AlbumListSort::RecentlyAdded => "newest",
        AlbumListSort::RecentlyPlayed => "recent",
        AlbumListSort::ReleaseDate | AlbumListSort::Year => "byYear",
        _ => "alphabeticalByName",
    };
    let genre = query
        .filter
        .genre_ids
        .as_ref()
        .and_then(|v| v.first())
        .map(GenreId::as_str);
    if genre.is_some() {
        kind = "byGenre";
    }
    let min = query.filter.min_year.filter(|v| *v != 0);
    let max = query.filter.max_year.filter(|v| *v != 0);
    if min.is_some() || max.is_some() {
        kind = "byYear";
    }
    let mut from = min.or_else(|| max.map(|_| 0));
    let mut to = max.or_else(|| min.map(|_| environment.calendar_year));
    if kind == "byYear" && from.is_none_or(|v| v == 0) && to.is_none_or(|v| v == 0) {
        (from, to) = if query.sort_order == SortOrder::Asc {
            (Some(0), Some(environment.calendar_year))
        } else {
            (Some(environment.calendar_year), Some(0))
        };
    }
    let from = from.map(|v| v.to_string());
    let to = to.map(|v| v.to_string());
    let mut pairs = Vec::new();
    if let Some(from) = &from {
        pairs.push(("fromYear", from.as_str()));
    }
    if let Some(genre) = genre {
        pairs.push(("genre", genre));
    }
    if let Some(folder) = folder {
        pairs.push(("musicFolderId", folder));
    }
    pairs.push(("offset", offset.as_str()));
    if let Some(limit) = &limit {
        pairs.push(("size", limit.as_str()));
    }
    if let Some(to) = &to {
        pairs.push(("toYear", to.as_str()));
    }
    pairs.push(("type", kind));
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getAlbumList2.view",
            &pairs,
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let obj = object(response.get("albumList2").ok_or(ExchangeError::Malformed)?)?;
    Ok(page(
        rows(
            array(obj, "album", context)?.unwrap_or(&[]),
            &query.server,
            context,
            &mut budget,
        )?,
        query.start_index,
        None,
    ))
}
