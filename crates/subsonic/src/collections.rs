//! Bounded complete collection reads, with endpoint-specific source semantics.
use crate::read_support::{Budget, validate_identity};
use crate::song::{array, id, object, required_text};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, ExchangeError>;

fn check(clock: &dyn Clock, context: OperationContext<'_>, server: &ServerId) -> Result<()> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context.check_items(1).map_err(ExchangeError::Context)?;
    validate_identity(server, "collection")
}
pub(super) fn count(value: Option<u64>) -> Result<Option<String>> {
    value
        .map(|v| {
            if v > 9_007_199_254_740_991 {
                Err(ExchangeError::Malformed)
            } else {
                Ok(v.to_string())
            }
        })
        .transpose()
}
pub(super) fn nested_array<'a>(
    response: &'a Map<String, Value>,
    parent: &str,
    key: &str,
    context: OperationContext<'_>,
) -> Result<&'a [Value]> {
    match response.get(parent) {
        None | Some(Value::Null) => Ok(&[]),
        Some(value) => Ok(array(object(value)?, key, context)?.unwrap_or(&[])),
    }
}
fn page<T>(items: Vec<T>) -> Result<Page<T>> {
    let total =
        u64::try_from(items.len()).map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
    Ok(Page {
        items,
        start_index: 0,
        total_record_count: Some(total),
    })
}

/// Caller binds server, base and auth to the same profile. No folder filtering.
pub async fn get_music_folders(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
) -> Result<Page<MusicFolder>> {
    check(clock, context, server)?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getMusicFolders.view",
            &[],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = (|| {
        let parent = object(
            response
                .get("musicFolders")
                .ok_or(ExchangeError::Malformed)?,
        )?;
        let values = array(parent, "musicFolder", context)?.ok_or(ExchangeError::Malformed)?;
        context
            .check_items(
                values
                    .len()
                    .checked_add(1)
                    .ok_or(ExchangeError::Context(DomainError::Capacity))?,
            )
            .map_err(ExchangeError::Context)?;
        page(
            values
                .iter()
                .map(|v| {
                    let obj = object(v)?;
                    Ok(MusicFolder {
                        server: server.clone(),
                        id: FolderId::new(id(obj, "id")?.ok_or(ExchangeError::Malformed)?),
                        name: required_text(obj, "name")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        )
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

/// Always starts with AllArtists, then unique roles in JavaScript UTF-16 order.
pub async fn get_artist_roles(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
) -> Result<Vec<ArtistRoleChoice>> {
    check(clock, context, server)?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getArtists.view",
            &[],
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
        let mut roles = BTreeSet::new();
        for index in nested_array(&response, "artists", "index", context)? {
            budget.take(1)?;
            for artist in
                array(object(index)?, "artist", context)?.ok_or(ExchangeError::Malformed)?
            {
                budget.take(1)?;
                for role in array(object(artist)?, "roles", context)?.unwrap_or(&[]) {
                    budget.take(1)?;
                    roles.insert(role.as_str().ok_or(ExchangeError::Malformed)?.to_owned());
                }
            }
        }
        let mut roles: Vec<_> = roles.into_iter().collect();
        roles.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        let mut result = vec![ArtistRoleChoice::AllArtists];
        result.extend(roles.into_iter().map(ArtistRoleChoice::Role));
        // The synthetic AllArtists choice counts against the output budget too.
        context
            .check_items(result.len())
            .map_err(ExchangeError::Context)?;
        Ok(result)
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

pub(super) fn songs(
    values: &[Value],
    server: &ServerId,
    context: OperationContext<'_>,
    exclude_raw_string: Option<&str>,
) -> Result<Vec<Song>> {
    let mut budget = Budget {
        limit: context.limits.items,
        used: 0,
    };
    budget.take(1)?;
    budget.take(values.len())?;
    let mut result = Vec::new();
    for value in values {
        let obj = object(value)?;
        // JS uses strict inequality before normalization: numeric IDs are not
        // equal to string request IDs, even when their normalized strings match.
        if exclude_raw_string.is_some()
            && obj.get("id").and_then(Value::as_str) == exclude_raw_string
        {
            continue;
        }
        budget.metadata(
            obj,
            &["artists", "albumArtists", "contributors", "genres"],
            context,
        )?;
        result.push(crate::song::normalize(obj, server, context)?);
    }
    Ok(result)
}

/// Only the first folder is sent; `played` has no effect in the source adapter.
pub async fn get_random_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
    query: &RandomSongQuery,
) -> Result<Page<Song>> {
    check(clock, context, server)?;
    if let Some(folders) = &query.music_folder_ids {
        context
            .check_items(folders.len())
            .map_err(ExchangeError::Context)?;
    }
    let size = count(query.limit)?;
    let from = query.min_year.map(|n| n.to_string());
    let to = query.max_year.map(|n| n.to_string());
    let folder = query
        .music_folder_ids
        .as_ref()
        .and_then(|ids| ids.first())
        .map(FolderId::as_str);
    let pairs = [
        ("fromYear", from.as_deref()),
        ("genre", query.genre.as_deref()),
        ("musicFolderId", folder),
        ("size", size.as_deref()),
        ("toYear", to.as_deref()),
    ];
    let pairs: Vec<_> = pairs
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getRandomSongs.view",
            &pairs,
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = nested_array(&response, "randomSongs", "song", context)
        .and_then(|values| songs(values, server, context, None))
        .and_then(page);
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

/// Preserves the source's strict raw-ID exclusion; folder selections are ignored.
pub async fn get_similar_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<SongId>,
    query: &SimilarSongsQuery,
) -> Result<Vec<Song>> {
    check(clock, context, &identity.server)?;
    validate_identity(&identity.server, identity.id.as_str())?;
    let count = count(query.count)?;
    let mut pairs = vec![("id", identity.id.as_str())];
    if let Some(count) = &count {
        pairs.insert(0, ("count", count.as_str()));
    }
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getSimilarSongs",
            &pairs,
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = nested_array(&response, "similarSongs", "song", context).and_then(|values| {
        songs(
            values,
            &identity.server,
            context,
            Some(identity.id.as_str()),
        )
    });
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
