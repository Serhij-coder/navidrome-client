//! Bounded mutation requests with explicit acknowledged-prefix/uncertain outcomes.
use crate::read_support::validate_identity;
use crate::{
    Auth, ExchangeError, PlannedRequest, PostPreference, ProtocolTransport, ServerBase,
    plan_request,
};
use feishin_domain::*;
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
    async fn send(
        &mut self,
        transport: &dyn ProtocolTransport,
        clock: &dyn Clock,
        context: OperationContext<'_>,
        request: PlannedRequest,
    ) -> Result<()> {
        context
            .check(clock.now())
            .map_err(|e| self.fail(ExchangeError::Context(e)))?;
        context
            .check_request_size(request.size())
            .map_err(|e| self.fail(ExchangeError::Context(e)))?;
        self.unknown = true;
        crate::protocol::exchange(transport, clock, context, request)
            .await
            .map_err(|e| self.fail(e))?;
        self.acknowledged += 1;
        self.unknown = false;
        Ok(())
    }
    fn finish(
        self,
        clock: &dyn Clock,
        context: OperationContext<'_>,
        result: Result<()>,
    ) -> Result<MutationReceipt> {
        context
            .check(clock.now())
            .map_err(|e| self.fail(ExchangeError::Context(e)))?;
        result?;
        Ok(MutationReceipt {
            acknowledged_requests: self.acknowledged,
        })
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
