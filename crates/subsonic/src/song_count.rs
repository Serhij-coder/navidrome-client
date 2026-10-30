//! Source song-count branches with bounded stride probes and album fan-out.
use crate::collections::nested_array;
use crate::lists::check;
use crate::read_support::Budget;
use crate::song::{array, id, object};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};

type Result<T> = std::result::Result<T, ExchangeError>;
const PAGE: u64 = 500;
const STRIDE: u64 = PAGE * 10;
const SAFE: u64 = 9_007_199_254_740_991;
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .filter(|v| *v <= SAFE)
        .ok_or(ExchangeError::Context(DomainError::Capacity))
}
struct Reader<'a> {
    transport: &'a dyn ProtocolTransport,
    clock: &'a dyn Clock,
    context: OperationContext<'a>,
    base: &'a ServerBase,
    auth: &'a Auth,
    budget: Budget,
}
impl Reader<'_> {
    async fn fetch(
        &mut self,
        endpoint: &str,
        pairs: &[(&str, &str)],
    ) -> Result<Map<String, Value>> {
        // Include requests even when a server repeatedly returns no observed rows.
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
    fn rows(&mut self, response: &Map<String, Value>, parent: &str) -> Result<u64> {
        let len = nested_array(response, parent, "song", self.context)?.len();
        self.budget.take(len)?;
        u64::try_from(len).map_err(|_| ExchangeError::Context(DomainError::Capacity))
    }
    async fn album(&mut self, album: &str) -> Result<u64> {
        let response = match self.fetch("getAlbum.view", &[("id", album)]).await {
            Ok(v) => v,
            Err(ExchangeError::Http(_)) => return Ok(0),
            Err(e) => return Err(e),
        };
        let obj = object(response.get("album").ok_or(ExchangeError::Malformed)?)?;
        // Unlike optional search results, the source schema requires this array.
        // Reject absent/null/scalar values instead of counting flatMap artifacts.
        let len = array(obj, "song", self.context)?
            .ok_or(ExchangeError::Malformed)?
            .len();
        self.budget.take(len)?;
        u64::try_from(len).map_err(|_| ExchangeError::Context(DomainError::Capacity))
    }
    async fn page(
        &mut self,
        folder: Option<&str>,
        genre: Option<&[GenreId]>,
        search: &str,
        offset: u64,
        size: u64,
    ) -> Result<u64> {
        let offset = offset.to_string();
        let size = size.to_string();
        let mut pairs = Vec::new();
        let (endpoint, parent) = if let Some(genres) = genre {
            pairs.push(("count", size.as_str()));
            if let Some(genre) = genres.first() {
                pairs.push(("genre", genre.as_str()));
            }
            if let Some(folder) = folder {
                pairs.push(("musicFolderId", folder));
            }
            pairs.push(("offset", offset.as_str()));
            ("getSongsByGenre.view", "songsByGenre")
        } else {
            pairs.extend([
                ("albumCount", "0"),
                ("albumOffset", "0"),
                ("artistCount", "0"),
                ("artistOffset", "0"),
            ]);
            if let Some(folder) = folder {
                pairs.push(("musicFolderId", folder));
            }
            pairs.extend([
                ("query", search),
                ("songCount", size.as_str()),
                ("songOffset", offset.as_str()),
            ]);
            ("search3.view", "searchResult3")
        };
        let response = self.fetch(endpoint, &pairs).await?;
        self.rows(&response, parent)
    }
    async fn count(&mut self, query: &SongListQuery) -> Result<u64> {
        let folder = query
            .filter
            .music_folder_ids
            .as_ref()
            .and_then(|v| v.first())
            .map(FolderId::as_str);
        if let Some(search) = query.search_term.as_deref().filter(|v| !v.is_empty()) {
            let mut total = 0;
            loop {
                let len = self.page(folder, None, search, total, PAGE).await?;
                total = add(total, len)?;
                if len != PAGE {
                    return Ok(total);
                }
            }
        }
        // A supplied empty genre array is truthy upstream and still chooses this path.
        let genre = query.filter.genre_ids.as_deref();
        if genre.is_none() {
            if query.filter.favorite == Some(true) {
                let pairs = folder
                    .map(|f| vec![("musicFolderId", f)])
                    .unwrap_or_default();
                let response = self.fetch("getStarred.view", &pairs).await?;
                // Count does not apply the list operation's artist filter.
                return self.rows(&response, "starred");
            }
            let artists = query
                .filter
                .album_artist_ids
                .as_ref()
                .or(query.filter.artist_ids.as_ref());
            if query.filter.album_ids.is_some() || artists.is_some() {
                let mut total = 0;
                for album in query.filter.album_ids.as_deref().unwrap_or(&[]) {
                    total = add(total, self.album(album.as_str()).await?)?;
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
                    let rows = array(obj, "album", self.context)?.unwrap_or(&[]);
                    self.budget.take(rows.len())?;
                    for row in rows {
                        albums.push(id(object(row)?, "id")?.ok_or(ExchangeError::Malformed)?);
                    }
                }
                for album in albums {
                    total = add(total, self.album(&album).await?)?;
                }
                return Ok(total);
            }
        }
        // Genre and unfiltered counts probe 0,5000,... with size=1, then
        // backtrack one stride and enumerate 500-row pages. Search skips probes.
        let mut section = 0;
        let mut offset = loop {
            if self.page(folder, genre, "", section, 1).await? != 1 {
                break section.saturating_sub(STRIDE);
            }
            section = add(section, STRIDE)?;
        };
        loop {
            let len = self.page(folder, genre, "", offset, PAGE).await?;
            offset = add(offset, len)?;
            if len != PAGE {
                return Ok(offset);
            }
        }
    }
}

/// Count raw song rows using source endpoint-specific rules, not a library snapshot.
/// Sort/page/year/rating inputs and favorite artist filters have no count effect.
/// Caller binds base/auth/server to one profile. No Send future requirement.
pub async fn get_song_count(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &SongListQuery,
) -> Result<u64> {
    check(
        clock,
        context,
        &query.server,
        0,
        None,
        query.search_term.as_deref(),
    )?;
    for len in [
        query.filter.album_ids.as_ref().map(Vec::len),
        query.filter.artist_ids.as_ref().map(Vec::len),
        query.filter.album_artist_ids.as_ref().map(Vec::len),
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
        budget: Budget {
            limit: context.limits.items,
            used: 1,
        },
    }
    .count(query)
    .await;
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
