//! Flattened artist indexes with distinct album-artist/artist query semantics.
use crate::album::normalize_artist_base;
use crate::collections::nested_array;
use crate::enrichment::normalize_related;
use crate::lists::{check, paginate};
use crate::read_support::Budget;
use crate::song::{array, object};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use std::cmp::Ordering;

type Result<T> = std::result::Result<T, ExchangeError>;
#[derive(Clone, Copy)]
enum Sort {
    Name,
    AlbumCount,
    SourceOrder,
}
struct Options<'a> {
    server: &'a ServerId,
    sort: Sort,
    order: SortOrder,
    start: u64,
    limit: Option<u64>,
    search: Option<&'a str>,
    folders: Option<&'a [FolderId]>,
    role: Option<&'a str>,
    favorite: bool,
}
fn album_options(q: &AlbumArtistListQuery, start: u64) -> Options<'_> {
    Options {
        server: &q.server,
        sort: match q.sort_by {
            AlbumArtistListSort::Name => Sort::Name,
            AlbumArtistListSort::AlbumCount => Sort::AlbumCount,
            _ => Sort::SourceOrder,
        },
        order: q.sort_order,
        start,
        limit: q.limit,
        search: q.search_term.as_deref(),
        folders: q.filter.music_folder_ids.as_deref(),
        role: None,
        favorite: q.filter.favorite == Some(true),
    }
}
fn artist_options(q: &ArtistListQuery, start: u64) -> Options<'_> {
    Options {
        server: &q.server,
        sort: match q.sort_by {
            ArtistListSort::Name => Sort::Name,
            ArtistListSort::AlbumCount => Sort::AlbumCount,
            _ => Sort::SourceOrder,
        },
        order: q.sort_order,
        start,
        limit: q.limit,
        search: q.search_term.as_deref(),
        folders: q.filter.music_folder_ids.as_deref(),
        role: q.filter.role.as_deref().filter(|s| !s.is_empty()),
        favorite: false,
    }
}
fn sorted(items: Vec<AlbumArtist>, sort: Sort, order: SortOrder) -> Vec<AlbumArtist> {
    if matches!(sort, Sort::SourceOrder) {
        return items;
    }
    let mut values: Vec<_> = items
        .into_iter()
        .map(|a| {
            let name = a.name.to_lowercase();
            (a, name)
        })
        .collect();
    values.sort_by(|(a, an), (b, bn)| {
        let primary = match sort {
            Sort::Name => an.encode_utf16().cmp(bn.encode_utf16()),
            Sort::AlbumCount => a.album_count.cmp(&b.album_count),
            Sort::SourceOrder => Ordering::Equal,
        };
        let primary = if order == SortOrder::Desc {
            primary.reverse()
        } else {
            primary
        };
        if primary == Ordering::Equal && matches!(sort, Sort::AlbumCount) {
            an.encode_utf16().cmp(bn.encode_utf16())
        } else {
            primary
        }
    });
    values.into_iter().map(|(a, _)| a).collect()
}
async fn read(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    options: Options<'_>,
) -> Result<Page<AlbumArtist>> {
    check(
        clock,
        context,
        options.server,
        options.start,
        options.limit,
        options.search,
    )?;
    if options.role.is_some_and(|r| r.len() > 8192) {
        return Err(ExchangeError::Context(DomainError::Capacity));
    }
    if let Some(folders) = options.folders {
        context
            .check_items(folders.len())
            .map_err(ExchangeError::Context)?;
    }
    let pairs = options
        .folders
        .and_then(|ids| ids.first())
        .map(|id| vec![("musicFolderId", id.as_str())])
        .unwrap_or_default();
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getArtists.view",
            &pairs,
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = (|| {
        let mut budget = Budget {
            limit: context.limits.items,
            used: 0,
        };
        budget.take(1)?;
        let mut items = Vec::new();
        for index in nested_array(&response, "artists", "index", context)? {
            budget.take(1)?;
            for value in
                array(object(index)?, "artist", context)?.ok_or(ExchangeError::Malformed)?
            {
                budget.take(1)?;
                let obj = object(value)?;
                if let Some(role) = options.role
                    && let Some(roles) = array(obj, "roles", context)?
                {
                    budget.take(roles.len())?;
                    let mut matches = false;
                    for value in roles {
                        matches |= value.as_str().ok_or(ExchangeError::Malformed)? == role;
                    }
                    if !matches {
                        continue;
                    }
                }
                let similar = array(obj, "similarArtists", context)?
                    .unwrap_or(&[])
                    .iter()
                    .map(|v| {
                        budget.take(1)?;
                        normalize_related(object(v)?, false)
                    })
                    .collect::<Result<Vec<_>>>()?;
                items.push(normalize_artist_base(obj, options.server, Some(similar))?);
            }
        }
        let search = options
            .search
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase);
        items.retain(|a| {
            (!options.favorite || a.user_favorite)
                && search
                    .as_ref()
                    .is_none_or(|s| a.name.to_lowercase().contains(s))
        });
        paginate(
            sorted(items, options.sort, options.order),
            options.start,
            options.limit,
        )
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

/// Optional true favorite filter applies after normalization; false means no filter.
pub async fn get_album_artists(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &AlbumArtistListQuery,
) -> Result<Page<AlbumArtist>> {
    read(
        transport,
        clock,
        context,
        base,
        auth,
        album_options(query, query.start_index),
    )
    .await
}
/// Source role filter retains artists without roles, and filters before normalization.
pub async fn get_artists(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &ArtistListQuery,
) -> Result<Page<Artist>> {
    let p = read(
        transport,
        clock,
        context,
        base,
        auth,
        artist_options(query, query.start_index),
    )
    .await?;
    let result = Page {
        items: p.items.into_iter().map(Artist).collect(),
        start_index: p.start_index,
        total_record_count: p.total_record_count,
    };
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    Ok(result)
}
pub async fn get_album_artist_count(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &AlbumArtistListQuery,
) -> Result<u64> {
    let p = read(
        transport,
        clock,
        context,
        base,
        auth,
        album_options(query, 0),
    )
    .await?;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    p.total_record_count.ok_or(ExchangeError::Malformed)
}
pub async fn get_artist_count(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &ArtistListQuery,
) -> Result<u64> {
    let p = read(
        transport,
        clock,
        context,
        base,
        auth,
        artist_options(query, 0),
    )
    .await?;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    p.total_record_count.ok_or(ExchangeError::Malformed)
}
