//! Artist enrichment and server recommendation reads, without a radio engine.
use crate::collections::{count, nested_array, songs};
use crate::read_support::{Budget, validate_identity};
use crate::song::{array, favorite, id, number, object, required_text, text};
use crate::{
    Auth, ExchangeError, PlannedRequest, PostPreference, ProtocolTransport, ServerBase,
    plan_request,
};
use feishin_domain::*;
use serde_json::{Map, Value};

type Result<T> = std::result::Result<T, ExchangeError>;

fn check(
    clock: &dyn Clock,
    context: OperationContext<'_>,
    server: &ServerId,
    entity: &str,
) -> Result<()> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context.check_items(1).map_err(ExchangeError::Context)?;
    validate_identity(server, entity)
}
fn parameters<'a>(entity: &'a str, count: &'a Option<String>) -> Vec<(&'a str, &'a str)> {
    let mut pairs = Vec::new();
    if let Some(count) = count {
        pairs.push(("count", count.as_str()));
    }
    pairs.push(("id", entity));
    pairs
}
async fn radio(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    request: PlannedRequest,
    server: &ServerId,
    parent: &str,
) -> Result<Vec<Song>> {
    let response = crate::protocol::exchange(transport, clock, context, request).await?;
    let result = nested_array(&response, parent, "song", context)
        .and_then(|values| songs(values, server, context, None));
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
/// Full ordered server recommendations for an album, with no source-ID exclusion.
pub async fn get_album_radio(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<AlbumId>,
    limit: Option<u64>,
) -> Result<Vec<Song>> {
    check(clock, context, &identity.server, identity.id.as_str())?;
    let count = count(limit)?;
    let pairs = parameters(identity.id.as_str(), &count);
    radio(
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
        &identity.server,
        "similarSongs",
    )
    .await
}
/// Artist recommendations use the distinct getSimilarSongs2 response family.
pub async fn get_artist_radio(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<ArtistId>,
    limit: Option<u64>,
) -> Result<Vec<Song>> {
    check(clock, context, &identity.server, identity.id.as_str())?;
    let count = count(limit)?;
    let pairs = parameters(identity.id.as_str(), &count);
    radio(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getSimilarSongs2",
            &pairs,
            PostPreference::Get,
            None,
        )?,
        &identity.server,
        "similarSongs2",
    )
    .await
}
fn normalize_info(
    response: &Map<String, Value>,
    context: OperationContext<'_>,
) -> Result<AlbumArtistInfo> {
    let empty = Map::new();
    let obj = match response.get("artistInfo2") {
        None | Some(Value::Null) => &empty,
        Some(v) => object(v)?,
    };
    let biography = match text(obj, "biography")?.filter(|s| !s.is_empty()) {
        Some(s) => Field::Value(s.into()),
        None => Field::Null,
    };
    let mut budget = Budget {
        limit: context.limits.items,
        used: 0,
    };
    budget.take(1)?;
    let similar_artists = array(obj, "similarArtist", context)?
        .map(|values| {
            values
                .iter()
                .map(|v| {
                    budget.take(1)?;
                    let obj = object(v)?;
                    normalize_related(obj, true)
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    Ok(AlbumArtistInfo {
        biography,
        image_url: Field::Absent,
        similar_artists,
    })
}
/// Source non-200 HTTP results are an absent enrichment value. Protocol/network
/// failures remain errors; no retry or silent fallback to a different endpoint.
pub async fn get_album_artist_info(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<ArtistId>,
    limit: Option<u64>,
) -> Result<Option<AlbumArtistInfo>> {
    check(clock, context, &identity.server, identity.id.as_str())?;
    let count = count(limit)?;
    let pairs = parameters(identity.id.as_str(), &count);
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getArtistInfo2.view",
            &pairs,
            PostPreference::Get,
            None,
        )?,
    )
    .await;
    let result = match response {
        Ok(response) => normalize_info(&response, context).map(Some),
        Err(ExchangeError::Http(_)) => Ok(None),
        Err(e) => Err(e),
    };
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

pub(super) fn normalize_related(
    obj: &Map<String, Value>,
    preserve_zero: bool,
) -> Result<RelatedArtist> {
    let artist_id = id(obj, "id")?.ok_or(ExchangeError::Malformed)?;
    Ok(RelatedArtist {
        image_id: Some(id(obj, "coverArt")?.unwrap_or_else(|| artist_id.clone())),
        id: ArtistId::new(artist_id),
        name: required_text(obj, "name")?,
        image_url: None,
        user_favorite: favorite(obj)?,
        user_rating: number(obj, "userRating")?.filter(|n| preserve_zero || *n != 0.0),
    })
}
