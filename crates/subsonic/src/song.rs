//! Source-derived getSong normalization; wire values never escape this module.
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, ExchangeError>;

/// Read one song from the caller-selected authenticated server. The caller must bind
/// `identity.server`, `base` and `auth` to the same profile; the context carries no server ID.
/// This does not implement the broader Backend facade or select a network runtime.
pub async fn get_song(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<SongId>,
) -> Result<Song> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context.check_items(1).map_err(ExchangeError::Context)?;
    // Match the persisted P02 profile identity boundary; source song IDs retain
    // their separate request-construction limit rather than this profile limit.
    if identity.id.as_str().is_empty()
        || identity.server.as_str().trim().is_empty()
        || identity.server.as_str().len() > 128
    {
        return Err(ExchangeError::Malformed);
    }
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getSong.view",
            &[("id", identity.id.as_str())],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = response
        .get("song")
        .and_then(Value::as_object)
        .ok_or(ExchangeError::Malformed)
        .and_then(|song| normalize(song, &identity.server, context))
        .and_then(|song| {
            if song.id == identity.id {
                Ok(song)
            } else {
                Err(ExchangeError::Stale)
            }
        });
    // As in protocol envelope parsing, completion context takes priority over
    // both normalized values and normalization/identity errors.
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
pub(super) fn text<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        _ => Err(ExchangeError::Malformed),
    }
}
pub(super) fn required_text(obj: &Map<String, Value>, key: &str) -> Result<String> {
    text(obj, key)?
        .map(str::to_owned)
        .ok_or(ExchangeError::Malformed)
}
pub(super) fn number(obj: &Map<String, Value>, key: &str) -> Result<Option<f64>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_f64()
            .filter(|n| n.is_finite())
            .map(Some)
            .ok_or(ExchangeError::Malformed),
        _ => Err(ExchangeError::Malformed),
    }
}
pub(super) fn integer(obj: &Map<String, Value>, key: &str) -> Result<Option<u64>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .or_else(|| {
                n.as_f64()
                    .filter(|v| {
                        v.is_finite() && *v >= 0.0 && *v < u64::MAX as f64 && v.fract() == 0.0
                    })
                    .map(|v| v as u64)
            })
            .filter(|n| *n <= 9_007_199_254_740_991)
            .map(Some)
            .ok_or(ExchangeError::Malformed),
        _ => Err(ExchangeError::Malformed),
    }
}
pub(super) fn nonzero(n: Option<f64>) -> Option<f64> {
    n.filter(|n| *n != 0.0)
}
fn nonzero_int(n: Option<u64>) -> Option<u64> {
    n.filter(|n| *n != 0)
}
pub(super) fn id(obj: &Map<String, Value>, key: &str) -> Result<Option<String>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Number(n)) => {
            // JS IDs beyond the safe-integer range cannot be represented faithfully.
            let n = n
                .as_f64()
                .filter(|n| n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0)
                .ok_or(ExchangeError::Malformed)?;
            Ok(Some(if n == 0.0 {
                "0".into()
            } else {
                format!("{n:.0}")
            }))
        }
        _ => Err(ExchangeError::Malformed),
    }
}
pub(super) fn array<'a>(
    obj: &'a Map<String, Value>,
    key: &str,
    context: OperationContext<'_>,
) -> Result<Option<&'a [Value]>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) => {
            context
                .check_items(a.len())
                .map_err(ExchangeError::Context)?;
            Ok(Some(a))
        }
        _ => Err(ExchangeError::Malformed),
    }
}
pub(super) fn object(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or(ExchangeError::Malformed)
}
fn related(id: String, name: String) -> RelatedArtist {
    RelatedArtist {
        id: ArtistId::new(id),
        name,
        image_id: None,
        image_url: None,
        user_favorite: false,
        user_rating: None,
    }
}
pub(super) fn artists(
    values: Option<&[Value]>,
    fallback_id: &str,
    fallback_name: &str,
    participants: Option<&BTreeMap<String, Vec<RelatedArtist>>>,
    context: OperationContext<'_>,
) -> Result<Vec<RelatedArtist>> {
    if values.is_none() && participants.is_none() {
        return Ok(vec![related(fallback_id.into(), fallback_name.into())]);
    }
    let mut result = Vec::new();
    for value in values.unwrap_or(&[]) {
        let obj = object(value)?;
        result.push(related(
            id(obj, "id")?.ok_or(ExchangeError::Malformed)?,
            required_text(obj, "name")?,
        ));
    }
    let existing: BTreeSet<_> = result.iter().map(|a| a.id.clone()).collect();
    if let Some(remixers) = participants.and_then(|p| p.get("remixer")) {
        for artist in remixers {
            if !existing.contains(&artist.id) {
                context
                    .check_items(result.len().saturating_add(1))
                    .map_err(ExchangeError::Context)?;
                result.push(artist.clone());
            }
        }
    }
    Ok(result)
}
pub(super) fn participants(
    obj: &Map<String, Value>,
    context: OperationContext<'_>,
) -> Result<Option<BTreeMap<String, Vec<RelatedArtist>>>> {
    let Some(values) = array(obj, "contributors", context)? else {
        return Ok(None);
    };
    let mut result: BTreeMap<String, Vec<RelatedArtist>> = BTreeMap::new();
    for value in values {
        let contributor = object(value)?;
        let artist = contributor
            .get("artist")
            .ok_or(ExchangeError::Malformed)
            .and_then(object)?;
        let role = required_text(contributor, "role")?;
        let role = match text(contributor, "subRole")?.filter(|s| !s.is_empty()) {
            Some(sub) => format!("{role} ({sub})"),
            None => role,
        };
        // Source groups into `{}` and inherited Object.prototype role names throw
        // when `.push` is attempted. Preserve failure rather than inventing success.
        if [
            "constructor",
            "__defineGetter__",
            "__defineSetter__",
            "hasOwnProperty",
            "__lookupGetter__",
            "__lookupSetter__",
            "isPrototypeOf",
            "propertyIsEnumerable",
            "toString",
            "valueOf",
            "__proto__",
            "toLocaleString",
        ]
        .contains(&role.as_str())
        {
            return Err(ExchangeError::Malformed);
        }
        result.entry(role).or_default().push(related(
            id(artist, "id")?.unwrap_or_default(),
            text(artist, "name")?.unwrap_or_default().into(),
        ));
    }
    Ok(Some(result))
}
fn year_fields(obj: &Map<String, Value>) -> Result<(Option<String>, Option<i32>)> {
    let Some(year) = nonzero(number(obj, "year")?) else {
        return Ok((None, None));
    };
    if year.fract() != 0.0 || year < i32::MIN as f64 || year > i32::MAX as f64 {
        return Err(ExchangeError::Malformed);
    }
    let year = year as i32;
    Ok((Some(year.to_string()), Some(year)))
}
pub(super) fn release_fields(obj: &Map<String, Value>) -> Result<(Option<String>, Option<i32>)> {
    if let Some(value) = obj.get("releaseDate").filter(|v| !v.is_null()) {
        let rd = object(value)?;
        if let (Some(y), Some(m), Some(d)) = (
            number(rd, "year")?,
            number(rd, "month")?,
            number(rd, "day")?,
        ) {
            let m = if m == 0.0 { 0.0 } else { m };
            let d = if d == 0.0 { 0.0 } else { d };
            let iso = format!("{y}-{m:02}-{d:02}");
            // The upstream helper checks lexical digits only, not calendar validity.
            let b = iso.as_bytes();
            let valid = b.len() == 10
                && b[4] == b'-'
                && b[7] == b'-'
                && b.iter()
                    .enumerate()
                    .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
            return if valid {
                Ok((Some(iso), Some(y as i32)))
            } else {
                Ok((None, Some(0)))
            };
        }
    }
    year_fields(obj)
}
fn gain(obj: &Map<String, Value>, album: &str, track: &str) -> Result<Option<GainInfo>> {
    let Some(value) = obj.get("replayGain").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let obj = object(value)?;
    let album = number(obj, album)?;
    let track = number(obj, track)?;
    Ok(if nonzero(album).is_some() || nonzero(track).is_some() {
        Some(GainInfo { album, track })
    } else {
        None
    })
}
pub(super) fn normalize(
    obj: &Map<String, Value>,
    server: &ServerId,
    context: OperationContext<'_>,
) -> Result<Song> {
    let id = SongId::new(id(obj, "id")?.ok_or(ExchangeError::Malformed)?);
    let title = required_text(obj, "title")?;
    let content_type = required_text(obj, "contentType")?;
    let created_at = required_text(obj, "created")?;
    let size = integer(obj, "size")?.ok_or(ExchangeError::Malformed)?;
    let artist_id = self::id(obj, "artistId")?.unwrap_or_default();
    let artist_name = text(obj, "artist")?.unwrap_or_default().to_owned();
    let participants = participants(obj, context)?;
    let album_values = array(obj, "albumArtists", context)?;
    let album_artists = artists(album_values, &artist_id, &artist_name, None, context)?;
    let album_artist_name = if album_values.is_some_and(|a| !a.is_empty()) {
        album_artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        artist_name.clone()
    };
    let artists = artists(
        array(obj, "artists", context)?,
        &artist_id,
        &artist_name,
        participants.as_ref(),
        context,
    )?;
    let genres = genres(obj, server, context)?;
    let (date, year) = year_fields(obj)?;
    let (release_date, release_year) = release_fields(obj)?;
    let duration = Milliseconds::from_seconds(number(obj, "duration")?.unwrap_or(0.0))
        .map_err(|_| ExchangeError::Malformed)?;
    // `starred` is boolean in the diagnostic schema but timestamps also occur on
    // servers; preserve the normalizer's Boolean(string) behavior for either form.
    let user_favorite = favorite(obj)?;
    Ok(Song {
        server: server.clone(),
        server_type: ServerType::Subsonic,
        album: Some(text(obj, "album")?.unwrap_or_default().into()),
        album_artist_name,
        album_artists,
        album_id: AlbumId::new(self::id(obj, "albumId")?.unwrap_or_default()),
        artist_name,
        artists,
        bit_depth: nonzero_int(integer(obj, "bitDepth")?),
        bit_rate: integer(obj, "bitRate")?.unwrap_or(0),
        blur_hash: None,
        bpm: nonzero(number(obj, "bpm")?),
        channels: nonzero_int(integer(obj, "channelCount")?),
        codec: None,
        comment: None,
        compilation: None,
        container: content_type
            .strip_prefix("audio/")
            .map(|s| s.split('/').next().unwrap_or_default().to_owned()),
        created_at,
        date: date.clone(),
        disc_number: nonzero_int(integer(obj, "discNumber")?).unwrap_or(1),
        disc_subtitle: None,
        duration,
        explicit_status: match text(obj, "explicitStatus")? {
            Some("explicit") => Some(ExplicitStatus::Explicit),
            Some("clean") => Some(ExplicitStatus::Clean),
            _ => None,
        },
        folder_id: None,
        gain: gain(obj, "albumGain", "trackGain")?,
        genres,
        id,
        image_id: self::id(obj, "coverArt")?.filter(|s| !s.is_empty()),
        image_url: None,
        is_resumable: None,
        last_played_at: None,
        library_id: None,
        library_name: None,
        lyrics: None,
        mbz_album_id: None,
        mbz_album_type: None,
        mbz_recording_id: text(obj, "musicBrainzId")?
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        mbz_release_group_id: None,
        mbz_track_id: None,
        missing: None,
        name: title.clone(),
        original_date: date,
        original_year: year,
        participants,
        path: Some(text(obj, "path")?.unwrap_or_default().into()),
        peak: gain(obj, "albumPeak", "trackPeak")?,
        play_count: integer(obj, "playCount")?.unwrap_or(0),
        playlist_item_id: None,
        release_date,
        release_year,
        resume_position_ms: None,
        sample_rate: nonzero_int(integer(obj, "samplingRate")?),
        size,
        sort_name: title,
        tags: None,
        thumb_hash: None,
        track_number: nonzero_int(integer(obj, "track")?).unwrap_or(1),
        track_subtitle: None,
        updated_at: String::new(),
        user_favorite,
        user_rating: nonzero(number(obj, "userRating")?),
        year,
    })
}

