//! Album/detail wire handling; all returned values are normalized domain models.
use crate::read_support::{Budget, validate_identity};
use crate::song::{
    array, artists, favorite, genres, id, integer, nonzero, number, object, participants,
    release_fields, required_text, text,
};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, ExchangeError>;

/// Caller binds base, auth and scoped identity to the same profile.
pub async fn get_album(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<AlbumId>,
) -> Result<Album> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context.check_items(1).map_err(ExchangeError::Context)?;
    validate_identity(&identity.server, identity.id.as_str())?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getAlbum.view",
            &[("id", identity.id.as_str())],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let mut budget = Budget {
        limit: context.limits.items,
        used: 0,
    };
    let result = response
        .get("album")
        .ok_or(ExchangeError::Malformed)
        .and_then(object)
        .and_then(|obj| normalize_album(obj, &identity.server, context, &mut budget))
        .and_then(|album| {
            if album.id == identity.id {
                Ok(album)
            } else {
                Err(ExchangeError::Stale)
            }
        });
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

/// Artist detail preserves absent versus empty embedded album lists.
pub async fn get_album_artist(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<ArtistId>,
) -> Result<AlbumArtistDetail> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context.check_items(1).map_err(ExchangeError::Context)?;
    validate_identity(&identity.server, identity.id.as_str())?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getArtist.view",
            &[("id", identity.id.as_str())],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let mut budget = Budget {
        limit: context.limits.items,
        used: 0,
    };
    let result = response
        .get("artist")
        .ok_or(ExchangeError::Malformed)
        .and_then(object)
        .and_then(|obj| normalize_artist(obj, &identity.server, context, &mut budget))
        .and_then(|detail| {
            if detail.artist.id == identity.id {
                Ok(detail)
            } else {
                Err(ExchangeError::Stale)
            }
        });
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

fn normalize_album(
    obj: &Map<String, Value>,
    server: &ServerId,
    context: OperationContext<'_>,
    budget: &mut Budget,
) -> Result<Album> {
    budget.take(1)?;
    budget.metadata(
        obj,
        &[
            "artists",
            "contributors",
            "genres",
            "discTitles",
            "recordLabels",
            "releaseTypes",
        ],
        context,
    )?;
    let name = required_text(obj, "name")?;
    let artist_name = required_text(obj, "artist")?;
    let created_at = required_text(obj, "created")?;
    let mut discs = BTreeMap::new();
    for value in array(obj, "discTitles", context)?.unwrap_or(&[]) {
        let entry = object(value)?;
        let disc = integer(entry, "disc")?
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(ExchangeError::Malformed)?;
        discs.insert(disc, required_text(entry, "title")?);
    }
    let mut songs = Vec::new();
    for value in array(obj, "song", context)?.unwrap_or(&[]) {
        budget.take(1)?;
        let song = object(value)?;
        budget.metadata(
            song,
            &["artists", "albumArtists", "contributors", "genres"],
            context,
        )?;
        songs.push(crate::song::normalize_with_discs(
            song, server, context, &discs,
        )?);
    }
    let record_labels = array(obj, "recordLabels", context)?
        .unwrap_or(&[])
        .iter()
        .map(|entry| required_text(object(entry)?, "name"))
        .collect::<Result<Vec<_>>>()?;
    let release_types = array(obj, "releaseTypes", context)?
        .unwrap_or(&[])
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or(ExchangeError::Malformed)
        })
        .collect::<Result<Vec<_>>>()?;
    let release_type = release_types
        .iter()
        .find(|kind| ["album", "ep", "single", "broadcast", "other"].contains(&kind.as_str()))
        .cloned();
    let (release_date, release_year) = release_fields(obj)?;
    let duration = number(obj, "duration")?.ok_or(ExchangeError::Malformed)?;
    Ok(Album {
        server: server.clone(),
        server_type: ServerType::Subsonic,
        album_artist_name: artist_name.clone(),
        album_artists: artists(
            array(obj, "artists", context)?,
            &id(obj, "artistId")?.unwrap_or_default(),
            &artist_name,
            None,
            context,
        )?,
        artists: vec![],
        blur_hash: None,
        comment: None,
        created_at: created_at.clone(),
        discs: if discs.is_empty() { None } else { Some(discs) },
        dominant_color: None,
        duration: Some(Milliseconds::from_seconds(duration).map_err(|_| ExchangeError::Malformed)?),
        explicit_status: match text(obj, "explicitStatus")? {
            Some("explicit") => Some(ExplicitStatus::Explicit),
            Some("clean") => Some(ExplicitStatus::Clean),
            _ => None,
        },
        gain: None,
        genres: genres(obj, server, context)?,
        id: AlbumId::new(id(obj, "id")?.ok_or(ExchangeError::Malformed)?),
        image_id: id(obj, "coverArt")?.filter(|id| !id.is_empty()),
        image_url: None,
        is_compilation: None,
        last_played_at: None,
        mbz_id: None,
        mbz_release_group_id: None,
        missing: None,
        name: name.clone(),
        original_date: release_date.clone(),
        original_year: release_year,
        participants: participants(obj, context)?,
        peak: None,
        play_count: None,
        rated_at: None,
        record_labels,
        release_date,
        release_type,
        release_types,
        release_year,
        size: None,
        song_count: integer(obj, "songCount")?,
        songs: Some(songs),
        // The source emits undefined/null for a missing title although Album.sortName
        // is declared as a string. Preserve a useful string with an explicit ADR difference.
        sort_name: text(obj, "title")?.unwrap_or(&name).to_owned(),
        starred_at: None,
        tags: None,
        thumb_hash: None,
        track_year_range: None,
        updated_at: created_at,
        user_favorite: favorite(obj)?,
        user_rating: nonzero(number(obj, "userRating")?),
        version: text(obj, "version")?
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
    })
}

