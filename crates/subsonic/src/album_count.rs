//! Endpoint-specific album count probing; no universal paging approximation.
use crate::collections::nested_array;
use crate::lists::check;
use crate::read_support::Budget;
use crate::song::{array, object};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};

type Result<T> = std::result::Result<T, ExchangeError>;
const PAGE: u64 = 500;
const SAFE: u64 = 9_007_199_254_740_991;
fn add(total: u64, count: u64) -> Result<u64> {
    total
        .checked_add(count)
        .filter(|n| *n <= SAFE)
        .ok_or(ExchangeError::Context(DomainError::Capacity))
}
fn header(value: Option<&str>) -> Result<Option<u64>> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let value = value.trim();
    let n = if value.is_empty() {
        0.0
    } else {
        value.parse::<f64>().map_err(|_| ExchangeError::Malformed)?
    };
    if n.is_finite() && (0.0..=SAFE as f64).contains(&n) && n.fract() == 0.0 {
        Ok(Some(n as u64))
    } else {
        Err(ExchangeError::Malformed)
    }
}
struct Work {
    budget: Budget,
    requests: usize,
}
impl Work {
    fn new(context: OperationContext<'_>) -> Self {
        Self {
            budget: Budget {
                limit: context.limits.items,
                used: 1,
            },
            requests: 0,
        }
    }
    fn request(&mut self, context: OperationContext<'_>) -> Result<()> {
        self.requests = self
            .requests
            .checked_add(1)
            .ok_or(ExchangeError::Context(DomainError::Capacity))?;
        context
            .check_items(self.requests)
            .map_err(ExchangeError::Context)
    }
    fn rows(&mut self, len: usize) -> Result<u64> {
        self.budget.take(len)?;
        u64::try_from(len).map_err(|_| ExchangeError::Context(DomainError::Capacity))
    }
}
fn optional_len(
    response: &Map<String, Value>,
    parent: &str,
    key: &str,
    context: OperationContext<'_>,
) -> Result<usize> {
    Ok(nested_array(response, parent, key, context)?.len())
}

/// Caller supplies the captured local calendar year used by upstream dayjs().year().
/// Monotonic deadline Clock is never treated as a wall clock. Page/limit/sort and
/// unconsumed filters are ignored by this count operation, matching its controller.
pub async fn get_album_count(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &AlbumListQuery,
    current_year: i32,
) -> Result<u64> {
    check(
        clock,
        context,
        &query.server,
        0,
        None,
        query.search_term.as_deref(),
    )?;
    for ids in [
        query.filter.artist_ids.as_ref().map(Vec::len),
        query.filter.genre_ids.as_ref().map(Vec::len),
        query.filter.music_folder_ids.as_ref().map(Vec::len),
    ]
    .into_iter()
    .flatten()
    {
        context.check_items(ids).map_err(ExchangeError::Context)?;
    }
    let result = count(transport, clock, context, base, auth, query, current_year).await;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

// Operation inputs stay together to keep work accounting under this one owner.
async fn count(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &AlbumListQuery,
    current_year: i32,
) -> Result<u64> {
    let mut work = Work::new(context);
    let folder = query
        .filter
        .music_folder_ids
        .as_ref()
        .and_then(|v| v.first())
        .map(FolderId::as_str);
    if let Some(search) = query.search_term.as_deref().filter(|v| !v.is_empty()) {
        let mut total = 0u64;
        let mut offset = 0u64;
        loop {
            work.request(context)?;
            let off = offset.to_string();
            let mut pairs = vec![
                ("albumCount", "500"),
                ("albumOffset", off.as_str()),
                ("artistCount", "0"),
                ("artistOffset", "0"),
            ];
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
            let len = work.rows(optional_len(&response, "searchResult3", "album", context)?)?;
            total = add(total, len)?;
            offset = add(offset, len)?;
            if len != PAGE {
                return Ok(total);
            }
        }
    }
    if let Some(artists) = &query.filter.artist_ids {
        let mut total = 0u64;
        for artist in artists {
            work.budget.take(1)?;
            work.request(context)?;
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
                Ok(response) => response,
                // The source reducer resets the running total on a non-200
                // artist result. Retain that characterized order-sensitive rule.
                Err(ExchangeError::Http(_)) => {
                    total = 0;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let obj = object(response.get("artist").ok_or(ExchangeError::Malformed)?)?;
            let len = work.rows(array(obj, "album", context)?.unwrap_or(&[]).len())?;
            total = add(total, len)?;
        }
        return Ok(total);
    }
    if query.filter.favorite == Some(true) {
        work.request(context)?;
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
        return work.rows(optional_len(&response, "starred", "album", context)?);
    }
    let genre = query
        .filter
        .genre_ids
        .as_ref()
        .and_then(|v| v.first())
        .map(GenreId::as_str);
    let min = query.filter.min_year.filter(|y| *y != 0);
    let max = query.filter.max_year.filter(|y| *y != 0);
    let kind = if min.is_some() || max.is_some() {
        "byYear"
    } else if genre.is_some() {
        "byGenre"
    } else {
        "alphabeticalByName"
    };
    let from = min.or_else(|| max.map(|_| 0)).map(|y| y.to_string());
    let to = max
        .or_else(|| min.map(|_| current_year))
        .map(|y| y.to_string());
    let mut total = 0u64;
    let mut offset = 0u64;
    loop {
        work.request(context)?;
        let off = offset.to_string();
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
        pairs.extend([("offset", off.as_str()), ("size", "500")]);
        if let Some(to) = &to {
            pairs.push(("toYear", to.as_str()));
        }
        pairs.push(("type", kind));
        let response = crate::protocol::exchange_with_metadata(
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
        if let Some(total) = header(response.total_count_header.as_deref())? {
            return Ok(total);
        }
        let obj = object(
            response
                .body
                .get("albumList2")
                .ok_or(ExchangeError::Malformed)?,
        )?;
        let len = work.rows(
            array(obj, "album", context)?
                .ok_or(ExchangeError::Malformed)?
                .len(),
        )?;
        total = add(total, len)?;
        offset = add(offset, len)?;
        if len != PAGE {
            return Ok(total);
        }
    }
}