pub(super) fn genres(
    obj: &Map<String, Value>,
    server: &ServerId,
    context: OperationContext<'_>,
) -> Result<Vec<Genre>> {
    let mut genres = Vec::new();
    let names = if let Some(values) = array(obj, "genres", context)? {
        values
            .iter()
            .map(|v| required_text(object(v)?, "name"))
            .collect::<Result<Vec<_>>>()?
    } else {
        text(obj, "genre")?
            .filter(|s| !s.is_empty())
            .map(|s| vec![s.into()])
            .unwrap_or_default()
    };
    for name in names {
        genres.push(Genre {
            server: server.clone(),
            server_type: ServerType::Subsonic,
            id: GenreId::new(name.clone()),
            name,
            album_count: None,
            song_count: None,
            image_id: None,
            image_url: None,
        });
    }
    Ok(genres)
}

pub(super) fn normalize_with_discs(
    obj: &Map<String, Value>,
    server: &ServerId,
    context: OperationContext<'_>,
    discs: &BTreeMap<u32, String>,
) -> Result<Song> {
    let mut song = normalize(obj, server, context)?;
    // Source uses nullish defaulting for the lookup, unlike discNumber's falsy default.
    let disc = integer(obj, "discNumber")?.unwrap_or(1);
    song.disc_subtitle = u32::try_from(disc)
        .ok()
        .and_then(|disc| discs.get(&disc).cloned());
    Ok(song)
}

pub(super) fn favorite(obj: &Map<String, Value>) -> Result<bool> {
    match obj.get("starred") {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::String(value)) => Ok(!value.is_empty()),
        _ => Err(ExchangeError::Malformed),
    }
}
