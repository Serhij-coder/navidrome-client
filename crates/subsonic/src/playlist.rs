//! Playlist metadata and snapshot-position entry identities, not mutation handles.
use crate::read_support::{Budget, validate_identity};
use crate::song::{array, id, integer, number, object, required_text, text};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};

type Result<T> = std::result::Result<T, ExchangeError>;

async fn fetch(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<PlaylistId>,
) -> Result<Map<String, Value>> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context.check_items(1).map_err(ExchangeError::Context)?;
    validate_identity(&identity.server, identity.id.as_str())?;
    crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getPlaylist.view",
            &[("id", identity.id.as_str())],
            PostPreference::Get,
            None,
        )?,
    )
    .await
}

fn playlist<'a>(
    response: &'a Map<String, Value>,
    identity: &Scoped<PlaylistId>,
) -> Result<&'a Map<String, Value>> {
    let obj = response
        .get("playlist")
        .ok_or(ExchangeError::Malformed)
        .and_then(object)?;
    let returned = id(obj, "id")?.ok_or(ExchangeError::Malformed)?;
    if returned != identity.id.as_str() {
        return Err(ExchangeError::Stale);
    }
    Ok(obj)
}

/// Read metadata only. Caller binds scoped identity, base and auth to one profile.
pub async fn get_playlist(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<PlaylistId>,
) -> Result<Playlist> {
    let response = fetch(transport, clock, context, base, auth, identity).await?;
    let result = playlist(&response, identity).and_then(|obj| normalize(obj, &identity.server));
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

pub(super) fn normalize(obj: &Map<String, Value>, server: &ServerId) -> Result<Playlist> {
    let cover = id(obj, "coverArt")?.filter(|s| !s.is_empty());
    let changed = text(obj, "changed")?.filter(|s| !s.is_empty());
    let image_id = match (cover, changed) {
        (Some(cover), Some(changed)) => Some(format!("{cover}&_={changed}")),
        (cover, _) => cover,
    };
    let owner = text(obj, "owner")?.map(str::to_owned);
    let public = match obj.get("public") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(v)) => Some(*v),
        _ => return Err(ExchangeError::Malformed),
    };
    Ok(Playlist {
        server: server.clone(),
        server_type: ServerType::Subsonic,
        blur_hash: None,
        description: text(obj, "comment")?
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        dominant_color: None,
        duration: Some(
            Milliseconds::from_seconds(number(obj, "duration")?.ok_or(ExchangeError::Malformed)?)
                .map_err(|_| ExchangeError::Malformed)?,
        ),
        evaluated_at: None,
        genres: vec![],
        id: PlaylistId::new(id(obj, "id")?.ok_or(ExchangeError::Malformed)?),
        image_id,
        image_url: None,
        name: required_text(obj, "name")?,
        owner_id: owner.as_ref().map(|s| UserId::new(s.clone())),
        owner,
        public,
        rules: Field::Absent,
        size: None,
        song_count: integer(obj, "songCount")?,
        sync: Field::Absent,
        thumb_hash: None,
        uploaded_image: None,
    })
}

/// Fetch the complete ordered snapshot, as upstream does without pagination.
/// Entry IDs are zero-based positions, valid only within this playlist snapshot.
pub async fn get_playlist_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<PlaylistId>,
) -> Result<Page<PlaylistSong>> {
    let response = fetch(transport, clock, context, base, auth, identity).await?;
    let result = playlist(&response, identity).and_then(|obj| {
        let mut budget = Budget {
            limit: context.limits.items,
            used: 0,
        };
        budget.take(1)?;
        let mut items = Vec::new();
        for (index, value) in array(obj, "entry", context)?
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            budget.take(1)?;
            let obj = object(value)?;
            budget.metadata(
                obj,
                &["artists", "albumArtists", "contributors", "genres"],
                context,
            )?;
            let mut song = crate::song::normalize(obj, &identity.server, context)?;
            let entry = PlaylistEntryId::new(index.to_string());
            song.playlist_item_id = Some(entry.clone());
            items.push(PlaylistSong { entry, song });
        }
        let count = u64::try_from(items.len())
            .map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
        Ok(Page {
            items,
            start_index: 0,
            total_record_count: Some(count),
        })
    });
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}

/// Matches the source ID projection, including repeated song IDs and order.
pub async fn get_playlist_song_ids(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    identity: &Scoped<PlaylistId>,
) -> Result<Page<SongId>> {
    let page = get_playlist_songs(transport, clock, context, base, auth, identity).await?;
    let result = Page {
        items: page.items.into_iter().map(|entry| entry.song.id).collect(),
        start_index: page.start_index,
        total_record_count: page.total_record_count,
    };
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    Ok(result)
}
