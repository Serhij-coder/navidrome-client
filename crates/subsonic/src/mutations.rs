//! Bounded mutation requests with explicit acknowledged-prefix/uncertain outcomes.
use crate::read_support::validate_identity;
use crate::{
    Auth, ExchangeError, PlannedRequest, PostPreference, ProtocolTransport, ServerBase,
    plan_request,
};
use feishin_domain::*;
use serde_json::{Map, Value};
/// Counts validated HTTP/protocol acknowledgements, not server-side exactly-once
/// effects. A batched favorite request counts as one request, not one per ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MutationReceipt {
    pub acknowledged_requests: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MutationFailure {
    pub acknowledged_requests: usize,
    /// Conservative once an exchange is attempted; later guards may still
    /// prevent dispatch. No automatic retry or
    /// rollback is safe merely because this request failed/cancelled/timed out.
    pub outcome_unknown: bool,
    pub cause: ExchangeError,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MutationValue<T> {
    pub value: T,
    pub receipt: MutationReceipt,
}
type Result<T> = std::result::Result<T, MutationFailure>;
struct Progress {
    acknowledged: usize,
    unknown: bool,
}
impl Progress {
    fn fail(&self, cause: ExchangeError) -> MutationFailure {
        MutationFailure {
            acknowledged_requests: self.acknowledged,
            outcome_unknown: self.unknown,
            cause,
        }
    }
    async fn reply(
        &mut self,
        transport: &dyn ProtocolTransport,
        clock: &dyn Clock,
        context: OperationContext<'_>,
        request: PlannedRequest,
    ) -> Result<Map<String, Value>> {
        context
            .check(clock.now())
            .map_err(|e| self.fail(ExchangeError::Context(e)))?;
        context
            .check_request_size(request.size())
            .map_err(|e| self.fail(ExchangeError::Context(e)))?;
        self.unknown = true;
        let response = crate::protocol::exchange(transport, clock, context, request)
            .await
            .map_err(|e| self.fail(e))?;
        self.acknowledged += 1;
        self.unknown = false;
        Ok(response)
    }
    async fn send(
        &mut self,
        transport: &dyn ProtocolTransport,
        clock: &dyn Clock,
        context: OperationContext<'_>,
        request: PlannedRequest,
    ) -> Result<()> {
        self.reply(transport, clock, context, request)
            .await
            .map(|_| ())
    }
    fn finish_value<T>(
        self,
        clock: &dyn Clock,
        context: OperationContext<'_>,
        result: Result<T>,
    ) -> Result<MutationValue<T>> {
        context
            .check(clock.now())
            .map_err(|e| self.fail(ExchangeError::Context(e)))?;
        let value = result?;
        Ok(MutationValue {
            value,
            receipt: MutationReceipt {
                acknowledged_requests: self.acknowledged,
            },
        })
    }
    fn finish(
        self,
        clock: &dyn Clock,
        context: OperationContext<'_>,
        result: Result<()>,
    ) -> Result<MutationReceipt> {
        self.finish_value(clock, context, result).map(|r| r.receipt)
    }
}
fn check(clock: &dyn Clock, context: OperationContext<'_>, server: &ServerId) -> Result<Progress> {
    let p = Progress {
        acknowledged: 0,
        unknown: false,
    };
    context
        .check(clock.now())
        .map_err(|e| p.fail(ExchangeError::Context(e)))?;
    context
        .check_items(1)
        .map_err(|e| p.fail(ExchangeError::Context(e)))?;
    validate_identity(server, "mutation").map_err(|e| p.fail(e))?;
    Ok(p)
}
async fn favorite(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &FavoriteMutation,
    create: bool,
) -> Result<MutationReceipt> {
    let mut p = check(clock, context, &query.server)?;
    let result = async {
        let len = match &query.target {
            FavoriteTarget::Albums(v) => v.len(),
            FavoriteTarget::Artists(v) | FavoriteTarget::AlbumArtists(v) => v.len(),
            FavoriteTarget::Songs(v) | FavoriteTarget::PlaylistSongs(v) => v.len(),
        };
        context
            .check_items(len)
            .map_err(|e| p.fail(ExchangeError::Context(e)))?;
        if len == 0 {
            return Err(p.fail(ExchangeError::Malformed));
        }
        let (key, ids): (&str, Vec<&str>) = match &query.target {
            FavoriteTarget::Albums(v) => ("albumId", v.iter().map(AlbumId::as_str).collect()),
            FavoriteTarget::Artists(v) | FavoriteTarget::AlbumArtists(v) => {
                ("artistId", v.iter().map(ArtistId::as_str).collect())
            }
            FavoriteTarget::Songs(v) | FavoriteTarget::PlaylistSongs(v) => {
                ("id", v.iter().map(SongId::as_str).collect())
            }
        };
        if ids.is_empty() || ids.iter().any(|id| id.is_empty()) {
            return Err(p.fail(ExchangeError::Malformed));
        }
        let pairs: Vec<_> = ids.into_iter().map(|id| (key, id)).collect();
        let request = plan_request(
            base,
            auth,
            if create { "star.view" } else { "unstar.view" },
            &pairs,
            PostPreference::Get,
            None,
        )
        .map_err(|e| p.fail(e))?;
        p.send(transport, clock, context, request).await
    }
    .await;
    p.finish(clock, context, result)
}
pub async fn create_favorite(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &FavoriteMutation,
) -> Result<MutationReceipt> {
    favorite(transport, clock, context, base, auth, query, true).await
}
pub async fn delete_favorite(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &FavoriteMutation,
) -> Result<MutationReceipt> {
    favorite(transport, clock, context, base, auth, query, false).await
}
/// Preserve source order/repetition and finite rating values without inventing
/// a score range. Fail on the first unsuccessful request and retain its prefix.
pub async fn set_rating(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &RatingMutation,
) -> Result<MutationReceipt> {
    let mut p = check(clock, context, &query.server)?;
    let result = async {
        context
            .check_items(query.items.len())
            .map_err(|e| p.fail(ExchangeError::Context(e)))?;
        if !query.rating.is_finite() || query.items.iter().any(|id| id.as_str().is_empty()) {
            return Err(p.fail(ExchangeError::Malformed));
        }
        let rating = if query.rating == 0.0 {
            "0".to_owned()
        } else {
            query.rating.to_string()
        };
        // Preflight all request construction/byte limits before the first mutation.
        // Do not allocate/store one authenticated URL per selected item.
        for id in &query.items {
            context
                .check(clock.now())
                .map_err(|e| p.fail(ExchangeError::Context(e)))?;
            let request = plan_request(
                base,
                auth,
                "setRating.view",
                &[("id", id.as_str()), ("rating", &rating)],
                PostPreference::Get,
                None,
            )
            .map_err(|e| p.fail(e))?;
            context
                .check_request_size(request.size())
                .map_err(|e| p.fail(ExchangeError::Context(e)))?;
        }
        for id in &query.items {
            let request = plan_request(
                base,
                auth,
                "setRating.view",
                &[("id", id.as_str()), ("rating", &rating)],
                PostPreference::Get,
                None,
            )
            .map_err(|e| p.fail(e))?;
            p.send(transport, clock, context, request).await?;
        }
        Ok(())
    }
    .await;
    p.finish(clock, context, result)
}
pub async fn start_library_scan(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
) -> Result<MutationReceipt> {
    let mut p = check(clock, context, server)?;
    let result = async {
        let request = plan_request(base, auth, "startScan.view", &[], PostPreference::Get, None)
            .map_err(|e| p.fail(e))?;
        p.send(transport, clock, context, request).await
    }
    .await;
    p.finish(clock, context, result)
}
pub async fn refresh_items(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
) -> Result<MutationReceipt> {
    start_library_scan(transport, clock, context, base, auth, server).await
}

pub async fn create_playlist(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
    name: &str,
) -> Result<MutationValue<CreatedPlaylist>> {
    let mut p = check(clock, context, server)?;
    let result = async {
        let request = plan_request(
            base,
            auth,
            "createPlaylist.view",
            &[("name", name)],
            PostPreference::Get,
            None,
        )
        .map_err(|e| p.fail(e))?;
        let response = p.reply(transport, clock, context, request).await?;
        let value = (|| {
            let obj =
                crate::song::object(response.get("playlist").ok_or(ExchangeError::Malformed)?)?;
            let id = crate::song::id(obj, "id")?.ok_or(ExchangeError::Malformed)?;
            validate_identity(server, &id)?;
            Ok(CreatedPlaylist {
                identity: Scoped {
                    server: server.clone(),
                    id: PlaylistId::new(id),
                },
                name: crate::song::required_text(obj, "name")?,
            })
        })();
        value.map_err(|e| p.fail(e))
    }
    .await;
    p.finish_value(clock, context, result)
}
pub async fn update_playlist(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &PlaylistMetadataMutation,
) -> Result<MutationReceipt> {
    let mut p = check(clock, context, &query.playlist.server)?;
    let result = async {
        validate_identity(&query.playlist.server, query.playlist.id.as_str())
            .map_err(|e| p.fail(e))?;
        let public = query.public.map(|v| if v { "true" } else { "false" });
        let mut pairs = Vec::new();
        if let Some(comment) = &query.comment {
            pairs.push(("comment", comment.as_str()));
        }
        pairs.extend([
            ("name", query.name.as_str()),
            ("playlistId", query.playlist.id.as_str()),
        ]);
        if let Some(public) = public {
            pairs.push(("public", public));
        }
        let request = plan_request(
            base,
            auth,
            "updatePlaylist.view",
            &pairs,
            PostPreference::Get,
            None,
        )
        .map_err(|e| p.fail(e))?;
        p.send(transport, clock, context, request).await
    }
    .await;
    p.finish(clock, context, result)
}
pub async fn delete_playlist(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    playlist: &Scoped<PlaylistId>,
) -> Result<MutationReceipt> {
    let mut p = check(clock, context, &playlist.server)?;
    let result = async {
        validate_identity(&playlist.server, playlist.id.as_str()).map_err(|e| p.fail(e))?;
        let request = plan_request(
            base,
            auth,
            "deletePlaylist.view",
            &[("id", playlist.id.as_str())],
            PostPreference::Get,
            None,
        )
        .map_err(|e| p.fail(e))?;
        p.send(transport, clock, context, request).await
    }
    .await;
    p.finish(clock, context, result)
}
async fn playlist_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &PlaylistSongsMutation,
    replace: bool,
) -> Result<MutationReceipt> {
    let mut p = check(clock, context, &query.playlist.server)?;
    let result = async {
        validate_identity(&query.playlist.server, query.playlist.id.as_str())
            .map_err(|e| p.fail(e))?;
        context
            .check_items(query.songs.len())
            .map_err(|e| p.fail(ExchangeError::Context(e)))?;
        if query.songs.iter().any(|s| s.as_str().is_empty()) {
            return Err(p.fail(ExchangeError::Malformed));
        }
        let mut pairs = vec![("playlistId", query.playlist.id.as_str())];
        let key = if replace { "songId" } else { "songIdToAdd" };
        pairs.extend(query.songs.iter().map(|s| (key, s.as_str())));
        let request = plan_request(
            base,
            auth,
            if replace {
                "createPlaylist.view"
            } else {
                "updatePlaylist.view"
            },
            &pairs,
            PostPreference::Get,
            None,
        )
        .map_err(|e| p.fail(e))?;
        p.send(transport, clock, context, request).await
    }
    .await;
    p.finish(clock, context, result)
}
pub async fn add_to_playlist(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &PlaylistSongsMutation,
) -> Result<MutationReceipt> {
    playlist_songs(transport, clock, context, base, auth, query, false).await
}
/// Direct source createPlaylist replacement request, distinct from the source's
/// backup/remove/add workflow. Empty song arrays omit the repeated parameter.
pub async fn set_playlist_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &PlaylistSongsMutation,
) -> Result<MutationReceipt> {
    playlist_songs(transport, clock, context, base, auth, query, true).await
}
pub async fn remove_from_playlist(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &PlaylistPositionsMutation,
) -> Result<MutationReceipt> {
    let mut p = check(clock, context, &query.playlist.server)?;
    let result = async {
        validate_identity(&query.playlist.server, query.playlist.id.as_str())
            .map_err(|e| p.fail(e))?;
        context
            .check_items(query.positions.len())
            .map_err(|e| p.fail(ExchangeError::Context(e)))?;
        if query.positions.iter().any(|i| i.0 > 9_007_199_254_740_991) {
            return Err(p.fail(ExchangeError::Malformed));
        }
        let indices: Vec<_> = query.positions.iter().map(|i| i.0.to_string()).collect();
        let mut pairs = vec![("playlistId", query.playlist.id.as_str())];
        pairs.extend(indices.iter().map(|i| ("songIndexToRemove", i.as_str())));
        let request = plan_request(
            base,
            auth,
            "updatePlaylist.view",
            &pairs,
            PostPreference::Get,
            None,
        )
        .map_err(|e| p.fail(e))?;
        p.send(transport, clock, context, request).await
    }
    .await;
    p.finish(clock, context, result)
}
