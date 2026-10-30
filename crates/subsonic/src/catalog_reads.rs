//! Composed artist song views and read-only server catalog metadata.
use crate::collections::{count, nested_array, songs};
use crate::read_support::validate_identity;
use crate::song::{id, integer, object, required_text, text};
use crate::{
    Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, get_songs, plan_request,
};
use feishin_domain::*;
use std::cmp::Ordering;
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
// Only a fixed, nonempty artist-selection query reaches this provider. That
// branch never shuffles, including when the source sort key is ignored.
struct NoShuffle;
impl Random for NoShuffle {
    fn next_u64(&mut self) -> u64 {
        unreachable!("fixed artist song selection cannot shuffle")
    }
}
async fn artist_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    artist: &Scoped<ArtistId>,
    sort: SongListSort,
) -> Result<Page<Song>> {
    get_songs(
        transport,
        clock,
        context,
        base,
        auth,
        &ListQuery {
            server: artist.server.clone(),
            sort_by: sort,
            sort_order: SortOrder::Desc,
            start_index: 0,
            limit: None,
            search_term: None,
            filter: SongFilter {
                artist_ids: Some(vec![artist.id.clone()]),
                ..SongFilter::default()
            },
        },
        &mut NoShuffle,
    )
    .await
}
fn rating(a: Option<f64>, b: Option<f64>) -> Ordering {
    // Source ascending nullable keys place null after numbers. Descending
    // favorites therefore places unrated songs before rated songs.
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, _) => Ordering::Greater,
        (_, None) => Ordering::Less,
        (Some(a), Some(b)) => a.total_cmp(&b),
    }
}
fn album_track(a: &Song, b: &Song) -> Ordering {
    a.album_id
        .as_str()
        .encode_utf16()
        .cmp(b.album_id.as_str().encode_utf16())
        .then_with(|| a.track_number.cmp(&b.track_number))
}
/// Source limit is ignored; total stays the unfiltered fetched song count.
pub async fn get_favorite_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &FavoriteSongListQuery,
) -> Result<Page<Song>> {
    check(
        clock,
        context,
        &query.artist.server,
        query.artist.id.as_str(),
    )?;
    let sort = if query.mode == FavoriteSongMode::Rating {
        SongListSort::Rating
    } else {
        SongListSort::Favorited
    };
    let result = async {
        let mut page =
            artist_songs(transport, clock, context, base, auth, &query.artist, sort).await?;
        page.items.retain(|s| {
            if query.mode == FavoriteSongMode::Rating {
                s.user_rating.is_some_and(|r| r > 2.0)
            } else {
                s.user_favorite
            }
        });
        page.items.sort_by(|a, b| {
            let primary = if query.mode == FavoriteSongMode::Rating {
                rating(b.user_rating, a.user_rating)
                    .then_with(|| b.user_favorite.cmp(&a.user_favorite))
            } else {
                b.user_favorite
                    .cmp(&a.user_favorite)
                    .then_with(|| rating(b.user_rating, a.user_rating))
            };
            primary
                .then_with(|| b.play_count.cmp(&a.play_count))
                .then_with(|| album_track(a, b))
        });
        Ok(page)
    }
    .await;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
/// Community uses the advertised ID extension or artist name; personal uses
/// composed artist songs, ignores limit and preserves its unfiltered total.
pub async fn get_top_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &TopSongListQuery,
    capabilities: &Capabilities,
) -> Result<Page<Song>> {
    check(
        clock,
        context,
        &query.artist.server,
        query.artist.id.as_str(),
    )?;
    let result = async {
        if query.mode == TopSongMode::Personal {
            let mut page = artist_songs(
                transport,
                clock,
                context,
                base,
                auth,
                &query.artist,
                SongListSort::PlayCount,
            )
            .await?;
            page.items.retain(|s| s.play_count > 0);
            page.items.sort_by(|a, b| {
                b.play_count
                    .cmp(&a.play_count)
                    .then_with(|| album_track(a, b))
            });
            Ok(page)
        } else {
            let limit = count(query.limit)?;
            let mut pairs = Vec::new();
            if capabilities.has(ServerFeature::TopSongsByArtistId) {
                pairs.push(("id", query.artist.id.as_str()));
            } else {
                pairs.push(("artist", query.artist_name.as_str()));
            }
            if let Some(limit) = &limit {
                pairs.push(("count", limit));
            }
            let response = crate::protocol::exchange(
                transport,
                clock,
                context,
                plan_request(
                    base,
                    auth,
                    "getTopSongs.view",
                    &pairs,
                    PostPreference::Get,
                    None,
                )?,
            )
            .await?;
            nested_array(&response, "topSongs", "song", context)
                .and_then(|rows| songs(rows, &query.artist.server, context, None))
                .and_then(|items| {
                    let total = u64::try_from(items.len())
                        .map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
                    Ok(Page {
                        items,
                        start_index: 0,
                        total_record_count: Some(total),
                    })
                })
        }
    }
    .await;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
/// URLs remain inert station metadata here; this operation does not open them.
pub async fn get_internet_radio_stations(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
) -> Result<Vec<InternetRadioStation>> {
    check(clock, context, server, "radio")?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getInternetRadioStations.view",
            &[],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = (|| {
        let rows = nested_array(
            &response,
            "internetRadioStations",
            "internetRadioStation",
            context,
        )?;
        context
            .check_items(
                rows.len()
                    .checked_add(1)
                    .ok_or(ExchangeError::Context(DomainError::Capacity))?,
            )
            .map_err(ExchangeError::Context)?;
        rows.iter()
            .map(|v| {
                let obj = object(v)?;
                Ok(InternetRadioStation {
                    server: server.clone(),
                    id: RadioStationId::new(required_text(obj, "id")?),
                    name: required_text(obj, "name")?,
                    stream_url: required_text(obj, "streamUrl")?,
                    homepage_url: text(obj, "homepageUrl")?
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                    image_id: id(obj, "coverArt")?
                        .filter(|id| !id.is_empty())
                        .map(Field::Value)
                        .unwrap_or(Field::Null),
                    image_url: Field::Null,
                    thumb_hash: Field::Absent,
                    uploaded_image: Field::Absent,
                })
            })
            .collect()
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
pub async fn get_scan_status(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    server: &ServerId,
) -> Result<ScanStatus> {
    check(clock, context, server, "scan")?;
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getScanStatus.view",
            &[],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = (|| {
        let obj = object(response.get("scanStatus").ok_or(ExchangeError::Malformed)?)?;
        Ok(ScanStatus {
            count: integer(obj, "count")?.ok_or(ExchangeError::Malformed)?,
            folder_count: integer(obj, "folderCount")?.ok_or(ExchangeError::Malformed)?,
            last_scan: text(obj, "lastScan")?.map(str::to_owned),
            scanning: obj
                .get("scanning")
                .and_then(serde_json::Value::as_bool)
                .ok_or(ExchangeError::Malformed)?,
        })
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
