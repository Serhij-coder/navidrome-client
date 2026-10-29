use feishin_domain::*;
use serde_json::{Value, json};
pub fn related(a: &RelatedArtist) -> Value {
    json!({"id":a.id.as_str(),"name":a.name,"imageId":a.image_id,"imageUrl":a.image_url,"userFavorite":a.user_favorite,"userRating":a.user_rating})
}
pub fn gain(g: &Option<GainInfo>) -> Value {
    g.as_ref()
        .map(|g| json!({"album":g.album,"track":g.track}))
        .unwrap_or(Value::Null)
}
pub fn song(s: &Song) -> Value {
    let participants = s.participants.as_ref().map(|p| {
        p.iter()
            .map(|(k, v)| (k.clone(), Value::Array(v.iter().map(related).collect())))
            .collect::<serde_json::Map<_, _>>()
    });
    json!({"_itemType":"song","_serverId":s.server.as_str(),"_serverType":"subsonic",
 "album":s.album,"albumArtistName":s.album_artist_name,"albumArtists":s.album_artists.iter().map(related).collect::<Vec<_>>(),"albumId":s.album_id.as_str(),"artistName":s.artist_name,"artists":s.artists.iter().map(related).collect::<Vec<_>>(),
 "bitDepth":s.bit_depth,"bitRate":s.bit_rate,"blurHash":s.blur_hash,"bpm":s.bpm,"channels":s.channels,"codec":s.codec,"comment":s.comment,"compilation":s.compilation,"container":s.container,"createdAt":s.created_at,"date":s.date,"discNumber":s.disc_number,"discSubtitle":s.disc_subtitle,"duration":s.duration.0,
 "explicitStatus":s.explicit_status.map(|e| match e {ExplicitStatus::Clean=>"CLEAN",ExplicitStatus::Explicit=>"EXPLICIT"}),"folderId":s.folder_id.as_ref().map(FolderId::as_str),"gain":gain(&s.gain),
 "genres":s.genres.iter().map(|g|json!({"_itemType":"genre","_serverId":g.server.as_str(),"_serverType":"subsonic","id":g.id.as_str(),"name":g.name,"albumCount":g.album_count,"songCount":g.song_count,"imageId":g.image_id,"imageUrl":g.image_url})).collect::<Vec<_>>(),
 "id":s.id.as_str(),"imageId":s.image_id,"imageUrl":s.image_url,"lastPlayedAt":s.last_played_at,"libraryId":s.library_id,"libraryName":s.library_name,"lyrics":s.lyrics,"mbzAlbumId":s.mbz_album_id,"mbzAlbumType":s.mbz_album_type,"mbzRecordingId":s.mbz_recording_id,"mbzReleaseGroupId":s.mbz_release_group_id,"mbzTrackId":s.mbz_track_id,"missing":s.missing,"name":s.name,"originalDate":s.original_date,"originalYear":s.original_year,"participants":participants,"path":s.path,"peak":gain(&s.peak),"playCount":s.play_count,"playlistItemId":s.playlist_item_id.as_ref().map(PlaylistEntryId::as_str),"releaseDate":s.release_date,"releaseYear":s.release_year,"sampleRate":s.sample_rate,"size":s.size,"sortName":s.sort_name,"tags":s.tags,"thumbHash":s.thumb_hash,"trackNumber":s.track_number,"trackSubtitle":s.track_subtitle,"updatedAt":s.updated_at,"userFavorite":s.user_favorite,"userRating":s.user_rating,"year":s.year})
}
pub fn equivalent(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (k, v) in a {
                equivalent(v, &b[k]);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                equivalent(a, b)
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
