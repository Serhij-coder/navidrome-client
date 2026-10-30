//! Combined search3 results with independent server-side family windows.
use crate::album::{normalize_album, normalize_artist_base};
use crate::collections::nested_array;
use crate::enrichment::normalize_related;
use crate::lists::check;
use crate::read_support::Budget;
use crate::song::{array, object};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;

/// Caller binds the scoped query server, base and auth. Result order and repeated
/// identities are retained separately in each family; no totals are inferred.
pub async fn search(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &SearchQuery,
) -> Result<SearchResults, ExchangeError> {
    check(
        clock,
        context,
        &query.server,
        0,
        None,
        query.query.as_deref(),
    )?;
    let windows = [
        ("albumCount", query.album_limit),
        ("albumOffset", query.album_start_index),
        ("artistCount", query.album_artist_limit),
        ("artistOffset", query.album_artist_start_index),
        ("songCount", query.song_limit),
        ("songOffset", query.song_start_index),
    ];
    if windows
        .iter()
        .any(|(_, v)| v.is_some_and(|v| v > 9_007_199_254_740_991))
    {
        return Err(ExchangeError::Malformed);
    }
    if let Some(folders) = &query.music_folder_ids {
        context
            .check_items(folders.len())
            .map_err(ExchangeError::Context)?;
    }
    let parameters: Vec<_> = windows
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v.to_string())))
        .collect();
    let mut pairs: Vec<_> = parameters.iter().map(|(k, v)| (*k, v.as_str())).collect();
    if let Some(folder) = query.music_folder_ids.as_ref().and_then(|v| v.first()) {
        pairs.push(("musicFolderId", folder.as_str()));
    }
    if let Some(term) = &query.query {
        pairs.push(("query", term));
    }
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
    let result = (|| {
        let mut budget = Budget {
            limit: context.limits.items,
            used: 1,
        };
        let mut album_artists = Vec::new();
        for value in nested_array(&response, "searchResult3", "artist", context)? {
            budget.take(1)?;
            let obj = object(value)?;
            let rows = array(obj, "similarArtists", context)?.unwrap_or(&[]);
            budget.take(rows.len())?;
            let similar = rows
                .iter()
                .map(|v| normalize_related(object(v)?, false))
                .collect::<Result<Vec<_>, _>>()?;
            album_artists.push(normalize_artist_base(obj, &query.server, Some(similar))?);
        }
        let mut albums = Vec::new();
        for value in nested_array(&response, "searchResult3", "album", context)? {
            albums.push(normalize_album(
                object(value)?,
                &query.server,
                context,
                &mut budget,
            )?);
        }
        let mut songs = Vec::new();
        for value in nested_array(&response, "searchResult3", "song", context)? {
            budget.take(1)?;
            let obj = object(value)?;
            budget.metadata(
                obj,
                &["artists", "albumArtists", "contributors", "genres"],
                context,
            )?;
            songs.push(crate::song::normalize(obj, &query.server, context)?);
        }
        Ok(SearchResults {
            album_artists,
            albums,
            songs,
        })
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