fn artist_count(obj: &Map<String, Value>) -> Result<u64> {
    let value = match obj.get("albumCount") {
        None | Some(Value::Null) => return Ok(0),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Ok(0);
            }
            value.parse::<f64>().map_err(|_| ExchangeError::Malformed)?
        }
        Some(Value::Number(_)) => {
            return integer(obj, "albumCount")?.ok_or(ExchangeError::Malformed);
        }
        _ => return Err(ExchangeError::Malformed),
    };
    if value.is_finite() && (0.0..=9_007_199_254_740_991.0).contains(&value) && value.fract() == 0.0
    {
        Ok(value as u64)
    } else {
        Err(ExchangeError::Malformed)
    }
}

fn normalize_artist(
    obj: &Map<String, Value>,
    server: &ServerId,
    context: OperationContext<'_>,
    budget: &mut Budget,
) -> Result<AlbumArtistDetail> {
    budget.take(1)?;
    // Source evaluates this map before overriding similarArtists with null. Keep
    // malformed collection failures without retaining unused enrichment values.
    if let Some(values) = array(obj, "similarArtists", context)? {
        budget.take(values.len())?;
        for value in values {
            let entry = object(value)?;
            id(entry, "id")?;
            id(entry, "coverArt")?;
            text(entry, "name")?;
            number(entry, "userRating")?;
            favorite(entry)?;
        }
    }
    let albums = array(obj, "album", context)?
        .map(|values| {
            values
                .iter()
                .map(|value| normalize_album(object(value)?, server, context, budget))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let starred_at = match obj.get("starred") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::String(value)) => (!value.is_empty()).then(|| value.clone()),
        _ => return Err(ExchangeError::Malformed),
    };
    Ok(AlbumArtistDetail {
        artist: AlbumArtist {
            server: server.clone(),
            server_type: ServerType::Subsonic,
            album_count: Some(artist_count(obj)?),
            biography: None,
            blur_hash: None,
            dominant_color: None,
            duration: None,
            genres: vec![],
            id: ArtistId::new(id(obj, "id")?.ok_or(ExchangeError::Malformed)?),
            image_id: id(obj, "coverArt")?.filter(|id| !id.is_empty()),
            image_url: None,
            last_played_at: None,
            mbz: None,
            missing: None,
            name: required_text(obj, "name")?,
            play_count: None,
            rated_at: None,
            similar_artists: None,
            song_count: None,
            starred_at,
            thumb_hash: None,
            uploaded_image: None,
            user_favorite: favorite(obj)?,
            user_rating: None,
        },
        albums,
    })
}
