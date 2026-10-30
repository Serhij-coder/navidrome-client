//! Source getSongList branches, keeping server and local pagination distinct.
use crate::collections::nested_array;
use crate::lists::{check, paginate};
use crate::read_support::Budget;
use crate::song::{array, id, number, object};
use crate::song_order::{Row, sorted};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
type Result<T> = std::result::Result<T, ExchangeError>;
struct Reader<'a> {
    transport: &'a dyn ProtocolTransport,
    clock: &'a dyn Clock,
    context: OperationContext<'a>,
    base: &'a ServerBase,
    auth: &'a Auth,
    server: &'a ServerId,
    budget: Budget,
}
fn page(rows: Vec<Row>, offset: u64, total: Option<u64>) -> Page<Song> {
    Page {
        items: rows.into_iter().map(|r| r.song).collect(),
        start_index: offset,
        total_record_count: total,
    }
}
impl Reader<'_> {
    async fn fetch(
        &mut self,
        endpoint: &str,
        pairs: &[(&str, &str)],
    ) -> Result<Map<String, Value>> {
        self.budget.take(1)?;
        crate::protocol::exchange(
            self.transport,
            self.clock,
            self.context,
            plan_request(
                self.base,
                self.auth,
                endpoint,
                pairs,
                PostPreference::Get,
                None,
            )?,
        )
        .await
    }
    fn rows(&mut self, values: &[Value]) -> Result<Vec<Row>> {
        let mut rows = Vec::new();
        for value in values {
            self.budget.take(1)?;
            let obj = object(value)?;
            self.budget.metadata(
                obj,
                &["artists", "albumArtists", "contributors", "genres"],
                self.context,
            )?;
            let song = crate::song::normalize(obj, self.server, self.context)?;
            let duration_ms = number(obj, "duration")?.unwrap_or(0.0) * 1000.0;
            rows.push(Row { song, duration_ms });
        }
        Ok(rows)
    }
    async fn album(&mut self, album: &str) -> Result<Vec<Row>> {
        let response = match self.fetch("getAlbum.view", &[("id", album)]).await {
            Ok(v) => v,
            Err(ExchangeError::Http(_)) => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        let obj = object(response.get("album").ok_or(ExchangeError::Malformed)?)?;
        self.rows(array(obj, "song", self.context)?.ok_or(ExchangeError::Malformed)?)
    }
    async fn read(&mut self, q: &SongListQuery, random: &mut dyn Random) -> Result<Page<Song>> {
        let folder = q
            .filter
            .music_folder_ids
            .as_ref()
            .and_then(|v| v.first())
            .map(FolderId::as_str);
        let limit = q.limit.map(|v| v.to_string());
        let offset = q.start_index.to_string();
        let search = q.search_term.as_deref().filter(|v| !v.is_empty());
        let artists = q
            .filter
            .album_artist_ids
            .as_ref()
            .or(q.filter.artist_ids.as_ref());
        if search.is_none() {
            if let Some(genres) = &q.filter.genre_ids {
                let mut pairs = Vec::new();
                if let Some(limit) = &limit {
                    pairs.push(("count", limit.as_str()));
                }
                if let Some(genre) = genres.first() {
                    pairs.push(("genre", genre.as_str()));
                }
                if let Some(folder) = folder {
                    pairs.push(("musicFolderId", folder));
                }
                pairs.push(("offset", offset.as_str()));
                let response = self.fetch("getSongsByGenre.view", &pairs).await?;
                return Ok(page(
                    self.rows(nested_array(
                        &response,
                        "songsByGenre",
                        "song",
                        self.context,
                    )?)?,
                    0,
                    None,
                ));
            }
            if q.filter.favorite == Some(true) {
                let pairs = folder
                    .map(|f| vec![("musicFolderId", f)])
                    .unwrap_or_default();
                let response = self.fetch("getStarred.view", &pairs).await?;
                let mut rows =
                    self.rows(nested_array(&response, "starred", "song", self.context)?)?;
                if let Some(artists) = artists.filter(|v| !v.is_empty()) {
                    let ids: BTreeSet<_> = artists.iter().collect();
                    rows.retain(|r| r.song.album_artists.iter().any(|a| ids.contains(&a.id)));
                }
                let rows = sorted(rows, q.sort_by, q.sort_order, random)?;
                let p = paginate(rows, q.start_index, q.limit)?;
                return Ok(page(p.items, p.start_index, p.total_record_count));
            }
            if q.filter.album_ids.is_some() || artists.is_some() {
                let mut rows = Vec::new();
                for album in q.filter.album_ids.as_deref().unwrap_or(&[]) {
                    rows.extend(self.album(album.as_str()).await?);
                }
                let mut albums = Vec::new();
                for artist in artists.into_iter().flatten() {
                    let response = match self
                        .fetch("getArtist.view", &[("id", artist.as_str())])
                        .await
                    {
                        Ok(v) => v,
                        Err(ExchangeError::Http(_)) => continue,
                        Err(e) => return Err(e),
                    };
                    let obj = object(response.get("artist").ok_or(ExchangeError::Malformed)?)?;
                    let values = array(obj, "album", self.context)?.unwrap_or(&[]);
                    self.budget.take(values.len())?;
                    for value in values {
                        albums.push(id(object(value)?, "id")?.ok_or(ExchangeError::Malformed)?);
                    }
                }
                for album in albums {
                    rows.extend(self.album(&album).await?);
                }
                let total = u64::try_from(rows.len())
                    .map_err(|_| ExchangeError::Context(DomainError::Capacity))?;
                // Source does not sort, slice or use album disc-title maps here.
                return Ok(page(rows, 0, Some(total)));
            }
        }
        let mut pairs = vec![
            ("albumCount", "0"),
            ("albumOffset", "0"),
            ("artistCount", "0"),
            ("artistOffset", "0"),
        ];
        if let Some(folder) = folder {
            pairs.push(("musicFolderId", folder));
        }
        pairs.push(("query", search.unwrap_or("")));
        if let Some(limit) = &limit {
            pairs.push(("songCount", limit));
        }
        pairs.push(("songOffset", offset.as_str()));
        let response = self.fetch("search3.view", &pairs).await?;
        Ok(page(
            self.rows(nested_array(
                &response,
                "searchResult3",
                "song",
                self.context,
            )?)?,
            if search.is_some() { q.start_index } else { 0 },
            None,
        ))
    }
}
/// Full source branch routing; RNG is consumed only by local favorite shuffle.
/// Caller binds base/auth/query server and schedules this local future off the UI.
pub async fn get_songs(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &SongListQuery,
    random: &mut dyn Random,
) -> Result<Page<Song>> {
    check(
        clock,
        context,
        &query.server,
        query.start_index,
        query.limit,
        query.search_term.as_deref(),
    )?;
    for len in [
        query.filter.album_artist_ids.as_ref().map(Vec::len),
        query.filter.artist_ids.as_ref().map(Vec::len),
        query.filter.album_ids.as_ref().map(Vec::len),
        query.filter.genre_ids.as_ref().map(Vec::len),
        query.filter.music_folder_ids.as_ref().map(Vec::len),
    ]
    .into_iter()
    .flatten()
    {
        context.check_items(len).map_err(ExchangeError::Context)?;
    }
    let result = Reader {
        transport,
        clock,
        context,
        base,
        auth,
        server: &query.server,
        budget: Budget {
            limit: context.limits.items,
            used: 1,
        },
    }
    .read(query, random)
    .await;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
